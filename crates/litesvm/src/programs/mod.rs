use {
    crate::LiteSVM,
    solana_address::address,
    solana_sdk_ids::{address_lookup_table, config},
};

pub fn load_default_programs(svm: &mut LiteSVM) {
    svm.add_program(
        address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
        include_bytes!("elf/spl_token-3.5.0.so"),
    )
    .unwrap();
    svm.add_program(
        address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
        include_bytes!("elf/spl_token_2022-10.0.0.so"),
    )
    .unwrap();
    svm.add_program(
        address!("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo"),
        include_bytes!("elf/spl_memo-1.0.0.so"),
    )
    .unwrap();
    svm.add_program(
        address!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"),
        include_bytes!("elf/spl_memo-3.0.0.so"),
    )
    .unwrap();
    svm.add_program(
        address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
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
        address!("Stake11111111111111111111111111111111111111"),
        include_bytes!("elf/core_bpf_stake-1.0.1.so"),
    )
    .unwrap()
}
