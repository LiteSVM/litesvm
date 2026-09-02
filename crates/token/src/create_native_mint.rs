use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_program_pack::Pack,
    spl_token_interface::{native_mint::DECIMALS, state::Mint},
};

pub fn create_native_mint(svm: &mut LiteSVM) {
    let mut data = vec![0; Mint::LEN];
    let mint = Mint {
        decimals: DECIMALS,
        is_initialized: true,
        ..Mint::default()
    };
    Mint::pack(mint, &mut data).unwrap();
    let account = Account {
        lamports: svm.minimum_balance_for_rent_exemption(data.len()),
        data,
        owner: spl_token_interface::ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(spl_token_interface::native_mint::ID, account)
        .unwrap();
}
