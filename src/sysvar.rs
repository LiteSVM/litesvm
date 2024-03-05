use solana_program_runtime::sysvar_cache::SysvarCache;
use solana_sdk::{
    account::{Account, ReadableAccount},
    sysvar::{Sysvar as SysvarTrait, SysvarId},
};

use crate::LiteSVM;

pub trait Sysvar {
    fn set_sysvar<T: SysvarTrait + SysvarId>(&mut self, sysvar: &T);

    fn get_sysvar<T: SysvarTrait + SysvarId>(&self) -> T;

    fn sysvar_cache(&self) -> SysvarCache;
}

impl Sysvar for LiteSVM {
    fn set_sysvar<T: SysvarTrait + SysvarId>(&mut self, sysvar: &T) {
        let account = Account::new_data(1, &sysvar, &solana_sdk::sysvar::id()).unwrap();

        self.set_account(T::id(), account);
    }

    fn get_sysvar<T: SysvarTrait + SysvarId>(&self) -> T {
        let account = self.get_account(&T::id());

        bincode::deserialize(account.data()).unwrap()
    }

    fn sysvar_cache(&self) -> SysvarCache {
        let mut cache = SysvarCache::default();

        cache.set_clock(self.get_sysvar());
        cache.set_rent(self.get_sysvar());

        cache
    }
}
