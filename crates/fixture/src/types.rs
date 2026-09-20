use {
    crate::Pubkey, core::fmt, solana_instruction_error::InstructionError,
    solana_transaction_error::TransactionError,
};

/// An account stored in a [`Ctx`](crate::Ctx) world.
///
/// This is the portable account shape used by the test harness. It deliberately
/// does not expose the account type of the runtime that executes the test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Address at which the account is installed.
    pub address: Pubkey,
    /// Account balance in lamports.
    pub lamports: u64,
    /// Raw account data.
    pub data: Vec<u8>,
    /// Program that owns the account.
    pub owner: Pubkey,
    /// Whether the account contains an executable program.
    pub executable: bool,
}

impl Account {
    /// Create a non-executable account.
    pub fn new(address: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> Self {
        Self {
            address,
            lamports,
            data,
            owner,
            executable: false,
        }
    }

    /// Mark this account as executable or non-executable.
    pub fn executable(mut self, executable: bool) -> Self {
        self.executable = executable;
        self
    }
}

/// A writable account's state before and after an execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountChange {
    address: Pubkey,
    before: Option<Account>,
    after: Option<Account>,
}

impl AccountChange {
    pub(crate) fn new(address: Pubkey, before: Option<Account>, after: Option<Account>) -> Self {
        Self {
            address,
            before,
            after,
        }
    }

    /// The account address.
    pub fn address(&self) -> Pubkey {
        self.address
    }

    /// State before execution, or `None` when the execution created it.
    pub fn before(&self) -> Option<&Account> {
        self.before.as_ref()
    }

    /// State after execution, or `None` when the execution removed it.
    pub fn after(&self) -> Option<&Account> {
        self.after.as_ref()
    }

    /// Whether this account did not exist before execution.
    pub fn was_created(&self) -> bool {
        self.before.is_none() && self.after.is_some()
    }

    /// Whether this account no longer exists after execution.
    pub fn was_removed(&self) -> bool {
        self.before.is_some() && self.after.is_none()
    }
}

/// A stable execution error for assertions in program tests.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProgramError {
    /// An instruction argument was invalid.
    InvalidArgument,
    /// Instruction data could not be decoded or validated.
    InvalidInstructionData,
    /// Account data could not be decoded or validated.
    InvalidAccountData,
    /// An account did not provide enough data bytes.
    AccountDataTooSmall,
    /// An account did not have enough funds.
    InsufficientFunds,
    /// An instruction targeted the wrong program.
    IncorrectProgramId,
    /// A required signer was absent.
    MissingRequiredSignature,
    /// An initialization target was already initialized.
    AccountAlreadyInitialized,
    /// An account was expected to be initialized but was not.
    UninitializedAccount,
    /// A required transaction account was absent.
    MissingAccount,
    /// PDA seed derivation failed.
    InvalidSeeds,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// An account was below the rent-exempt minimum.
    AccountNotRentExempt,
    /// An account had an invalid owner.
    InvalidAccountOwner,
    /// The supplied authority was not authorized.
    IncorrectAuthority,
    /// A write targeted an immutable account.
    Immutable,
    /// Borsh serialization or deserialization failed.
    BorshIoError,
    /// Execution exhausted the transaction compute budget.
    ComputeBudgetExceeded,
    /// A program-defined error code.
    Custom(u32),
    /// A runtime error outside the stable, backend-neutral set above.
    Runtime(String),
}

impl From<InstructionError> for ProgramError {
    fn from(error: InstructionError) -> Self {
        // Collapse the runtime's instruction error into the stable, backend-
        // neutral set tests assert against; anything outside it keeps its debug
        // form under `Runtime`.
        #[allow(deprecated)]
        match error {
            InstructionError::InvalidArgument => Self::InvalidArgument,
            InstructionError::InvalidInstructionData => Self::InvalidInstructionData,
            InstructionError::InvalidAccountData => Self::InvalidAccountData,
            InstructionError::AccountDataTooSmall => Self::AccountDataTooSmall,
            InstructionError::InsufficientFunds => Self::InsufficientFunds,
            InstructionError::IncorrectProgramId => Self::IncorrectProgramId,
            InstructionError::MissingRequiredSignature => Self::MissingRequiredSignature,
            InstructionError::AccountAlreadyInitialized => Self::AccountAlreadyInitialized,
            InstructionError::UninitializedAccount => Self::UninitializedAccount,
            InstructionError::MissingAccount | InstructionError::NotEnoughAccountKeys => {
                Self::MissingAccount
            }
            InstructionError::InvalidSeeds => Self::InvalidSeeds,
            InstructionError::ArithmeticOverflow => Self::ArithmeticOverflow,
            InstructionError::AccountNotRentExempt => Self::AccountNotRentExempt,
            InstructionError::InvalidAccountOwner => Self::InvalidAccountOwner,
            InstructionError::IncorrectAuthority => Self::IncorrectAuthority,
            InstructionError::Immutable => Self::Immutable,
            InstructionError::BorshIoError => Self::BorshIoError,
            InstructionError::ComputationalBudgetExceeded => Self::ComputeBudgetExceeded,
            InstructionError::Custom(code) => Self::Custom(code),
            other => Self::Runtime(format!("{other:?}")),
        }
    }
}

impl From<TransactionError> for ProgramError {
    fn from(error: TransactionError) -> Self {
        // A program-level failure carries an `InstructionError`; every other
        // transaction failure (fee payer, account locks, rent, ...) is a runtime
        // error outside the stable set.
        match error {
            TransactionError::InstructionError(_, instruction_error) => instruction_error.into(),
            other => Self::Runtime(format!("{other:?}")),
        }
    }
}

impl fmt::Display for ProgramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument => formatter.write_str("invalid argument"),
            Self::InvalidInstructionData => formatter.write_str("invalid instruction data"),
            Self::InvalidAccountData => formatter.write_str("invalid account data"),
            Self::AccountDataTooSmall => formatter.write_str("account data too small"),
            Self::InsufficientFunds => formatter.write_str("insufficient funds"),
            Self::IncorrectProgramId => formatter.write_str("incorrect program id"),
            Self::MissingRequiredSignature => formatter.write_str("missing required signature"),
            Self::AccountAlreadyInitialized => formatter.write_str("account already initialized"),
            Self::UninitializedAccount => formatter.write_str("uninitialized account"),
            Self::MissingAccount => formatter.write_str("missing account"),
            Self::InvalidSeeds => formatter.write_str("invalid seeds"),
            Self::ArithmeticOverflow => formatter.write_str("arithmetic overflow"),
            Self::AccountNotRentExempt => formatter.write_str("account not rent-exempt"),
            Self::InvalidAccountOwner => formatter.write_str("invalid account owner"),
            Self::IncorrectAuthority => formatter.write_str("incorrect authority"),
            Self::Immutable => formatter.write_str("account is immutable"),
            Self::BorshIoError => formatter.write_str("borsh serialization error"),
            Self::ComputeBudgetExceeded => formatter.write_str("compute budget exceeded"),
            Self::Custom(code) => write!(formatter, "custom program error: {code} ({code:#x})"),
            Self::Runtime(message) => write!(formatter, "runtime error: {message}"),
        }
    }
}
