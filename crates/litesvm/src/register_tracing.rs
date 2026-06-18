#[allow(unused_imports)]
use solana_account::ReadableAccount;
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
                .unwrap_or_default(),
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

    #[cfg_attr(not(feature = "sbpf-debugger"), expect(unused_variables))]
    pub fn pre_handler(
        &self,
        svm: &LiteSVM,
        tx: &SanitizedTransaction,
        _program_indices: &[IndexOfAccount],
        invoke_context: &mut InvokeContext,
    ) {
        #[cfg(feature = "sbpf-debugger")]
        {
            if let Some(debug_port) = self.sbf_debug_port {
                // Collect pre-load hashes for these accounts.
                // We need them later to judge what object to
                // load in the debugger client.
                if let Err(e) = self.elf_accounts_to_sha256(svm, tx) {
                    eprintln!("Failed to persist the ELF SHA-256 mappings: {e}");
                }

                let mut program_ids = std::collections::HashSet::new();

                // Programs directly invoked by the transaction's instructions.
                let top_level_program_ids: Vec<_> = tx
                    .message()
                    .program_instructions_iter()
                    .map(|(pid, _)| pid.to_string())
                    .collect();
                program_ids.extend(top_level_program_ids);

                // Collect executable accounts from non-system/non-loader instructions.
                // These are potential CPI targets that won't appear as top-level program_ids.
                // This may produce false positives triggering the debugger (an executable
                // account included in the instruction but never actually CPI'd into).
                // Without wrapping the CPI syscall (possible with anza-xyz/sbpf#153),
                // this can't be made more granular.
                let might_cpi_program_ids: Vec<_> = tx
                    .message()
                    .program_instructions_iter()
                    .filter(|(program_id, _)| {
                        !solana_sdk_ids::bpf_loader_upgradeable::check_id(program_id)
                            && !solana_sdk_ids::bpf_loader::check_id(program_id)
                            && !solana_sdk_ids::bpf_loader_deprecated::check_id(program_id)
                            && !solana_sdk_ids::system_program::check_id(program_id)
                    })
                    .flat_map(|(_, instruction)| {
                        instruction
                            .accounts
                            .iter()
                            .filter_map(|index| tx.account_keys().get(*index as usize))
                    })
                    .filter(|addr| {
                        svm.accounts_db()
                            .get_account(addr)
                            .is_some_and(|acc| acc.executable())
                    })
                    .map(|addr| addr.to_string())
                    .collect();
                program_ids.extend(might_cpi_program_ids);

                let signatures: Vec<_> =
                    tx.signatures().iter().map(|sig| sig.to_string()).collect();
                if self.match_filter(signatures, program_ids.into_iter().collect()) {
                    invoke_context.debug_port = Some(debug_port);
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

    /// Persists a mapping of program_id -> SHA-256(ELF) for all programs
    /// referenced in the transaction (top-level and instruction accounts).
    /// Used by the debugger client to resolve which debug symbols to load.
    pub fn elf_accounts_to_sha256(
        &self,
        svm: &LiteSVM,
        tx: &SanitizedTransaction,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let current_dir = std::env::current_dir()?;
        let sbf_trace_dir = current_dir.join(&self.sbf_trace_dir);
        std::fs::create_dir_all(&sbf_trace_dir)?;
        let base_fname = sbf_trace_dir.join("program_ids");
        let mut program_ids_file = File::create(base_fname.with_extension("map"))?;

        let mut maybe_elf_program_ids = std::collections::HashSet::new();
        for (program_id, instruction) in tx.message().program_instructions_iter() {
            // Map the top-level program being invoked.
            maybe_elf_program_ids.insert(program_id);
            // Map any instruction accounts that are programs (potential CPI targets).
            instruction
                .accounts
                .iter()
                .filter_map(|index| tx.account_keys().get(*index as usize))
                .for_each(|key| {
                    maybe_elf_program_ids.insert(key);
                });
        }

        maybe_elf_program_ids.iter().for_each(|maybe_program_id| {
            if let Ok(elf_data) = svm.accounts_db().try_program_elf_bytes(maybe_program_id) {
                let _ = program_ids_file
                    .write(format!("{}={}\n", maybe_program_id, compute_hash(elf_data)).as_bytes());
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
                        eprintln!("Error collecting the register tracing: {e}");
                    }
                },
            );
        }
    }
}

pub(crate) fn as_bytes<T>(slice: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, std::mem::size_of_val(slice)) }
}

/// Returns the SHA-256 hash of the given bytes as a lowercase hex string.
pub fn compute_hash(slice: &[u8]) -> String {
    hex::encode(Sha256::digest(slice).as_slice())
}
