use {
    crate::{
        register_tracing_filter::{eval, expr},
        InvocationInspectCallback, LiteSVM,
    },
    sha2::{Digest, Sha256},
    solana_address::Address,
    solana_program_runtime::invoke_context::{Executable, InvokeContext, RegisterTrace},
    solana_svm_transaction::svm_message::SVMMessage,
    solana_transaction::sanitized::SanitizedTransaction,
    solana_transaction_context::{instruction::InstructionContext, IndexOfAccount},
    std::{collections::HashMap, fs::File, io::Write},
};

const DEFAULT_PATH: &str = "target/sbf/trace";
#[cfg(feature = "sbpf-debugger")]
const DEFAULT_DEBUG_PORT: Option<u16> = None;

pub struct DefaultRegisterTracingCallback {
    pub sbf_trace_dir: String,
    pub sbf_trace_disassemble: bool,
    pub sbf_trace_filter: String,
    #[cfg(feature = "sbpf-debugger")]
    pub sbf_debug_port: Option<u16>,
}

impl Default for DefaultRegisterTracingCallback {
    fn default() -> Self {
        Self {
            // User can override default path with `SBF_TRACE_DIR` environment variable.
            sbf_trace_dir: std::env::var("SBF_TRACE_DIR").unwrap_or(DEFAULT_PATH.to_string()),
            sbf_trace_disassemble: std::env::var("SBF_TRACE_DISASSEMBLE").is_ok(),
            sbf_trace_filter: std::env::var("SBF_TRACE_FILTER").unwrap_or_default(),
            // The port that will be used for debugging.
            // Will invoke the debugger if set.
            #[cfg(feature = "sbpf-debugger")]
            sbf_debug_port: std::env::var("SBF_DEBUG_PORT")
                .map(|port| port.parse::<u16>().ok())
                .unwrap_or(DEFAULT_DEBUG_PORT),
        }
    }
}

impl DefaultRegisterTracingCallback {
    pub fn disassemble_register_trace<W: std::io::Write>(
        &self,
        writer: &mut W,
        program_id: &Address,
        executable: &Executable,
        register_trace: RegisterTrace,
    ) {
        match solana_program_runtime::solana_sbpf::static_analysis::Analysis::from_executable(
            executable,
        ) {
            Ok(analysis) => {
                if let Err(e) = analysis.disassemble_register_trace(writer, register_trace) {
                    eprintln!("Can't disassemble register trace for {program_id}: {e:#?}");
                }
            }
            Err(e) => {
                eprintln!("Can't create trace disassemble analysis for {program_id}: {e:#?}")
            }
        }
    }

    pub fn match_filter(&self, tx_signatures: Vec<String>, program_ids: Vec<String>) -> bool {
        let Ok(ast) = expr(&self.sbf_trace_filter) else {
            return true;
        };
        let row = HashMap::from([("txsig", tx_signatures), ("program_id", program_ids)]);
        eval(&ast, &row)
    }

    pub fn pre_handler(
        &self,
        svm: &LiteSVM,
        tx: &SanitizedTransaction,
        program_indices: &[IndexOfAccount],
        invoke_context: &InvokeContext,
    ) {
        #[cfg(feature = "sbpf-debugger")]
        {
            // Collect pre-load hashes for these accounts.
            // We need them later to judge what object to
            // load in the debugger client.
            let _ = self.tx_accounts_to_elf_sha256(svm, tx, program_indices, invoke_context);

            if let Some(_debug_port) = self.sbf_debug_port {
                let program_ids: Vec<_> = program_indices
                    .iter()
                    .filter_map(|program_index| tx.account_keys().get(*program_index as usize))
                    .map(|program_key| program_key.to_string())
                    .collect();
                let signatures: Vec<_> =
                    tx.signatures().iter().map(|sig| sig.to_string()).collect();
                if self.match_filter(signatures, program_ids) {
                    // invoke_context.debug_port = Some(debug_port); // TODO
                }
            }
        }
    }

    pub fn post_handler(
        &self,
        svm: &LiteSVM,
        tx: &SanitizedTransaction,
        instruction_context: InstructionContext,
        executable: &Executable,
        register_trace: RegisterTrace,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if register_trace.is_empty() {
            // Can't do much with an empty trace.
            return Ok(());
        }

        // Get program_id.
        let program_id = instruction_context.get_program_key()?;
        let signatures: Vec<_> = tx.signatures().iter().map(|sig| sig.to_string()).collect();
        if !self.match_filter(signatures, vec![program_id.to_string()]) {
            // Skip this one since no filter has matched.
            return Ok(());
        }

        let current_dir = std::env::current_dir()?;
        let sbf_trace_dir = current_dir.join(&self.sbf_trace_dir);
        std::fs::create_dir_all(&sbf_trace_dir)?;

        let trace_digest = compute_hash(as_bytes(register_trace));
        let base_fname = sbf_trace_dir.join(&trace_digest[..16]);
        let mut regs_file = File::create(base_fname.with_extension("regs"))?;
        let mut insns_file = File::create(base_fname.with_extension("insns"))?;
        let mut program_id_file = File::create(base_fname.with_extension("program_id"))?;

        // Persist a full trace disassembly if requested.
        if self.sbf_trace_disassemble {
            let mut trace_disassemble_file = File::create(base_fname.with_extension("trace"))?;
            self.disassemble_register_trace(
                &mut trace_disassemble_file,
                program_id,
                executable,
                register_trace,
            );
        }

        // Persist the program id.
        let _ = program_id_file.write(program_id.to_string().as_bytes());

        if let Ok(elf_data) = svm.accounts_db().try_program_elf_bytes(program_id) {
            // Persist the preload hash of the executable.
            let mut so_hash_file = File::create(base_fname.with_extension("exec.sha256"))?;
            let _ = so_hash_file.write(compute_hash(elf_data).as_bytes());
        }

        // Get the relocated executable.
        let (_, program) = executable.get_text_bytes();
        for regs in register_trace.iter() {
            // The program counter is stored in r11.
            let pc = regs[11];
            // From the executable fetch the instruction this program counter points to.
            let insn =
                solana_program_runtime::solana_sbpf::ebpf::get_insn_unchecked(program, pc as usize)
                    .to_array();

            // Persist them in files.
            let _ = regs_file.write(as_bytes(regs.as_slice()))?;
            let _ = insns_file.write(insn.as_slice())?;
        }

        Ok(())
    }

    pub fn tx_accounts_to_elf_sha256(
        &self,
        svm: &LiteSVM,
        tx: &SanitizedTransaction,
        program_indices: &[IndexOfAccount],
        _invoke_context: &InvokeContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let current_dir = std::env::current_dir()?;
        let sbf_trace_dir = current_dir.join(&self.sbf_trace_dir);
        std::fs::create_dir_all(&sbf_trace_dir)?;
        let base_fname = sbf_trace_dir.join("program_ids");
        let mut program_ids_file = File::create(base_fname.with_extension("exec.sha256"))?;

        program_indices.iter().for_each(|program_index| {
            if let Some(key) = tx.account_keys().get(*program_index as usize) {
                if let Ok(elf_data) = svm.accounts_db().try_program_elf_bytes(key) {
                    let _ = program_ids_file
                        .write(format!("{}={}", key, compute_hash(elf_data)).as_bytes());
                }
            }
        });

        Ok(())
    }
}

impl InvocationInspectCallback for DefaultRegisterTracingCallback {
    fn before_invocation(
        &self,
        svm: &LiteSVM,
        tx: &SanitizedTransaction,
        program_indices: &[IndexOfAccount],
        invoke_context: &mut InvokeContext,
        register_tracing_enabled: bool,
    ) {
        if register_tracing_enabled {
            self.pre_handler(svm, tx, program_indices, invoke_context);
        }
    }

    fn after_invocation(
        &self,
        svm: &LiteSVM,
        tx: &SanitizedTransaction,
        _: &[IndexOfAccount],
        invoke_context: &InvokeContext,
        register_tracing_enabled: bool,
    ) {
        if register_tracing_enabled {
            // Only read the register traces if they were actually enabled.
            invoke_context.iterate_vm_traces(
                &|instruction_context: InstructionContext,
                  executable: &Executable,
                  register_trace: RegisterTrace| {
                    if let Err(e) =
                        self.post_handler(svm, tx, instruction_context, executable, register_trace)
                    {
                        eprintln!("Error collecting the register tracing: {}", e);
                    }
                },
            );
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub(crate) fn as_bytes<T>(slice: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, std::mem::size_of_val(slice)) }
}

fn compute_hash(slice: &[u8]) -> String {
    hex::encode(Sha256::digest(slice).as_slice())
}
