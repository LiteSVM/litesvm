//! This code is based on https://github.com/anza-xyz/agave/blob/master/svm/src/rent_calculator.rs.
use {
    solana_address::Address,
    solana_rent::Rent,
    solana_transaction_context::IndexOfAccount,
    solana_transaction_error::{TransactionError, TransactionResult},
};

/// Rent state of a Solana account.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RentState {
    /// account.lamports == 0
    Uninitialized,
    /// 0 < account.lamports < rent-exempt-minimum
    RentPaying {
        lamports: u64,    // account.lamports()
        data_size: usize, // account.data().len()
    },
    /// account.lamports >= rent-exempt-minimum
    RentExempt,
}

/// Rent-relevant state captured for a writable transaction account before
/// instruction execution.
#[derive(Debug)]
pub(crate) struct RentStateInfo {
    rent_state: RentState,
    balance: u64,
    data_size: usize,
    owner: Address,
    relax_post_exec_min_balance_check: bool,
}

impl RentStateInfo {
    pub(crate) fn new_pre_exec(
        rent: &Rent,
        balance: u64,
        data_size: usize,
        owner: Address,
        relax_post_exec_min_balance_check: bool,
    ) -> Self {
        // SIMD-0392: an existing account with a positive balance counts as
        // rent-exempt even if it is below the current minimum, and its
        // pre-execution balance becomes the grandfathered minimum.
        let rent_state = match get_account_rent_state(rent, balance, data_size) {
            RentState::RentPaying { .. } if relax_post_exec_min_balance_check => {
                RentState::RentExempt
            }
            rent_state => rent_state,
        };
        Self {
            rent_state,
            balance,
            data_size,
            owner,
            relax_post_exec_min_balance_check,
        }
    }

    /// Rent state of the account after instruction execution.
    pub(crate) fn post_exec_rent_state(
        &self,
        rent: &Rent,
        balance: u64,
        data_size: usize,
        owner: Address,
    ) -> RentState {
        // SIMD-0392: a same-owner account that did not grow may stay below the
        // current rent-exempt minimum as long as its balance did not drop.
        let grandfathered = self.relax_post_exec_min_balance_check
            && self.rent_state == RentState::RentExempt
            && self.owner == owner
            && self.data_size >= data_size
            && balance >= self.balance;

        match get_account_rent_state(rent, balance, data_size) {
            RentState::RentPaying { .. } if grandfathered => RentState::RentExempt,
            rent_state => rent_state,
        }
    }

    pub(crate) fn rent_state(&self) -> &RentState {
        &self.rent_state
    }
}

/// Check rent state transition for an account directly.
///
/// This method has a default implementation that checks whether the
/// transition is allowed and returns an error if it is not. It also
/// verifies that the account is not the incinerator.
pub(crate) fn check_rent_state_with_account(
    pre_rent_state: &RentState,
    post_rent_state: &RentState,
    address: &Address,
    account_index: IndexOfAccount,
) -> TransactionResult<()> {
    if !solana_sdk_ids::incinerator::check_id(address)
        && !transition_allowed(pre_rent_state, post_rent_state)
    {
        let account_index = account_index as u8;
        Err(TransactionError::InsufficientFundsForRent { account_index })
    } else {
        Ok(())
    }
}

/// Determine the rent state of an account.
///
/// This method has a default implementation that treats accounts with zero
/// lamports as uninitialized and uses the implemented `get_rent` to
/// determine whether an account is rent-exempt.
fn get_account_rent_state(rent: &Rent, account_lamports: u64, account_size: usize) -> RentState {
    if account_lamports == 0 {
        RentState::Uninitialized
    } else if rent.is_exempt(account_lamports, account_size) {
        RentState::RentExempt
    } else {
        RentState::RentPaying {
            data_size: account_size,
            lamports: account_lamports,
        }
    }
}

/// Check whether a transition from the pre_rent_state to the
/// post_rent_state is valid.
///
/// This method has a default implementation that allows transitions from
/// uninitialized to any state, from rent-paying to rent-exempt, and from
/// rent-exempt to rent-exempt. It also allows transitions from rent-paying
/// to rent-paying if the data size is the same and the lamports are not
/// decreasing.
fn transition_allowed(pre_rent_state: &RentState, post_rent_state: &RentState) -> bool {
    match post_rent_state {
        RentState::Uninitialized | RentState::RentExempt => true,
        RentState::RentPaying {
            data_size: post_data_size,
            lamports: post_lamports,
        } => {
            match pre_rent_state {
                RentState::Uninitialized | RentState::RentExempt => false,
                RentState::RentPaying {
                    data_size: pre_data_size,
                    lamports: pre_lamports,
                } => {
                    // Cannot remain RentPaying if resized or credited.
                    post_data_size == pre_data_size && post_lamports <= pre_lamports
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_0392_grandfathers_static_and_smaller_same_owner_accounts() {
        let rent = Rent::default();
        let owner = Address::new_unique();
        let data_size = 64;
        let pre_balance = rent.minimum_balance(data_size) / 2;
        assert!(!rent.is_exempt(pre_balance, data_size));

        let pre = RentStateInfo::new_pre_exec(&rent, pre_balance, data_size, owner, true);
        assert_eq!(pre.rent_state(), &RentState::RentExempt);

        let topped_up = pre.post_exec_rent_state(&rent, pre_balance + 1, data_size, owner);
        assert_eq!(topped_up, RentState::RentExempt);
        assert!(transition_allowed(pre.rent_state(), &topped_up));

        let smaller_data_size = data_size - 1;
        assert!(!rent.is_exempt(pre_balance, smaller_data_size));
        let shrunk = pre.post_exec_rent_state(&rent, pre_balance, smaller_data_size, owner);
        assert_eq!(shrunk, RentState::RentExempt);
        assert!(transition_allowed(pre.rent_state(), &shrunk));
    }

    #[test]
    fn simd_0392_requires_pre_balance_current_owner_and_no_growth() {
        let rent = Rent::default();
        let owner = Address::new_unique();
        let data_size = 64;
        let pre_balance = rent.minimum_balance(data_size) / 2;
        let pre = RentStateInfo::new_pre_exec(&rent, pre_balance, data_size, owner, true);

        let reduced = pre.post_exec_rent_state(&rent, pre_balance - 1, data_size, owner);
        assert!(!transition_allowed(pre.rent_state(), &reduced));

        let grown = pre.post_exec_rent_state(&rent, pre_balance, data_size + 1, owner);
        assert!(!transition_allowed(pre.rent_state(), &grown));

        let changed_owner =
            pre.post_exec_rent_state(&rent, pre_balance, data_size, Address::new_unique());
        assert!(!transition_allowed(pre.rent_state(), &changed_owner));
    }

    #[test]
    fn simd_0392_does_not_relax_new_or_legacy_transitions() {
        let rent = Rent::default();
        let owner = Address::new_unique();
        let data_size = 64;
        let pre_balance = rent.minimum_balance(data_size) / 2;

        let legacy_pre = RentStateInfo::new_pre_exec(&rent, pre_balance, data_size, owner, false);
        let legacy_topped_up =
            legacy_pre.post_exec_rent_state(&rent, pre_balance + 1, data_size, owner);
        assert!(!transition_allowed(
            legacy_pre.rent_state(),
            &legacy_topped_up
        ));

        let new_pre = RentStateInfo::new_pre_exec(&rent, 0, 0, owner, true);
        let underfunded_new = new_pre.post_exec_rent_state(&rent, 1, 0, owner);
        assert!(!transition_allowed(new_pre.rent_state(), &underfunded_new));
    }
}
