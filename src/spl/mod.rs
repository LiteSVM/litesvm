use solana_sdk::pubkey;

use crate::bank::LiteSVM;

pub fn load_spl_programs(bank: &mut LiteSVM) {
    bank.add_program(
        pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
        include_bytes!("programs/spl_token-3.5.0.so"),
    );
    bank.add_program(
        pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
        include_bytes!("programs/spl_token_2022-1.0.0.so"),
    );
    bank.add_program(
        pubkey!("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo"),
        include_bytes!("programs/spl_memo-1.0.0.so"),
    );
    bank.add_program(
        pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"),
        include_bytes!("programs/spl_memo-3.0.0.so"),
    );
    bank.add_program(
        pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
        include_bytes!("programs/spl_associated_token_account-1.1.1.so"),
    );
}
