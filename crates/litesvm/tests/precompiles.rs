use {
    ed25519_dalek::ed25519::signature::Signer,
    litesvm::LiteSVM,
    solana_ed25519_program::{self as ed25519_instruction, new_ed25519_instruction_with_signature},
    solana_instruction::error::InstructionError,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_secp256k1_program::{
        self as secp256k1_instruction, eth_address_from_pubkey,
        new_secp256k1_instruction_with_signature, sign_message,
    },
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

    // Act - Produce an valid secp256k1 instruction.
    let msg = b"hello world";
    let secp_pubkey = libsecp256k1::PublicKey::from_secret_key(&kp_secp256k1);
    let eth_address = eth_address_from_pubkey(&secp_pubkey.serialize()[1..].try_into().unwrap());
    let (signature, recovery_id) = sign_message(&kp_secp256k1.serialize(), msg).unwrap();
    let ix = new_secp256k1_instruction_with_signature(msg, &signature, recovery_id, &eth_address);

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
    let msg = b"hello world";
    let secp_pubkey = libsecp256k1::PublicKey::from_secret_key(&kp_secp256k1);
    let eth_address = eth_address_from_pubkey(&secp_pubkey.serialize()[1..].try_into().unwrap());
    let (signature, recovery_id) = sign_message(&kp_secp256k1.serialize(), msg).unwrap();
    let mut ix =
        new_secp256k1_instruction_with_signature(msg, &signature, recovery_id, &eth_address);

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
