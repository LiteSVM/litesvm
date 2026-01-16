use {litesvm::LiteSVM, solana_account::Account, solana_address::Address, solana_rent::Rent};

// Avoid importing external dependencies
pub(crate) mod inline_spl {
    use super::*;

    pub const SPL_TOKEN_PROGRAM_ID: Address =
        Address::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    pub const SPL_TOKEN_2022_PROGRAM_ID: Address =
        Address::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
}

fn create_native_mint_with_program_id(svm: &mut LiteSVM, address: Address, token_program: Address) {
    let account = Account {
        lamports: svm.get_sysvar::<Rent>().minimum_balance(82),
        data: vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(address, account).unwrap();
}

pub fn create_native_mint(svm: &mut LiteSVM) {
    create_native_mint_with_program_id(
        svm,
        Address::from_str_const("So11111111111111111111111111111111111111112"),
        inline_spl::SPL_TOKEN_PROGRAM_ID,
    );
}

pub fn create_native_mint_2022(svm: &mut LiteSVM) {
    create_native_mint_with_program_id(
        svm,
        Address::from_str_const("9pan9bMn5HatX4EJdBwg9VgCa7Uz5HL8N1m5D3NdXejP"),
        inline_spl::SPL_TOKEN_2022_PROGRAM_ID,
    );
}
