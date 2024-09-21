use agave_geyser_plugin_interface::geyser_plugin_interface::GeyserPlugin;
use solana_sdk::{
    account::AccountSharedData, clock::Slot, pubkey::Pubkey, transaction::SanitizedTransaction,
};

use crate::accounts_update_notifier::AccountsUpdateNotifier;

mod accounts_update_notifier;

#[derive(Default)]
pub struct GeyserPluginService {
    accounts_update_notifier: Option<AccountsUpdateNotifier>,
}

impl GeyserPluginService {
    pub fn set_plugin(&mut self, plugin: Box<dyn GeyserPlugin>) {
        if plugin.account_data_notifications_enabled() {
            self.accounts_update_notifier = Some(AccountsUpdateNotifier::new(plugin));
        }
    }

    pub fn notify_account_update(
        &self,
        slot: Slot,
        txn: &Option<&SanitizedTransaction>,
        account: &AccountSharedData,
        pubkey: &Pubkey,
    ) {
        if let Some(notifier) = self.accounts_update_notifier.as_ref() {
            notifier.notify_account_update(
                slot, account, txn, pubkey, 1, // NOTE: hardcoded write_version
            );
        }
    }
}
