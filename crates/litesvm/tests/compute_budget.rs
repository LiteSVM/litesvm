use {
    jupnet_compute_budget::compute_budget::ComputeBudget,
    jupnet_sdk::{
        compute_budget::ComputeBudgetInstruction,
        instruction::InstructionError,
        message::Message,
        native_token::MOTES_PER_JUP,
        pubkey::Pubkey,
        rent::Rent,
        signer::{keypair::Keypair, Signer},
        system_instruction::transfer,
        transaction::{Transaction, TransactionError},
    },
    litesvm::LiteSVM,
};

#[test_log::test]
fn test_set_compute_budget() {
    // see that the tx fails if we set a tiny limit
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new();
    let tx_fee = 5000;
    svm.airdrop(
        &from,
        svm.get_sysvar::<Rent>().minimum_balance(0) + tx_fee + 100,
    )
    .unwrap();
    svm.airdrop(&to, MOTES_PER_JUP).unwrap();

    // need to set the low compute budget after the airdrop tx
    let compute_budget = ComputeBudget {
        compute_unit_limit: 10,
        ..Default::default()
    };
    svm = svm.with_compute_budget(compute_budget);
    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::InstructionError(0, InstructionError::ComputationalBudgetExceeded)
    );
}

#[test]
fn test_set_compute_unit_limit() {
    // see that the tx fails if we set a tiny limit
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new();
    let tx_fee = 5000;

    svm.airdrop(
        &from,
        svm.get_sysvar::<Rent>().minimum_balance(0) + tx_fee + 100,
    )
    .unwrap();
    svm.airdrop(&to, MOTES_PER_JUP).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(10),
                instruction,
            ],
            Some(&from),
        ),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::InstructionError(0, InstructionError::ComputationalBudgetExceeded)
    );
}

#[test]
fn test_priority_fee_is_charged() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new();

    // Priority fee calculation:
    // compute_unit_price = 1_000_000 micro-lamports (= 1 lamport per CU)
    // compute_unit_limit = 10_000
    // priority_fee = 1_000_000 * 10_000 / 1_000_000 = 10_000 lamports
    let compute_unit_price: u64 = 1_000_000;
    let compute_unit_limit: u32 = 10_000;
    let expected_priority_fee: u64 = 10_000;
    let base_fee: u64 = 5000;
    let total_fee = base_fee + expected_priority_fee;
    let transfer_amount: u64 = 100;

    let initial_balance = svm.get_sysvar::<Rent>().minimum_balance(0) + total_fee + transfer_amount;
    svm.airdrop(&from, initial_balance).unwrap();
    let initial_recipient_balance = MOTES_PER_JUP;
    svm.airdrop(&to, initial_recipient_balance).unwrap();

    let instruction = transfer(&from, &to, transfer_amount);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(
            &[
                ComputeBudgetInstruction::set_compute_unit_price(compute_unit_price),
                ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit),
                instruction,
            ],
            Some(&from),
        ),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);
    assert!(tx_res.is_ok(), "Transaction should succeed");

    let meta = tx_res.unwrap();

    // Verify the fee is correctly reported in transaction metadata
    assert_eq!(
        meta.fee, total_fee,
        "Transaction metadata should report correct fee (base {} + priority {})",
        base_fee, expected_priority_fee
    );

    // Check that fee payer balance is reduced by total fee (base + priority)
    // Note: get_balance returns None if account doesn't exist (0 balance accounts may be pruned)
    let final_balance = svm.get_balance(&from).unwrap_or(0);
    assert_eq!(
        final_balance, initial_balance - total_fee - transfer_amount,
        "Fee payer should have 0 balance after paying total_fee ({total_fee}) + transfer ({transfer_amount})"
    );

    // Verify recipient received the transfer
    let recipient_balance = svm.get_balance(&to).unwrap();
    assert_eq!(
        recipient_balance,
        initial_recipient_balance + transfer_amount
    );
}
