use {
    litesvm::LiteSVM,
    litesvm_token::{
        get_spl_account,
        spl_token::{
            instruction::AuthorityType,
            state::{Account, Mint},
        },
        Approve, ApproveChecked, Burn, BurnChecked, CloseAccount, CreateAccount,
        CreateAssociatedTokenAccount, CreateMint, MintTo, MintToChecked, Revoke, SetAuthority,
        Transfer, TransferChecked,
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

    let owner_kp = Keypair::new();
    let owner_pk = owner_kp.pubkey();

    let mint_pk = CreateMint::new(svm, &payer_kp)
        .authority(&owner_pk)
        .send()
        .unwrap();

    let payer_ata_pk = CreateAssociatedTokenAccount::new(svm, &payer_kp, &mint_pk)
        .send()
        .unwrap();

    let mint: Mint = get_spl_account(svm, &mint_pk).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.supply, 0);
    assert_eq!(mint.mint_authority, Some(owner_pk).into());
    assert!(mint.is_initialized);
    assert_eq!(mint.freeze_authority, None.into());

    let owner_account_pk = CreateAccount::new(svm, &payer_kp, &mint_pk)
        .owner(&owner_pk)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 0);
    assert_eq!(owner_account.mint, mint_pk);
    assert_eq!(owner_account.owner, owner_pk);

    MintTo::new(svm, &payer_kp, &mint_pk, &owner_account_pk, 1000)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 1000);

    MintToChecked::new(svm, &payer_kp, &mint_pk, &owner_account_pk, 1000)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 2000);

    Burn::new(svm, &payer_kp, &mint_pk, &owner_account_pk, 500)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 1500);

    BurnChecked::new(svm, &payer_kp, &mint_pk, &owner_account_pk, 500)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 1000);

    Approve::new(svm, &payer_kp, &payer_pk, &owner_account_pk, 500)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 1000);
    assert_eq!(owner_account.delegate.unwrap(), payer_pk);
    assert_eq!(owner_account.delegated_amount, 500);

    Transfer::new(svm, &payer_kp, &mint_pk, &payer_ata_pk, 500)
        .source(&owner_account_pk)
        .send()
        .unwrap();

    let payer_ata: Account = get_spl_account(svm, &payer_ata_pk).unwrap();
    assert_eq!(payer_ata.amount, 500);

    Revoke::new(svm, &payer_kp, &owner_account_pk)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert!(owner_account.delegate.is_none());
    assert_eq!(owner_account.delegated_amount, 0);

    ApproveChecked::new(svm, &payer_kp, &payer_pk, &mint_pk, 500)
        .source(&owner_account_pk)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 500);
    assert_eq!(owner_account.delegate.unwrap(), payer_pk);
    assert_eq!(owner_account.delegated_amount, 500);

    svm.expire_blockhash();

    Transfer::new(svm, &payer_kp, &mint_pk, &payer_ata_pk, 500)
        .source(&owner_account_pk)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 0);

    Revoke::new(svm, &payer_kp, &owner_account_pk)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert!(owner_account.delegate.is_none());
    assert_eq!(owner_account.delegated_amount, 0);

    TransferChecked::new(svm, &payer_kp, &mint_pk, &owner_account_pk, 1000)
        .source(&payer_ata_pk)
        .send()
        .unwrap();

    let owner_account: Account = get_spl_account(svm, &owner_account_pk).unwrap();
    assert_eq!(owner_account.amount, 1000);

    SetAuthority::new(svm, &payer_kp, &payer_ata_pk, AuthorityType::CloseAccount)
        .new_authority(&owner_pk)
        .send()
        .unwrap();

    let payer_ata: Account = get_spl_account(svm, &payer_ata_pk).unwrap();
    assert_eq!(payer_ata.close_authority, Some(owner_pk).into());

    CloseAccount::new(svm, &payer_kp, &payer_ata_pk, &owner_account_pk)
        .owner(&owner_kp)
        .send()
        .unwrap();

    assert!(svm.get_account(&payer_ata_pk).unwrap().data.is_empty());
}
