use {
    litesvm::{
        types::{TokenBalance, TransactionMetadata},
        LiteSVM,
    },
    solana_account::Account,
    solana_address::Address,
    solana_address_lookup_table_interface::instruction::{
        create_lookup_table, extend_lookup_table,
    },
    solana_clock::Clock,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{v0, AddressLookupTableAccount, VersionedMessage},
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_system_interface::instruction::{create_account, transfer},
    solana_transaction::{versioned::VersionedTransaction, Transaction},
    solana_transaction_error::TransactionError,
    spl_associated_token_account_interface::{
        address::get_associated_token_address_with_program_id,
        instruction::create_associated_token_account,
    },
    spl_token_2022_interface::{
        extension::ExtensionType,
        instruction as token_instruction,
        state::{Account as TokenAccount, Mint},
    },
};

fn funded_svm(record: bool) -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    (svm.with_transaction_balance_recording(record), payer)
}

fn transaction(svm: &LiteSVM, payer: &Keypair, instructions: &[Instruction]) -> Transaction {
    Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &[payer],
        svm.latest_blockhash(),
    )
}

fn create_tokens(
    svm: &mut LiteSVM,
    payer: &Keypair,
    program: Address,
) -> (Address, Address, Address, TransactionMetadata) {
    let mint = Keypair::new();
    let owner = payer.pubkey();
    let other_owner = Address::new_unique();
    let source = get_associated_token_address_with_program_id(&owner, &mint.pubkey(), &program);
    let destination =
        get_associated_token_address_with_program_id(&other_owner, &mint.pubkey(), &program);
    let extended = program == spl_token_2022_interface::ID;
    let len = if extended {
        ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::MintCloseAuthority])
            .unwrap()
    } else {
        Mint::LEN
    };
    let mut instructions = vec![create_account(
        &owner,
        &mint.pubkey(),
        svm.minimum_balance_for_rent_exemption(len),
        len as u64,
        &program,
    )];
    if extended {
        instructions.push(
            token_instruction::initialize_mint_close_authority(
                &program,
                &mint.pubkey(),
                Some(&owner),
            )
            .unwrap(),
        );
    }
    instructions.extend([
        token_instruction::initialize_mint2(&program, &mint.pubkey(), &owner, None, 6).unwrap(),
        create_associated_token_account(&owner, &owner, &mint.pubkey(), &program),
        create_associated_token_account(&owner, &other_owner, &mint.pubkey(), &program),
        token_instruction::mint_to(&program, &mint.pubkey(), &source, &owner, &[], 1_000).unwrap(),
    ]);
    let tx = Transaction::new_signed_with_payer(
        &instructions,
        Some(&owner),
        &[payer, &mint],
        svm.latest_blockhash(),
    );
    let simulated = svm.simulate_transaction(tx.clone()).unwrap();
    assert!(svm.get_account(&mint.pubkey()).is_none());
    assert!(svm.get_account(&source).is_none());
    let sent = svm.send_transaction(tx).unwrap();
    assert_eq!(simulated.meta, sent);
    (mint.pubkey(), source, destination, sent)
}

fn token_transfer(
    program: Address,
    mint: Address,
    source: Address,
    destination: Address,
    payer: &Keypair,
    amount: u64,
) -> Instruction {
    token_instruction::transfer_checked(
        &program,
        &source,
        &mint,
        &destination,
        &payer.pubkey(),
        &[],
        amount,
        6,
    )
    .unwrap()
}

#[test]
fn system_transfer_records_ordered_balances_and_matches_simulation() {
    let (mut svm, payer) = funded_svm(true);
    let recipient = Address::new_unique();
    let tx = transaction(
        &svm,
        &payer,
        &[transfer(&payer.pubkey(), &recipient, 1_000_000)],
    );
    let keys = tx.message.account_keys.clone();
    let before: Vec<_> = keys
        .iter()
        .map(|key| svm.get_balance(key).unwrap_or_default())
        .collect();
    let simulated = svm.simulate_transaction(tx.clone()).unwrap();
    assert_eq!(svm.get_balance(&payer.pubkey()), Some(1_000_000_000));
    assert!(svm.get_account(&recipient).is_none());
    let sent = svm.send_transaction(tx).unwrap();
    assert_eq!(simulated.meta, sent);
    let balances = sent.balances.unwrap();
    assert_eq!(balances.pre_balances, before);
    assert_eq!(
        balances.post_balances,
        keys.iter()
            .map(|key| svm.get_balance(key).unwrap_or_default())
            .collect::<Vec<_>>()
    );
    assert_eq!(balances.post_balances[0], before[0] - 1_000_000 - sent.fee);
    assert!(balances.pre_token_balances.is_empty());
    assert!(balances.post_token_balances.is_empty());
}

#[test]
fn records_token_transfers_and_new_mints_including_extensions() {
    for program in [spl_token_interface::ID, spl_token_2022_interface::ID] {
        let (mut svm, payer) = funded_svm(true);
        let (mint, source, destination, created) = create_tokens(&mut svm, &payer, program);
        let created = created.balances.unwrap();
        assert!(created.pre_token_balances.is_empty());
        assert_eq!(created.post_token_balances.len(), 2);
        assert_eq!(
            created
                .post_token_balances
                .iter()
                .map(|b| b.amount)
                .sum::<u64>(),
            1_000
        );
        if program == spl_token_2022_interface::ID {
            assert!(svm.get_account(&mint).unwrap().data.len() > Mint::LEN);
            assert!(svm.get_account(&source).unwrap().data.len() > TokenAccount::LEN);
        }
        let tx = transaction(
            &svm,
            &payer,
            &[token_transfer(
                program,
                mint,
                source,
                destination,
                &payer,
                40,
            )],
        );
        let source_index = tx
            .message
            .account_keys
            .iter()
            .position(|k| *k == source)
            .unwrap() as u8;
        let destination_index = tx
            .message
            .account_keys
            .iter()
            .position(|k| *k == destination)
            .unwrap() as u8;
        let simulated = svm.simulate_transaction(tx.clone()).unwrap();
        let sent = svm.send_transaction(tx).unwrap();
        assert_eq!(simulated.meta, sent);
        let balances = sent.balances.unwrap();
        let expected = TokenBalance {
            account_index: source_index,
            mint,
            owner: payer.pubkey(),
            amount: 1_000,
            decimals: 6,
            program_id: program,
        };
        assert_eq!(
            balances
                .pre_token_balances
                .iter()
                .find(|b| b.account_index == source_index),
            Some(&expected)
        );
        assert_eq!(
            balances
                .post_token_balances
                .iter()
                .find(|b| b.account_index == source_index),
            Some(&TokenBalance {
                amount: 960,
                ..expected
            })
        );
        assert_eq!(balances.pre_token_balances.len(), 2);
        assert_eq!(balances.post_token_balances.len(), 2);
        assert_eq!(
            balances
                .pre_token_balances
                .iter()
                .find(|b| b.account_index == destination_index)
                .unwrap()
                .amount,
            0
        );
        assert_eq!(
            balances
                .post_token_balances
                .iter()
                .find(|b| b.account_index == destination_index)
                .unwrap()
                .amount,
            40
        );
    }
}

#[test]
fn failed_execution_rolls_back_writes_and_reports_fee_with_recording_on_or_off() {
    for record in [false, true] {
        let (mut svm, payer) = funded_svm(record);
        let program = spl_token_interface::ID;
        let (mint, source, destination, _) = create_tokens(&mut svm, &payer, program);
        let tx = transaction(
            &svm,
            &payer,
            &[
                token_transfer(program, mint, source, destination, &payer, 40),
                token_transfer(program, mint, source, destination, &payer, 2_000),
            ],
        );
        let keys = tx.message.account_keys.clone();
        let before: Vec<_> = keys.iter().map(|key| svm.get_account(key)).collect();
        let simulated = svm.simulate_transaction(tx.clone()).unwrap_err();
        assert!(matches!(
            simulated.err,
            TransactionError::InstructionError(1, _)
        ));
        assert!(simulated.fee_charged);
        assert_eq!(
            before,
            keys.iter()
                .map(|key| svm.get_account(key))
                .collect::<Vec<_>>()
        );
        let sent = svm.send_transaction(tx).unwrap_err();
        assert_eq!(simulated, sent);
        assert!(sent.fee_charged);
        assert_eq!(
            svm.get_balance(&payer.pubkey()).unwrap(),
            before[0].as_ref().unwrap().lamports - sent.meta.fee
        );
        assert_eq!(
            svm.get_account(&source),
            before[keys.iter().position(|key| *key == source).unwrap()]
        );
        if record {
            let balances = sent.meta.balances.unwrap();
            let mut expected = balances.pre_balances.clone();
            expected[0] -= sent.meta.fee;
            assert_eq!(balances.post_balances, expected);
            assert_eq!(balances.pre_token_balances, balances.post_token_balances);
            assert_eq!(balances.pre_token_balances.len(), 2);
        } else {
            assert!(sent.meta.balances.is_none());
        }
    }
}

#[test]
fn rejections_and_fees_only_load_failures_do_not_charge_fees() {
    let (mut svm, payer) = funded_svm(true);
    let recipient = Address::new_unique();
    let expired = transaction(&svm, &payer, &[transfer(&payer.pubkey(), &recipient, 1)]);
    svm.expire_blockhash();
    let missing_program = transaction(
        &svm,
        &payer,
        &[Instruction {
            program_id: Address::new_unique(),
            accounts: vec![],
            data: vec![],
        }],
    );
    let size_limit = transaction(
        &svm,
        &payer,
        &[
            ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(1),
            transfer(&payer.pubkey(), &recipient, 1),
        ],
    );
    for (tx, expected) in [
        (expired, TransactionError::BlockhashNotFound),
        (
            missing_program,
            TransactionError::InvalidProgramForExecution,
        ),
        (
            size_limit,
            TransactionError::MaxLoadedAccountsDataSizeExceeded,
        ),
    ] {
        let simulated = svm.simulate_transaction(tx.clone()).unwrap_err();
        let sent = svm.send_transaction(tx).unwrap_err();
        assert_eq!(simulated, sent);
        assert_eq!(sent.err, expected);
        assert!(!sent.fee_charged);
        assert!(sent.meta.balances.is_none());
        assert_eq!(svm.get_balance(&payer.pubkey()), Some(1_000_000_000));
    }
    let unfunded = Keypair::new();
    let invalid_payer = transaction(
        &svm,
        &unfunded,
        &[transfer(&unfunded.pubkey(), &recipient, 1)],
    );
    let simulated = svm.simulate_transaction(invalid_payer.clone()).unwrap_err();
    let sent = svm.send_transaction(invalid_payer).unwrap_err();
    assert_eq!(sent, simulated);
    assert!(!sent.fee_charged);
    assert!(sent.meta.balances.is_none());
}

#[test]
fn default_recording_is_disabled_for_success() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let tx = transaction(
        &svm,
        &payer,
        &[transfer(&payer.pubkey(), &Address::new_unique(), 1_000_000)],
    );
    assert!(svm
        .simulate_transaction(tx.clone())
        .unwrap()
        .meta
        .balances
        .is_none());
    assert!(svm.send_transaction(tx).unwrap().balances.is_none());
}

#[test]
fn unchecked_transfer_loads_mint_outside_transaction_keys() {
    let (mut svm, payer) = funded_svm(true);
    let program = spl_token_interface::ID;
    let (mint, source, destination, _) = create_tokens(&mut svm, &payer, program);
    #[allow(deprecated)]
    let instruction =
        token_instruction::transfer(&program, &source, &destination, &payer.pubkey(), &[], 40)
            .unwrap();
    let tx = transaction(&svm, &payer, &[instruction]);
    assert!(!tx.message.account_keys.contains(&mint));
    let balances = svm.send_transaction(tx).unwrap().balances.unwrap();
    assert_eq!(balances.pre_token_balances.len(), 2);
    assert_eq!(balances.post_token_balances.len(), 2);
    assert!(balances
        .post_token_balances
        .iter()
        .all(|b| b.mint == mint && b.decimals == 6));
}

#[test]
fn token_recording_requires_program_and_valid_live_mint() {
    let (mut svm, payer) = funded_svm(true);
    let program = spl_token_interface::ID;
    let (mint, source, _, _) = create_tokens(&mut svm, &payer, program);
    let memo = solana_address::address!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
    let make_tx = |svm: &LiteSVM, include_program: bool| {
        let mut accounts = vec![AccountMeta::new_readonly(source, false)];
        if include_program {
            accounts.push(AccountMeta::new_readonly(program, false));
        }
        transaction(
            svm,
            &payer,
            &[Instruction {
                program_id: memo,
                accounts,
                data: b"balance filters".to_vec(),
            }],
        )
    };
    // The memo fails because its accounts are not signers, but still reaches execution.
    let no_program = svm.simulate_transaction(make_tx(&svm, false)).unwrap_err();
    assert!(no_program
        .meta
        .balances
        .unwrap()
        .pre_token_balances
        .is_empty());
    assert_eq!(
        svm.simulate_transaction(make_tx(&svm, true))
            .unwrap_err()
            .meta
            .balances
            .unwrap()
            .pre_token_balances
            .len(),
        1
    );
    let original = svm.get_account(&mint).unwrap();
    for invalid in [
        Account {
            owner: program,
            lamports: 0,
            ..original.clone()
        },
        Account {
            owner: solana_sdk_ids::system_program::ID,
            ..original.clone()
        },
        Account {
            data: vec![0; Mint::LEN],
            ..original
        },
    ] {
        svm.set_account(mint, invalid).unwrap();
        let balances = svm
            .simulate_transaction(make_tx(&svm, true))
            .unwrap_err()
            .meta
            .balances
            .unwrap();
        assert!(balances.pre_token_balances.is_empty());
        assert!(balances.post_token_balances.is_empty());
    }
}

#[test]
fn v0_balances_include_alt_writable_and_readonly_addresses() {
    let (mut svm, payer) = funded_svm(true);
    let program = spl_token_interface::ID;
    let (mint, source, destination, _) = create_tokens(&mut svm, &payer, program);
    let slot = svm.get_sysvar::<Clock>().slot;
    let (create, table) = create_lookup_table(payer.pubkey(), payer.pubkey(), slot);
    let addresses = vec![source, destination, program, mint];
    let extend = extend_lookup_table(
        table,
        payer.pubkey(),
        Some(payer.pubkey()),
        addresses.clone(),
    );
    svm.send_transaction(transaction(&svm, &payer, &[create, extend]))
        .unwrap();
    svm.warp_to_slot(slot + 1);
    let memo = solana_address::address!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
    // Program IDs invoked at the top level stay static; passing Token as an ordinary
    // account resolves it through the ALT and still enables token balance recording.
    let instruction = Instruction {
        program_id: memo,
        accounts: vec![
            AccountMeta::new(source, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(program, false),
            AccountMeta::new_readonly(mint, false),
        ],
        data: b"loaded balances".to_vec(),
    };
    let message = v0::Message::try_compile(
        &payer.pubkey(),
        &[instruction],
        &[AddressLookupTableAccount {
            key: table,
            addresses: addresses.clone(),
        }],
        svm.latest_blockhash(),
    )
    .unwrap();
    assert!(!message.account_keys.contains(&program));
    let lookup = &message.address_table_lookups[0];
    let mut writable_indexes = lookup.writable_indexes.clone();
    let mut readonly_indexes = lookup.readonly_indexes.clone();
    writable_indexes.sort_unstable();
    readonly_indexes.sort_unstable();
    assert_eq!(writable_indexes, vec![0, 1]);
    assert_eq!(readonly_indexes, vec![2, 3]);
    let keys: Vec<_> = message
        .account_keys
        .iter()
        .copied()
        .chain(
            lookup
                .writable_indexes
                .iter()
                .chain(&lookup.readonly_indexes)
                .map(|index| addresses[*index as usize]),
        )
        .collect();
    let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&payer]).unwrap();
    let simulated = svm.simulate_transaction(tx.clone()).unwrap_err();
    let sent = svm.send_transaction(tx).unwrap_err();
    assert_eq!(simulated, sent);
    let balances = sent.meta.balances.unwrap();
    assert_eq!(balances.pre_balances.len(), keys.len());
    assert_eq!(balances.pre_token_balances.len(), 2);
    for (index, key) in keys.iter().enumerate() {
        assert_eq!(
            balances.post_balances[index],
            svm.get_balance(key).unwrap_or_default()
        );
    }
    for balance in balances.pre_token_balances {
        assert!([source, destination].contains(&keys[balance.account_index as usize]));
        assert_eq!(balance.mint, mint);
        assert_eq!(
            balance.amount,
            if keys[balance.account_index as usize] == source {
                1_000
            } else {
                0
            }
        );
    }
}
