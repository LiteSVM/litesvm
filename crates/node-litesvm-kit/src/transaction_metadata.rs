use {
    crate::{
        account::Account,
        to_string_js,
        transaction_error::{convert_transaction_error, TransactionError},
    },
    litesvm::types::{
        FailedTransactionMetadata as FailedTransactionMetadataOriginal,
        SimulatedTransactionInfo as SimulatedTransactionInfoOriginal,
        TransactionMetadata as TransactionMetadataOriginal,
    },
    napi::bindgen_prelude::*,
    solana_account::Account as AccountOriginal,
    solana_message::{
        compiled_instruction::CompiledInstruction as CompiledInstructionOriginal,
        inner_instruction::InnerInstruction as InnerInstructionOriginal,
    },
    solana_transaction_context::TransactionReturnData as TransactionReturnDataOriginal,
};

#[derive(Debug, Clone)]
#[napi]
pub struct CompiledInstruction(CompiledInstructionOriginal);

#[napi]
impl CompiledInstruction {
    #[napi(constructor)]
    pub fn new(program_id_index: u8, accounts: Uint8Array, data: Uint8Array) -> Self {
        Self(CompiledInstructionOriginal {
            program_id_index,
            accounts: accounts.to_vec(),
            data: data.to_vec(),
        })
    }

    #[napi]
    pub fn program_id_index(&self) -> u8 {
        self.0.program_id_index
    }

    #[napi]
    pub fn accounts(&self) -> Uint8Array {
        Uint8Array::new(self.0.accounts.clone())
    }

    #[napi]
    pub fn data(&self) -> Uint8Array {
        Uint8Array::new(self.0.data.clone())
    }
}

to_string_js!(CompiledInstruction);

#[derive(Debug, Clone)]
#[napi]
pub struct InnerInstruction(InnerInstructionOriginal);

#[napi]
impl InnerInstruction {
    #[napi]
    pub fn instruction(&self) -> CompiledInstruction {
        CompiledInstruction(self.0.instruction.clone())
    }

    #[napi]
    pub fn stack_height(&self) -> u8 {
        self.0.stack_height
    }
}

to_string_js!(InnerInstruction);

#[derive(Debug, Clone)]
#[napi]
pub struct TransactionReturnData(TransactionReturnDataOriginal);

#[napi]
impl TransactionReturnData {
    #[napi]
    pub fn program_id(&self) -> Uint8Array {
        Uint8Array::with_data_copied(self.0.program_id)
    }

    #[napi]
    pub fn data(&self) -> Uint8Array {
        Uint8Array::new(self.0.data.clone())
    }
}

to_string_js!(TransactionReturnData);

#[derive(Debug, Clone)]
#[napi]
pub struct TransactionMetadata(pub(crate) TransactionMetadataOriginal);

#[napi]
impl TransactionMetadata {
    #[napi]
    pub fn signature(&self) -> Uint8Array {
        Uint8Array::with_data_copied(self.0.signature)
    }

    #[napi]
    pub fn logs(&self) -> Vec<String> {
        self.0.logs.clone()
    }

    #[napi]
    pub fn inner_instructions(&self) -> Vec<Vec<InnerInstruction>> {
        self.0
            .inner_instructions
            .clone()
            .into_iter()
            .map(|outer| outer.into_iter().map(InnerInstruction).collect())
            .collect()
    }

    #[napi]
    pub fn compute_units_consumed(&self) -> u64 {
        self.0.compute_units_consumed
    }

    #[napi]
    pub fn return_data(&self) -> TransactionReturnData {
        TransactionReturnData(self.0.return_data.clone())
    }

    #[napi]
    pub fn pretty_logs(&self) -> String {
        self.0.pretty_logs()
    }
}

to_string_js!(TransactionMetadata);

#[derive(Debug, Clone)]
#[napi]
pub struct FailedTransactionMetadata(pub(crate) FailedTransactionMetadataOriginal);

#[napi]
impl FailedTransactionMetadata {
    #[napi(
        ts_return_type = "TransactionErrorFieldless | TransactionErrorInstructionError | TransactionErrorDuplicateInstruction | TransactionErrorInsufficientFundsForRent | TransactionErrorProgramExecutionTemporarilyRestricted"
    )]
    pub fn err(&self) -> TransactionError {
        convert_transaction_error(self.0.err.clone())
    }

    #[napi]
    pub fn meta(&self) -> TransactionMetadata {
        TransactionMetadata(self.0.meta.clone())
    }
}

to_string_js!(FailedTransactionMetadata);

#[napi]
pub struct AddressAndAccount {
    pub address: Uint8Array,
    account: Account,
}

#[napi]
impl AddressAndAccount {
    #[napi]
    pub fn account(&self) -> Account {
        self.account.clone()
    }
}

#[derive(Debug, Clone)]
#[napi]
pub struct SimulatedTransactionInfo(pub(crate) SimulatedTransactionInfoOriginal);

#[napi]
impl SimulatedTransactionInfo {
    #[napi]
    pub fn meta(&self) -> TransactionMetadata {
        TransactionMetadata(self.0.meta.clone())
    }

    #[napi]
    pub fn post_accounts(&self) -> Vec<AddressAndAccount> {
        self.0
            .post_accounts
            .clone()
            .into_iter()
            .map(|x| AddressAndAccount {
                address: Uint8Array::with_data_copied(x.0),
                account: Account(AccountOriginal::from(x.1)),
            })
            .collect()
    }
}

to_string_js!(SimulatedTransactionInfo);
