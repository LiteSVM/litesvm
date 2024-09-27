use agave_geyser_plugin_interface::geyser_plugin_interface::{
    GeyserPlugin, ReplicaAccountInfoV3, ReplicaAccountInfoVersions,
};
use solana_sdk::account::ReadableAccount;
use solana_sdk::{
    account::AccountSharedData, clock::Slot, pubkey::Pubkey, transaction::SanitizedTransaction,
};

pub(crate) struct AccountsUpdateNotifier {
    plugin: Box<dyn GeyserPlugin>,
}

impl AccountsUpdateNotifier {
    pub fn new(plugin: Box<dyn GeyserPlugin>) -> Self {
        Self { plugin }
    }

    pub fn notify_account_update(
        &self,
        slot: Slot,
        account: &AccountSharedData,
        txn: &Option<&SanitizedTransaction>,
        pubkey: &Pubkey,
        write_version: u64,
    ) {
        let account_info = ReplicaAccountInfoV3 {
            pubkey: pubkey.as_ref(),
            lamports: account.lamports(),
            owner: account.owner().as_ref(),
            executable: account.executable(),
            rent_epoch: account.rent_epoch(),
            data: account.data(),
            write_version,
            txn: *txn,
        };

        if let Err(e) = self.plugin.update_account(
            ReplicaAccountInfoVersions::V0_0_3(&account_info),
            slot,
            false,
        ) {
            log::warn!("Failed to notify plugin of account update: {:?}", e);
        }
    }
}
