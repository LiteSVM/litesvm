use {
    crate::LiteSVM,
    agave_feature_set::replace_spl_token_with_p_token,
    solana_address::address,
    solana_sdk_ids::{
        address_lookup_table, bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable,
        stake,
    },
};

pub fn load_default_programs(svm: &mut LiteSVM) {
    // if replace spl-token with p-token feature is enabled, the SPL token contract is loaded from
    // a different .so
    if svm
        .feature_set
        .is_active(&replace_spl_token_with_p_token::id())
    {
        svm.add_program_preverified(
            address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            include_bytes!("elf/pinocchio_token_program.so"),
            &bpf_loader_upgradeable::id(),
        )
        .unwrap();
    } else {
        svm.add_program_preverified(
            address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            include_bytes!("elf/spl_token-3.5.0.so"),
            &bpf_loader::id(),
        )
        .unwrap();
    }

    svm.add_program_preverified(
        address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
        include_bytes!("elf/spl_token_2022-10.0.0.so"),
        &bpf_loader_upgradeable::id(),
    )
    .unwrap();
    svm.add_program_preverified(
        address!("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo"),
        include_bytes!("elf/spl_memo-1.0.0.so"),
        &bpf_loader_deprecated::id(),
    )
    .unwrap();
    svm.add_program_preverified(
        address!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"),
        include_bytes!("elf/spl_memo-3.0.0.so"),
        &bpf_loader::id(),
    )
    .unwrap();
    svm.add_program_preverified(
        address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
        include_bytes!("elf/spl_associated_token_account-1.1.1.so"),
        &bpf_loader::id(),
    )
    .unwrap();
    svm.add_program_preverified(
        address_lookup_table::ID,
        include_bytes!("elf/address_lookup_table.so"),
        &bpf_loader_upgradeable::id(),
    )
    .unwrap();
    svm.add_program_preverified(
        stake::ID,
        include_bytes!("elf/core_bpf_stake-1.0.1.so"),
        &bpf_loader_upgradeable::id(),
    )
    .unwrap();
}
