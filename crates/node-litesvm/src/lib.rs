#![deny(clippy::all)]
#![allow(clippy::new_without_default, clippy::unit_arg)]
use {
    crate::{
        account::Account,
        compute_budget::ComputeBudget,
        feature_set::FeatureSet,
        sysvar::{
            clock::Clock, epoch_rewards::EpochRewards, epoch_schedule::EpochSchedule, rent::Rent,
            slot_hashes::SlotHash, slot_history::SlotHistory, stake_history::StakeHistory,
        },
        transaction_metadata::{
            FailedTransactionMetadata, SimulatedTransactionInfo, TransactionMetadata,
        },
        util::{convert_pubkey, try_parse_hash},
    },
    bincode::deserialize,
    litesvm::{
        error::LiteSVMError,
        types::{
            FailedTransactionMetadata as FailedTransactionMetadataOriginal,
            SimulatedTransactionInfo as SimulatedTransactionInfoOriginal,
            TransactionResult as TransactionResultOriginal,
        },
        LiteSVM as LiteSVMOriginal,
    },
    napi::bindgen_prelude::*,
    solana_clock::Clock as ClockOriginal,
    solana_epoch_rewards::EpochRewards as EpochRewardsOriginal,
    solana_epoch_schedule::EpochSchedule as EpochScheduleOriginal,
    solana_last_restart_slot::LastRestartSlot,
    solana_rent::Rent as RentOriginal,
    solana_signature::Signature,
    solana_slot_hashes::SlotHashes,
    solana_slot_history::SlotHistory as SlotHistoryOriginal,
    solana_stake_interface::stake_history::StakeHistory as StakeHistoryOriginal,
    solana_transaction::{versioned::VersionedTransaction, Transaction},
    util::{bigint_to_u64, bigint_to_usize},
};
mod account;
mod compute_budget;
mod feature_set;
mod sysvar;
mod transaction_error;
mod transaction_metadata;
mod util;

#[macro_use]
extern crate napi_derive;

fn to_js_error(e: LiteSVMError, msg: &str) -> Error {
    Error::new(Status::GenericFailure, format!("{msg}: {e}"))
}

#[macro_export]
macro_rules! to_string_js {
    ($name:ident) => {
        #[napi]
        impl $name {
            #[napi(js_name = "toString")]
            pub fn js_to_string(&self) -> String {
                format!("{self:?}")
            }
        }
    };
}

pub type TransactionResult = Either<TransactionMetadata, FailedTransactionMetadata>;
pub type SimulateResult = Either<SimulatedTransactionInfo, FailedTransactionMetadata>;

fn convert_transaction_result(inner: TransactionResultOriginal) -> TransactionResult {
    match inner {
        Ok(x) => TransactionResult::A(TransactionMetadata(x)),
        Err(e) => TransactionResult::B(FailedTransactionMetadata(e)),
    }
}

fn convert_sim_result(
    inner: std::result::Result<SimulatedTransactionInfoOriginal, FailedTransactionMetadataOriginal>,
) -> SimulateResult {
    match inner {
        Ok(x) => SimulateResult::A(SimulatedTransactionInfo(x)),
        Err(e) => SimulateResult::B(FailedTransactionMetadata(e)),
    }
}

#[napi]
pub struct LiteSvm(LiteSVMOriginal);

#[napi]
impl LiteSvm {
    /// Creates the basic test environment.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self(LiteSVMOriginal::new())
    }

    #[napi(factory, js_name = "default")]
    pub fn new_default() -> Self {
        Self(LiteSVMOriginal::default())
    }

    #[napi]
    pub fn set_compute_budget(&mut self, budget: &ComputeBudget) {
        self.0.set_compute_budget(budget.0);
    }

    #[napi]
    /// Enables or disables sigverify
    pub fn set_sigverify(&mut self, sigverify: bool) {
        self.0.set_sigverify(sigverify);
    }

    #[napi]
    /// Enables or disables the blockhash check
    pub fn set_blockhash_check(&mut self, check: bool) {
        self.0.set_blockhash_check(check);
    }

    #[napi]
    /// Includes the default sysvars
    pub fn set_sysvars(&mut self) {
        self.0.set_sysvars()
    }

    #[napi]
    /// Changes the default builtins
    pub fn set_feature_set(&mut self, feature_set: &FeatureSet) {
        self.0.set_feature_set(feature_set.0.clone());
    }

    #[napi]
    /// Changes the default builtins
    pub fn set_builtins(&mut self) {
        self.0.set_builtins();
    }

    #[napi]
    /// Changes the initial lamports in LiteSVM's airdrop account
    pub fn set_lamports(&mut self, lamports: BigInt) -> Result<()> {
        Ok(self.0.set_lamports(bigint_to_u64(&lamports)?))
    }

    #[napi]
    /// Includes the standard SPL programs
    pub fn set_spl_programs(&mut self) {
        self.0.set_spl_programs();
    }

    #[napi]
    /// Changes the capacity of the transaction history.
    /// Set this to 0 to disable transaction history and allow duplicate transactions.
    pub fn set_transaction_history(&mut self, capacity: BigInt) -> Result<()> {
        Ok(self.0.set_transaction_history(bigint_to_usize(&capacity)?))
    }

    #[napi]
    pub fn set_log_bytes_limit(&mut self, limit: Option<BigInt>) -> Result<()> {
        Ok(match limit {
            None => self.0.set_log_bytes_limit(None),
            Some(x) => {
                let converted = bigint_to_usize(&x)?;
                self.0.set_log_bytes_limit(Some(converted))
            }
        })
    }

    #[napi]
    pub fn set_precompiles(&mut self) {
        self.0.set_precompiles();
    }

    #[napi]
    /// Returns minimum balance required to make an account with specified data length rent exempt.
    pub fn minimum_balance_for_rent_exemption(&self, data_len: BigInt) -> Result<u64> {
        Ok(self
            .0
            .minimum_balance_for_rent_exemption(bigint_to_usize(&data_len)?))
    }

    #[napi]
    /// Returns all information associated with the account of the provided pubkey.
    pub fn get_account(&self, pubkey: Uint8Array) -> Option<Account> {
        self.0.get_account(&convert_pubkey(pubkey)).map(Account)
    }

    #[napi]
    /// Sets all information associated with the account of the provided pubkey.
    pub fn set_account(&mut self, pubkey: Uint8Array, data: &Account) -> Result<()> {
        self.0
            .set_account(convert_pubkey(pubkey), data.0.clone())
            .map_err(|e| to_js_error(e, "Failed to set account"))
    }

    #[napi]
    /// Gets the balance of the provided account pubkey.
    pub fn get_balance(&self, pubkey: Uint8Array) -> Option<u64> {
        self.0.get_balance(&convert_pubkey(pubkey))
    }

    #[napi]
    /// Gets the latest blockhash.
    pub fn latest_blockhash(&self) -> String {
        self.0.latest_blockhash().to_string()
    }

    #[napi(ts_return_type = "TransactionMetadata | FailedTransactionMetadata | null")]
    /// Gets a transaction from the transaction history.
    pub fn get_transaction(&self, signature: Uint8Array) -> Option<TransactionResult> {
        self.0
            .get_transaction(&Signature::try_from(signature.as_ref()).unwrap())
            .map(|x| convert_transaction_result(x.clone()))
    }

    #[napi(ts_return_type = "TransactionMetadata | FailedTransactionMetadata | null")]
    /// Airdrops the account with the lamports specified.
    pub fn airdrop(&mut self, pubkey: Uint8Array, lamports: BigInt) -> Result<TransactionResult> {
        Ok(convert_transaction_result(self.0.airdrop(
            &convert_pubkey(pubkey),
            bigint_to_u64(&lamports)?,
        )))
    }

    #[napi]
    /// Adds am SBF program to the test environment from the file specified.
    pub fn add_program_from_file(&mut self, program_id: Uint8Array, path: String) -> Result<()> {
        self.0
            .add_program_from_file(convert_pubkey(program_id), path)
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("Failed to add program: {e}"),
                )
            })
    }

    #[napi]
    /// Adds am SBF program to the test environment.
    pub fn add_program(&mut self, program_id: Uint8Array, program_bytes: &[u8]) {
        self.0
            .add_program(convert_pubkey(program_id), program_bytes)
    }

    #[napi(ts_return_type = "TransactionMetadata | FailedTransactionMetadata")]
    pub fn send_legacy_transaction(&mut self, tx_bytes: Uint8Array) -> TransactionResult {
        let tx: Transaction = deserialize(&tx_bytes).unwrap();
        let res = self.0.send_transaction(tx);
        convert_transaction_result(res)
    }

    #[napi(ts_return_type = "TransactionMetadata | FailedTransactionMetadata")]
    pub fn send_versioned_transaction(&mut self, tx_bytes: Uint8Array) -> TransactionResult {
        let tx: VersionedTransaction = deserialize(&tx_bytes).unwrap();
        let res = self.0.send_transaction(tx);
        convert_transaction_result(res)
    }

    #[napi(ts_return_type = "SimulatedTransactionInfo | FailedTransactionMetadata")]
    pub fn simulate_legacy_transaction(&mut self, tx_bytes: Uint8Array) -> SimulateResult {
        let tx: Transaction = deserialize(&tx_bytes).unwrap();
        let res = self.0.simulate_transaction(tx);
        convert_sim_result(res)
    }

    #[napi(ts_return_type = "SimulatedTransactionInfo | FailedTransactionMetadata")]
    pub fn simulate_versioned_transaction(&mut self, tx_bytes: Uint8Array) -> SimulateResult {
        let tx: VersionedTransaction = deserialize(&tx_bytes).unwrap();
        let res = self.0.simulate_transaction(tx);
        convert_sim_result(res)
    }

    #[napi]
    /// Expires the current blockhash
    pub fn expire_blockhash(&mut self) {
        self.0.expire_blockhash()
    }

    #[napi]
    /// Warps the clock to the specified slot
    pub fn warp_to_slot(&mut self, slot: BigInt) -> Result<()> {
        Ok(self.0.warp_to_slot(bigint_to_u64(&slot)?))
    }

    #[napi]
    pub fn get_compute_budget(&self) -> Option<ComputeBudget> {
        self.0.get_compute_budget().map(ComputeBudget)
    }

    #[napi]
    pub fn get_sigverify(&self) -> bool {
        self.0.get_sigverify()
    }

    #[napi]
    pub fn get_clock(&self) -> Clock {
        Clock(self.0.get_sysvar::<ClockOriginal>())
    }

    #[napi]
    pub fn set_clock(&mut self, clock: &Clock) {
        self.0.set_sysvar(&clock.0)
    }

    #[napi]
    pub fn get_rent(&self) -> Rent {
        Rent(self.0.get_sysvar::<RentOriginal>())
    }

    #[napi]
    pub fn set_rent(&mut self, rent: &Rent) {
        self.0.set_sysvar(&rent.0)
    }

    #[napi]
    pub fn get_epoch_rewards(&self) -> EpochRewards {
        EpochRewards(self.0.get_sysvar::<EpochRewardsOriginal>())
    }

    #[napi]
    pub fn set_epoch_rewards(&mut self, rewards: &EpochRewards) {
        self.0.set_sysvar(&rewards.0)
    }

    #[napi]
    pub fn get_epoch_schedule(&self) -> EpochSchedule {
        EpochSchedule(self.0.get_sysvar::<EpochScheduleOriginal>())
    }

    #[napi]
    pub fn set_epoch_schedule(&mut self, schedule: &EpochSchedule) {
        self.0.set_sysvar(&schedule.0)
    }

    #[napi]
    pub fn get_last_restart_slot(&self) -> u64 {
        self.0.get_sysvar::<LastRestartSlot>().last_restart_slot
    }

    #[napi]
    pub fn set_last_restart_slot(&mut self, slot: BigInt) -> Result<()> {
        Ok(self.0.set_sysvar::<LastRestartSlot>(&LastRestartSlot {
            last_restart_slot: bigint_to_u64(&slot)?,
        }))
    }

    #[napi]
    pub fn get_slot_hashes(&self) -> Vec<SlotHash> {
        let fetched = self.0.get_sysvar::<SlotHashes>();
        fetched
            .slot_hashes()
            .iter()
            .map(|x| SlotHash {
                slot: BigInt::from(x.0),
                hash: x.1.to_string(),
            })
            .collect()
    }

    #[napi]
    pub fn set_slot_hashes(&mut self, hashes: Vec<&SlotHash>) -> Result<()> {
        let mut intermediate: Vec<(u64, solana_hash::Hash)> = Vec::with_capacity(hashes.len());
        for h in hashes {
            let converted_hash = try_parse_hash(&h.hash)?;
            intermediate.push((bigint_to_u64(&h.slot)?, converted_hash));
        }
        let converted = SlotHashes::from_iter(intermediate);
        self.0.set_sysvar::<SlotHashes>(&converted);
        Ok(())
    }

    #[napi]
    pub fn get_slot_history(&self) -> SlotHistory {
        SlotHistory(self.0.get_sysvar::<SlotHistoryOriginal>())
    }

    #[napi]
    pub fn set_slot_history(&mut self, history: &SlotHistory) {
        self.0.set_sysvar::<SlotHistoryOriginal>(&history.0)
    }

    #[napi]
    pub fn get_stake_history(&self) -> StakeHistory {
        StakeHistory(self.0.get_sysvar::<StakeHistoryOriginal>())
    }

    #[napi]
    pub fn set_stake_history(&mut self, history: &StakeHistory) {
        self.0.set_sysvar::<StakeHistoryOriginal>(&history.0)
    }

    #[napi]
    pub fn snapshot(&mut self)  {
        self.0.snapshot();
    }

    #[napi]
    pub fn revert(&mut self) {
        self.0.revert();
    }
}
