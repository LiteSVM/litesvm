use solana_program::last_restart_slot::LastRestartSlot;
use solana_program_runtime::{
    compute_budget::ComputeBudget, invoke_context::InvokeContext, loaded_programs::LoadedProgram,
    solana_rbpf::program::BuiltinProgram,
};
#[allow(deprecated)]
use solana_sdk::sysvar::{
    fees::Fees,
    recent_blockhashes::{IterItem, RecentBlockhashes},
};
use solana_sdk::{
    account::AccountSharedData, clock::Clock, epoch_rewards::EpochRewards,
    epoch_schedule::EpochSchedule, feature_set::FeatureSet, native_loader, pubkey::Pubkey,
    rent::Rent, signer::Signer, slot_hashes::SlotHashes, slot_history::SlotHistory,
    stake_history::StakeHistory, system_program,
};
use std::sync::Arc;

use crate::{builtin::BuiltinPrototype, LiteSVM};

#[derive(Default)]
pub struct LiteSVMBuilder {
    svm: LiteSVM,
}

impl LiteSVMBuilder {
    /// Creates new builder for the svm.
    pub fn new() -> Self {
        LiteSVMBuilder {
            svm: LiteSVM::default(),
        }
    }

    /// Sets the compute budget of the svm.
    pub fn compute_budget(mut self, compute_budget: ComputeBudget) -> Self {
        self.svm.compute_budget = Some(compute_budget);
        self
    }

    /// Sets the total lamports of the svm.
    pub fn lamports(mut self, lamports: u64) -> Self {
        self.svm.accounts.add_account_no_checks(
            self.svm.airdrop_kp.pubkey(),
            AccountSharedData::new(lamports, 0, &system_program::id()),
        );
        self
    }

    /// Allows to set a new capacity on the transactions history.
    pub fn transaction_history_capacity(mut self, capacity: usize) -> Self {
        self.svm.history.set_capacity(capacity);
        self
    }

    /// Sets the checks for the signature verification.
    pub fn sigverify(mut self, sigverify: bool) -> Self {
        self.svm.sigverify = sigverify;
        self
    }

    /// Sets the checks for the blockhash expiration.
    pub fn blockhash_check(mut self, check: bool) -> Self {
        self.svm.blockhash_check = check;
        self
    }

    /// Loads programs at the start of the svm.
    pub fn add_programs(mut self, programs: &[(Pubkey, &[u8])]) -> Self {
        programs.iter().for_each(|(program_id, program_bytes)| {
            self.svm.add_program(*program_id, program_bytes);
        });
        self
    }

    /// Sets the program runtime V1.
    pub fn program_runtime_v1(mut self, context: BuiltinProgram<InvokeContext<'static>>) -> Self {
        self.svm
            .accounts
            .programs_cache
            .environments
            .program_runtime_v1 = Arc::new(context);
        self
    }

    /// Sets the program runtime V2.
    pub fn program_runtime_v2(mut self, context: BuiltinProgram<InvokeContext<'static>>) -> Self {
        self.svm
            .accounts
            .programs_cache
            .environments
            .program_runtime_v2 = Arc::new(context);
        self
    }

    /// Loads the default sysvar needed.
    pub fn load_default_sysvar(mut self) -> Self {
        self.svm.set_sysvar(&Clock::default());
        self.svm.set_sysvar(&EpochRewards::default());
        self.svm.set_sysvar(&EpochSchedule::default());
        #[allow(deprecated)]
        let fees = Fees::default();
        self.svm.set_sysvar(&fees);
        self.svm.set_sysvar(&LastRestartSlot::default());
        let latest_blockhash = self.svm.latest_blockhash;
        #[allow(deprecated)]
        self.svm.set_sysvar(&RecentBlockhashes::from_iter([IterItem(
            0,
            &latest_blockhash,
            fees.fee_calculator.lamports_per_signature,
        )]));
        self.svm.set_sysvar(&Rent::default());
        self.svm.set_sysvar(&SlotHashes::new(&[(
            self.svm.accounts.sysvar_cache.get_clock().unwrap().slot,
            latest_blockhash,
        )]));
        self.svm.set_sysvar(&SlotHistory::default());
        self.svm.set_sysvar(&StakeHistory::default());
        self
    }

    pub(crate) fn built_ins(
        mut self,
        built_ins: &[BuiltinPrototype],
        mut feature_set: FeatureSet,
    ) -> Self {
        built_ins.iter().for_each(|built_in| {
            let loaded_program =
                LoadedProgram::new_builtin(0, built_in.name.len(), built_in.entrypoint);
            self.svm
                .accounts
                .programs_cache
                .replenish(built_in.program_id, Arc::new(loaded_program));
            self.svm.accounts.add_builtin_account(
                built_in.program_id,
                native_loader::create_loadable_account_for_test(built_in.name),
            );

            if let Some(feature_id) = built_in.feature_id {
                feature_set.activate(&feature_id, 0);
            }
        });
        self.svm.feature_set = Arc::new(feature_set);
        self
    }

    /// Builds the svm and returns the inner type.
    pub fn build(self) -> LiteSVM {
        self.svm
    }
}
