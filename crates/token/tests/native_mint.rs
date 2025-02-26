use {
    litesvm::LiteSVM,
    litesvm_token::{
        get_spl_account, spl_token::state::Mint, CreateAssociatedTokenAccount, CreateNativeMint,
        SyncNative,
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

    CreateNativeMint::new(svm, &payer_kp).send().unwrap();

    let mint: Mint = get_spl_account(svm, &spl_token_2022::native_mint::ID).unwrap();

    assert_eq!(mint.decimals, 9);
    assert_eq!(mint.supply, 0);
    assert!(mint.mint_authority.is_none());
    assert!(mint.is_initialized);
    assert_eq!(mint.freeze_authority, None.into());

    let account_pk =
        CreateAssociatedTokenAccount::new(svm, &payer_kp, &spl_token_2022::native_mint::ID)
            .send()
            .unwrap();

    SyncNative::new(svm, &payer_kp, &account_pk).send().unwrap();
}
