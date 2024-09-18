use agave_geyser_plugin_interface::geyser_plugin_interface::GeyserPlugin;

#[derive(Default, Debug)]
pub struct GeyserPluginManager {
    pub plugin: Option<GeyserPlugin>,
}

impl GeyserPluginManager {
    pub fn unload(&mut self) {
        if let Some(mut plugin) = self.plugin {
            plugin.on_unload();
        }
    }

    pub fn account_data_notifications_enabled(&self) -> bool {
        if let Some(plugin) = self.plugin {
            plugin.account_data_notifications_enabled()
        }
        false
    }

    pub fn transaction_notifications_enabled(&self) -> bool {
        if let Some(plugin) = self.plugin {
            plugin.transaction_notifications_enabled()
        }
        false
    }

    pub fn entry_notifications_enabled(&self) -> bool {
        if let Some(plugin) = self.plugin {
            plugin.entry_notifications_enabled()
        }
        false
    }
}
