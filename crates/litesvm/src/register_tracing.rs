use {
    crate::{InvocationInspectCallback, LiteSVM},
    sha2::{Digest, Sha256},
    solana_program_runtime::invoke_context::{Executable, InvokeContext, RegisterTrace},
    solana_transaction::sanitized::SanitizedTransaction,
    solana_transaction_context::{IndexOfAccount, InstructionContext},
    std::{fs::File, io::Write},
};

const DEFAULT_PATH: &str = "target/sbf/trace";

pub struct DefaultRegisterTracingCallback {
    pub sbf_trace_dir: String,
}

impl Default for DefaultRegisterTracingCallback {
    fn default() -> Self {
        Self {
            // User can override default path with `SBF_TRACE_DIR` environment variable.
            sbf_trace_dir: std::env::var("SBF_TRACE_DIR").unwrap_or(DEFAULT_PATH.to_string()),
        }
    }
}

impl DefaultRegisterTracingCallback {
    pub fn handler(
        &self,
        svm: &LiteSVM,
        instruction_context: InstructionContext,
        executable: &Executable,
        register_trace: RegisterTrace,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if register_trace.is_empty() {
            // Can't do much with an empty trace.
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

        // Get program_id.
        let program_id = instruction_context.get_program_key()?;
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
}

impl InvocationInspectCallback for DefaultRegisterTracingCallback {
    fn before_invocation(
        &self,
        _: &LiteSVM,
        _: &SanitizedTransaction,
        _: &[IndexOfAccount],
        _: &InvokeContext,
    ) {
    }

    fn after_invocation(
        &self,
        svm: &LiteSVM,
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
                        self.handler(svm, instruction_context, executable, register_trace)
                    {
                        eprintln!("Error collecting the register tracing: {}", e);
                    }
                },
            );
        }
    }
}

pub(crate) fn as_bytes<T>(slice: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, std::mem::size_of_val(slice)) }
}

fn compute_hash(slice: &[u8]) -> String {
    hex::encode(Sha256::digest(slice).as_slice())
}
