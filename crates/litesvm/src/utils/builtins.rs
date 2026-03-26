use {
    jupnet_feature_set::enable_program_runtime_v2_and_loader_v4,
    jupnet_program_runtime::invoke_context::BuiltinFunctionWithContext,
    jupnet_sdk::{bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable, pubkey::Pubkey},
};

pub struct BuiltinPrototype {
    pub enable_feature_id: Option<Pubkey>,
    pub program_id: Pubkey,
    pub name: &'static str,
    pub entrypoint: BuiltinFunctionWithContext,
}

pub static BUILTINS: &[BuiltinPrototype] = &[
    BuiltinPrototype {
        name: "system_program",
        enable_feature_id: None,
        program_id: jupnet_system_program::id(),
        entrypoint: jupnet_system_program::system_processor::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "vote_program",
        enable_feature_id: None,
        program_id: jupnet_vote_program::id(),
        entrypoint: jupnet_vote_program::vote_processor::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "stake_program",
        enable_feature_id: None,
        program_id: jupnet_stake_program::id(),
        entrypoint: jupnet_stake_program::stake_instruction::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "config_program",
        enable_feature_id: None,
        program_id: jupnet_config_program::id(),
        entrypoint: jupnet_config_program::config_processor::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "cluster_identity_management",
        enable_feature_id: None,
        program_id: jupnet_sdk::cluster_identity_management::id(),
        entrypoint: jupnet_cluster_identity_management::processor::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "jupnet_bpf_loader_deprecated_program",
        enable_feature_id: None,
        program_id: bpf_loader_deprecated::id(),
        entrypoint: jupnet_bpf_loader_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "jupnet_bpf_loader_program",
        enable_feature_id: None,
        program_id: bpf_loader::id(),
        entrypoint: jupnet_bpf_loader_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "jupnet_bpf_loader_upgradeable_program",
        enable_feature_id: None,
        program_id: bpf_loader_upgradeable::id(),
        entrypoint: jupnet_bpf_loader_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "compute_budget_program",
        enable_feature_id: None,
        program_id: jupnet_sdk::compute_budget::id(),
        entrypoint: jupnet_compute_budget_program::Entrypoint::vm,
    },
    BuiltinPrototype {
        name: "loader_v4",
        enable_feature_id: Some(enable_program_runtime_v2_and_loader_v4::id()),
        program_id: jupnet_sdk::loader_v4::id(),
        entrypoint: jupnet_loader_v4_program::Entrypoint::vm,
    },
];
