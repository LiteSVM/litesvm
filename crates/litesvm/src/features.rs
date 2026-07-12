use solana_address::Address;

/// Feature gates currently activated on Solana mainnet-beta, paired with their
/// activation slot, sourced from the cluster on 2026-07-10.
pub const MAINNET_ACTIVE_FEATURES: &[(Address, u64)] = &[
    (agave_feature_set::deprecate_rewards_sysvar::ID, 55728001),
    (agave_feature_set::pico_inflation::ID, 57456000),
    (
        agave_feature_set::full_inflation::mainnet::certusone::vote::ID,
        64800004,
    ),
    (
        agave_feature_set::full_inflation::mainnet::certusone::enable::ID,
        64800004,
    ),
    (agave_feature_set::secp256k1_program_enabled::ID, 41040000),
    (agave_feature_set::spl_token_v2_multisig_fix::ID, 41040000),
    (
        agave_feature_set::no_overflow_rent_distribution::ID,
        51408000,
    ),
    (
        agave_feature_set::filter_stake_delegation_accounts::ID,
        57888004,
    ),
    (
        agave_feature_set::require_custodian_for_locked_stake_authorize::ID,
        71712000,
    ),
    (
        agave_feature_set::spl_token_v2_self_transfer_fix::ID,
        66528004,
    ),
    (agave_feature_set::warp_timestamp_again::ID, 66528004),
    (agave_feature_set::check_init_vote_data::ID, 68688000),
    (
        agave_feature_set::secp256k1_recover_syscall_enabled::ID,
        104976000,
    ),
    (agave_feature_set::system_transfer_zero_check::ID, 93312000),
    (
        agave_feature_set::dedupe_config_program_signers::ID,
        110592000,
    ),
    (agave_feature_set::verify_tx_signatures_len::ID, 102816004),
    (
        agave_feature_set::vote_stake_checked_instructions::ID,
        92448000,
    ),
    (agave_feature_set::rent_for_sysvars::ID, 104976000),
    (
        agave_feature_set::libsecp256k1_0_5_upgrade_enabled::ID,
        110592000,
    ),
    (agave_feature_set::tx_wide_compute_cap::ID, 135216004),
    (
        agave_feature_set::spl_token_v2_set_authority_fix::ID,
        93312000,
    ),
    (
        agave_feature_set::merge_nonce_error_into_system_error::ID,
        151632012,
    ),
    (agave_feature_set::disable_fees_sysvar::ID, 208656004),
    (
        agave_feature_set::stake_merge_with_unmatched_credits_observed::ID,
        104112000,
    ),
    (agave_feature_set::curve25519_syscall_enabled::ID, 275184000),
    (
        agave_feature_set::curve25519_restrict_msm_length::ID,
        262224008,
    ),
    (
        agave_feature_set::versioned_tx_message_enabled::ID,
        154656004,
    ),
    (
        agave_feature_set::libsecp256k1_fail_on_bad_count2::ID,
        200880004,
    ),
    (
        agave_feature_set::instructions_sysvar_owned_by_sysvar::ID,
        152496000,
    ),
    (
        agave_feature_set::stake_program_advance_activating_credits_observed::ID,
        104112000,
    ),
    (agave_feature_set::credits_auto_rewind::ID, 200448008),
    (agave_feature_set::demote_program_write_locks::ID, 100656000),
    (agave_feature_set::ed25519_program_enabled::ID, 117936008),
    (
        agave_feature_set::return_data_syscall_enabled::ID,
        117936008,
    ),
    (
        agave_feature_set::reduce_required_deploy_balance::ID,
        102816004,
    ),
    (
        agave_feature_set::sol_log_data_syscall_enabled::ID,
        117936008,
    ),
    (
        agave_feature_set::stakes_remove_delegation_if_inactive::ID,
        110592000,
    ),
    (agave_feature_set::do_support_realloc::ID, 133920008),
    (
        agave_feature_set::prevent_calling_precompiles_as_programs::ID,
        143424004,
    ),
    (
        agave_feature_set::optimize_epoch_boundary_updates::ID,
        109728000,
    ),
    (agave_feature_set::remove_native_loader::ID, 117072004),
    (agave_feature_set::send_to_tpu_vote_port::ID, 101088000),
    (agave_feature_set::requestable_heap_size::ID, 135216004),
    (agave_feature_set::disable_fee_calculator::ID, 147744004),
    (agave_feature_set::add_compute_budget_program::ID, 117072004),
    (agave_feature_set::nonce_must_be_writable::ID, 136944004),
    (agave_feature_set::spl_token_v3_3_0_release::ID, 117072004),
    (agave_feature_set::leave_nonce_on_success::ID, 133056012),
    (
        agave_feature_set::reject_empty_instruction_without_program::ID,
        137376016,
    ),
    (
        agave_feature_set::fixed_memcpy_nonoverlapping_check::ID,
        137808012,
    ),
    (
        agave_feature_set::reject_non_rent_exempt_vote_withdraws::ID,
        117072004,
    ),
    (
        agave_feature_set::evict_invalid_stakes_cache_entries::ID,
        117072004,
    ),
    (
        agave_feature_set::allow_votes_to_directly_update_vote_state::ID,
        212112000,
    ),
    (agave_feature_set::max_tx_account_locks::ID, 140400004),
    (
        agave_feature_set::require_rent_exempt_accounts::ID,
        133488000,
    ),
    (
        agave_feature_set::filter_votes_outside_slot_hashes::ID,
        157680012,
    ),
    (agave_feature_set::update_syscall_base_costs::ID, 138672000),
    (
        agave_feature_set::stake_deactivate_delinquent_instruction::ID,
        198720004,
    ),
    (
        agave_feature_set::vote_withdraw_authority_may_change_authorized_voter::ID,
        138672000,
    ),
    (
        agave_feature_set::spl_associated_token_account_v1_0_4::ID,
        130464000,
    ),
    (
        agave_feature_set::reject_vote_account_close_unless_zero_credit_epoch::ID,
        170640000,
    ),
    (
        agave_feature_set::add_get_processed_sibling_instruction_syscall::ID,
        134352008,
    ),
    (agave_feature_set::bank_transaction_count_fix::ID, 171072012),
    (
        agave_feature_set::disable_bpf_deprecated_load_instructions::ID,
        139104000,
    ),
    (
        agave_feature_set::disable_bpf_unresolved_symbols_at_runtime::ID,
        139104000,
    ),
    (
        agave_feature_set::record_instruction_in_transaction_context_push::ID,
        141696000,
    ),
    (agave_feature_set::syscall_saturated_math::ID, 150768000),
    (agave_feature_set::check_physical_overlapping::ID, 142560008),
    (
        agave_feature_set::limit_secp256k1_recovery_id::ID,
        142560008,
    ),
    (agave_feature_set::disable_deprecated_loader::ID, 167184008),
    (
        agave_feature_set::check_slice_translation_size::ID,
        207792008,
    ),
    (
        agave_feature_set::stake_split_uses_rent_sysvar::ID,
        203904008,
    ),
    (
        agave_feature_set::add_get_minimum_delegation_instruction_to_stake_program::ID,
        199584000,
    ),
    (
        agave_feature_set::error_on_syscall_bpf_function_hash_collisions::ID,
        280800004,
    ),
    (agave_feature_set::reject_callx_r10::ID, 279072000),
    (
        agave_feature_set::drop_redundant_turbine_path::ID,
        199152000,
    ),
    (
        agave_feature_set::executables_incur_cpi_data_cost::ID,
        139536000,
    ),
    (agave_feature_set::fix_recent_blockhashes::ID, 204768000),
    (
        agave_feature_set::update_rewards_from_cached_accounts::ID,
        206064004,
    ),
    (
        agave_feature_set::partitioned_epoch_rewards_superfeature::ID,
        305424000,
    ),
    (agave_feature_set::spl_token_v3_4_0::ID, 144288004),
    (
        agave_feature_set::spl_associated_token_account_v1_1_0::ID,
        144288004,
    ),
    (
        agave_feature_set::default_units_per_instruction::ID,
        141264004,
    ),
    (
        agave_feature_set::stake_allow_zero_undelegated_amount::ID,
        200016004,
    ),
    (
        agave_feature_set::require_static_program_ids_in_transaction::ID,
        153360000,
    ),
    (
        agave_feature_set::add_set_compute_unit_price_ix::ID,
        142128000,
    ),
    (
        agave_feature_set::disable_deploy_of_alloc_free_syscall::ID,
        209088008,
    ),
    (
        agave_feature_set::include_account_index_in_rent_error::ID,
        154224000,
    ),
    (
        agave_feature_set::add_shred_type_to_shred_seed::ID,
        137376016,
    ),
    (
        agave_feature_set::warp_timestamp_with_a_vengeance::ID,
        136512012,
    ),
    (
        agave_feature_set::separate_nonce_from_blockhash::ID,
        138240000,
    ),
    (agave_feature_set::enable_durable_nonce::ID, 138240000),
    (
        agave_feature_set::vote_state_update_credit_per_dequeue::ID,
        212112000,
    ),
    (agave_feature_set::quick_bail_on_panic::ID, 140400004),
    (agave_feature_set::nonce_must_be_authorized::ID, 138240000),
    (agave_feature_set::nonce_must_be_advanceable::ID, 138240000),
    (agave_feature_set::vote_authorize_with_seed::ID, 148608004),
    (
        agave_feature_set::preserve_rent_epoch_for_rent_exempt_accounts::ID,
        156384000,
    ),
    (
        agave_feature_set::enable_bpf_loader_extend_program_ix::ID,
        229824000,
    ),
    (
        agave_feature_set::enable_early_verification_of_account_modifications::ID,
        222048008,
    ),
    (agave_feature_set::skip_rent_rewrites::ID, 326160000),
    (
        agave_feature_set::prevent_crediting_accounts_that_end_rent_paying::ID,
        161136000,
    ),
    (
        agave_feature_set::cap_bpf_program_instruction_accounts::ID,
        205200004,
    ),
    (
        agave_feature_set::loosen_cpi_size_restriction::ID,
        312768000,
    ),
    (
        agave_feature_set::use_default_units_in_fee_calculation::ID,
        206496008,
    ),
    (agave_feature_set::compact_vote_state_updates::ID, 212112000),
    (
        agave_feature_set::incremental_snapshot_only_incremental_hash_calculation::ID,
        243648004,
    ),
    (
        agave_feature_set::disable_cpi_setting_executable_and_rent_epoch::ID,
        225936004,
    ),
    (
        agave_feature_set::on_load_preserve_rent_epoch_for_rent_exempt_accounts::ID,
        204336000,
    ),
    (agave_feature_set::account_hash_ignore_slot::ID, 225504004),
    (agave_feature_set::set_exempt_rent_epoch_max::ID, 246240000),
    (
        agave_feature_set::relax_authority_signer_check_for_lookup_table_creation::ID,
        249264000,
    ),
    (
        agave_feature_set::stop_sibling_instruction_search_at_parent::ID,
        236304016,
    ),
    (agave_feature_set::vote_state_update_root_fix::ID, 202176000),
    (
        agave_feature_set::cap_accounts_data_allocations_per_transaction::ID,
        224640020,
    ),
    (agave_feature_set::epoch_accounts_hash::ID, 228528004),
    (
        agave_feature_set::remove_deprecated_request_unit_ix::ID,
        233280000,
    ),
    (
        agave_feature_set::disable_rehash_for_rent_epoch::ID,
        204336000,
    ),
    (
        agave_feature_set::limit_max_instruction_trace_length::ID,
        224208000,
    ),
    (
        agave_feature_set::check_syscall_outputs_do_not_overlap::ID,
        174096000,
    ),
    (
        agave_feature_set::enable_bpf_loader_set_authority_checked_ix::ID,
        251424000,
    ),
    (agave_feature_set::enable_alt_bn128_syscall::ID, 275616000),
    (
        agave_feature_set::simplify_alt_bn128_syscall_error_codes::ID,
        274320000,
    ),
    (
        agave_feature_set::enable_alt_bn128_compression_syscall::ID,
        276912000,
    ),
    (
        agave_feature_set::fix_alt_bn128_multiplication_input_length::ID,
        361152000,
    ),
    (
        agave_feature_set::enable_program_redeployment_cooldown::ID,
        228960012,
    ),
    (
        agave_feature_set::commission_updates_only_allowed_in_first_half_of_epoch::ID,
        210384016,
    ),
    (
        agave_feature_set::enable_turbine_fanout_experiments::ID,
        247104008,
    ),
    (
        agave_feature_set::disable_turbine_fanout_experiments::ID,
        380160000,
    ),
    (
        agave_feature_set::move_serialized_len_ptr_in_cpi::ID,
        202608000,
    ),
    (agave_feature_set::update_hashes_per_tick::ID, 232848000),
    (
        agave_feature_set::disable_builtin_loader_ownership_chains::ID,
        207360004,
    ),
    (
        agave_feature_set::cap_transaction_accounts_data_size::ID,
        230256020,
    ),
    (
        agave_feature_set::remove_congestion_multiplier_from_fee_calculation::ID,
        236736000,
    ),
    (
        agave_feature_set::enable_request_heap_frame_ix::ID,
        217296000,
    ),
    (
        agave_feature_set::prevent_rent_paying_rent_recipients::ID,
        234144000,
    ),
    (
        agave_feature_set::delay_visibility_of_program_deployment::ID,
        228960012,
    ),
    (
        agave_feature_set::apply_cost_tracker_during_replay::ID,
        329616000,
    ),
    (
        agave_feature_set::syscall_parameter_address_restrictions::ID,
        429840000,
    ),
    (
        agave_feature_set::add_set_tx_loaded_accounts_data_size_instruction::ID,
        231984004,
    ),
    (agave_feature_set::switch_to_new_elf_parser::ID, 273888000),
    (agave_feature_set::round_up_heap_size::ID, 235440004),
    (
        agave_feature_set::remove_bpf_loader_incorrect_program_id::ID,
        237168000,
    ),
    (agave_feature_set::native_programs_consume_cu::ID, 240624008),
    (
        agave_feature_set::simplify_writable_program_account_check::ID,
        241056004,
    ),
    (
        agave_feature_set::stop_truncating_strings_in_syscalls::ID,
        240192004,
    ),
    (agave_feature_set::clean_up_delegation_errors::ID, 211680000),
    (
        agave_feature_set::vote_state_add_vote_latency::ID,
        252720000,
    ),
    (
        agave_feature_set::checked_arithmetic_in_fee_validation::ID,
        234576004,
    ),
    (agave_feature_set::last_restart_slot_sysvar::ID, 282096004),
    (
        agave_feature_set::reduce_stake_warmup_cooldown::ID,
        244080000,
    ),
    (
        agave_feature_set::revise_turbine_epoch_stakes::ID,
        244944000,
    ),
    (agave_feature_set::enable_poseidon_syscall::ID, 278208000),
    (agave_feature_set::timely_vote_credits::ID, 303696000),
    (
        agave_feature_set::require_rent_exempt_split_destination::ID,
        254016004,
    ),
    (
        agave_feature_set::better_error_codes_for_tx_lamport_check::ID,
        222480020,
    ),
    (agave_feature_set::update_hashes_per_tick2::ID, 253584001),
    (agave_feature_set::update_hashes_per_tick3::ID, 255312004),
    (agave_feature_set::update_hashes_per_tick4::ID, 255744008),
    (agave_feature_set::update_hashes_per_tick5::ID, 257040000),
    (agave_feature_set::update_hashes_per_tick6::ID, 257904000),
    (
        agave_feature_set::validate_fee_collector_account::ID,
        258336004,
    ),
    (
        agave_feature_set::disable_rent_fees_collection::ID,
        326592000,
    ),
    (agave_feature_set::drop_legacy_shreds::ID, 259200004),
    (
        agave_feature_set::allow_commission_decrease_at_any_time::ID,
        304128000,
    ),
    (
        agave_feature_set::add_new_reserved_account_keys::ID,
        320976000,
    ),
    (
        agave_feature_set::consume_blockstore_duplicate_proofs::ID,
        260064000,
    ),
    (
        agave_feature_set::index_erasure_conflict_duplicate_proofs::ID,
        260496000,
    ),
    (
        agave_feature_set::merkle_conflict_duplicate_proofs::ID,
        283824004,
    ),
    (
        agave_feature_set::disable_bpf_loader_instructions::ID,
        259632004,
    ),
    (
        agave_feature_set::cost_model_requested_write_lock_cost::ID,
        312336000,
    ),
    (
        agave_feature_set::enable_gossip_duplicate_proof_ingestion::ID,
        284688004,
    ),
    (
        agave_feature_set::enable_chained_merkle_shreds::ID,
        306720000,
    ),
    (
        agave_feature_set::remove_rounding_in_fee_calculation::ID,
        317088000,
    ),
    (agave_feature_set::enable_tower_sync_ix::ID, 323568000),
    (
        agave_feature_set::deprecate_unused_legacy_vote_plumbing::ID,
        283392000,
    ),
    (agave_feature_set::reward_full_priority_fee::ID, 320112000),
    (agave_feature_set::get_sysvar_syscall_enabled::ID, 321840000),
    (agave_feature_set::abort_on_invalid_curve::ID, 311904000),
    (
        agave_feature_set::migrate_feature_gate_program_to_core_bpf::ID,
        324864000,
    ),
    (agave_feature_set::vote_only_full_fec_sets::ID, 332208000),
    (
        agave_feature_set::migrate_config_program_to_core_bpf::ID,
        325296000,
    ),
    (
        agave_feature_set::enable_get_epoch_stake_syscall::ID,
        330912000,
    ),
    (
        agave_feature_set::migrate_address_lookup_table_program_to_core_bpf::ID,
        329184000,
    ),
    (
        agave_feature_set::zk_elgamal_proof_program_enabled::ID,
        315792000,
    ),
    (
        agave_feature_set::move_stake_and_move_lamports_ixs::ID,
        314064000,
    ),
    (
        agave_feature_set::ed25519_precompile_verify_strict::ID,
        308448000,
    ),
    (
        agave_feature_set::move_precompile_verification_to_svm::ID,
        328320000,
    ),
    (
        agave_feature_set::enable_transaction_loading_failure_fees::ID,
        327024000,
    ),
    (
        agave_feature_set::enable_sbpf_v1_deployment_and_execution::ID,
        349488000,
    ),
    (
        agave_feature_set::enable_sbpf_v2_deployment_and_execution::ID,
        356400000,
    ),
    (
        agave_feature_set::enable_sbpf_v3_deployment_and_execution::ID,
        428976000,
    ),
    (
        agave_feature_set::remove_accounts_executable_flag_checks::ID,
        350352004,
    ),
    (
        agave_feature_set::disable_account_loader_special_case::ID,
        314496000,
    ),
    (
        agave_feature_set::enable_secp256r1_precompile::ID,
        345600000,
    ),
    (agave_feature_set::accounts_lt_hash::ID, 347328000),
    (agave_feature_set::snapshots_lt_hash::ID, 353376000),
    (agave_feature_set::remove_accounts_delta_hash::ID, 348624000),
    (
        agave_feature_set::migrate_stake_program_to_core_bpf::ID,
        355536000,
    ),
    (
        agave_feature_set::deplete_cu_meter_on_vm_failure::ID,
        327888000,
    ),
    (
        agave_feature_set::reserve_minimal_cus_for_builtin_instructions::ID,
        327456000,
    ),
    (agave_feature_set::raise_block_limits_to_50m::ID, 332640000),
    (
        agave_feature_set::drop_unchained_merkle_shreds::ID,
        354672000,
    ),
    (
        agave_feature_set::relax_intrabatch_account_locks::ID,
        411696000,
    ),
    (
        agave_feature_set::disable_partitioned_rent_collection::ID,
        349056000,
    ),
    (
        agave_feature_set::enable_vote_address_leader_schedule::ID,
        363312000,
    ),
    (
        agave_feature_set::require_static_nonce_account::ID,
        384480000,
    ),
    (agave_feature_set::raise_block_limits_to_60m::ID, 355104000),
    (
        agave_feature_set::mask_out_rent_epoch_in_vm_serialization::ID,
        346032000,
    ),
    (
        agave_feature_set::formalize_loaded_transaction_data_size::ID,
        381888000,
    ),
    (
        agave_feature_set::disable_zk_elgamal_proof_program::ID,
        347760000,
    ),
    (
        agave_feature_set::reenable_zk_elgamal_proof_program::ID,
        424224000,
    ),
    (agave_feature_set::raise_account_cu_limit::ID, 379296000),
    (agave_feature_set::delay_commission_updates::ID, 428112000),
    (agave_feature_set::enforce_fixed_fec_set::ID, 403920000),
    (
        agave_feature_set::provide_instruction_data_offset_in_vm_r2::ID,
        410400000,
    ),
    (
        agave_feature_set::create_account_allow_prefund::ID,
        422928004,
    ),
    (agave_feature_set::static_instruction_limit::ID, 404352000),
    (agave_feature_set::vote_state_v4::ID, 409968000),
    (agave_feature_set::switch_to_chacha8_turbine::ID, 408672000),
    (
        agave_feature_set::increase_cpi_account_info_limit::ID,
        403056000,
    ),
    (
        agave_feature_set::deprecate_rent_exemption_threshold::ID,
        407376000,
    ),
    (agave_feature_set::poseidon_enforce_padding::ID, 406080000),
    (
        agave_feature_set::fix_alt_bn128_pairing_length_check::ID,
        406944000,
    ),
    (
        agave_feature_set::replace_spl_token_with_p_token::ID,
        419472000,
    ),
    (agave_feature_set::alt_bn128_little_endian::ID, 425088000),
    (
        agave_feature_set::bls_pubkey_management_in_vote_account::ID,
        431568000,
    ),
    (
        agave_feature_set::relax_programdata_account_check_migration::ID,
        412992000,
    ),
    (
        agave_feature_set::enable_alt_bn128_g2_syscalls::ID,
        425520000,
    ),
    (agave_feature_set::enable_bls12_381_syscall::ID, 425952004),
    (
        agave_feature_set::remove_simple_vote_from_cost_model::ID,
        423792000,
    ),
    (agave_feature_set::limit_instruction_accounts::ID, 432000000),
    (agave_feature_set::validate_chained_block_id::ID, 428544000),
    (
        agave_feature_set::validate_chained_block_id_2::ID,
        428544000,
    ),
    (
        agave_feature_set::upgrade_bpf_stake_program_to_v5::ID,
        427248000,
    ),
];
