use {crate::LiteSVM, solana_address::Address, solana_svm_callback::InvokeContextCallback};
#[cfg(feature = "precompiles")]
use {
    agave_precompiles::{get_precompile, is_precompile},
    solana_precompile_error::PrecompileError,
};

impl InvokeContextCallback for LiteSVM {
    fn get_epoch_stake(&self) -> u64 {
        self.epoch_total_stake
    }

    fn get_epoch_stake_for_vote_account(&self, vote_address: &Address) -> u64 {
        self.epoch_vote_stakes
            .get(vote_address)
            .copied()
            .unwrap_or(0)
    }

    #[cfg(feature = "precompiles")]
    fn is_precompile(&self, program_id: &Address) -> bool {
        is_precompile(program_id, |feature_id: &Address| {
            self.feature_set.is_active(feature_id)
        })
    }

    #[cfg(feature = "precompiles")]
    fn process_precompile(
        &self,
        program_id: &Address,
        data: &[u8],
        instruction_datas: Vec<&[u8]>,
    ) -> Result<(), PrecompileError> {
        if let Some(precompile) = get_precompile(program_id, |feature_id: &Address| {
            self.feature_set.is_active(feature_id)
        }) {
            precompile.verify(data, &instruction_datas, &self.feature_set)
        } else {
            Err(PrecompileError::InvalidPublicKey)
        }
    }
}
