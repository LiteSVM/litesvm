use {
    litesvm::LiteSVM,
    litesvm_token::{
        get_spl_account,
        spl_token::state::{Account, Mint},
        CreateAssociatedTokenAccountIdempotent, CreateMint, FreezeAccount, ThawAccount,
    },
    solana_keypair::Keypair,
    solana_native_token::LAMPORTS_PER_SOL,
    solana_signer::Signer,
};

#[test]
fn test() {
    let svm = &mut LiteSVM::new();

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();

    svm.airdrop(&payer_pk, LAMPORTS_PER_SOL * 10).unwrap();

    let mint_pk = CreateMint::new(svm, &payer_kp)
        .authority(&payer_pk)
        .freeze_authority(&payer_pk)
        .send()
        .unwrap();

    let mint: Mint = get_spl_account(svm, &mint_pk).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.supply, 0);
    assert_eq!(mint.mint_authority, Some(payer_pk).into());
    assert!(mint.is_initialized);
    assert_eq!(mint.freeze_authority, Some(payer_pk).into());

    let account_pk = CreateAssociatedTokenAccountIdempotent::new(svm, &payer_kp, &mint_pk)
        .send()
        .unwrap();

    FreezeAccount::new(svm, &payer_kp, &mint_pk).send().unwrap();

    let account: Account = get_spl_account(svm, &account_pk).unwrap();
    assert!(account.is_frozen());

    ThawAccount::new(svm, &payer_kp, &mint_pk).send().unwrap();

    let account: Account = get_spl_account(svm, &account_pk).unwrap();
    assert!(!account.is_frozen());
}
