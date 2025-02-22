use solana_feature_set::{
    enable_loader_v4, zk_elgamal_proof_program_enabled, zk_token_sdk_enabled,
};
use solana_program_runtime::invoke_context::BuiltinFunctionWithContext;
use solana_pubkey::Pubkey;

pub(crate) struct BuiltinPrototype {
    pub feature_id: Option<Pubkey>,
    pub program_id: Pubkey,
    pub name: &'static str,
    pub entrypoint: BuiltinFunctionWithContext,
}

pub(crate) static BUILTINS: &[BuiltinPrototype] = &[
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_system_program::id(),
        name: "system_program",
        entrypoint: solana_system_program::system_processor::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_vote_program::id(),
        name: "vote_program",
        entrypoint: solana_vote_program::vote_processor::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_stake_program::id(),
        name: "stake_program",
        entrypoint: solana_stake_program::stake_instruction::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_config_program::id(),
        name: "config_program",
        entrypoint: solana_config_program::config_processor::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_sdk_ids::bpf_loader_deprecated::id(),
        name: "solana_bpf_loader_deprecated_program",
        entrypoint: solana_bpf_loader_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_sdk_ids::bpf_loader::id(),
        name: "solana_bpf_loader_program",
        entrypoint: solana_bpf_loader_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_sdk_ids::bpf_loader_upgradeable::id(),
        name: "solana_bpf_loader_upgradeable_program",
        entrypoint: solana_bpf_loader_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_sdk_ids::compute_budget::id(),
        name: "compute_budget_program",
        entrypoint: solana_compute_budget_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: None,
        program_id: solana_sdk_ids::address_lookup_table::id(),
        name: "address_lookup_table_program",
        entrypoint: solana_address_lookup_table_program::processor::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: Some(zk_token_sdk_enabled::id()),
        program_id: solana_sdk_ids::zk_token_proof_program::id(),
        name: "zk_token_proof_program",
        entrypoint: solana_zk_token_proof_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: Some(enable_loader_v4::id()),
        program_id: solana_sdk_ids::loader_v4::id(),
        name: "loader_v4",
        entrypoint: solana_loader_v4_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        feature_id: Some(zk_elgamal_proof_program_enabled::id()),
        program_id: solana_sdk_ids::zk_elgamal_proof_program::id(),
        name: "zk_elgamal_proof_program",
        entrypoint: solana_zk_elgamal_proof_program::Entrypoint::vm,
    },
];
