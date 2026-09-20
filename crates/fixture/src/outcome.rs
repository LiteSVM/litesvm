use {
    crate::{backend::ExecutionResult, Account, AccountChange, ProgramError, Pubkey},
    base64::{engine::general_purpose::STANDARD, Engine as _},
};

pub(crate) struct TrackedAccount {
    pub(crate) address: Pubkey,
    pub(crate) writable: bool,
    pub(crate) signer: bool,
    pub(crate) before: Option<Account>,
    pub(crate) after: Option<Account>,
}

/// The structured result of executing one transaction.
///
/// `Outcome` owns the stable data tests normally need: the program error,
/// logs, return data, compute units, resulting accounts, and writable account
/// changes. The runtime's internal result type is intentionally private.
#[must_use = "check the outcome, e.g. .check(Outcome::success())"]
pub struct Outcome {
    error: Option<ProgramError>,
    compute_units: u64,
    logs: Vec<String>,
    return_data: Vec<u8>,
    accounts: Vec<Account>,
    changes: Vec<AccountChange>,
    hint: Option<String>,
}

impl Outcome {
    pub(crate) fn from_backend(result: ExecutionResult, tracked: Vec<TrackedAccount>) -> Self {
        // Tracked addresses are unique, so the resulting set needs no dedup, and
        // its order is never observed (accounts are looked up by address). A
        // single pass moves each post-state into the resulting set, cloning only
        // the changed writable accounts that also feed an `AccountChange`.
        let mut accounts = Vec::with_capacity(tracked.len());
        let mut changes = Vec::new();
        for account in tracked {
            let changed = account.writable && account.before != account.after;
            match account.after {
                Some(after) if changed => {
                    accounts.push(after.clone());
                    changes.push(AccountChange::new(
                        account.address,
                        account.before,
                        Some(after),
                    ));
                }
                Some(after) => accounts.push(after),
                None if changed => {
                    changes.push(AccountChange::new(account.address, account.before, None));
                }
                None => {}
            }
        }

        Self {
            error: result.error,
            compute_units: result.compute_units_consumed,
            logs: result.logs,
            return_data: result.return_data,
            accounts,
            changes,
            hint: None,
        }
    }

    /// Attach a guided-error hint (e.g. a missing dumped account), surfaced by
    /// the failure assertions. See [`crate::fixture::Dump`].
    pub(crate) fn with_hint(mut self, hint: Option<String>) -> Self {
        self.hint = hint;
        self
    }

    pub(crate) fn simulated_account(result: &ExecutionResult, address: &Pubkey) -> Option<Account> {
        result.account(address).cloned()
    }

    /// The guided-error hint attached to this outcome, if any.
    #[doc(hidden)]
    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    /// Whether execution succeeded.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    /// Whether execution failed.
    pub fn is_err(&self) -> bool {
        self.error.is_some()
    }

    /// The execution error, if any. (The verdict *fact* is
    /// [`Outcome::error`] taking an expectation — this is the raw read.)
    pub fn failure(&self) -> Option<&ProgramError> {
        self.error.as_ref()
    }

    /// Compute units consumed by the transaction.
    pub fn compute_units(&self) -> u64 {
        self.compute_units
    }

    /// Program logs in execution order.
    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    /// Raw Solana return data.
    pub fn return_data(&self) -> &[u8] {
        &self.return_data
    }

    /// Decode return data with a generated or application-provided decoder.
    pub fn return_value<T>(&self, decode: impl FnOnce(&[u8]) -> Option<T>) -> Option<T> {
        decode(&self.return_data)
    }

    /// The resulting account state for an address involved in the transaction.
    pub fn account(&self, address: Pubkey) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| account.address == address)
    }

    /// Every tracked account's resulting post-state, in first-appearance
    /// instruction order.
    ///
    /// Accounts that no longer exist after execution (closed or never created)
    /// are omitted, as they have no post-state; a removed writable account
    /// still surfaces through [`Self::account_changes`] with a `None` after.
    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    /// Decode a resulting account with a caller-supplied decoder.
    pub fn account_as<T>(
        &self,
        address: Pubkey,
        decode: impl FnOnce(&[u8]) -> Option<T>,
    ) -> Option<T> {
        self.account(address)
            .and_then(|account| decode(&account.data))
    }

    /// Writable account changes, in first-appearance instruction order.
    pub fn account_changes(&self) -> &[AccountChange] {
        &self.changes
    }

    /// Decode every matching `sol_log_data` payload with a generated client
    /// event decoder. Unrelated program-data logs are ignored.
    pub fn events<T>(&self, decode: impl Fn(&[u8]) -> Option<T>) -> Vec<T> {
        self.logs
            .iter()
            .filter_map(|log| log.strip_prefix("Program data: "))
            .filter_map(|encoded| STANDARD.decode(encoded).ok())
            .filter_map(|bytes| decode(&bytes))
            .collect()
    }

    /// Run one check or one [`bundle`](crate::bundle). Chainable.
    ///
    /// Facts self-diagnose: any account or transaction fact running against a
    /// failed transaction panics leading with the transaction's error and
    /// logs, so an unstated [`Outcome::success`] still fails loudly with the
    /// real cause. Verdicts are checks too: [`Outcome::success`] and
    /// [`Outcome::error`].
    pub fn check(&self, check: crate::CheckFn) -> &Self {
        check.run(self);
        self
    }

    /// Run several checks and/or bundles. Chainable.
    pub fn checks(&self, checks: impl IntoIterator<Item = crate::CheckFn>) -> &Self {
        for check in checks {
            check.run(self);
        }
        self
    }

    pub(crate) fn formatted_logs(&self) -> String {
        let mut formatted = if self.logs.is_empty() {
            String::new()
        } else {
            format!("\nprogram logs:\n  {}", self.logs.join("\n  "))
        };
        if let Some(hint) = &self.hint {
            formatted.push_str("\nhint: ");
            formatted.push_str(hint);
        }
        formatted
    }
}

pub(crate) fn token_amount(account: &Account) -> u64 {
    use spl_token::{solana_program::program_pack::Pack, state::Account as TokenAccount};

    TokenAccount::unpack(&account.data)
        .unwrap_or_else(|error| {
            panic!(
                "could not decode {} as a token account: {error}",
                account.address
            )
        })
        .amount
}

pub(crate) fn mint_supply(account: &Account) -> u64 {
    use spl_token::{solana_program::program_pack::Pack, state::Mint};

    Mint::unpack(&account.data)
        .unwrap_or_else(|error| {
            panic!(
                "could not decode {} as a token mint: {error}",
                account.address
            )
        })
        .supply
}
