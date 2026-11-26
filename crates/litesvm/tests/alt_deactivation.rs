//! Address lookup table resolution against the deactivation window.

use {
    litesvm::LiteSVM,
    solana_address::Address,
    solana_address_lookup_table_interface::instruction::{
        create_lookup_table, deactivate_lookup_table, extend_lookup_table,
    },
    solana_clock::Clock,
    solana_keypair::Keypair,
    solana_message::{
        v0::Message as MessageV0, AddressLookupTableAccount, Message, VersionedMessage,
    },
    solana_native_token::LAMPORTS_PER_SOL,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::{versioned::VersionedTransaction, Transaction},
    solana_transaction_error::TransactionError,
};

/// Sends one lamport to `destination`, resolving the destination through
/// `table` instead of listing it statically.
fn transfer_via_lookup_table(
    svm: &mut LiteSVM,
    payer: &Keypair,
    table: Address,
    destination: Address,
) -> Result<(), TransactionError> {
    let blockhash = svm.latest_blockhash();
    let table_account = AddressLookupTableAccount {
        key: table,
        addresses: vec![destination],
    };
    let message = MessageV0::try_compile(
        &payer.pubkey(),
        &[transfer(&payer.pubkey(), &destination, 1)],
        &[table_account],
        blockhash,
    )
    .unwrap();
    let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[payer]).unwrap();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|failed| failed.err)
}

/// Creates and extends a lookup table that holds `destination`, at the current
/// slot, and returns the table's address.
fn create_and_extend_table(svm: &mut LiteSVM, payer: &Keypair, destination: Address) -> Address {
    let start = svm.get_sysvar::<Clock>().slot;
    let (create_ix, table) = create_lookup_table(payer.pubkey(), payer.pubkey(), start);
    let extend_ix = extend_lookup_table(
        table,
        payer.pubkey(),
        Some(payer.pubkey()),
        vec![destination],
    );
    let setup = Transaction::new(
        &[payer],
        Message::new(&[create_ix, extend_ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(setup).unwrap();
    table
}

fn new_svm() -> (LiteSVM, Keypair, Keypair) {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let destination = Keypair::new();
    svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&destination.pubkey(), LAMPORTS_PER_SOL)
        .unwrap();
    (svm, payer, destination)
}

#[test]
fn deactivated_lookup_table_still_resolves_during_its_cooldown() {
    let (mut svm, payer, destination) = new_svm();
    let start = svm.get_sysvar::<Clock>().slot;
    let table = create_and_extend_table(&mut svm, &payer, destination.pubkey());

    // The extension is only active in a slot after `last_extended_slot`.
    svm.warp_to_slot(start + 1);
    let deactivate_ix = deactivate_lookup_table(table, payer.pubkey());
    let deactivate_tx = Transaction::new(
        &[&payer],
        Message::new(&[deactivate_ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(deactivate_tx).unwrap();

    // In the slot the table was deactivated, it is still active.
    transfer_via_lookup_table(&mut svm, &payer, table, destination.pubkey())
        .expect("same-slot lookup on a deactivated table should resolve");

    svm.expire_blockhash();
    svm.warp_to_slot(start + 2);

    // One slot later the table is inside its 512-slot deactivation cooldown: a
    // validator still resolves it.
    let result = transfer_via_lookup_table(&mut svm, &payer, table, destination.pubkey());
    assert!(
        result.is_ok(),
        "a lookup table inside its deactivation cooldown was rejected: {:?}",
        result.err()
    );
}

#[test]
fn fully_deactivated_lookup_table_is_rejected() {
    let (mut svm, payer, destination) = new_svm();
    let start = svm.get_sysvar::<Clock>().slot;
    let table = create_and_extend_table(&mut svm, &payer, destination.pubkey());

    let deactivate_ix = deactivate_lookup_table(table, payer.pubkey());
    let deactivate_tx = Transaction::new(
        &[&payer],
        Message::new(&[deactivate_ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(deactivate_tx).unwrap();

    // Past the 512-slot window the deactivation slot leaves SlotHashes and the
    // table can no longer be resolved.
    svm.warp_to_slot(start + 600);
    let result = transfer_via_lookup_table(&mut svm, &payer, table, destination.pubkey());
    assert!(
        result.is_err(),
        "a fully deactivated lookup table was still resolved"
    );
}

#[test]
fn deactivation_window_boundary_is_512_slots() {
    let (mut svm, payer, destination) = new_svm();
    let start = svm.get_sysvar::<Clock>().slot;
    let table = create_and_extend_table(&mut svm, &payer, destination.pubkey());

    let deactivate_ix = deactivate_lookup_table(table, payer.pubkey());
    let deactivate_tx = Transaction::new(
        &[&payer],
        Message::new(&[deactivate_ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(deactivate_tx).unwrap();

    // 512 slots later the deactivation slot is still the oldest entry in the
    // 512-slot window, so the table still resolves: while slot C is
    // processing, a validator's SlotHashes holds C-1 down to C-512.
    svm.warp_to_slot(start + 512);
    svm.expire_blockhash();
    let result = transfer_via_lookup_table(&mut svm, &payer, table, destination.pubkey());
    assert!(
        result.is_ok(),
        "a lookup table at the edge of its deactivation window was rejected: {:?}",
        result.err()
    );

    // One slot further the deactivation slot leaves SlotHashes and the table
    // can no longer be resolved.
    svm.warp_to_slot(start + 513);
    svm.expire_blockhash();
    let result = transfer_via_lookup_table(&mut svm, &payer, table, destination.pubkey());
    assert!(
        result.is_err(),
        "a fully deactivated lookup table was still resolved"
    );
}
