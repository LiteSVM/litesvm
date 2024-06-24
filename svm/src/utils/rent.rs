use solana_sdk::{
    account::{AccountSharedData, ReadableAccount},
    rent::Rent,
};

//this code is taken from https://github.com/solana-labs/solana/blob/master/runtime/src/accounts/account_rent_state.rs

#[derive(Debug, PartialEq, Eq)]
pub enum RentState {
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

impl RentState {
    pub fn from_account(account: &AccountSharedData, rent: &Rent) -> Self {
        if account.lamports() == 0 {
            Self::Uninitialized
        } else if rent.is_exempt(account.lamports(), account.data().len()) {
            Self::RentExempt
        } else {
            Self::RentPaying {
                data_size: account.data().len(),
                lamports: account.lamports(),
            }
        }
    }

    pub fn transition_allowed_from(&self, pre_rent_state: &RentState) -> bool {
        match self {
            Self::Uninitialized | Self::RentExempt => true,
            Self::RentPaying {
                data_size: post_data_size,
                lamports: post_lamports,
            } => {
                match pre_rent_state {
                    Self::Uninitialized | Self::RentExempt => false,
                    Self::RentPaying {
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
}

#[cfg(test)]
mod tests {
    use {super::*, solana_sdk::pubkey::Pubkey};

    #[test]
    fn test_from_account() {
        let program_id = Pubkey::new_unique();
        let uninitialized_account = AccountSharedData::new(0, 0, &Pubkey::default());

        let account_data_size = 100;

        let rent = Rent::free();
        let rent_exempt_account = AccountSharedData::new(1, account_data_size, &program_id); // if rent is free, all accounts with non-zero lamports and non-empty data are rent-exempt

        assert_eq!(
            RentState::from_account(&uninitialized_account, &rent),
            RentState::Uninitialized
        );
        assert_eq!(
            RentState::from_account(&rent_exempt_account, &rent),
            RentState::RentExempt
        );

        let rent = Rent::default();
        let rent_minimum_balance = rent.minimum_balance(account_data_size);
        let rent_paying_account = AccountSharedData::new(
            rent_minimum_balance.saturating_sub(1),
            account_data_size,
            &program_id,
        );
        let rent_exempt_account = AccountSharedData::new(
            rent.minimum_balance(account_data_size),
            account_data_size,
            &program_id,
        );

        assert_eq!(
            RentState::from_account(&uninitialized_account, &rent),
            RentState::Uninitialized
        );
        assert_eq!(
            RentState::from_account(&rent_paying_account, &rent),
            RentState::RentPaying {
                data_size: account_data_size,
                lamports: rent_paying_account.lamports(),
            }
        );
        assert_eq!(
            RentState::from_account(&rent_exempt_account, &rent),
            RentState::RentExempt
        );
    }

    #[test]
    fn test_transition_allowed_from() {
        let post_rent_state = RentState::Uninitialized;
        assert!(post_rent_state.transition_allowed_from(&RentState::Uninitialized));
        assert!(post_rent_state.transition_allowed_from(&RentState::RentExempt));
        assert!(
            post_rent_state.transition_allowed_from(&RentState::RentPaying {
                data_size: 0,
                lamports: 1,
            })
        );

        let post_rent_state = RentState::RentExempt;
        assert!(post_rent_state.transition_allowed_from(&RentState::Uninitialized));
        assert!(post_rent_state.transition_allowed_from(&RentState::RentExempt));
        assert!(
            post_rent_state.transition_allowed_from(&RentState::RentPaying {
                data_size: 0,
                lamports: 1,
            })
        );
        let post_rent_state = RentState::RentPaying {
            data_size: 2,
            lamports: 5,
        };
        assert!(!post_rent_state.transition_allowed_from(&RentState::Uninitialized));
        assert!(!post_rent_state.transition_allowed_from(&RentState::RentExempt));
        assert!(
            !post_rent_state.transition_allowed_from(&RentState::RentPaying {
                data_size: 3,
                lamports: 5
            })
        );
        assert!(
            !post_rent_state.transition_allowed_from(&RentState::RentPaying {
                data_size: 1,
                lamports: 5
            })
        );
        // Transition is always allowed if there is no account data resize or
        // change in account's lamports.
        assert!(
            post_rent_state.transition_allowed_from(&RentState::RentPaying {
                data_size: 2,
                lamports: 5
            })
        );
        // Transition is always allowed if there is no account data resize and
        // account's lamports is reduced.
        assert!(
            post_rent_state.transition_allowed_from(&RentState::RentPaying {
                data_size: 2,
                lamports: 7
            })
        );
        // Transition is not allowed if the account is credited with more
        // lamports and remains rent-paying.
        assert!(
            !post_rent_state.transition_allowed_from(&RentState::RentPaying {
                data_size: 2,
                lamports: 3
            }),
        );
    }
}
