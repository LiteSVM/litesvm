/*
 * litesvm.h - C ABI for LiteSVM.
 *
 * This header is hand-maintained and must stay in sync with src/lib.rs.
 * All handles are opaque and must be freed with the matching *_free fn.
 * All pubkeys and hashes are 32 raw bytes unless noted otherwise.
 * Status returns: 0 = ok, non-zero = error; call litesvm_last_error_copy
 * to read a UTF-8 description of the last error on this thread.
 */

#ifndef LITESVM_H
#define LITESVM_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct LiteSvmHandle LiteSvmHandle;
typedef struct LiteSvmTxOutcome LiteSvmTxOutcome;
typedef struct LiteSvmAccount LiteSvmAccount;

/* Copy the last thread-local error into `buf`. Returns the full length of
 * the error string. If that exceeds `buf_len`, the buffer is truncated.
 * `buf` may be NULL when `buf_len` is 0 (probe for required size). */
size_t litesvm_last_error_copy(uint8_t *buf, size_t buf_len);

/* Construct / destroy. litesvm_new returns NULL on panic. */
LiteSvmHandle *litesvm_new(void);
void litesvm_free(LiteSvmHandle *handle);

/* Mutating ops. Return 0 on success. */
int32_t litesvm_airdrop(LiteSvmHandle *handle,
                        const uint8_t *pubkey, uint64_t lamports);
int32_t litesvm_expire_blockhash(LiteSvmHandle *handle);

/* Reads. */
int32_t litesvm_get_balance(const LiteSvmHandle *handle,
                            const uint8_t *pubkey, uint64_t *out);
int32_t litesvm_latest_blockhash(const LiteSvmHandle *handle, uint8_t *out32);
uint64_t litesvm_minimum_balance_for_rent_exemption(
    const LiteSvmHandle *handle, size_t data_len);

/* Static NUL-terminated version string. Do not free. */
const uint8_t *litesvm_version(void);

/* ------------------------------------------------------------------- */
/* Transactions                                                         */
/* ------------------------------------------------------------------- */

/* Submit a bincode-encoded legacy Transaction. Returns a new outcome
 * handle (free with litesvm_tx_outcome_free) on both success and failure.
 * Returns NULL on deserialization / internal error. */
LiteSvmTxOutcome *litesvm_send_legacy_transaction(
    LiteSvmHandle *handle, const uint8_t *tx_bytes, size_t tx_len);

void litesvm_tx_outcome_free(LiteSvmTxOutcome *handle);

/* 1 if ok, 0 if failed, -1 if handle is NULL. */
int32_t litesvm_tx_outcome_is_ok(const LiteSvmTxOutcome *handle);

int32_t litesvm_tx_outcome_signature(const LiteSvmTxOutcome *handle,
                                     uint8_t *out64);
uint64_t litesvm_tx_outcome_compute_units(const LiteSvmTxOutcome *handle);
uint64_t litesvm_tx_outcome_fee(const LiteSvmTxOutcome *handle);
size_t   litesvm_tx_outcome_logs_count(const LiteSvmTxOutcome *handle);

/* Copy semantics identical to litesvm_last_error_copy. */
size_t litesvm_tx_outcome_log_copy(const LiteSvmTxOutcome *handle,
                                   size_t idx, uint8_t *buf, size_t buf_len);
size_t litesvm_tx_outcome_error_copy(const LiteSvmTxOutcome *handle,
                                     uint8_t *buf, size_t buf_len);

/* Return data. Returns 1 if set, 0 if absent, negative on error. */
int32_t litesvm_tx_outcome_return_data_program_id(
    const LiteSvmTxOutcome *handle, uint8_t *out32);
size_t  litesvm_tx_outcome_return_data_len(const LiteSvmTxOutcome *handle);
size_t  litesvm_tx_outcome_return_data_copy(const LiteSvmTxOutcome *handle,
                                            uint8_t *buf, size_t buf_len);

/* Post-accounts. Populated only for successful simulations; 0 otherwise. */
size_t litesvm_tx_outcome_post_accounts_count(const LiteSvmTxOutcome *handle);

/* Returns a newly-allocated Account handle (free with litesvm_account_free)
 * and writes the 32-byte address to `out_address`. NULL on error / OOB. */
LiteSvmAccount *litesvm_tx_outcome_post_account_at(
    const LiteSvmTxOutcome *handle, size_t idx, uint8_t *out_address);

/* Inner instructions (2D: Vec<Vec<InnerInstruction>>).
 * inner_instructions[outer_idx] is the list of CPIs invoked by the
 * outer_idx-th top-level instruction. */
size_t  litesvm_tx_outcome_inner_outer_count(const LiteSvmTxOutcome *handle);
size_t  litesvm_tx_outcome_inner_inner_count(const LiteSvmTxOutcome *handle,
                                             size_t outer_idx);

/* u8 fields returned as int32_t; -1 on error. */
int32_t litesvm_tx_outcome_inner_program_id_index(
    const LiteSvmTxOutcome *handle, size_t outer_idx, size_t inner_idx);
int32_t litesvm_tx_outcome_inner_stack_height(
    const LiteSvmTxOutcome *handle, size_t outer_idx, size_t inner_idx);

size_t litesvm_tx_outcome_inner_accounts_len(
    const LiteSvmTxOutcome *handle, size_t outer_idx, size_t inner_idx);
size_t litesvm_tx_outcome_inner_accounts_copy(
    const LiteSvmTxOutcome *handle, size_t outer_idx, size_t inner_idx,
    uint8_t *buf, size_t buf_len);

size_t litesvm_tx_outcome_inner_data_len(
    const LiteSvmTxOutcome *handle, size_t outer_idx, size_t inner_idx);
size_t litesvm_tx_outcome_inner_data_copy(
    const LiteSvmTxOutcome *handle, size_t outer_idx, size_t inner_idx,
    uint8_t *buf, size_t buf_len);

/* Versioned / simulate / warp / history */

LiteSvmTxOutcome *litesvm_send_versioned_transaction(
    LiteSvmHandle *handle, const uint8_t *tx_bytes, size_t tx_len);

LiteSvmTxOutcome *litesvm_simulate_legacy_transaction(
    LiteSvmHandle *handle, const uint8_t *tx_bytes, size_t tx_len);

LiteSvmTxOutcome *litesvm_simulate_versioned_transaction(
    LiteSvmHandle *handle, const uint8_t *tx_bytes, size_t tx_len);

int32_t litesvm_warp_to_slot(LiteSvmHandle *handle, uint64_t slot);

/* Looks up a transaction by 64-byte signature. Returns a new outcome handle
 * (free with litesvm_tx_outcome_free), or NULL if not in history. */
LiteSvmTxOutcome *litesvm_get_transaction(
    const LiteSvmHandle *handle, const uint8_t *signature);

/* ------------------------------------------------------------------- */
/* Configuration                                                        */
/* ------------------------------------------------------------------- */

int32_t litesvm_set_sigverify(LiteSvmHandle *handle, bool enabled);
int32_t litesvm_get_sigverify(const LiteSvmHandle *handle); /* 1/0/-1 */
int32_t litesvm_set_blockhash_check(LiteSvmHandle *handle, bool enabled);

/* 0 disables dedup (allows replay of identical txs). */
int32_t litesvm_set_transaction_history(LiteSvmHandle *handle, size_t capacity);

/* has_limit = false means unlimited. */
int32_t litesvm_set_log_bytes_limit(LiteSvmHandle *handle,
                                    bool has_limit, size_t limit);

int32_t litesvm_set_lamports(LiteSvmHandle *handle, uint64_t lamports);

int32_t litesvm_set_sysvars(LiteSvmHandle *handle);
int32_t litesvm_set_builtins(LiteSvmHandle *handle);
int32_t litesvm_set_default_programs(LiteSvmHandle *handle);
int32_t litesvm_set_precompiles(LiteSvmHandle *handle);

/* Seed SPL Token / Token-2022 native-mint accounts if the matching program
 * is loaded. No-op otherwise. */
int32_t litesvm_with_native_mints(LiteSvmHandle *handle);

int32_t litesvm_add_program_with_loader(LiteSvmHandle *handle,
                                        const uint8_t *program_id,
                                        const uint8_t *bytes, size_t bytes_len,
                                        const uint8_t *loader_id);

/* ------------------------------------------------------------------- */
/* Accounts                                                             */
/* ------------------------------------------------------------------- */

/* Construct an Account. `data` may be NULL when `data_len` is 0.
 * Returns NULL on failure. Free with litesvm_account_free. */
LiteSvmAccount *litesvm_account_new(uint64_t lamports,
                                    const uint8_t *data, size_t data_len,
                                    const uint8_t *owner,    /* 32 bytes */
                                    bool executable,
                                    uint64_t rent_epoch);
void litesvm_account_free(LiteSvmAccount *handle);

uint64_t litesvm_account_lamports(const LiteSvmAccount *handle);
int32_t  litesvm_account_executable(const LiteSvmAccount *handle);
uint64_t litesvm_account_rent_epoch(const LiteSvmAccount *handle);
int32_t  litesvm_account_owner(const LiteSvmAccount *handle, uint8_t *out32);
size_t   litesvm_account_data_len(const LiteSvmAccount *handle);
size_t   litesvm_account_data_copy(const LiteSvmAccount *handle,
                                   uint8_t *buf, size_t buf_len);

/* Returns a newly-allocated Account handle (free with litesvm_account_free),
 * or NULL if the account does not exist. */
LiteSvmAccount *litesvm_get_account(const LiteSvmHandle *handle,
                                    const uint8_t *pubkey);

/* Stores a copy of `acct` at `pubkey`. Caller retains ownership of `acct`. */
int32_t litesvm_set_account(LiteSvmHandle *handle,
                            const uint8_t *pubkey,
                            const LiteSvmAccount *acct);

/* ------------------------------------------------------------------- */
/* Programs                                                             */
/* ------------------------------------------------------------------- */

int32_t litesvm_add_program(LiteSvmHandle *handle,
                            const uint8_t *program_id,
                            const uint8_t *bytes, size_t bytes_len);
int32_t litesvm_add_program_from_file(LiteSvmHandle *handle,
                                      const uint8_t *program_id,
                                      const uint8_t *path_utf8,
                                      size_t path_len);

/* ------------------------------------------------------------------- */
/* Sysvars (fixed layout, passed by reference)                          */
/* ------------------------------------------------------------------- */

typedef struct {
    uint64_t slot;
    int64_t  epoch_start_timestamp;
    uint64_t epoch;
    uint64_t leader_schedule_epoch;
    int64_t  unix_timestamp;
} LiteSvmClock;

int32_t litesvm_get_clock(const LiteSvmHandle *handle, LiteSvmClock *out);
int32_t litesvm_set_clock(LiteSvmHandle *handle, const LiteSvmClock *clock);

typedef struct {
    uint64_t lamports_per_byte_year;
    double   exemption_threshold;
    uint8_t  burn_percent;
} LiteSvmRent;

int32_t litesvm_get_rent(const LiteSvmHandle *handle, LiteSvmRent *out);
int32_t litesvm_set_rent(LiteSvmHandle *handle, const LiteSvmRent *rent);

typedef struct {
    uint64_t slots_per_epoch;
    uint64_t leader_schedule_slot_offset;
    uint8_t  warmup;               /* 0 or 1 */
    uint64_t first_normal_epoch;
    uint64_t first_normal_slot;
} LiteSvmEpochSchedule;

int32_t litesvm_get_epoch_schedule(const LiteSvmHandle *handle,
                                   LiteSvmEpochSchedule *out);
int32_t litesvm_set_epoch_schedule(LiteSvmHandle *handle,
                                   const LiteSvmEpochSchedule *schedule);

int32_t litesvm_get_last_restart_slot(const LiteSvmHandle *handle,
                                      uint64_t *out);
int32_t litesvm_set_last_restart_slot(LiteSvmHandle *handle, uint64_t slot);

/* EpochRewards sysvar. total_points is u128 split as lo/hi u64 halves. */
typedef struct {
    uint64_t distribution_starting_block_height;
    uint64_t num_partitions;
    uint8_t  parent_blockhash[32];
    uint64_t total_points_lo;
    uint64_t total_points_hi;
    uint64_t total_rewards;
    uint64_t distributed_rewards;
    uint8_t  active;
} LiteSvmEpochRewards;

int32_t litesvm_get_epoch_rewards(const LiteSvmHandle *handle,
                                  LiteSvmEpochRewards *out);
int32_t litesvm_set_epoch_rewards(LiteSvmHandle *handle,
                                  const LiteSvmEpochRewards *rewards);

/* SlotHashes: Vec<(slot, hash)>. */
typedef struct {
    uint64_t slot;
    uint8_t  hash[32];
} LiteSvmSlotHashItem;

size_t  litesvm_get_slot_hashes_count(const LiteSvmHandle *handle);
size_t  litesvm_get_slot_hashes_copy(const LiteSvmHandle *handle,
                                     LiteSvmSlotHashItem *out, size_t out_count);
int32_t litesvm_set_slot_hashes(LiteSvmHandle *handle,
                                const LiteSvmSlotHashItem *items, size_t count);

/* StakeHistory: Vec<(epoch, StakeHistoryEntry)>. */
typedef struct {
    uint64_t epoch;
    uint64_t effective;
    uint64_t activating;
    uint64_t deactivating;
} LiteSvmStakeHistoryItem;

size_t  litesvm_get_stake_history_count(const LiteSvmHandle *handle);
size_t  litesvm_get_stake_history_copy(const LiteSvmHandle *handle,
                                       LiteSvmStakeHistoryItem *out,
                                       size_t out_count);
int32_t litesvm_set_stake_history(LiteSvmHandle *handle,
                                  const LiteSvmStakeHistoryItem *items,
                                  size_t count);

/* SlotHistory: opaque handle (the bitvec is 128 KB). */
typedef struct LiteSvmSlotHistoryHandle LiteSvmSlotHistoryHandle;

LiteSvmSlotHistoryHandle *litesvm_slot_history_new_default(void);
void                      litesvm_slot_history_free(
    LiteSvmSlotHistoryHandle *handle);

int32_t  litesvm_slot_history_add(LiteSvmSlotHistoryHandle *handle,
                                  uint64_t slot);
/* 0 = Future, 1 = TooOld, 2 = Found, 3 = NotFound; -1 on error. */
int32_t  litesvm_slot_history_check(const LiteSvmSlotHistoryHandle *handle,
                                    uint64_t slot);
uint64_t litesvm_slot_history_oldest(const LiteSvmSlotHistoryHandle *handle);
uint64_t litesvm_slot_history_newest(const LiteSvmSlotHistoryHandle *handle);
uint64_t litesvm_slot_history_next_slot(const LiteSvmSlotHistoryHandle *handle);
int32_t  litesvm_slot_history_set_next_slot(
    LiteSvmSlotHistoryHandle *handle, uint64_t slot);

LiteSvmSlotHistoryHandle *litesvm_get_slot_history(const LiteSvmHandle *handle);
int32_t                   litesvm_set_slot_history(
    LiteSvmHandle *handle, const LiteSvmSlotHistoryHandle *history);

/* ------------------------------------------------------------------- */
/* ComputeBudget (~44 fields; fixed layout)                             */
/* ------------------------------------------------------------------- */

typedef struct {
    uint64_t compute_unit_limit;
    uint64_t log_64_units;
    uint64_t create_program_address_units;
    uint64_t invoke_units;
    uint64_t max_instruction_stack_depth;
    uint64_t max_instruction_trace_length;
    uint64_t sha256_base_cost;
    uint64_t sha256_byte_cost;
    uint64_t sha256_max_slices;
    uint64_t max_call_depth;
    uint64_t stack_frame_size;
    uint64_t log_pubkey_units;
    uint64_t cpi_bytes_per_unit;
    uint64_t sysvar_base_cost;
    uint64_t secp256k1_recover_cost;
    uint64_t syscall_base_cost;
    uint64_t curve25519_edwards_validate_point_cost;
    uint64_t curve25519_edwards_add_cost;
    uint64_t curve25519_edwards_subtract_cost;
    uint64_t curve25519_edwards_multiply_cost;
    uint64_t curve25519_edwards_msm_base_cost;
    uint64_t curve25519_edwards_msm_incremental_cost;
    uint64_t curve25519_ristretto_validate_point_cost;
    uint64_t curve25519_ristretto_add_cost;
    uint64_t curve25519_ristretto_subtract_cost;
    uint64_t curve25519_ristretto_multiply_cost;
    uint64_t curve25519_ristretto_msm_base_cost;
    uint64_t curve25519_ristretto_msm_incremental_cost;
    uint32_t heap_size;
    uint64_t heap_cost;
    uint64_t mem_op_base_cost;
    uint64_t alt_bn128_addition_cost;
    uint64_t alt_bn128_multiplication_cost;
    uint64_t alt_bn128_pairing_one_pair_cost_first;
    uint64_t alt_bn128_pairing_one_pair_cost_other;
    uint64_t big_modular_exponentiation_base_cost;
    uint64_t big_modular_exponentiation_cost_divisor;
    uint64_t poseidon_cost_coefficient_a;
    uint64_t poseidon_cost_coefficient_c;
    uint64_t get_remaining_compute_units_cost;
    uint64_t alt_bn128_g1_compress;
    uint64_t alt_bn128_g1_decompress;
    uint64_t alt_bn128_g2_compress;
    uint64_t alt_bn128_g2_decompress;
} LiteSvmComputeBudget;

/* Returns 0 if a custom budget is set, 1 if none configured, <0 on error. */
int32_t litesvm_get_compute_budget(const LiteSvmHandle *handle,
                                   LiteSvmComputeBudget *out);
int32_t litesvm_set_compute_budget(LiteSvmHandle *handle,
                                   const LiteSvmComputeBudget *budget);

/* ------------------------------------------------------------------- */
/* FeatureSet (opaque handle)                                           */
/* ------------------------------------------------------------------- */

typedef struct LiteSvmFeatureSetHandle LiteSvmFeatureSetHandle;

LiteSvmFeatureSetHandle *litesvm_feature_set_new_default(void);
LiteSvmFeatureSetHandle *litesvm_feature_set_new_all_enabled(void);
void                     litesvm_feature_set_free(LiteSvmFeatureSetHandle *handle);

int32_t litesvm_feature_set_is_active(const LiteSvmFeatureSetHandle *handle,
                                      const uint8_t *feature_id);
int32_t litesvm_feature_set_activated_slot(const LiteSvmFeatureSetHandle *handle,
                                           const uint8_t *feature_id,
                                           uint64_t *out_slot);
int32_t litesvm_feature_set_activate(LiteSvmFeatureSetHandle *handle,
                                     const uint8_t *feature_id, uint64_t slot);
int32_t litesvm_feature_set_deactivate(LiteSvmFeatureSetHandle *handle,
                                       const uint8_t *feature_id);

size_t litesvm_feature_set_active_count(const LiteSvmFeatureSetHandle *handle);
size_t litesvm_feature_set_inactive_count(const LiteSvmFeatureSetHandle *handle);
size_t litesvm_feature_set_active_copy(const LiteSvmFeatureSetHandle *handle,
                                       uint8_t *out_buf, size_t out_count);
size_t litesvm_feature_set_inactive_copy(const LiteSvmFeatureSetHandle *handle,
                                         uint8_t *out_buf, size_t out_count);

int32_t litesvm_set_feature_set(LiteSvmHandle *handle,
                                const LiteSvmFeatureSetHandle *features);

/* Test helper: builds and signs a legacy transfer transaction and
 * bincode-encodes it into `out_buf`. Writes the required length into
 * *out_written; if out_buf_len is smaller, content is unspecified and the
 * caller should retry. Returns 0 on success. */
int32_t litesvm_build_transfer_tx(
    const uint8_t *payer_seed,   /* 32 bytes */
    const uint8_t *to_pubkey,    /* 32 bytes */
    uint64_t lamports,
    const uint8_t *blockhash,    /* 32 bytes */
    uint8_t *out_buf,
    size_t out_buf_len,
    size_t *out_written);

#ifdef __cplusplus
}
#endif

#endif /* LITESVM_H */
