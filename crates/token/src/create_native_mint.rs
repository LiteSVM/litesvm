use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_program_pack::Pack,
    solana_rent::Rent,
    spl_token::{native_mint::DECIMALS, solana_program::program_option::COption, state::Mint},
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
        owner: spl_token::ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(spl_token::native_mint::ID, account)
        .unwrap();
}
