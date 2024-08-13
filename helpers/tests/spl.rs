use litesvm::LiteSVM;
use litesvm_helpers::spl::spl_token::state::{Account, Mint, Multisig};
use litesvm_helpers::spl::{
    get_spl_account, Approve, ApproveChecked, Burn, BurnChecked, CloseAccount, CreateAccount,
    CreateMint, CreateMultisig, CreateNativeMint, MintTo, MintToChecked, Revoke, Transfer,
    TransferChecked,
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

    // Transfer::new(svm, &payer_kp, &mint_pk, &random_account_pk, 500)
    //     .source(&multisig_account_pk)
    //     .owner(&random_kp)
    //     .send()
    //     .unwrap();

    // let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    // assert_eq!(multisig_account.amount, 500);

    Revoke::new(svm, &payer_kp, &multisig_account_pk)
        .multisig(&multisig_pk, &[&signer1, &signer2])
        .send()
        .unwrap();

    TransferChecked::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 500)
        .source(&random_account_pk)
        .owner(&random_kp)
        .send()
        .unwrap();

    CloseAccount::new(svm, &payer_kp, &random_account_pk, &multisig_account_pk)
        .owner(&random_kp)
        .send()
        .unwrap();
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

    let mint: Mint = get_spl_account(svm, &mint_pk).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.supply, 0);
    assert_eq!(mint.mint_authority, Some(owner_pk).into());
    assert!(mint.is_initialized);
    assert_eq!(mint.freeze_authority, None.into());

    let multisig_account_pk = CreateAccount::new(svm, &payer_kp, &mint_pk)
        .owner(&owner_pk)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 0);
    assert_eq!(multisig_account.mint, mint_pk);
    assert_eq!(multisig_account.owner, owner_pk);

    MintTo::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 1000)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1000);

    MintToChecked::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 1000)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 2000);

    Burn::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 500)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1500);

    BurnChecked::new(svm, &payer_kp, &mint_pk, &multisig_account_pk, 500)
        .owner(&owner_kp)
        .send()
        .unwrap();

    let multisig_account: Account = get_spl_account(svm, &multisig_account_pk).unwrap();
    assert_eq!(multisig_account.amount, 1000);
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
}
