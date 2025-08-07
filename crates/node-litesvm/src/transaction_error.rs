use {
    crate::to_string_js,
    core::fmt,
    napi::bindgen_prelude::{Either3, Either5},
    solana_instruction::error::InstructionError as InstructionErrorOriginal,
    solana_transaction_error::TransactionError as TransactionErrorOriginal,
};

#[derive(Clone, Debug)]
#[napi]
pub struct InstructionErrorCustom {
    pub code: u32,
}

to_string_js!(InstructionErrorCustom);

#[derive(Clone, Debug)]
#[napi]
pub struct InstructionErrorBorshIO {
    pub msg: String,
}

to_string_js!(InstructionErrorBorshIO);

#[derive(Debug, Clone)]
#[napi]
pub enum InstructionErrorFieldless {
    GenericError,
    InvalidArgument,
    InvalidInstructionData,
    InvalidAccountData,
    AccountDataTooSmall,
    InsufficientFunds,
    IncorrectProgramId,
    MissingRequiredSignature,
    AccountAlreadyInitialized,
    UninitializedAccount,
    UnbalancedInstruction,
    ModifiedProgramId,
    ExternalAccountLamportSpend,
    ExternalAccountDataModified,
    ReadonlyLamportChange,
    ReadonlyDataModified,
    DuplicateAccountIndex,
    ExecutableModified,
    RentEpochModified,
    NotEnoughAccountKeys,
    AccountDataSizeChanged,
    AccountNotExecutable,
    AccountBorrowFailed,
    AccountBorrowOutstanding,
    DuplicateAccountOutOfSync,
    InvalidError,
    ExecutableDataModified,
    ExecutableLamportChange,
    ExecutableAccountNotRentExempt,
    UnsupportedProgramId,
    CallDepth,
    MissingAccount,
    ReentrancyNotAllowed,
    MaxSeedLengthExceeded,
    InvalidSeeds,
    InvalidRealloc,
    ComputationalBudgetExceeded,
    PrivilegeEscalation,
    ProgramEnvironmentSetupFailure,
    ProgramFailedToComplete,
    ProgramFailedToCompile,
    Immutable,
    IncorrectAuthority,
    AccountNotRentExempt,
    InvalidAccountOwner,
    ArithmeticOverflow,
    UnsupportedSysvar,
    IllegalOwner,
    MaxAccountsDataAllocationsExceeded,
    MaxAccountsExceeded,
    MaxInstructionTraceLengthExceeded,
    BuiltinProgramsMustConsumeComputeUnits,
}

to_string_js!(InstructionErrorFieldless);

pub type InstructionError =
    Either3<InstructionErrorFieldless, InstructionErrorCustom, InstructionErrorBorshIO>;

// fn debug_instruction_error(err: InstructionError, f: &mut fmt::Formatter) -> fmt::Result {
//     match err {
//         InstructionError::A(fieldless) => write!(f, "{fieldless:?}"),
//         InstructionError::B(custom) => write!(f, "{custom:?}"),
//         InstructionError::C(borsh_err) => write!(f, "{borsh_err:?}"),
//     }
// }

fn convert_instruction_error(e: InstructionErrorOriginal) -> InstructionError {
    match e {
        InstructionErrorOriginal::Custom(code) => {
            InstructionError::B(InstructionErrorCustom { code })
        }
        InstructionErrorOriginal::BorshIoError(msg) => {
            InstructionError::C(InstructionErrorBorshIO { msg })
        }
        InstructionErrorOriginal::GenericError => {
            InstructionError::A(InstructionErrorFieldless::GenericError)
        }
        InstructionErrorOriginal::InvalidArgument => {
            InstructionError::A(InstructionErrorFieldless::InvalidArgument)
        }
        InstructionErrorOriginal::InvalidInstructionData => {
            InstructionError::A(InstructionErrorFieldless::InvalidInstructionData)
        }
        InstructionErrorOriginal::InvalidAccountData => {
            InstructionError::A(InstructionErrorFieldless::InvalidAccountData)
        }
        InstructionErrorOriginal::AccountDataTooSmall => {
            InstructionError::A(InstructionErrorFieldless::AccountDataTooSmall)
        }
        InstructionErrorOriginal::InsufficientFunds => {
            InstructionError::A(InstructionErrorFieldless::InsufficientFunds)
        }
        InstructionErrorOriginal::IncorrectProgramId => {
            InstructionError::A(InstructionErrorFieldless::IncorrectProgramId)
        }
        InstructionErrorOriginal::MissingRequiredSignature => {
            InstructionError::A(InstructionErrorFieldless::MissingRequiredSignature)
        }
        InstructionErrorOriginal::AccountAlreadyInitialized => {
            InstructionError::A(InstructionErrorFieldless::AccountAlreadyInitialized)
        }
        InstructionErrorOriginal::UninitializedAccount => {
            InstructionError::A(InstructionErrorFieldless::UninitializedAccount)
        }
        InstructionErrorOriginal::UnbalancedInstruction => {
            InstructionError::A(InstructionErrorFieldless::UnbalancedInstruction)
        }
        InstructionErrorOriginal::ModifiedProgramId => {
            InstructionError::A(InstructionErrorFieldless::ModifiedProgramId)
        }
        InstructionErrorOriginal::ExternalAccountLamportSpend => {
            InstructionError::A(InstructionErrorFieldless::ExternalAccountLamportSpend)
        }
        InstructionErrorOriginal::ExternalAccountDataModified => {
            InstructionError::A(InstructionErrorFieldless::ExternalAccountDataModified)
        }
        InstructionErrorOriginal::ReadonlyLamportChange => {
            InstructionError::A(InstructionErrorFieldless::ReadonlyLamportChange)
        }
        InstructionErrorOriginal::ReadonlyDataModified => {
            InstructionError::A(InstructionErrorFieldless::ReadonlyDataModified)
        }
        InstructionErrorOriginal::DuplicateAccountIndex => {
            InstructionError::A(InstructionErrorFieldless::DuplicateAccountIndex)
        }
        InstructionErrorOriginal::ExecutableModified => {
            InstructionError::A(InstructionErrorFieldless::ExecutableModified)
        }
        InstructionErrorOriginal::RentEpochModified => {
            InstructionError::A(InstructionErrorFieldless::RentEpochModified)
        }
        InstructionErrorOriginal::NotEnoughAccountKeys => {
            InstructionError::A(InstructionErrorFieldless::NotEnoughAccountKeys)
        }
        InstructionErrorOriginal::AccountDataSizeChanged => {
            InstructionError::A(InstructionErrorFieldless::AccountDataSizeChanged)
        }
        InstructionErrorOriginal::AccountNotExecutable => {
            InstructionError::A(InstructionErrorFieldless::AccountNotExecutable)
        }
        InstructionErrorOriginal::AccountBorrowFailed => {
            InstructionError::A(InstructionErrorFieldless::AccountBorrowFailed)
        }
        InstructionErrorOriginal::AccountBorrowOutstanding => {
            InstructionError::A(InstructionErrorFieldless::AccountBorrowOutstanding)
        }
        InstructionErrorOriginal::DuplicateAccountOutOfSync => {
            InstructionError::A(InstructionErrorFieldless::DuplicateAccountOutOfSync)
        }
        InstructionErrorOriginal::InvalidError => {
            InstructionError::A(InstructionErrorFieldless::InvalidError)
        }
        InstructionErrorOriginal::ExecutableDataModified => {
            InstructionError::A(InstructionErrorFieldless::ExecutableDataModified)
        }
        InstructionErrorOriginal::ExecutableLamportChange => {
            InstructionError::A(InstructionErrorFieldless::ExecutableLamportChange)
        }
        InstructionErrorOriginal::ExecutableAccountNotRentExempt => {
            InstructionError::A(InstructionErrorFieldless::ExecutableAccountNotRentExempt)
        }
        InstructionErrorOriginal::UnsupportedProgramId => {
            InstructionError::A(InstructionErrorFieldless::UnsupportedProgramId)
        }
        InstructionErrorOriginal::CallDepth => {
            InstructionError::A(InstructionErrorFieldless::CallDepth)
        }
        InstructionErrorOriginal::MissingAccount => {
            InstructionError::A(InstructionErrorFieldless::MissingAccount)
        }
        InstructionErrorOriginal::ReentrancyNotAllowed => {
            InstructionError::A(InstructionErrorFieldless::ReentrancyNotAllowed)
        }
        InstructionErrorOriginal::MaxSeedLengthExceeded => {
            InstructionError::A(InstructionErrorFieldless::MaxSeedLengthExceeded)
        }
        InstructionErrorOriginal::InvalidSeeds => {
            InstructionError::A(InstructionErrorFieldless::InvalidSeeds)
        }
        InstructionErrorOriginal::InvalidRealloc => {
            InstructionError::A(InstructionErrorFieldless::InvalidRealloc)
        }
        InstructionErrorOriginal::ComputationalBudgetExceeded => {
            InstructionError::A(InstructionErrorFieldless::ComputationalBudgetExceeded)
        }
        InstructionErrorOriginal::PrivilegeEscalation => {
            InstructionError::A(InstructionErrorFieldless::PrivilegeEscalation)
        }
        InstructionErrorOriginal::ProgramEnvironmentSetupFailure => {
            InstructionError::A(InstructionErrorFieldless::ProgramEnvironmentSetupFailure)
        }
        InstructionErrorOriginal::ProgramFailedToComplete => {
            InstructionError::A(InstructionErrorFieldless::ProgramFailedToComplete)
        }
        InstructionErrorOriginal::ProgramFailedToCompile => {
            InstructionError::A(InstructionErrorFieldless::ProgramFailedToCompile)
        }
        InstructionErrorOriginal::Immutable => {
            InstructionError::A(InstructionErrorFieldless::Immutable)
        }
        InstructionErrorOriginal::IncorrectAuthority => {
            InstructionError::A(InstructionErrorFieldless::IncorrectAuthority)
        }
        InstructionErrorOriginal::AccountNotRentExempt => {
            InstructionError::A(InstructionErrorFieldless::AccountNotRentExempt)
        }
        InstructionErrorOriginal::InvalidAccountOwner => {
            InstructionError::A(InstructionErrorFieldless::InvalidAccountOwner)
        }
        InstructionErrorOriginal::ArithmeticOverflow => {
            InstructionError::A(InstructionErrorFieldless::ArithmeticOverflow)
        }
        InstructionErrorOriginal::UnsupportedSysvar => {
            InstructionError::A(InstructionErrorFieldless::UnsupportedSysvar)
        }
        InstructionErrorOriginal::IllegalOwner => {
            InstructionError::A(InstructionErrorFieldless::IllegalOwner)
        }
        InstructionErrorOriginal::MaxAccountsDataAllocationsExceeded => {
            InstructionError::A(InstructionErrorFieldless::MaxAccountsDataAllocationsExceeded)
        }
        InstructionErrorOriginal::MaxAccountsExceeded => {
            InstructionError::A(InstructionErrorFieldless::MaxAccountsExceeded)
        }
        InstructionErrorOriginal::MaxInstructionTraceLengthExceeded => {
            InstructionError::A(InstructionErrorFieldless::MaxInstructionTraceLengthExceeded)
        }
        InstructionErrorOriginal::BuiltinProgramsMustConsumeComputeUnits => {
            InstructionError::A(InstructionErrorFieldless::BuiltinProgramsMustConsumeComputeUnits)
        }
    }
}

#[derive(Debug)]
#[napi]
pub enum TransactionErrorFieldless {
    AccountInUse,
    AccountLoadedTwice,
    AccountNotFound,
    ProgramAccountNotFound,
    InsufficientFundsForFee,
    InvalidAccountForFee,
    AlreadyProcessed,
    BlockhashNotFound,
    CallChainTooDeep,
    MissingSignatureForFee,
    InvalidAccountIndex,
    SignatureFailure,
    InvalidProgramForExecution,
    SanitizeFailure,
    ClusterMaintenance,
    AccountBorrowOutstanding,
    WouldExceedMaxBlockCostLimit,
    UnsupportedVersion,
    InvalidWritableAccount,
    WouldExceedMaxAccountCostLimit,
    WouldExceedAccountDataBlockLimit,
    TooManyAccountLocks,
    AddressLookupTableNotFound,
    InvalidAddressLookupTableOwner,
    InvalidAddressLookupTableData,
    InvalidAddressLookupTableIndex,
    InvalidRentPayingAccount,
    WouldExceedMaxVoteCostLimit,
    WouldExceedAccountDataTotalLimit,
    MaxLoadedAccountsDataSizeExceeded,
    ResanitizationNeeded,
    InvalidLoadedAccountsDataSizeLimit,
    UnbalancedTransaction,
    ProgramCacheHitMaxLimit,
    CommitCancelled,
}

to_string_js!(TransactionErrorFieldless);

#[derive(Clone)]
#[napi]
pub struct TransactionErrorInstructionError {
    pub index: u8,
    error: InstructionError,
}

impl fmt::Debug for TransactionErrorInstructionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TransactionErrorInstructionError")
            .field("index", &self.index)
            .field(
                "error",
                match &self.error {
                    InstructionError::A(a) => a,
                    InstructionError::B(b) => b,
                    InstructionError::C(c) => c,
                },
            )
            .finish()
    }
}

#[napi]
impl TransactionErrorInstructionError {
    #[napi(
        ts_return_type = "InstructionErrorFieldless | InstructionErrorCustom | InstructionErrorBorshIO"
    )]
    pub fn err(&self) -> InstructionError {
        self.error.clone()
    }
}

to_string_js!(TransactionErrorInstructionError);

#[derive(Debug)]
#[napi]
pub struct TransactionErrorDuplicateInstruction {
    pub index: u8,
}

to_string_js!(TransactionErrorDuplicateInstruction);

#[derive(Debug)]
#[napi]
pub struct TransactionErrorInsufficientFundsForRent {
    pub account_index: u8,
}

to_string_js!(TransactionErrorInsufficientFundsForRent);

#[derive(Debug)]
#[napi]
pub struct TransactionErrorProgramExecutionTemporarilyRestricted {
    pub account_index: u8,
}

to_string_js!(TransactionErrorProgramExecutionTemporarilyRestricted);

pub type TransactionError = Either5<
    TransactionErrorFieldless,
    TransactionErrorInstructionError,
    TransactionErrorDuplicateInstruction,
    TransactionErrorInsufficientFundsForRent,
    TransactionErrorProgramExecutionTemporarilyRestricted,
>;

pub(crate) fn convert_transaction_error(w: TransactionErrorOriginal) -> TransactionError {
    match w {
        TransactionErrorOriginal::InstructionError(index, err) => {
            TransactionError::B(TransactionErrorInstructionError {
                index,
                error: convert_instruction_error(err),
            })
        }
        TransactionErrorOriginal::DuplicateInstruction(index) => {
            TransactionError::C(TransactionErrorDuplicateInstruction { index })
        }
        TransactionErrorOriginal::InsufficientFundsForRent { account_index } => {
            TransactionError::D(TransactionErrorInsufficientFundsForRent { account_index })
        }
        TransactionErrorOriginal::ProgramExecutionTemporarilyRestricted { account_index } => {
            TransactionError::E(TransactionErrorProgramExecutionTemporarilyRestricted {
                account_index,
            })
        }
        TransactionErrorOriginal::AccountInUse => {
            TransactionError::A(TransactionErrorFieldless::AccountInUse)
        }
        TransactionErrorOriginal::AccountLoadedTwice => {
            TransactionError::A(TransactionErrorFieldless::AccountLoadedTwice)
        }
        TransactionErrorOriginal::AccountNotFound => {
            TransactionError::A(TransactionErrorFieldless::AccountNotFound)
        }
        TransactionErrorOriginal::ProgramAccountNotFound => {
            TransactionError::A(TransactionErrorFieldless::ProgramAccountNotFound)
        }
        TransactionErrorOriginal::InsufficientFundsForFee => {
            TransactionError::A(TransactionErrorFieldless::InsufficientFundsForFee)
        }
        TransactionErrorOriginal::InvalidAccountForFee => {
            TransactionError::A(TransactionErrorFieldless::InvalidAccountForFee)
        }
        TransactionErrorOriginal::AlreadyProcessed => {
            TransactionError::A(TransactionErrorFieldless::AlreadyProcessed)
        }
        TransactionErrorOriginal::BlockhashNotFound => {
            TransactionError::A(TransactionErrorFieldless::BlockhashNotFound)
        }
        TransactionErrorOriginal::CallChainTooDeep => {
            TransactionError::A(TransactionErrorFieldless::CallChainTooDeep)
        }
        TransactionErrorOriginal::MissingSignatureForFee => {
            TransactionError::A(TransactionErrorFieldless::MissingSignatureForFee)
        }
        TransactionErrorOriginal::InvalidAccountIndex => {
            TransactionError::A(TransactionErrorFieldless::InvalidAccountIndex)
        }
        TransactionErrorOriginal::SignatureFailure => {
            TransactionError::A(TransactionErrorFieldless::SignatureFailure)
        }
        TransactionErrorOriginal::InvalidProgramForExecution => {
            TransactionError::A(TransactionErrorFieldless::InvalidProgramForExecution)
        }
        TransactionErrorOriginal::SanitizeFailure => {
            TransactionError::A(TransactionErrorFieldless::SanitizeFailure)
        }
        TransactionErrorOriginal::ClusterMaintenance => {
            TransactionError::A(TransactionErrorFieldless::ClusterMaintenance)
        }
        TransactionErrorOriginal::AccountBorrowOutstanding => {
            TransactionError::A(TransactionErrorFieldless::AccountBorrowOutstanding)
        }
        TransactionErrorOriginal::WouldExceedMaxBlockCostLimit => {
            TransactionError::A(TransactionErrorFieldless::WouldExceedMaxBlockCostLimit)
        }
        TransactionErrorOriginal::UnsupportedVersion => {
            TransactionError::A(TransactionErrorFieldless::UnsupportedVersion)
        }
        TransactionErrorOriginal::InvalidWritableAccount => {
            TransactionError::A(TransactionErrorFieldless::InvalidWritableAccount)
        }
        TransactionErrorOriginal::WouldExceedMaxAccountCostLimit => {
            TransactionError::A(TransactionErrorFieldless::WouldExceedMaxAccountCostLimit)
        }
        TransactionErrorOriginal::WouldExceedAccountDataBlockLimit => {
            TransactionError::A(TransactionErrorFieldless::WouldExceedAccountDataBlockLimit)
        }
        TransactionErrorOriginal::TooManyAccountLocks => {
            TransactionError::A(TransactionErrorFieldless::TooManyAccountLocks)
        }
        TransactionErrorOriginal::AddressLookupTableNotFound => {
            TransactionError::A(TransactionErrorFieldless::AddressLookupTableNotFound)
        }
        TransactionErrorOriginal::InvalidAddressLookupTableOwner => {
            TransactionError::A(TransactionErrorFieldless::InvalidAddressLookupTableOwner)
        }
        TransactionErrorOriginal::InvalidAddressLookupTableData => {
            TransactionError::A(TransactionErrorFieldless::InvalidAddressLookupTableData)
        }
        TransactionErrorOriginal::InvalidAddressLookupTableIndex => {
            TransactionError::A(TransactionErrorFieldless::InvalidAddressLookupTableIndex)
        }
        TransactionErrorOriginal::InvalidRentPayingAccount => {
            TransactionError::A(TransactionErrorFieldless::InvalidRentPayingAccount)
        }
        TransactionErrorOriginal::WouldExceedMaxVoteCostLimit => {
            TransactionError::A(TransactionErrorFieldless::WouldExceedMaxVoteCostLimit)
        }
        TransactionErrorOriginal::WouldExceedAccountDataTotalLimit => {
            TransactionError::A(TransactionErrorFieldless::WouldExceedAccountDataTotalLimit)
        }
        TransactionErrorOriginal::MaxLoadedAccountsDataSizeExceeded => {
            TransactionError::A(TransactionErrorFieldless::MaxLoadedAccountsDataSizeExceeded)
        }
        TransactionErrorOriginal::ResanitizationNeeded => {
            TransactionError::A(TransactionErrorFieldless::ResanitizationNeeded)
        }
        TransactionErrorOriginal::InvalidLoadedAccountsDataSizeLimit => {
            TransactionError::A(TransactionErrorFieldless::InvalidLoadedAccountsDataSizeLimit)
        }
        TransactionErrorOriginal::UnbalancedTransaction => {
            TransactionError::A(TransactionErrorFieldless::UnbalancedTransaction)
        }
        TransactionErrorOriginal::ProgramCacheHitMaxLimit => {
            TransactionError::A(TransactionErrorFieldless::ProgramCacheHitMaxLimit)
        }
        TransactionErrorOriginal::CommitCancelled => {
            TransactionError::A(TransactionErrorFieldless::CommitCancelled)
        }
    }
}
