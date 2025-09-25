use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_rent::Rent,
    spl_token_interface::{native_mint::DECIMALS, state::Mint},
};

pub fn create_native_mint(svm: &mut LiteSVM) {
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
        owner: spl_token_interface::ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(spl_token_interface::native_mint::ID, account)
        .unwrap();
}
