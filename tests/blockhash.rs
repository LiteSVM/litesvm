use litesvm::LiteSVM;
use solana_program::{message::Message, pubkey::Pubkey, system_instruction::transfer};
use solana_sdk::{
    account::ReadableAccount,
    account_utils::StateMut,
    nonce::{
        state::{Data, Versions},
        State as NonceState,
    },
    rent::Rent,
    signature::Keypair,
    signer::Signer,
    system_instruction::advance_nonce_account,
    transaction::{Transaction, TransactionError},
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
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new();

    svm.airdrop(&from, svm.get_sysvar::<Rent>().minimum_balance(0))
        .unwrap();
    let instruction = transfer(&from, &to, 1);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        solana_sdk::hash::Hash::new_unique(),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(tx_res.unwrap_err().err, TransactionError::BlockhashNotFound);
}

#[test_log::test]
fn test_durable_nonce() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();
    let nonce_kp = Keypair::new();

    let mut svm = LiteSVM::new();

    svm.airdrop(&from, 1_000_000_000).unwrap();
    let create_nonce_ixns = solana_program::system_instruction::create_nonce_account(
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
