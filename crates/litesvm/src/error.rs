use {solana_instruction::error::InstructionError, thiserror::Error};

#[derive(Error, Debug)]
pub enum InvalidSysvarDataError {
    #[error("Invalid Clock sysvar data.")]
    Clock,
    #[error("Invalid EpochRewards sysvar data.")]
    EpochRewards,
    #[error("Invalid EpochSchedule sysvar data.")]
    EpochSchedule,
    #[error("Invalid Fees sysvar data.")]
    Fees,
    #[error("Invalid LastRestartSlot sysvar data.")]
    LastRestartSlot,
    #[error("Invalid RecentBlockhashes sysvar data.")]
    RecentBlockhashes,
    #[error("Invalid Rent sysvar data.")]
    Rent,
    #[error("Invalid SlotHashes sysvar data.")]
    SlotHashes,
    #[error("Invalid StakeHistory sysvar data.")]
    StakeHistory,
}

#[derive(Error, Debug)]
pub enum LiteSVMError {
    #[error("{0}")]
    InvalidSysvarData(#[from] InvalidSysvarDataError),
    #[error("{0}")]
    Instruction(#[from] InstructionError),
    #[error("{0}")]
    InvalidPath(#[from] std::io::Error),
}
