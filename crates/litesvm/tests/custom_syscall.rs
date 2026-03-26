use {
    jupnet_feature_set::FeatureSet,
    jupnet_program_runtime::{
        invoke_context::InvokeContext,
        jupnet_rbpf::{declare_builtin_function, memory_region::MemoryMapping},
    },
    jupnet_sdk::{
        instruction::Instruction,
        message::Message,
        native_token::MOTES_PER_JUP,
        pubkey,
        signer::{keypair::Keypair, Signer},
        transaction::Transaction,
    },
    litesvm::LiteSVM,
    std::path::PathBuf,
};

const CUS_TO_BURN: u64 = 1234;

declare_builtin_function!(
    /// A custom syscall to burn CUs.
    SyscallBurnCus,
    fn rust(
        invoke_context: &mut InvokeContext,
        to_burn: u64,
        _arg2: u64,
        _arg3: u64,
        _arg4: u64,
        _arg5: u64,
        _memory_mapping: &mut MemoryMapping,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        assert_eq!(to_burn, CUS_TO_BURN);
        invoke_context.consume_checked(to_burn)?;
        Ok(0)
    }
);

fn read_custom_syscall_program() -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/test_program_custom_syscall.so");
    std::fs::read(so_path).unwrap()
}

fn litesvm_ctor() -> LiteSVM {
    LiteSVM::default()
        .with_feature_set(FeatureSet::all_enabled())
        .with_builtins()
        .with_custom_syscall("sol_burn_cus", SyscallBurnCus::vm)
        .with_lamports(1_000_000u64.wrapping_mul(MOTES_PER_JUP))
        .with_sysvars()
        .with_default_programs()
        .with_sigverify(true)
        .with_blockhash_check(true)
}

#[test]
pub fn test_custom_syscall() {
    let mut svm = litesvm_ctor();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = pubkey!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.add_program(program_id, &read_custom_syscall_program())
        .unwrap();
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id,
            accounts: vec![],
            data: CUS_TO_BURN.to_le_bytes().to_vec(),
        }],
        Some(&payer_pk),
        &blockhash,
    );
    let tx = Transaction::new(&[payer_kp], msg, blockhash);
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "custom syscall tx failed: {:?}", res.err());
}
