#![cfg(feature = "token-2022")]

use {
    litesvm::LiteSVM,
    litesvm_token::{
        spl_token::{
            extension::{
                transfer_fee::{instruction::initialize_transfer_fee_config, TransferFeeAmount},
                transfer_hook::instruction::initialize as initialize_transfer_hook,
                BaseStateWithExtensions, ExtensionType, StateWithExtensions,
            },
            instruction::{initialize_mint2, initialize_non_transferable_mint},
            state::{Account, Mint},
        },
        CreateAccount, CreateAssociatedTokenAccount, CreateMint, MintToChecked, TransferChecked,
        TOKEN_ID,
    },
    solana_address::Address,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_transaction::Transaction,
};

#[test]
fn accounts_support_mint_required_extensions() {
    for extension in [
        ExtensionType::TransferFeeConfig,
        ExtensionType::TransferHook,
        ExtensionType::NonTransferable,
    ] {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        let mint = Keypair::new();
        let mint_pk = mint.pubkey();
        let payer_pk = payer.pubkey();
        svm.airdrop(&payer_pk, 1_000_000_000).unwrap();
        let mint_len = ExtensionType::try_calculate_account_len::<Mint>(&[extension]).unwrap();
        let initialize_extension = match extension {
            ExtensionType::TransferFeeConfig => initialize_transfer_fee_config(
                &TOKEN_ID,
                &mint_pk,
                Some(&payer_pk),
                Some(&payer_pk),
                100,
                100,
            )
            .unwrap(),
            ExtensionType::TransferHook => {
                initialize_transfer_hook(&TOKEN_ID, &mint_pk, Some(payer_pk), None).unwrap()
            }
            ExtensionType::NonTransferable => {
                initialize_non_transferable_mint(&TOKEN_ID, &mint_pk).unwrap()
            }
            _ => unreachable!(),
        };
        let instructions = [
            create_account(
                &payer_pk,
                &mint_pk,
                svm.minimum_balance_for_rent_exemption(mint_len),
                mint_len as u64,
                &TOKEN_ID,
            ),
            initialize_extension,
            initialize_mint2(&TOKEN_ID, &mint_pk, &payer_pk, None, 6).unwrap(),
        ];
        svm.send_transaction(Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer_pk),
            &[&payer, &mint],
            svm.latest_blockhash(),
        ))
        .unwrap();

        let account_kp = Keypair::new();
        let account_pk = account_kp.pubkey();
        let returned = CreateAccount::new(&mut svm, &payer, &mint_pk)
            .owner(&owner.pubkey())
            .account_kp(account_kp)
            .send()
            .unwrap();
        assert_eq!(returned, account_pk);
        MintToChecked::new(&mut svm, &payer, &mint_pk, &account_pk, 1000)
            .send()
            .unwrap();
        let raw = svm.get_account(&account_pk).unwrap();
        let account = StateWithExtensions::<Account>::unpack(&raw.data).unwrap();
        assert_eq!(account.base.owner, owner.pubkey());
        assert_eq!(account.base.amount, 1000);
        assert_eq!(raw.owner, TOKEN_ID);
        assert_eq!(
            raw.lamports,
            svm.minimum_balance_for_rent_exemption(raw.data.len())
        );
        let expected_extension = match extension {
            ExtensionType::TransferFeeConfig => ExtensionType::TransferFeeAmount,
            ExtensionType::TransferHook => ExtensionType::TransferHookAccount,
            ExtensionType::NonTransferable => ExtensionType::NonTransferableAccount,
            _ => unreachable!(),
        };
        assert!(account
            .get_extension_types()
            .unwrap()
            .contains(&expected_extension));
        if extension == ExtensionType::TransferFeeConfig {
            let destination = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint_pk)
                .send()
                .unwrap();
            TransferChecked::new(&mut svm, &payer, &mint_pk, &destination, 100)
                .source(&account_pk)
                .owner(&owner)
                .send()
                .unwrap();
            let raw = svm.get_account(&destination).unwrap();
            let destination = StateWithExtensions::<Account>::unpack(&raw.data).unwrap();
            assert_eq!(destination.base.amount, 99);
            assert_eq!(
                u64::from(
                    destination
                        .get_extension::<TransferFeeAmount>()
                        .unwrap()
                        .withheld_amount
                ),
                1
            );
        }
    }
}

#[test]
fn explicit_classic_program_works_with_token_2022_feature_enabled() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let classic_id = solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let mint = CreateMint::new(&mut svm, &payer)
        .token_program_id(&classic_id)
        .send()
        .unwrap();
    let account = CreateAccount::new(&mut svm, &payer, &mint)
        .token_program_id(&classic_id)
        .send()
        .unwrap();
    let raw = svm.get_account(&account).unwrap();
    assert_eq!(raw.owner, classic_id);
    assert_eq!(raw.data.len(), 165);
}

#[test]
fn invalid_mints_do_not_create_accounts() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    for mint in [Address::new_unique(), payer.pubkey()] {
        let account = Keypair::new();
        let account_pk = account.pubkey();
        assert!(CreateAccount::new(&mut svm, &payer, &mint)
            .account_kp(account)
            .send()
            .is_err());
        assert!(svm.get_account(&account_pk).is_none());
    }
}
