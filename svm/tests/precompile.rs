use expect_test::expect;
use litesvm::LiteSVM;
use solana_sdk::{
    ed25519_instruction::{self, new_ed25519_instruction},
    message::Message,
    secp256k1_instruction::{self, new_secp256k1_instruction},
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};

#[test_log::test]
fn ed25519_precompile_ok() {
    let kp = Keypair::new();
    let kp_dalek = ed25519_dalek::Keypair::from_bytes(&kp.to_bytes()).unwrap();

    let mut svm = LiteSVM::new();
    svm.airdrop(&kp.pubkey(), 10u64.pow(9)).unwrap();

    // Act - Produce a valid ed25519 instruction.
    let ix = new_ed25519_instruction(&kp_dalek, b"hello world");
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
    let mut ix = new_ed25519_instruction(&kp_dalek, b"hello world");
    ix.data[ed25519_instruction::DATA_START + 32] += 1;
    let tx = Transaction::new(
        &[&kp],
        Message::new(&[ix], Some(&kp.pubkey())),
        svm.latest_blockhash(),
    );
    let res = svm.send_transaction(tx);

    // Assert - Transaction fails.
    expect![[r#"
        Err(
            FailedTransactionMetadata {
                err: InvalidAccountIndex,
                meta: TransactionMetadata {
                    signature: 1111111111111111111111111111111111111111111111111111111111111111,
                    logs: [],
                    inner_instructions: [],
                    compute_units_consumed: 0,
                    return_data: TransactionReturnData {
                        program_id: 11111111111111111111111111111111,
                        data: [],
                    },
                },
            },
        )
    "#]]
    .assert_debug_eq(&res);
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
    expect![[r#"
        Err(
            FailedTransactionMetadata {
                err: InvalidAccountIndex,
                meta: TransactionMetadata {
                    signature: 1111111111111111111111111111111111111111111111111111111111111111,
                    logs: [],
                    inner_instructions: [],
                    compute_units_consumed: 0,
                    return_data: TransactionReturnData {
                        program_id: 11111111111111111111111111111111,
                        data: [],
                    },
                },
            },
        )
    "#]]
    .assert_debug_eq(&res);
}