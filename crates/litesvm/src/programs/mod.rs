use {
    crate::LiteSVM,
    solana_pubkey::pubkey,
    solana_sdk_ids::{address_lookup_table, config},
};

pub fn load_default_programs(svm: &mut LiteSVM) {
    svm.add_program(
        pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
        include_bytes!("elf/spl_token-3.5.0.so"),
    )
    .unwrap();
    svm.add_program(
        pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
        include_bytes!("elf/spl_token_2022-8.0.0.so"),
    )
    .unwrap();
    svm.add_program(
        pubkey!("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo"),
        include_bytes!("elf/spl_memo-1.0.0.so"),
    )
    .unwrap();
    svm.add_program(
        pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"),
        include_bytes!("elf/spl_memo-3.0.0.so"),
    )
    .unwrap();
    svm.add_program(
        pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
        include_bytes!("elf/spl_associated_token_account-1.1.1.so"),
    )
    .unwrap();
    svm.add_program(config::ID, include_bytes!("elf/config.so"))
        .unwrap();
    svm.add_program(
        address_lookup_table::ID,
        include_bytes!("elf/address_lookup_table.so"),
    )
    .unwrap();
    svm.add_program(
        pubkey!("Stake11111111111111111111111111111111111111"),
        include_bytes!("elf/core_bpf_stake-1.0.1.so"),
    )
    .unwrap()
}
