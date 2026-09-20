use {
    crate::{
        fixture::{Account, Instruction, ProgramError, Pubkey, DEFAULT_WALLET_LAMPORTS},
        LiteSVM,
    },
    solana_account::{Account as SolanaAccount, AccountSharedData, ReadableAccount},
    solana_clock::Clock,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_fee_structure::FeeStructure,
    solana_message::{Message, VersionedMessage},
    solana_signature::Signature,
    solana_transaction::versioned::VersionedTransaction,
};

/// Deterministic fee payer used only when a transaction names no signer of its
/// own. Fees are zero, so this account never moves lamports, and because the
/// tracked set is built from instruction metas it never appears in an outcome.
const FEE_PAYER: Pubkey =
    Pubkey::new_from_array(*b"parallax/fee-payer\0\0\0\0\0\0\0\0\0\0\0\0\0\0");

/// A backend-neutral view of one transaction's outcome.
///
/// This intentionally exposes only the data the harness consumes: the mapped
/// program error, compute units, logs, return data, and the post-execution
/// accounts a simulation produced. Committed reads come from the live store.
pub(crate) struct ExecutionResult {
    pub(crate) error: Option<ProgramError>,
    pub(crate) compute_units_consumed: u64,
    pub(crate) logs: Vec<String>,
    pub(crate) return_data: Vec<u8>,
    post_accounts: Vec<Account>,
}

impl ExecutionResult {
    /// Whether execution succeeded.
    pub(crate) fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    /// A simulated post-execution account, if the runtime reported one. Only
    /// writable accounts are captured during simulation; callers fall back to
    /// the pre-state for read-only accounts, which cannot change.
    pub(crate) fn account(&self, address: &Pubkey) -> Option<&Account> {
        self.post_accounts
            .iter()
            .find(|account| account.address == *address)
    }
}

pub(crate) struct Backend {
    svm: LiteSVM,
}

impl Backend {
    pub(crate) fn new() -> Self {
        let mut svm = LiteSVM::new()
            .with_sigverify(false)
            .with_blockhash_check(false);
        // The live runtime charges no fees here: the harness measures compute,
        // not economics, and spoofed signers never fund a fee.
        svm.fee_structure = FeeStructure {
            lamports_per_signature: 0,
            lamports_per_write_lock: 0,
            compute_fee_bins: vec![],
        };
        Self { svm }
    }

    pub(crate) fn set_compute_unit_limit(&mut self, limit: u64) {
        let mut budget = self
            .svm
            .get_compute_budget()
            .unwrap_or_else(|| ComputeBudget::new_with_defaults(true));
        budget.compute_unit_limit = limit;
        // `with_compute_budget` consumes and returns the VM; swap it out so the
        // fixed budget replaces the runtime-derived default while preserving
        // every loaded program and account.
        let svm = std::mem::take(&mut self.svm);
        self.svm = svm.with_compute_budget(budget);
    }

    pub(crate) fn load_program(&mut self, program_id: &Pubkey, elf: &[u8]) {
        // LiteSVM's default loader is the upgradeable (v3) loader, matching the
        // loader used for on-chain program deployment.
        self.svm
            .add_program(*program_id, elf)
            .expect("program artifact is a valid SBF ELF");
    }

    /// Load a program under an explicit loader, matching the loader a dumped
    /// mainnet program was deployed under so its compute-unit behavior is
    /// faithful. Returns a message rather than panicking so a corrupt dumped ELF
    /// surfaces as a clean harness error.
    pub(crate) fn load_program_with_loader(
        &mut self,
        program_id: &Pubkey,
        elf: &[u8],
        loader: Pubkey,
    ) -> Result<(), String> {
        self.svm
            .add_program_with_loader(*program_id, elf, loader)
            .map_err(|error| format!("could not load dumped program {program_id}: {error:?}"))
    }

    /// Adopt a dumped slot's clock: set the runtime clock's `slot` and derived
    /// Unix `timestamp` together. Used by `Dump::sync_clock`.
    pub(crate) fn sync_clock(&mut self, slot: u64, unix_timestamp: i64) {
        let mut clock: Clock = self.svm.get_sysvar();
        clock.slot = slot;
        clock.unix_timestamp = unix_timestamp;
        self.svm.set_sysvar(&clock);
    }

    pub(crate) fn set_account(&mut self, account: Account) {
        // A zero-lamport account is removed by LiteSVM, matching Solana's rule
        // that an empty account does not exist.
        let _ = self
            .svm
            .set_account(account.address, to_backend_account(&account));
    }

    pub(crate) fn account(&self, address: &Pubkey) -> Option<Account> {
        self.svm
            .get_account(address)
            .map(|account| from_backend_account(*address, account))
    }

    pub(crate) fn execute(
        &mut self,
        instructions: &[Instruction],
        inputs: &[Account],
        commit: bool,
    ) -> ExecutionResult {
        // Snapshot the live state of every input so a simulation or a failed
        // commit leaves the world untouched: the SVM only ever persists
        // transaction inputs on a successful commit.
        let snapshot: Vec<(Pubkey, Option<SolanaAccount>)> = inputs
            .iter()
            .map(|account| (account.address, self.svm.get_account(&account.address)))
            .collect();
        for account in inputs {
            let _ = self
                .svm
                .set_account(account.address, to_backend_account(account));
        }

        let payer = fee_payer(instructions);
        if payer == FEE_PAYER && self.svm.get_account(&payer).is_none() {
            let _ = self.svm.airdrop(&payer, DEFAULT_WALLET_LAMPORTS);
        }

        let message = Message::new(instructions, Some(&payer));
        let transaction = VersionedTransaction {
            signatures: vec![Signature::default(); message.header.num_required_signatures as usize],
            message: VersionedMessage::Legacy(message),
        };

        let result = if commit {
            match self.svm.send_transaction(transaction) {
                Ok(meta) => ExecutionResult {
                    error: None,
                    compute_units_consumed: meta.compute_units_consumed,
                    logs: meta.logs,
                    return_data: meta.return_data.data,
                    post_accounts: Vec::new(),
                },
                Err(failure) => ExecutionResult {
                    error: Some(ProgramError::from(failure.err)),
                    compute_units_consumed: failure.meta.compute_units_consumed,
                    logs: failure.meta.logs,
                    return_data: failure.meta.return_data.data,
                    post_accounts: Vec::new(),
                },
            }
        } else {
            match self.svm.simulate_transaction(transaction) {
                Ok(info) => ExecutionResult {
                    error: None,
                    compute_units_consumed: info.meta.compute_units_consumed,
                    logs: info.meta.logs,
                    return_data: info.meta.return_data.data,
                    post_accounts: info
                        .post_accounts
                        .into_iter()
                        .map(|(address, account)| from_shared_account(address, &account))
                        .collect(),
                },
                Err(failure) => ExecutionResult {
                    error: Some(ProgramError::from(failure.err)),
                    compute_units_consumed: failure.meta.compute_units_consumed,
                    logs: failure.meta.logs,
                    return_data: failure.meta.return_data.data,
                    post_accounts: Vec::new(),
                },
            }
        };

        // A successful commit is the only case that keeps input state: restore
        // everything else so simulations and failures do not leak into the world.
        if !(commit && result.is_ok()) {
            for (address, snapshot) in snapshot {
                let _ = self.svm.set_account(address, snapshot.unwrap_or_default());
            }
        }

        result
    }

    pub(crate) fn warp_to_timestamp(&mut self, timestamp: i64) {
        let mut clock: Clock = self.svm.get_sysvar();
        clock.unix_timestamp = timestamp;
        self.svm.set_sysvar(&clock);
    }
}

/// The first signer named across the instructions becomes the fee payer, as it
/// does on chain. Permissionless transactions borrow the inert [`FEE_PAYER`].
fn fee_payer(instructions: &[Instruction]) -> Pubkey {
    instructions
        .iter()
        .flat_map(|instruction| instruction.accounts.iter())
        .find(|meta| meta.is_signer)
        .map_or(FEE_PAYER, |meta| meta.pubkey)
}

fn from_backend_account(address: Pubkey, account: SolanaAccount) -> Account {
    Account {
        address,
        lamports: account.lamports,
        data: account.data,
        owner: account.owner,
        executable: account.executable,
    }
}

fn from_shared_account(address: Pubkey, account: &AccountSharedData) -> Account {
    Account {
        address,
        lamports: account.lamports(),
        data: account.data().to_vec(),
        owner: *account.owner(),
        executable: account.executable(),
    }
}

fn to_backend_account(account: &Account) -> SolanaAccount {
    SolanaAccount {
        lamports: account.lamports,
        data: account.data.clone(),
        owner: account.owner,
        executable: account.executable,
        rent_epoch: 0,
    }
}
