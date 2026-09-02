use {
    solana_account::{Account, AccountSharedData},
    solana_hash::Hash,
    solana_instructions_sysvar::construct_instructions_data,
    solana_message::SanitizedMessage,
    solana_sha256_hasher::Hasher,
    solana_transaction_error::TransactionError,
};

pub mod inner_instructions;
pub mod rent;
#[cfg(feature = "serde")]
pub mod serde_with_str;

// Per SIMD-0186, all accounts are assigned a base size of 64 bytes to cover
// the storage cost of metadata.
pub(crate) const TRANSACTION_ACCOUNT_BASE_SIZE: usize = 64;
// Per SIMD-0186, resolved address lookup tables are assigned a base size of 8248
// bytes: 8192 bytes for the maximum table size plus 56 bytes for metadata.
pub(crate) const ADDRESS_LOOKUP_TABLE_BASE_SIZE: usize = 8248;

/// Create a blockhash from the given bytes
pub fn create_blockhash(bytes: &[u8]) -> Hash {
    let mut hasher = Hasher::default();
    hasher.hash(bytes);
    hasher.result()
}

pub fn construct_instructions_account(
    message: &SanitizedMessage,
) -> Result<AccountSharedData, TransactionError> {
    // Fails when serialized instruction offsets exceed u16::MAX; LiteSVM does
    // not enforce the packet size limit, so oversized transactions can get here.
    let data = construct_instructions_data(&message.decompile_instructions())
        .map_err(|_| TransactionError::SanitizeFailure)?;
    Ok(AccountSharedData::from(Account {
        data,
        owner: solana_sdk_ids::sysvar::id(),
        ..Account::default()
    }))
}

/// Tracks the size of loaded accounts data for a transaction, and the limit
/// on that size requested by the transaction.
/// Mostly copied from agave codebase:
/// https://github.com/anza-xyz/agave/blob/v4.2.0/svm/src/account_loader.rs#L457
#[derive(PartialEq, Eq, Debug, Clone)]
pub(crate) struct LoadedTransactionDataSize {
    loaded_accounts_data_size: u32,
    requested_loaded_accounts_data_size_limit: u32,
}

impl LoadedTransactionDataSize {
    pub(crate) fn with_max_size(requested_loaded_accounts_data_size_limit: u32) -> Self {
        Self {
            loaded_accounts_data_size: 0,
            requested_loaded_accounts_data_size_limit,
        }
    }

    /// Increases the loaded accounts data size by the given delta, and checks if it exceeds the requested limit.
    pub(crate) fn increase_calculated_data_size(
        &mut self,
        data_size_delta: usize,
    ) -> Result<(), TransactionError> {
        // this branch is unreachable in practice (though not by construction),
        // since it would imply an account >4gb in size
        let Ok(data_size_delta) = u32::try_from(data_size_delta) else {
            self.loaded_accounts_data_size = u32::MAX;
            return Err(TransactionError::MaxLoadedAccountsDataSizeExceeded);
        };

        self.loaded_accounts_data_size = self
            .loaded_accounts_data_size
            .saturating_add(data_size_delta);

        if self.loaded_accounts_data_size > self.requested_loaded_accounts_data_size_limit {
            Err(TransactionError::MaxLoadedAccountsDataSizeExceeded)
        } else {
            Ok(())
        }
    }
}
