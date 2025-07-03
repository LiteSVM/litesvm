use {
    ed25519_dalek::ed25519::signature::Signer,
    litesvm::LiteSVM,
    solana_ed25519_program::{self as ed25519_instruction, new_ed25519_instruction_with_signature},
    solana_instruction::error::InstructionError,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_secp256k1_program::{self as secp256k1_instruction, new_secp256k1_instruction},
    solana_signer::Signer as SolanaSigner,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

#[test_log::test]
fn ed25519_precompile_ok() {
    let kp = Keypair::new();
    let kp_dalek = ed25519_dalek::Keypair::from_bytes(&kp.to_bytes()).unwrap();

    let mut svm = LiteSVM::new();
    svm.airdrop(&kp.pubkey(), 10u64.pow(9)).unwrap();

    // Act - Produce a valid ed25519 instruction.
    let message = b"hello world";
    let signature = kp_dalek.sign(message);
    let ix = new_ed25519_instruction_with_signature(
        message,
        &signature.to_bytes(),
        kp.pubkey().as_array(),
    );
    let tx = Transaction::new(
        &[&kp],
        Message::new(&[ix], Some(&kp.pubkey())),
        svm.latest_blockhash(),
    );
    let res = svm.send_transaction(tx);

    // Assert - Transaction passes.
    assert!(res.is_ok());
}

#[test_log::test]
fn ed25519_precompile_err() {
    let kp = Keypair::new();
    let kp_dalek = ed25519_dalek::Keypair::from_bytes(&kp.to_bytes()).unwrap();

    let mut svm = LiteSVM::new();
    svm.airdrop(&kp.pubkey(), 10u64.pow(9)).unwrap();

    // Act - Produce an invalid ed25519 instruction.
    let message = b"hello world";
    let signature = kp_dalek.sign(message);
    let mut ix = new_ed25519_instruction_with_signature(
        message,
        &signature.to_bytes(),
        kp.pubkey().as_array(),
    );
    ix.data[ed25519_instruction::DATA_START + 32] = 0;
    let tx = Transaction::new(
        &[&kp],
        Message::new(&[ix], Some(&kp.pubkey())),
        svm.latest_blockhash(),
    );
    let res = svm.send_transaction(tx);

    // Assert - Transaction fails.
    assert_eq!(
        res.err().map(|fail| fail.err),
        Some(TransactionError::InstructionError(
            0,
            InstructionError::Custom(2)
        ))
    );
}

#[test_log::test]
fn secp256k1_precompile_ok() {
    let kp = Keypair::new();
    let kp_secp256k1 = libsecp256k1::SecretKey::parse_slice(&[1; 32]).unwrap();

    let mut svm = LiteSVM::new();
    svm.airdrop(&kp.pubkey(), 10u64.pow(9)).unwrap();

    // Act - Produce a valid secp256k1 instruction.
    let ix = new_secp256k1_instruction(&kp_secp256k1, b"hello world");
    let tx = Transaction::new(
        &[&kp],
        Message::new(&[ix], Some(&kp.pubkey())),
        svm.latest_blockhash(),
    );
    let res = svm.send_transaction(tx);

    // Assert - Transaction passes.
    assert!(res.is_ok());
}

#[test_log::test]
fn secp256k1_precompile_err() {
    let kp = Keypair::new();
    let kp_secp256k1 = libsecp256k1::SecretKey::parse_slice(&[1; 32]).unwrap();

    let mut svm = LiteSVM::new();
    svm.airdrop(&kp.pubkey(), 10u64.pow(9)).unwrap();

    // Act - Produce an invalid secp256k1 instruction.
    let mut ix = new_secp256k1_instruction(&kp_secp256k1, b"hello world");
    ix.data[secp256k1_instruction::DATA_START + 32] += 1;
    let tx = Transaction::new(
        &[&kp],
        Message::new(&[ix], Some(&kp.pubkey())),
        svm.latest_blockhash(),
    );
    let res = svm.send_transaction(tx);

    // Assert - Transaction fails.
    assert_eq!(
        res.err().map(|fail| fail.err),
        Some(TransactionError::InstructionError(
            0,
            InstructionError::Custom(2)
        ))
    );
}
