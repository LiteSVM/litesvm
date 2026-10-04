use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::Address,
    solana_address_lookup_table_interface::instruction::{
        create_lookup_table, extend_lookup_table,
    },
    solana_clock::Clock,
    solana_compute_budget::{
        compute_budget::ComputeBudget, compute_budget_limits::MAX_LOADED_ACCOUNTS_DATA_SIZE_BYTES,
    },
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_instruction::{error::InstructionError, AccountMeta},
    solana_keypair::Keypair,
    solana_loader_v3_interface::get_program_data_address,
    solana_message::{
        v0::Message as MessageV0, AddressLookupTableAccount, Message, VersionedMessage,
    },
    solana_native_token::LAMPORTS_PER_SOL,
    solana_rent::Rent,
    solana_signer::Signer,
    solana_system_interface::{instruction::transfer, program as system_program},
    solana_transaction::{versioned::VersionedTransaction, Transaction},
    solana_transaction_error::TransactionError,
};

/// Per SIMD-0186 every transaction account is charged a 64 byte base size on
/// top of its data length.
const TRANSACTION_ACCOUNT_BASE_SIZE: u32 = 64;
/// Per SIMD-0186 every resolved address lookup table is charged 8248 bytes.
const ADDRESS_LOOKUP_TABLE_BASE_SIZE: u32 = 8248;

#[test_log::test]
fn test_set_compute_budget() {
    // see that the tx fails if we set a tiny limit
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();
    let tx_fee = 5000;
    svm.airdrop(
        &from,
        svm.get_sysvar::<Rent>().minimum_balance(0) + tx_fee + 100,
    )
    .unwrap();
    svm.airdrop(&to, LAMPORTS_PER_SOL).unwrap();

    // need to set the low compute budget after the airdrop tx
    let mut compute_budget = ComputeBudget::new_with_defaults(false);
    compute_budget.compute_unit_limit = 10;
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
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();
    let tx_fee = 5000;

    svm.airdrop(
        &from,
        svm.get_sysvar::<Rent>().minimum_balance(0) + tx_fee + 100,
    )
    .unwrap();
    svm.airdrop(&to, LAMPORTS_PER_SOL).unwrap();

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
    let to = Address::new_unique();

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
    let initial_recipient_balance = LAMPORTS_PER_SOL;
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

/// Creates an account holding `data_len` bytes, funded so that it stays rent
/// exempt after receiving a transfer.
fn create_account_with_data(svm: &mut LiteSVM, data_len: usize) -> Address {
    let address = Address::new_unique();
    svm.set_account(
        address,
        Account {
            lamports: svm.get_sysvar::<Rent>().minimum_balance(data_len),
            data: vec![0; data_len],
            owner: system_program::ID,
            ..Default::default()
        },
    )
    .unwrap();
    address
}

fn transfer_tx_with_data_size_limit(
    svm: &LiteSVM,
    from_keypair: &Keypair,
    to: &Address,
    limit: u32,
) -> Transaction {
    let from = from_keypair.pubkey();
    Transaction::new(
        &[from_keypair],
        Message::new(
            &[
                ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(limit),
                transfer(&from, to, 1),
            ],
            Some(&from),
        ),
        svm.latest_blockhash(),
    )
}

/// The loaded accounts data size of `tx`: a base size plus the data length of
/// every account in the message.
fn expected_loaded_data_size(svm: &LiteSVM, tx: &Transaction) -> u32 {
    tx.message
        .account_keys
        .iter()
        .map(|key| TRANSACTION_ACCOUNT_BASE_SIZE + svm.get_account(key).unwrap().data.len() as u32)
        .sum()
}

#[test_log::test]
fn test_set_loaded_accounts_data_size_limit() {
    // see that the tx fails if it loads more account data than it asked for
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    let to = create_account_with_data(&mut svm, 10_000);

    let tx = transfer_tx_with_data_size_limit(&svm, &from_keypair, &to, 1024);
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::MaxLoadedAccountsDataSizeExceeded
    );
}

#[test_log::test]
fn test_loaded_accounts_data_size_within_limit() {
    // the same tx succeeds when the requested limit covers the loaded accounts
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    let to = create_account_with_data(&mut svm, 10_000);

    let tx = transfer_tx_with_data_size_limit(
        &svm,
        &from_keypair,
        &to,
        MAX_LOADED_ACCOUNTS_DATA_SIZE_BYTES.get(),
    );

    svm.send_transaction(tx).unwrap();
}

#[test_log::test]
fn test_loaded_accounts_data_size_counts_account_base_size() {
    // every account contributes its data length plus a base size, so a limit
    // one byte below the total is not enough
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    let to = create_account_with_data(&mut svm, 10_000);

    let exact_limit = expected_loaded_data_size(
        &svm,
        &transfer_tx_with_data_size_limit(&svm, &from_keypair, &to, 1),
    );

    let tx = transfer_tx_with_data_size_limit(&svm, &from_keypair, &to, exact_limit - 1);
    let tx_res = svm.send_transaction(tx);
    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::MaxLoadedAccountsDataSizeExceeded
    );

    let tx = transfer_tx_with_data_size_limit(&svm, &from_keypair, &to, exact_limit);
    svm.send_transaction(tx).unwrap();
}

#[test_log::test]
fn test_transaction_metadata_reports_loaded_accounts_data_size() {
    // the size metered against the limit is reported when the transaction is
    // simulated, when it succeeds, and when it fails in execution
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    let to = create_account_with_data(&mut svm, 10_000);

    let limit = MAX_LOADED_ACCOUNTS_DATA_SIZE_BYTES.get();
    let tx = transfer_tx_with_data_size_limit(&svm, &from_keypair, &to, limit);
    let expected = expected_loaded_data_size(&svm, &tx);
    let overdraft = Transaction::new(
        &[&from_keypair],
        Message::new(
            &[
                ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(limit),
                transfer(&from, &to, 2 * LAMPORTS_PER_SOL),
            ],
            Some(&from),
        ),
        svm.latest_blockhash(),
    );

    let simulated = svm.simulate_transaction(tx.clone()).unwrap().meta;
    let succeeded = svm.send_transaction(tx).unwrap();
    let failed = svm.send_transaction(overdraft).unwrap_err().meta;

    assert_eq!(
        [simulated, succeeded, failed].map(|meta| meta.loaded_accounts_data_size),
        [expected; 3]
    );
}

#[test_log::test]
fn test_loaded_accounts_data_size_counts_address_lookup_tables() {
    // a resolved lookup table is charged a base size of its own, on top of the
    // accounts it resolves to
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&to, LAMPORTS_PER_SOL).unwrap();

    let recent_slot = svm.get_sysvar::<Clock>().slot;
    let (create_ix, lookup_table_address) = create_lookup_table(from, from, recent_slot);
    let extend_ix = extend_lookup_table(lookup_table_address, from, Some(from), vec![to]);
    let lookup_table_tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[create_ix, extend_ix], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(lookup_table_tx).unwrap();
    svm.warp_to_slot(recent_slot + 1);

    // a limit this size is more than enough for the accounts themselves
    let limit = ADDRESS_LOOKUP_TABLE_BASE_SIZE - 1;
    let legacy_tx = transfer_tx_with_data_size_limit(&svm, &from_keypair, &to, limit);
    assert!(expected_loaded_data_size(&svm, &legacy_tx) < limit);
    svm.send_transaction(legacy_tx).unwrap();

    // but the same tx sourcing the recipient from a lookup table exceeds it
    let lookup_table = AddressLookupTableAccount {
        key: lookup_table_address,
        addresses: vec![to],
    };
    let message = MessageV0::try_compile(
        &from,
        &[
            ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(limit),
            transfer(&from, &to, 1),
        ],
        &[lookup_table],
        svm.latest_blockhash(),
    )
    .unwrap();
    let tx =
        VersionedTransaction::try_new(VersionedMessage::V0(message), &[&from_keypair]).unwrap();
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::MaxLoadedAccountsDataSizeExceeded
    );
}

#[test_log::test]
fn test_loaded_accounts_data_size_counts_programdata() {
    // a loader-v3 program also counts its programdata, once: on its own when
    // the transaction does not list it, and only as a listed account when it does
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let program_id = Address::new_unique();
    let programdata_address = get_program_data_address(&program_id);

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, LAMPORTS_PER_SOL).unwrap();
    let to = create_account_with_data(&mut svm, 0);
    svm.add_program(
        program_id,
        include_bytes!("../test_programs/DF1ow3DqMj3HvTj8i8J9yM2hE9hCrLLXpdbaKZu4ZPnz.so"),
    )
    .unwrap();
    let programdata_size = TRANSACTION_ACCOUNT_BASE_SIZE
        + svm.get_account(&programdata_address).unwrap().data.len() as u32;

    let tx = |extra_accounts: &[Address], limit| {
        let mut transfer = transfer(&from, &to, 1);
        transfer.accounts.extend(
            extra_accounts
                .iter()
                .map(|address| AccountMeta::new_readonly(*address, false)),
        );
        Transaction::new(
            &[&from_keypair],
            Message::new(
                &[
                    ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(limit),
                    transfer,
                ],
                Some(&from),
            ),
            svm.latest_blockhash(),
        )
    };
    let unlisted = [program_id];
    let listed = [program_id, programdata_address];
    let unlisted_limit = expected_loaded_data_size(&svm, &tx(&unlisted, 1)) + programdata_size;
    let listed_limit = expected_loaded_data_size(&svm, &tx(&listed, 1));
    let txs = [
        tx(&unlisted, unlisted_limit - 1),
        tx(&unlisted, unlisted_limit),
        tx(&listed, listed_limit - 1),
        tx(&listed, listed_limit),
    ];

    let results = txs.map(|tx| svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err));
    let exceeded = Err(TransactionError::MaxLoadedAccountsDataSizeExceeded);
    assert_eq!(results, [exceeded.clone(), Ok(()), exceeded, Ok(())]);
}
