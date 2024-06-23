use litesvm::LiteSVM;
use litesvm_helpers::spl::{get_account, get_mint, CreateAccount, CreateMint};
use solana_sdk::{native_token::LAMPORTS_PER_SOL, signature::Keypair, signer::Signer};

#[test]
fn create_mint_test() {
    let mut svm = LiteSVM::new();

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), LAMPORTS_PER_SOL * 10)
        .unwrap();

    let mint = CreateMint::new(&mut svm, &authority).send().unwrap();

    let mint_state = get_mint(&svm, &mint).unwrap();

    assert_eq!(mint_state.decimals, 8);
    assert_eq!(mint_state.mint_authority, Some(authority.pubkey()).into());
}

#[test]
fn create_account_test() {
    let mut svm = LiteSVM::new();

    let authority_kp = Keypair::new();
    let authority_pk = authority_kp.pubkey();

    svm.airdrop(&authority_pk, LAMPORTS_PER_SOL * 10).unwrap();

    let new_account = Keypair::new();

    let mint = CreateMint::new(&mut svm, &authority_kp).send().unwrap();
    let account_pk = CreateAccount::new(&mut svm, &authority_kp, &mint)
        .account_kp(new_account)
        .send()
        .unwrap();

    let account = get_account(&svm, &account_pk).unwrap();

    assert_eq!(account.amount, 0);
    assert_eq!(account.mint, mint);
    assert_eq!(account.owner, authority_pk)
}
