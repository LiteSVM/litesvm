#![deny(clippy::all)]
use {
    litesvm::{error::LiteSVMError, LiteSVM as LiteSVMOriginal},
    napi::bindgen_prelude::*,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_sdk::{account::Account as AccountOriginal, pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction},
};

#[macro_use]
extern crate napi_derive;

fn convert_pubkey(address: Uint8Array) -> Pubkey {
    Pubkey::try_from(address.as_ref()).unwrap()
}

fn to_js_error(e: LiteSVMError, msg: &str) -> Error {
    Error::new(Status::GenericFailure, format!("{msg}: {e}"))
}

#[derive(Debug, Clone)]
#[napi]
pub struct Account(AccountOriginal);

impl AsRef<AccountOriginal> for Account {
    fn as_ref(&self) -> &AccountOriginal {
        &self.0
    }
}

#[napi]
impl Account {
    #[napi(constructor)]
    pub fn new(
        lamports: BigInt,
        data: Uint8Array,
        owner: Uint8Array,
        executable: bool,
        rent_epoch: BigInt,
    ) -> Self {
        Self(AccountOriginal {
            lamports: lamports.get_u64().1,
            data: data.to_vec(),
            owner: Pubkey::try_from(owner.as_ref()).unwrap(),
            executable,
            rent_epoch: rent_epoch.get_u64().1,
        })
    }

    #[napi(getter)]
    pub fn lamports(&self) -> u64 {
        self.0.lamports
    }

    #[napi(getter)]
    pub fn data(&self) -> Uint8Array {
        Uint8Array::new(self.0.data.clone())
    }

    #[napi(getter)]
    pub fn owner(&self) -> Uint8Array {
        Uint8Array::new(self.0.owner.to_bytes().to_vec())
    }

    #[napi(getter)]
    pub fn executable(&self) -> bool {
        self.0.executable
    }

    #[napi(getter)]
    pub fn rent_epoch(&self) -> u64 {
        self.0.rent_epoch
    }
}

#[napi]
pub struct LiteSVM(LiteSVMOriginal);

#[napi]
impl LiteSVM {
    /// Creates the basic test environment.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self(LiteSVMOriginal::new())
    }

    /// Sets the compute budget.
    pub fn with_compute_budget(mut self, compute_budget: ComputeBudget) -> Self {
        Self(self.0.with_compute_budget(compute_budget.into()))
    }
    /// Enables or disables sigverify
    pub fn with_sigverify(mut self, sigverify: bool) -> Self {
        Self(self.0.with_sigverify(sigverify))
    }
    /// Enables or disables the blockhash check
    pub fn with_blockhash_check(mut self, check: bool) -> Self {
        Self(self.0.with_blockhash_check(check))
    }
    /// Includes the default sysvars
    pub fn with_sysvars(mut self) -> Self {
        Self(self.0.with_sysvars())
    }

    /// Changes the default builtins
    pub fn with_builtins(mut self, feature_set: Option<FeatureSet>) -> Self {
        Self(self.0.with_builtins(feature_set.map(|x| x.into())))
    }

    /// Changes the initial lamports in LiteSVM's airdrop account
    pub fn with_lamports(mut self, lamports: u64) -> Self {
        Self(self.0.with_lamports(lamports))
    }

    /// Includes the standard SPL programs
    pub fn with_spl_programs(mut self) -> Self {
        Self(self.0.with_spl_programs())
    }

    /// Changes the capacity of the transaction history.
    /// Set this to 0 to disable transaction history and allow duplicate transactions.
    pub fn with_transaction_history(mut self, capacity: usize) -> Self {
        Self(self.0.with_transaction_history(capacity))
    }

    pub fn with_log_bytes_limit(mut self, limit: Option<usize>) -> Self {
        Self(self.0.with_log_bytes_limit(limit))
    }
    pub fn with_precompiles(mut self, feature_set: Option<FeatureSet>) -> Self {
        Self(self.0.with_precompiles(feature_set.map(|x| x.into())))
    }

    /// Returns minimum balance required to make an account with specified data length rent exempt.
    pub fn minimum_balance_for_rent_exemption(&self, data_len: usize) -> u64 {
        self.0.minimum_balance_for_rent_exemption(data_len)
    }

    /// Returns all information associated with the account of the provided pubkey.
    pub fn get_account(&self, pubkey: Uint8Array) -> Option<Account> {
        self.0.get_account(&convert_pubkey(pubkey)).map(Account)
    }

    /// Sets all information associated with the account of the provided pubkey.
    pub fn set_account(&mut self, pubkey: Uint8Array, data: Account) -> Result<()> {
        self.0.set_account(convert_pubkey(pubkey), data.into())
    }

    /// Gets the balance of the provided account pubkey.
    pub fn get_balance(&self, pubkey: Uint8Array) -> Option<u64> {
        self.0.get_balance(&convert_pubkey(pubkey))
    }

    /// Gets the latest blockhash.
    pub fn latest_blockhash(&self) -> String {
        self.0.latest_blockhash().to_string()
    }

    /// Gets a transaction from the transaction history.
    pub fn get_transaction(&self, signature: Uint8Array) -> Option<&TransactionResult> {
        self.0
            .get_transaction(&Signature::try_from(signature.as_ref()).unwrap())
    }

    /// Airdrops the account with the lamports specified.
    pub fn airdrop(&mut self, pubkey: Uint8Array, lamports: u64) -> TransactionResult {
        self.0.airdrop(&convert_pubkey(pubkey), lamports)
    }

    /// Adds am SBF program to the test environment from the file specified.
    pub fn add_program_from_file(
        &mut self,
        program_id: Uint8Array,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        self.0
            .add_program_from_file(convert_pubkey(program_id), path)
    }

    /// Adds am SBF program to the test environment.
    pub fn add_program(&mut self, program_id: Uint8Array, program_bytes: &[u8]) {
        self.0.add_program(convert_pubkey(program_id), program_bytes)
    }

    /// Submits a transaction.
    pub fn send_transaction(&mut self, tx: impl Into<VersionedTransaction>) -> Result<()> {
        self.0.send_transaction(tx)
    }
    /// Simulates a transaction.
    pub fn simulate_transaction(
        &self,
        tx: impl Into<VersionedTransaction>,
    ) -> Result<SimulatedTransactionInfo, FailedTransactionMetadata> {
        self.0.simulate_transaction(tx)
    }

    /// Expires the current blockhash
    pub fn expire_blockhash(&mut self) {
        self.0.expire_blockhash()
    }

    /// Warps the clock to the specified sllot
    pub fn warp_to_slot(&mut self, slot: u64) {
        self.0.warp_to_slot(slot)
    }

    /// Gets the current compute budget
    pub fn get_compute_budget(&self) -> Option<ComputeBudget> {
        self.0.get_compute_budget()
    }
}
