use {
    crate::LiteSVM,
    agave_precompiles::{get_precompile, is_precompile},
    solana_precompile_error::PrecompileError,
    solana_pubkey::Pubkey,
    solana_svm_callback::InvokeContextCallback,
};

impl InvokeContextCallback for LiteSVM {
    fn is_precompile(&self, program_id: &solana_pubkey::Pubkey) -> bool {
        is_precompile(program_id, |feature_id: &Pubkey| {
            self.feature_set.is_active(feature_id)
        })
    }

    fn process_precompile(
        &self,
        program_id: &solana_pubkey::Pubkey,
        data: &[u8],
        instruction_datas: Vec<&[u8]>,
    ) -> Result<(), solana_precompile_error::PrecompileError> {
        if let Some(precompile) = get_precompile(program_id, |feature_id: &Pubkey| {
            self.feature_set.is_active(feature_id)
        }) {
            precompile.verify(data, &instruction_datas, &self.feature_set)
        } else {
            Err(PrecompileError::InvalidPublicKey)
        }
    }
}
