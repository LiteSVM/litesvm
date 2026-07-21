use {
    litesvm::{error::LiteSVMError, LiteSVM},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_message::{Instruction, Message},
    solana_signer::Signer,
    solana_svm_callback::InvokeContextCallback,
    solana_transaction::Transaction,
    std::{collections::HashMap, path::PathBuf},
};

fn read_epoch_stake_program() -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/test_program_epoch_stake.so");
    std::fs::read(&so_path).unwrap_or_else(|e| {
        panic!(
            "failed to read {}: {e}. Build with: cd crates/litesvm/test_programs && cargo build-sbf",
            so_path.display()
        )
    })
}

#[test_log::test]
fn epoch_stake_defaults_to_zero() {
    let svm = LiteSVM::new();
    let vote = Address::new_unique();

    assert_eq!(svm.epoch_total_stake(), 0);
    assert_eq!(svm.epoch_stake(&vote), 0);
    assert_eq!(InvokeContextCallback::get_epoch_stake(&svm), 0);
    assert_eq!(
        InvokeContextCallback::get_epoch_stake_for_vote_account(&svm, &vote),
        0
    );
}

#[test_log::test]
fn set_epoch_stake_updates_total() {
    let mut svm = LiteSVM::new();
    let vote_a = Address::new_unique();
    let vote_b = Address::new_unique();

    svm.set_epoch_stake(vote_a, 1_000).unwrap();
    svm.set_epoch_stake(vote_b, 2_500).unwrap();
    assert_eq!(svm.epoch_total_stake(), 3_500);
    assert_eq!(InvokeContextCallback::get_epoch_stake(&svm), 3_500);
    assert_eq!(
        InvokeContextCallback::get_epoch_stake_for_vote_account(&svm, &vote_a),
        1_000
    );
}

#[test_log::test]
fn overwriting_vote_stake_adjusts_total() {
    let mut svm = LiteSVM::new();
    let vote = Address::new_unique();

    svm.set_epoch_stake(vote, 1_000).unwrap();
    svm.set_epoch_stake(vote, 250).unwrap();

    assert_eq!(svm.epoch_stake(&vote), 250);
    assert_eq!(svm.epoch_total_stake(), 250);
}

#[test_log::test]
fn setting_vote_stake_to_zero_removes_it_from_total() {
    let mut svm = LiteSVM::new();
    let vote_a = Address::new_unique();
    let vote_b = Address::new_unique();

    svm.set_epoch_stake(vote_a, 1_000).unwrap();
    svm.set_epoch_stake(vote_b, 2_000).unwrap();
    svm.set_epoch_stake(vote_a, 0).unwrap();

    assert_eq!(svm.epoch_stake(&vote_a), 0);
    assert_eq!(svm.epoch_stake(&vote_b), 2_000);
    assert_eq!(svm.epoch_total_stake(), 2_000);
}

#[test_log::test]
fn set_epoch_stake_rejects_overflow() {
    let mut svm = LiteSVM::new();
    let vote_a = Address::new_unique();
    let vote_b = Address::new_unique();

    svm.set_epoch_stake(vote_a, u64::MAX).unwrap();
    let err = svm.set_epoch_stake(vote_b, 1).unwrap_err();
    assert!(matches!(err, LiteSVMError::EpochStakeOverflow));
    // Failed update must leave state unchanged.
    assert_eq!(svm.epoch_stake(&vote_a), u64::MAX);
    assert_eq!(svm.epoch_stake(&vote_b), 0);
    assert_eq!(svm.epoch_total_stake(), u64::MAX);
}

#[test_log::test]
fn set_epoch_stakes_sums_with_overflow_check() {
    let mut svm = LiteSVM::new();
    let vote = Address::new_unique();
    svm.set_epoch_stake(Address::new_unique(), 99).unwrap();

    let mut stakes = HashMap::new();
    stakes.insert(vote, 7_000);
    stakes.insert(Address::new_unique(), 3_000);
    svm.set_epoch_stakes(stakes).unwrap();

    assert_eq!(svm.epoch_total_stake(), 10_000);
    assert_eq!(svm.epoch_stake(&vote), 7_000);

    let mut overflowing = HashMap::new();
    overflowing.insert(Address::new_unique(), u64::MAX);
    overflowing.insert(Address::new_unique(), 1);
    assert!(matches!(
        svm.set_epoch_stakes(overflowing).unwrap_err(),
        LiteSVMError::EpochStakeOverflow
    ));
    // Failed bulk replace must leave prior state unchanged.
    assert_eq!(svm.epoch_total_stake(), 10_000);
    assert_eq!(svm.epoch_stake(&vote), 7_000);
}

#[test_log::test]
fn clone_preserves_epoch_stakes() {
    let mut svm = LiteSVM::new();
    let vote = Address::new_unique();
    svm.set_epoch_stake(vote, 123).unwrap();
    svm.set_epoch_stake(Address::new_unique(), 456).unwrap();

    let cloned = svm.clone();
    assert_eq!(cloned.epoch_stake(&vote), 123);
    assert_eq!(cloned.epoch_total_stake(), 579);
}

#[test_log::test]
fn sol_get_epoch_stake_syscall_reads_configured_values() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let vote = Address::new_unique();
    let vote_stake = 1_000_000_000u64;
    // Total is derived: one other vote + this vote.
    let other = Address::new_unique();
    svm.set_epoch_stake(other, 9_000_000_000).unwrap();
    svm.set_epoch_stake(vote, vote_stake).unwrap();
    let total_stake = svm.epoch_total_stake();
    assert_eq!(total_stake, 10_000_000_000);

    let program_id = Address::new_unique();
    svm.add_program(program_id, &read_epoch_stake_program())
        .unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let mut ix_data = Vec::with_capacity(48);
    ix_data.extend_from_slice(vote.as_ref());
    ix_data.extend_from_slice(&vote_stake.to_le_bytes());
    ix_data.extend_from_slice(&total_stake.to_le_bytes());

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id,
            accounts: vec![],
            data: ix_data,
        }],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let tx = Transaction::new(&[&payer], msg, blockhash);
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "sol_get_epoch_stake program failed: {:?}",
        res.err()
    );
}
