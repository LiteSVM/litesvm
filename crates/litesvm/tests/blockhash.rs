use {
    litesvm::LiteSVM,
    solana_account::{state_traits::StateMut, ReadableAccount},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_nonce::{
        state::{Data, State as NonceState},
        versions::Versions,
    },
    solana_rent::Rent,
    solana_signer::Signer,
    solana_system_interface::instruction::{advance_nonce_account, transfer},
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

fn data_from_state(state: &NonceState) -> &Data {
    match state {
        NonceState::Uninitialized => panic!("Expecting Initialized here"),
        NonceState::Initialized(data) => data,
    }
}

fn data_from_account<T: ReadableAccount + StateMut<Versions>>(account: &T) -> Data {
    data_from_state(&state_from_account(account).clone()).clone()
}

fn state_from_account<T: ReadableAccount + StateMut<Versions>>(account: &T) -> NonceState {
    let versions = StateMut::<Versions>::state(account).unwrap();
    NonceState::from(versions)
}

#[test_log::test]
fn test_invalid_blockhash() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();

    svm.airdrop(&from, svm.get_sysvar::<Rent>().minimum_balance(0))
        .unwrap();
    let instruction = transfer(&from, &to, 1);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        solana_hash::Hash::new_unique(),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(tx_res.unwrap_err().err, TransactionError::BlockhashNotFound);
}

#[test_log::test]
fn test_durable_nonce() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();
    let nonce_kp = Keypair::new();

    let mut svm = LiteSVM::new();

    svm.airdrop(&from, 1_000_000_000).unwrap();
    let create_nonce_ixns = solana_system_interface::instruction::create_nonce_account(
        &from,
        &nonce_kp.pubkey(),
        &from,
        1_500_000,
    );
    let tx = Transaction::new(
        &[&from_keypair, &nonce_kp],
        Message::new_with_blockhash(&create_nonce_ixns, Some(&from), &svm.latest_blockhash()),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
    let nonce_account_raw = svm.get_account(&nonce_kp.pubkey()).unwrap();
    let transfer_ix = transfer(&from, &to, 1);
    let advance_ix = advance_nonce_account(&nonce_kp.pubkey(), &from);
    let parsed = data_from_account(&nonce_account_raw);
    let nonce = parsed.blockhash();
    let msg = Message::new_with_blockhash(&[advance_ix, transfer_ix], Some(&from), &nonce);
    let tx_using_nonce = Transaction::new(&[&from_keypair], msg, nonce);
    svm.expire_blockhash();
    svm.send_transaction(tx_using_nonce).unwrap();
}
