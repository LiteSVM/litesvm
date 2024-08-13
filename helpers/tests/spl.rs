use litesvm::LiteSVM;
use litesvm_helpers::spl::{
    get_spl_account,
    spl_token::{
        instruction::AuthorityType,
        state::{Account, Mint, Multisig},
    },
    Approve, ApproveChecked, Burn, BurnChecked, CloseAccount, CreateAccount,
    CreateAssociatedTokenAccount, CreateAssociatedTokenAccountIdempotent, CreateMint,
    CreateMultisig, CreateNativeMint, FreezeAccount, MintTo, MintToChecked, Revoke, SetAuthority,
    SyncNative, ThawAccount, Transfer, TransferChecked,
};
use solana_sdk::{native_token::LAMPORTS_PER_SOL, signature::Keypair, signer::Signer};

#[test]
fn spl_multisig() {
    let svm = &mut LiteSVM::new();

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();

    svm.airdrop(&payer_pk, LAMPORTS_PER_SOL * 10).unwrap();

    let signer1 = Keypair::new();
    let signer2 = Keypair::new();
    let signer3 = Keypair::new();

    let random_kp = Keypair::new();
    let random_pk = random_kp.pubkey();

    let multisig_pk = CreateMultisig::new(
        svm,
        &payer_kp,
        &[&signer1.pubkey(), &signer2.pubkey(), &signer3.pubkey()],
        2,
    )
    .send()
    .unwrap();

    let multisig: Multisig = get_spl_account(svm, &multisig_pk).unwrap();

    assert_eq!(multisig.m, 2);
    assert!(multisig.is_initialized);
    assert_eq!(multisig.n, 3);
    assert!(multisig.signers.contains(&signer1.pubkey()));
    assert!(multisig.signers.contains(&signer2.pubkey()));
    assert!(multisig.signers.contains(&signer3.pubkey()));

    let mint_pk = CreateMint::new(svm, &payer_kp)
        .authority(&multisig_pk)
        .send()
        .unwrap();

    let mint: Mint = get_spl_account(svm, &mint_pk).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.supply, 0);
    assert_eq!(mint.mint_authority, Some(multisig_pk).into());
    assert!(mint.is_initialized);
    assert_eq!(mint.freeze_authority, None.into());

    let multisig_account_pk = CreateAccount::new(svm, &payer_kp, &mint_pk)
        .owner(&multisig_pk)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 0);
    assert_eq!(multisig_account.mint, mint_pk);
    assert_eq!(multisig_account.owner, multisig_pk);

    let random_account_pk = CreateAccount::new(svm, &payer_kp, &mint_pk)
        .owner(&random_pk)
        .send()
        .unwrap();

    MintTo::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 1000)
        .multisig(&multisig_pk, &[&signer1, &signer3])
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1000);

    MintToChecked::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 1000)
        .multisig(&multisig_pk, &[&signer2, &signer3])
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 2000);

    Burn::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 250)
        .multisig(&multisig_pk, &[&signer1, &signer3])
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1750);

    BurnChecked::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 250)
        .multisig(&multisig_pk, &[&signer1, &signer3])
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1500);

    Approve::new(svm, &payer_kp, &random_pk, &multisig_account_pk, 500)
        .multisig(&multisig_pk, &[&signer1, &signer3])
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1500);
    assert_eq!(multisig_account.delegate.unwrap(), random_pk);
    assert_eq!(multisig_account.delegated_amount, 500);

    Transfer::new(svm, &payer_kp, &mint_pk, &random_account_pk, 500)
        .source(&multisig_account_pk)
        .owner(&random_kp)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1000);

    Revoke::new(svm, &payer_kp, &multisig_account_pk)
        .multisig(&multisig_pk, &[&signer1, &signer2])
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert!(multisig_account.delegate.is_none());
    assert_eq!(multisig_account.delegated_amount, 0);

    ApproveChecked::new(svm, &payer_kp, &random_pk, &mint_pk, 500)
        .source(&multisig_account_pk)
        .multisig(&multisig_pk, &[&signer1, &signer3])
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1000);
    assert_eq!(multisig_account.delegate.unwrap(), random_pk);
    assert_eq!(multisig_account.delegated_amount, 500);

    svm.expire_blockhash();

    Transfer::new(svm, &payer_kp, &mint_pk, &random_account_pk, 500)
        .source(&multisig_account_pk)
        .owner(&random_kp)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 500);

    Revoke::new(svm, &payer_kp, &multisig_account_pk)
        .multisig(&multisig_pk, &[&signer1, &signer2])
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert!(multisig_account.delegate.is_none());
    assert_eq!(multisig_account.delegated_amount, 0);

    TransferChecked::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 1000)
        .source(&random_account_pk)
        .owner(&random_kp)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1500);

    SetAuthority::new(
        svm,
        &payer_kp,
        &random_account_pk,
        AuthorityType::CloseAccount,
    )
    .owner(&random_kp)
    .new_authority(&multisig_pk)
    .send()
    .unwrap();

    let random_account: Account = get_spl_account(svm, &random_account_pk).unwrap();
    assert_eq!(random_account.close_authority, Some(multisig_pk).into());

    CloseAccount::new(svm, &payer_kp, &random_account_pk, &multisig_account_pk)
        .multisig(&multisig_pk, &[&signer1, &signer2])
        .send()
        .unwrap();

    assert!(svm.get_account(&random_account_pk).unwrap().data.is_empty());
}

#[test]
fn spl_one_owner() {
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

#[test]
fn spl_native_mint() {
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

#[test]
fn spl_freeze() {
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
