//! End-to-end decoding against real executions in LiteSVM: native programs
//! (System, SPL Token) with their built-in layouts, a failing transaction,
//! address-lookup-table resolution, and an Anchor-shaped program through a
//! hand-written IDL (decoded without execution — no fixture program needed).

use {
    litesvm::LiteSVM,
    litesvm_scope::{decode_account, decode_instructions, IdlRegistry, ScopeExt},
    solana_account::Account,
    solana_address::{address, Address},
    solana_keypair::Keypair,
    solana_message::{
        v0, AccountMeta, AddressLookupTableAccount, Instruction, Message, VersionedMessage,
    },
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_system_interface::instruction as system_ix,
    solana_transaction::{versioned::VersionedTransaction, Transaction},
    spl_token_interface::{
        instruction as token_ix,
        state::{Account as TokenAccount, Mint},
        ID as TOKEN_ID,
    },
    std::str::FromStr,
};

fn funded_svm() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn legacy_tx(
    svm: &LiteSVM,
    payer: &Keypair,
    signers: &[&Keypair],
    ixs: &[Instruction],
) -> VersionedTransaction {
    let mut all: Vec<&Keypair> = vec![payer];
    all.extend(signers);
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &svm.latest_blockhash());
    VersionedTransaction::from(Transaction::new(&all, msg, svm.latest_blockhash()))
}

/// A System transfer names both roles and the amount; a Token flow (create
/// mint, create account, mint, transfer-checked) names every instruction,
/// every role and every amount, with per-instruction compute attached.
#[test]
fn native_programs_decode_names_roles_args_and_compute() {
    let (mut svm, payer) = funded_svm();
    let registry = IdlRegistry::new();
    let recipient = Address::new_unique();

    let tx = legacy_tx(
        &svm,
        &payer,
        &[],
        &[system_ix::transfer(&payer.pubkey(), &recipient, 1_234_567)],
    );
    let meta = svm.send_transaction(tx.clone()).unwrap();
    let decoded = meta.decode(&svm, &tx.message, &registry);
    assert!(decoded.error.is_none());
    let ix = &decoded.instructions[0];
    assert_eq!(ix.name.as_deref(), Some("Transfer"));
    assert_eq!(ix.program, address!("11111111111111111111111111111111"));
    assert_eq!(ix.args[0].name, "lamports");
    assert_eq!(ix.args[0].value, "1234567");
    assert_eq!(ix.accounts[0].name.as_deref(), Some("From"));
    assert!(ix.accounts[0].signer && ix.accounts[0].writable);
    assert_eq!(ix.accounts[1].name.as_deref(), Some("To"));
    assert_eq!(ix.accounts[1].address, recipient);
    assert!(!ix.accounts[1].signer);
    assert_eq!(ix.success, Some(true));
    assert_eq!(ix.stack_height, 1);

    let mint = Keypair::new();
    let holder = Keypair::new();
    let sink = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(Mint::LEN);
    let rent_acc = svm.minimum_balance_for_rent_exemption(TokenAccount::LEN);
    let ixs = vec![
        system_ix::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            rent,
            Mint::LEN as u64,
            &TOKEN_ID,
        ),
        token_ix::initialize_mint2(&TOKEN_ID, &mint.pubkey(), &payer.pubkey(), None, 6).unwrap(),
        system_ix::create_account(
            &payer.pubkey(),
            &holder.pubkey(),
            rent_acc,
            TokenAccount::LEN as u64,
            &TOKEN_ID,
        ),
        token_ix::initialize_account3(&TOKEN_ID, &holder.pubkey(), &mint.pubkey(), &payer.pubkey())
            .unwrap(),
        system_ix::create_account(
            &payer.pubkey(),
            &sink.pubkey(),
            rent_acc,
            TokenAccount::LEN as u64,
            &TOKEN_ID,
        ),
        token_ix::initialize_account3(&TOKEN_ID, &sink.pubkey(), &mint.pubkey(), &payer.pubkey())
            .unwrap(),
        token_ix::mint_to(
            &TOKEN_ID,
            &mint.pubkey(),
            &holder.pubkey(),
            &payer.pubkey(),
            &[],
            5_000_000,
        )
        .unwrap(),
        token_ix::transfer_checked(
            &TOKEN_ID,
            &holder.pubkey(),
            &mint.pubkey(),
            &sink.pubkey(),
            &payer.pubkey(),
            &[],
            1_500_000,
            6,
        )
        .unwrap(),
    ];
    let tx = legacy_tx(&svm, &payer, &[&mint, &holder, &sink], &ixs);
    let meta = svm.send_transaction(tx.clone()).unwrap();
    let decoded = meta.decode(&svm, &tx.message, &registry);
    let names: Vec<&str> = decoded
        .instructions
        .iter()
        .map(|i| i.name.as_deref().unwrap_or("?"))
        .collect();
    assert_eq!(
        names,
        [
            "Create Account",
            "Initialize Mint 2",
            "Create Account",
            "Initialize Account 3",
            "Create Account",
            "Initialize Account 3",
            "Mint To",
            "Transfer Checked"
        ]
    );
    let create = &decoded.instructions[0];
    assert_eq!(
        create
            .args
            .iter()
            .map(|a| (a.name.as_str(), a.value.as_str()))
            .collect::<Vec<_>>(),
        [("lamports", rent.to_string().as_str()), ("space", "82")]
    );
    let mint_to = &decoded.instructions[6];
    assert_eq!(mint_to.args[0].value, "5000000");
    assert_eq!(
        mint_to
            .accounts
            .iter()
            .map(|a| a.name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["Mint", "Destination", "Authority"]
    );
    let xfer = &decoded.instructions[7];
    assert_eq!(
        xfer.args
            .iter()
            .map(|a| a.value.as_str())
            .collect::<Vec<_>>(),
        ["1500000", "6"]
    );
    assert_eq!(xfer.accounts[0].address, holder.pubkey());
    assert_eq!(xfer.accounts[2].name.as_deref(), Some("Destination"));
    assert!(decoded.instructions.iter().all(|i| i.success == Some(true)));
    // SPL Token logs `consumed N of M`; the System program (a builtin) does not.
    assert!(
        decoded
            .instructions
            .iter()
            .filter(|i| i.program == TOKEN_ID)
            .all(|i| i.compute_units.is_some_and(|c| c > 0)),
        "token instructions carry compute"
    );
    assert!(decoded
        .instructions
        .iter()
        .filter(|i| i.program != TOKEN_ID)
        .all(|i| i.compute_units.is_none()));
    assert!(decoded.failing_instruction().is_none());

    // Account decoding through the built-in SPL layouts.
    let acc = svm.get_account(&sink.pubkey()).unwrap();
    let dec = decode_account(&registry, &acc.owner, &acc.data).unwrap();
    assert_eq!(dec.type_name, "SPL Token Account");
    let amount = dec.fields.iter().find(|f| f.name == "amount").unwrap();
    assert_eq!(amount.value, "1500000");
    let mint_acc = svm.get_account(&mint.pubkey()).unwrap();
    let dec = decode_account(&registry, &mint_acc.owner, &mint_acc.data).unwrap();
    assert_eq!(dec.type_name, "SPL Mint");
    assert_eq!(
        dec.fields
            .iter()
            .find(|f| f.name == "supply")
            .unwrap()
            .value,
        "5000000"
    );

    let pretty = decoded.pretty();
    assert!(
        pretty.contains("Transfer Checked")
            && pretty.contains("amount=1500000")
            && pretty.contains("✓")
    );
}

/// A transaction that fails inside SPL Token: the failing instruction is
/// pinpointed, the Token error is named from its code, and the
/// transaction-level error agrees.
#[test]
fn failing_transaction_names_the_error_and_the_instruction() {
    let (mut svm, payer) = funded_svm();
    let registry = IdlRegistry::new();
    let mint = Keypair::new();
    let holder = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(Mint::LEN);
    let rent_acc = svm.minimum_balance_for_rent_exemption(TokenAccount::LEN);
    let setup = vec![
        system_ix::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            rent,
            Mint::LEN as u64,
            &TOKEN_ID,
        ),
        token_ix::initialize_mint2(&TOKEN_ID, &mint.pubkey(), &payer.pubkey(), None, 0).unwrap(),
        system_ix::create_account(
            &payer.pubkey(),
            &holder.pubkey(),
            rent_acc,
            TokenAccount::LEN as u64,
            &TOKEN_ID,
        ),
        token_ix::initialize_account3(&TOKEN_ID, &holder.pubkey(), &mint.pubkey(), &payer.pubkey())
            .unwrap(),
    ];
    svm.send_transaction(legacy_tx(&svm, &payer, &[&mint, &holder], &setup))
        .unwrap();

    // Transfer more than the (empty) account holds, after a harmless transfer.
    let ixs = vec![
        system_ix::transfer(&payer.pubkey(), &Address::new_unique(), 1),
        token_ix::transfer(
            &TOKEN_ID,
            &holder.pubkey(),
            &holder.pubkey(),
            &payer.pubkey(),
            &[],
            99,
        )
        .unwrap(),
    ];
    let tx = legacy_tx(&svm, &payer, &[], &ixs);
    let failed = svm.send_transaction(tx.clone()).unwrap_err();
    let decoded = failed.decode(&svm, &tx.message, &registry);

    let err = decoded.error.as_ref().expect("transaction-level error");
    assert_eq!(err.name, "InsufficientFunds");
    assert_eq!(err.code, Some(1));
    assert_eq!(err.program, Some(TOKEN_ID));
    let failing = decoded.failing_instruction().expect("failing instruction");
    assert_eq!(failing.name.as_deref(), Some("Transfer"));
    assert_eq!(failing.program, TOKEN_ID);
    assert_eq!(
        failing.error.as_ref().map(|e| e.name.as_str()),
        Some("InsufficientFunds")
    );
    assert_eq!(
        decoded.instructions[0].success,
        Some(true),
        "the transfer before it ran fine"
    );
    assert_eq!(decoded.instructions[1].success, Some(false));
}

/// An Anchor-shaped program decoded through a hand-written IDL, without
/// executing anything: name, arguments, account roles, the variadic tail,
/// a custom error, an event, and an account by discriminator.
#[test]
fn anchor_program_decodes_through_its_idl() {
    let program = Address::new_unique();
    let mut registry = IdlRegistry::new();
    registry.insert(
        program,
        &serde_json::json!({
            "instructions": [{
                "name": "claim_vested",
                "discriminator": [10, 20, 30, 40, 50, 60, 70, 80],
                "accounts": [
                    { "name": "beneficiary", "writable": true, "signer": true },
                    { "name": "schedule", "writable": true }
                ],
                "args": [{ "name": "schedule_id", "type": "u64" }, { "name": "note", "type": "string" }]
            }],
            "accounts": [{ "name": "VestingSchedule", "discriminator": [1, 1, 1, 1, 1, 1, 1, 1] }],
            "types": [
                { "name": "VestingSchedule", "type": { "kind": "struct", "fields": [
                    { "name": "creator", "type": "pubkey" },
                    { "name": "cliff_ts", "type": "i64" },
                    { "name": "claimed_amount", "type": "u64" }
                ] } },
                { "name": "VestingClaimed", "type": { "kind": "struct", "fields": [{ "name": "amount", "type": "u64" }] } }
            ],
            "events": [{ "name": "VestingClaimed", "discriminator": [7, 7, 7, 7, 7, 7, 7, 7] }],
            "errors": [{ "code": 6003, "name": "CliffNotReached", "msg": "The vesting cliff has not been reached" }]
        }),
    );

    let beneficiary = Keypair::new();
    let schedule = Address::new_unique();
    let extra = Address::new_unique();
    let mut data = vec![10u8, 20, 30, 40, 50, 60, 70, 80];
    data.extend_from_slice(&42u64.to_le_bytes());
    let ix = Instruction {
        program_id: program,
        accounts: vec![
            AccountMeta::new(beneficiary.pubkey(), true),
            AccountMeta::new(schedule, false),
            AccountMeta::new_readonly(extra, false),
        ],
        data,
    };
    let message = VersionedMessage::Legacy(Message::new(&[ix], Some(&beneficiary.pubkey())));
    let keys = message.static_account_keys().to_vec();
    let decoded = decode_instructions(&message, &keys, &Vec::new(), &registry);
    let ix = &decoded[0];
    assert_eq!(ix.name.as_deref(), Some("Claim Vested"));
    assert_eq!(ix.idl_name.as_deref(), Some("claim_vested"));
    assert_eq!(ix.args[0].name, "schedule_id");
    assert_eq!(ix.args[0].value, "42");
    assert_eq!(ix.args[1].name, "note");
    assert_eq!(
        ix.args[1].value, "",
        "a variable-length arg is named but not read"
    );
    let roles: Vec<Option<&str>> = ix.accounts.iter().map(|a| a.name.as_deref()).collect();
    assert_eq!(
        roles,
        [
            Some("Beneficiary"),
            Some("Schedule"),
            Some("Remaining Account #1")
        ]
    );
    assert!(ix.accounts[0].signer && ix.accounts[0].writable);
    assert!(!ix.accounts[2].writable);

    // Custom error, framework error, unknown code.
    let e = litesvm_scope::error_name(&registry, &program, 6003);
    assert_eq!(
        (e.name.as_str(), e.message.as_deref()),
        (
            "CliffNotReached",
            Some("The vesting cliff has not been reached")
        )
    );
    assert_eq!(
        litesvm_scope::error_name(&registry, &program, 2005).name,
        "ConstraintRentExempt"
    );
    assert_eq!(
        litesvm_scope::error_name(&registry, &program, 6999).name,
        "Custom error 6999"
    );

    // Event and account by discriminator.
    let mut ev = vec![7u8; 8];
    ev.extend_from_slice(&500u64.to_le_bytes());
    let ev = litesvm_scope::decode_event(&registry, Some(&program), &ev).unwrap();
    assert_eq!(
        (ev.name.as_str(), ev.fields[0].value.as_str()),
        ("VestingClaimed", "500")
    );
    let mut acc = vec![1u8; 8];
    acc.extend_from_slice(&[9u8; 32]);
    acc.extend_from_slice(&(-86_400i64).to_le_bytes());
    acc.extend_from_slice(&0u64.to_le_bytes());
    let dec = decode_account(&registry, &program, &acc).unwrap();
    assert_eq!(dec.type_name, "VestingSchedule");
    assert_eq!(
        dec.fields
            .iter()
            .find(|f| f.name == "cliff_ts")
            .unwrap()
            .value,
        "-86400"
    );
    assert_eq!(
        dec.fields
            .iter()
            .find(|f| f.name == "creator")
            .unwrap()
            .value,
        Address::from([9u8; 32]).to_string()
    );
    assert!(
        decode_account(&registry, &Address::new_unique(), &acc).is_none(),
        "no layout, no IDL: nothing"
    );
}

/// A v0 message whose accounts live in an address lookup table: the keys
/// resolve from the table account in the SVM, in the runtime's order
/// (static, then loaded-writable, then loaded-readonly), and the decoded
/// instruction sees the right addresses and privileges.
#[test]
fn lookup_table_addresses_resolve_from_the_svm() {
    let (mut svm, payer) = funded_svm();
    let table = Address::new_unique();
    let writable_addr = Address::new_unique();
    let readonly_addr = Address::new_unique();
    let mut data = vec![0u8; 56];
    data[0] = 1; // LookupTable variant
    data[21] = 0; // no authority
    data.extend_from_slice(readonly_addr.as_ref());
    data.extend_from_slice(writable_addr.as_ref());
    svm.set_account(
        table,
        Account {
            lamports: 10_000_000,
            data,
            owner: address!("AddressLookupTab1e1111111111111111111111111"),
            ..Default::default()
        },
    )
    .unwrap();
    let lookup = AddressLookupTableAccount {
        key: table,
        addresses: vec![readonly_addr, writable_addr],
    };
    let ix = system_ix::transfer(&payer.pubkey(), &writable_addr, 7);
    let mut ix = ix;
    ix.accounts
        .push(AccountMeta::new_readonly(readonly_addr, false));
    let msg = v0::Message::try_compile(&payer.pubkey(), &[ix], &[lookup], svm.latest_blockhash())
        .unwrap();
    let message = VersionedMessage::V0(msg);
    let keys = litesvm_scope::resolve_account_keys(&svm, &message);
    assert_eq!(keys.len(), message.static_account_keys().len() + 2);
    assert_eq!(
        &keys[keys.len() - 2..],
        [writable_addr, readonly_addr],
        "writable loaded first, then readonly"
    );
    let decoded = decode_instructions(&message, &keys, &Vec::new(), &IdlRegistry::new());
    let ix = &decoded[0];
    assert_eq!(ix.accounts[1].address, writable_addr);
    assert!(ix.accounts[1].writable && !ix.accounts[1].signer);
    assert_eq!(ix.accounts[2].address, readonly_addr);
    assert!(!ix.accounts[2].writable);
    assert!(Address::from_str(&writable_addr.to_string()).is_ok());
}
