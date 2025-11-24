use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_rent::Rent,
    solana_pubkey::Pubkey,
    spl_token_interface::{native_mint::DECIMALS, state::Mint},
};

fn create_native_mint_with_program_id(svm: &mut LiteSVM, address: Pubkey, token_program: Pubkey) {
    let mut data = vec![0; Mint::LEN];
    let mint = Mint {
        mint_authority: COption::None,
        supply: 0,
        decimals: DECIMALS,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    Mint::pack(mint, &mut data).unwrap();
    let account = Account {
        lamports: svm.get_sysvar::<Rent>().minimum_balance(data.len()),
        data,
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(address, account)
        .unwrap();
}

pub fn create_native_mint(svm: &mut LiteSVM) {
    create_native_mint_with_program_id(svm, spl_token_interface::native_mint::ID, spl_token_interface::ID);
}

pub fn create_native_mint_2022(svm: &mut LiteSVM) {
    create_native_mint_with_program_id(svm, spl_token_2022_interface::native_mint::ID, spl_token_2022_interface::ID);
}