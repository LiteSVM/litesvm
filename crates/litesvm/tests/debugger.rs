#[cfg(feature = "sbpf-debugger")]
use {
    litesvm::LiteSVM,
    solana_address::address,
    solana_keypair::Keypair,
    solana_message::{AccountMeta, Instruction, Message},
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::path::PathBuf,
};

#[cfg(feature = "sbpf-debugger")]
fn read_program(so_path: &str) -> Vec<u8> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(so_path);
    std::fs::read(path).unwrap()
}

#[cfg(feature = "sbpf-debugger")]
#[test]
pub fn test_cpi_with_debugger() {
    let enable_register_tracing = true;
    let mut svm = LiteSVM::new_debuggable(enable_register_tracing);

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let cpi_maker_program_id = address!("4fXuRQH9Xd7aZ25MG1nwDZhb9WNBC4bMfYE2AJTWTnR1");
    let cpi_target_program_id = address!("HAnysC5mLjYWPhSYMDyp31WzdCxmMDaij2Bkts9doedP");

    svm.add_program(
        cpi_maker_program_id,
        &read_program("test_programs/target/deploy/test_program_cpi_maker.so"),
    )
    .unwrap();
    svm.add_program(
        cpi_target_program_id,
        &read_program("test_programs/target/deploy/litesvm_clock_example.so"),
    )
    .unwrap();

    svm.airdrop(&payer_pk, 1000000000).unwrap();

    let blockhash = svm.latest_blockhash();

    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id: cpi_maker_program_id,
            accounts: vec![AccountMeta::new(cpi_target_program_id, false)],
            data: cpi_target_program_id.to_bytes().to_vec(),
        }],
        Some(&payer_pk),
        &blockhash,
    );
    let tx = Transaction::new(&[payer_kp], msg, blockhash);
    let _meta = svm.send_transaction(tx).unwrap();
}
