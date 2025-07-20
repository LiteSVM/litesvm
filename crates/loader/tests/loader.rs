use {
    crate::programs_bytes::HELLO_WORLD_BYTES,
    litesvm::LiteSVM,
    litesvm_loader::{deploy_upgradeable_program, set_upgrade_authority},
    solana_feature_set::FeatureSet,
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

mod programs_bytes;

#[test]
fn hello_world_with_store() {
    let mut svm = LiteSVM::new();

    let payer = Keypair::new();
    let program_bytes = HELLO_WORLD_BYTES;

    svm.airdrop(&payer.pubkey(), 1000000000).unwrap();

    let program_kp = Keypair::new();
    let program_id = program_kp.pubkey();
    svm.add_program(program_id, program_bytes).unwrap();

    let instruction = Instruction::new_with_bytes(
        program_id,
        &[],
        vec![AccountMeta::new(payer.pubkey(), true)],
    );
    let message = Message::new(&[instruction], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], message, svm.latest_blockhash());
    let tx_result = svm.send_transaction(tx);

    assert!(tx_result.is_ok());
    assert!(tx_result
        .unwrap()
        .logs
        .contains(&"Program log: Hello world!".to_string()));
}

#[test_log::test]
fn hello_world_with_deploy_upgradeable() {
    let mut feature_set = FeatureSet::all_enabled();
    // need to deactivate the disable_new_loader_v3_deployments feature
    feature_set.deactivate(&solana_feature_set::disable_new_loader_v3_deployments::id());
    let mut svm = LiteSVM::default()
        .with_feature_set(feature_set)
        .with_builtins()
        .with_lamports(1_000_000_000_000_000)
        .with_sysvars();

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_bytes = HELLO_WORLD_BYTES;

    svm.airdrop(&payer_pk, 10000000000).unwrap();

    let program_keypair = Keypair::new();
    deploy_upgradeable_program(&mut svm, &payer_kp, &program_keypair, program_bytes).unwrap();
    let program_id = program_keypair.pubkey();
    let instruction =
        Instruction::new_with_bytes(program_id, &[], vec![AccountMeta::new(payer_pk, true)]);
    let message = Message::new(&[instruction], Some(&payer_pk));
    let tx = Transaction::new(&[&payer_kp], message, svm.latest_blockhash());
    let tx_result = svm.send_transaction(tx);
    assert!(tx_result
        .unwrap()
        .logs
        .contains(&"Program log: Hello world!".to_string()));
    let new_authority = Keypair::new();
    set_upgrade_authority(
        &mut svm,
        &payer_kp,
        &program_id,
        &payer_kp,
        Some(&new_authority.pubkey()),
    )
    .unwrap();
}
