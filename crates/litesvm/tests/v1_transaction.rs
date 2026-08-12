use {
    litesvm::LiteSVM,
    solana_address::Address,
    solana_compute_budget::compute_budget_limits::{
        MAX_COMPUTE_UNIT_LIMIT, MAX_LOADED_ACCOUNTS_DATA_SIZE_BYTES,
    },
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_instruction::{error::InstructionError, Instruction},
    solana_keypair::Keypair,
    solana_message::{v1, v1::TransactionConfig, VersionedMessage},
    solana_native_token::LAMPORTS_PER_SOL,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::versioned::VersionedTransaction,
    solana_transaction_error::TransactionError,
};

const BASE_FEE: u64 = 5000;

/// Unset fields in a v1 transaction config are treated as zero, so tests that
/// aren't exercising a particular limit start from generous values.
fn permissive_config() -> TransactionConfig {
    TransactionConfig::empty()
        .with_compute_unit_limit(MAX_COMPUTE_UNIT_LIMIT)
        .with_loaded_accounts_data_size_limit(MAX_LOADED_ACCOUNTS_DATA_SIZE_BYTES.get())
}

fn v1_tx(
    svm: &LiteSVM,
    payer: &Keypair,
    instructions: &[Instruction],
    config: TransactionConfig,
) -> VersionedTransaction {
    let message = v1::Message::try_compile_with_config(
        &payer.pubkey(),
        instructions,
        svm.latest_blockhash(),
        config,
    )
    .unwrap();
    VersionedTransaction::try_new(VersionedMessage::V1(message), &[payer]).unwrap()
}

#[test_log::test]
fn test_v1_transaction_transfer() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&to, LAMPORTS_PER_SOL).unwrap();

    let transfer_amount = 100;
    let tx = v1_tx(
        &svm,
        &from_keypair,
        &[transfer(&from, &to, transfer_amount)],
        permissive_config(),
    );
    let meta = svm.send_transaction(tx).unwrap();

    assert_eq!(meta.fee, BASE_FEE);
    assert_eq!(
        svm.get_balance(&from).unwrap(),
        LAMPORTS_PER_SOL - BASE_FEE - transfer_amount
    );
    assert_eq!(
        svm.get_balance(&to).unwrap(),
        LAMPORTS_PER_SOL + transfer_amount
    );
}

#[test_log::test]
fn test_v1_compute_unit_limit_from_config() {
    // see that the tx fails if the config asks for a tiny compute unit limit
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();

    let tx = v1_tx(
        &svm,
        &from_keypair,
        &[transfer(&from, &to, 100)],
        permissive_config().with_compute_unit_limit(10),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::InstructionError(0, InstructionError::ComputationalBudgetExceeded)
    );
}

#[test_log::test]
fn test_v1_priority_fee_from_config() {
    // v1 txs specify a priority fee in lamports directly, instead of a price
    // per compute unit
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&to, LAMPORTS_PER_SOL).unwrap();

    let priority_fee = 10_000;
    let total_fee = BASE_FEE + priority_fee;
    let transfer_amount = 100;
    let tx = v1_tx(
        &svm,
        &from_keypair,
        &[transfer(&from, &to, transfer_amount)],
        permissive_config().with_priority_fee(priority_fee),
    );
    let meta = svm.send_transaction(tx).unwrap();

    assert_eq!(meta.fee, total_fee);
    assert_eq!(
        svm.get_balance(&from).unwrap(),
        LAMPORTS_PER_SOL - total_fee - transfer_amount
    );
    assert_eq!(
        svm.get_balance(&to).unwrap(),
        LAMPORTS_PER_SOL + transfer_amount
    );
}

#[test_log::test]
fn test_v1_ignores_compute_budget_instructions() {
    // compute budget instructions are no-ops in a v1 tx: the limits come from
    // the message config, so neither the tiny compute unit limit nor the
    // compute unit price below has any effect
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&to, LAMPORTS_PER_SOL).unwrap();

    let transfer_amount = 100;
    let tx = v1_tx(
        &svm,
        &from_keypair,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(10),
            ComputeBudgetInstruction::set_compute_unit_price(1_000_000),
            transfer(&from, &to, transfer_amount),
        ],
        permissive_config(),
    );
    let meta = svm.send_transaction(tx).unwrap();

    assert_eq!(meta.fee, BASE_FEE);
    assert_eq!(
        svm.get_balance(&from).unwrap(),
        LAMPORTS_PER_SOL - BASE_FEE - transfer_amount
    );
    assert_eq!(
        svm.get_balance(&to).unwrap(),
        LAMPORTS_PER_SOL + transfer_amount
    );
}
