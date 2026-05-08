use {
    crate::{
        error::LiteSVMError,
        types::{FailedTransactionMetadata, TransactionMetadata, TransactionResult},
        LiteSVM,
    },
    dlp_api::discriminator::DlpDiscriminator,
    magicblock_magic_program_api::{
        args::{
            CommitAndUndelegateArgs, CommitTypeArgs, MagicBaseIntentArgs, MagicIntentBundleArgs,
        },
        instruction::MagicBlockInstruction,
    },
    solana_account::{Account, AccountSharedData, ReadableAccount, WritableAccount},
    solana_address::{address, Address},
    solana_hash::Hash,
    solana_instruction::error::InstructionError,
    solana_message::VersionedMessage,
    solana_program_runtime::{declare_process_instruction, invoke_context::InvokeContext},
    solana_signature::Signature,
    solana_transaction::versioned::VersionedTransaction,
    solana_transaction_context::{IndexOfAccount, InstructionContext},
    solana_transaction_error::TransactionError,
    std::{
        collections::HashSet,
        ops::{Deref, DerefMut},
        path::Path,
    },
};

pub const DELEGATION_PROGRAM_ID: Address = Address::new_from_array(dlp_api::ID.to_bytes());
pub const MAGIC_PROGRAM_ID: Address =
    Address::new_from_array(magicblock_magic_program_api::ID.to_bytes());
pub const MAGIC_CONTEXT_ID: Address =
    Address::new_from_array(magicblock_magic_program_api::MAGIC_CONTEXT_PUBKEY.to_bytes());
pub const DEFAULT_VALIDATOR_IDENTITY: Address =
    address!("MAS1Dt9qreoRMQ14YQuhg8UTZMMzDdKhmkZMECCzk57");

const DEFAULT_MAGIC_PROGRAM_COMPUTE_UNITS: u64 = 150;
const MAGIC_PAYER_IDX: IndexOfAccount = 0;
const MAGIC_CONTEXT_IDX: IndexOfAccount = 1;
const MAGIC_COMMITTEES_START_IDX: IndexOfAccount = 2;

declare_process_instruction!(
    MagicProgramEntrypoint,
    DEFAULT_MAGIC_PROGRAM_COMPUTE_UNITS,
    |invoke_context| { process_magic_program_instruction(invoke_context) }
);

fn process_magic_program_instruction(
    invoke_context: &mut InvokeContext,
) -> Result<(), InstructionError> {
    let instruction_context = invoke_context
        .transaction_context
        .get_current_instruction_context()?;
    match magic_instruction(instruction_context.get_instruction_data())? {
        MagicInstruction::ScheduleCommit => {
            process_direct_schedule_commit(&instruction_context, false)
        }
        MagicInstruction::ScheduleCommitAndUndelegate => {
            process_direct_schedule_commit(&instruction_context, true)
        }
        MagicInstruction::ScheduleCommitFinalize {
            request_undelegation,
        } => process_direct_schedule_commit(&instruction_context, request_undelegation),
        MagicInstruction::ScheduleBaseIntent {
            committed_accounts,
            undelegated_accounts,
        }
        | MagicInstruction::ScheduleIntentBundle {
            committed_accounts,
            undelegated_accounts,
        } => process_indexed_schedule_commit(
            &instruction_context,
            &committed_accounts,
            &undelegated_accounts,
        ),
        MagicInstruction::Noop => Ok(()),
    }
}

fn process_direct_schedule_commit(
    instruction_context: &InstructionContext<'_, '_>,
    request_undelegation: bool,
) -> Result<(), InstructionError> {
    validate_magic_schedule_header(instruction_context)?;
    let account_count = instruction_context.get_number_of_instruction_accounts();
    if account_count <= MAGIC_COMMITTEES_START_IDX {
        return Err(InstructionError::MissingAccount);
    }

    let account_indices: Vec<_> = (MAGIC_COMMITTEES_START_IDX..account_count).collect();
    validate_commit_accounts(instruction_context, &account_indices, request_undelegation)
}

fn process_indexed_schedule_commit(
    instruction_context: &InstructionContext<'_, '_>,
    committed_accounts: &[u8],
    undelegated_accounts: &[u8],
) -> Result<(), InstructionError> {
    validate_magic_schedule_header(instruction_context)?;
    let committed_accounts: Vec<_> = committed_accounts
        .iter()
        .map(|index| u16::from(*index))
        .collect();
    let undelegated_accounts: Vec<_> = undelegated_accounts
        .iter()
        .map(|index| u16::from(*index))
        .collect();

    validate_commit_accounts(instruction_context, &committed_accounts, false)?;
    validate_commit_accounts(instruction_context, &undelegated_accounts, true)
}

fn validate_magic_schedule_header(
    instruction_context: &InstructionContext<'_, '_>,
) -> Result<(), InstructionError> {
    if instruction_context.get_program_key()? != &MAGIC_PROGRAM_ID {
        return Err(InstructionError::UnsupportedProgramId);
    }
    instruction_context.check_number_of_instruction_accounts(MAGIC_CONTEXT_IDX + 1)?;
    if instruction_context.get_key_of_instruction_account(MAGIC_CONTEXT_IDX)? != &MAGIC_CONTEXT_ID {
        return Err(InstructionError::MissingAccount);
    }
    if !instruction_context.is_instruction_account_signer(MAGIC_PAYER_IDX)? {
        return Err(InstructionError::MissingRequiredSignature);
    }
    if !instruction_context.is_instruction_account_writable(MAGIC_CONTEXT_IDX)? {
        return Err(InstructionError::ReadonlyDataModified);
    }
    Ok(())
}

fn validate_commit_accounts(
    instruction_context: &InstructionContext<'_, '_>,
    account_indices: &[IndexOfAccount],
    request_undelegation: bool,
) -> Result<(), InstructionError> {
    if account_indices.is_empty() {
        return Ok(());
    }

    for account_index in account_indices {
        let account = instruction_context.try_borrow_instruction_account(*account_index)?;
        if account.get_key() == &MAGIC_CONTEXT_ID || account.get_key() == &MAGIC_PROGRAM_ID {
            return Err(InstructionError::MissingAccount);
        }
        if request_undelegation
            && !instruction_context.is_instruction_account_writable(*account_index)?
        {
            return Err(InstructionError::ReadonlyDataModified);
        }
    }
    Ok(())
}

enum MagicInstruction {
    ScheduleCommit,
    ScheduleCommitAndUndelegate,
    ScheduleCommitFinalize {
        request_undelegation: bool,
    },
    ScheduleBaseIntent {
        committed_accounts: Vec<u8>,
        undelegated_accounts: Vec<u8>,
    },
    ScheduleIntentBundle {
        committed_accounts: Vec<u8>,
        undelegated_accounts: Vec<u8>,
    },
    Noop,
}

fn magic_instruction(data: &[u8]) -> Result<MagicInstruction, InstructionError> {
    let instruction: MagicBlockInstruction =
        bincode::deserialize(data).map_err(|_| InstructionError::InvalidInstructionData)?;
    match instruction {
        MagicBlockInstruction::ScheduleCommit => Ok(MagicInstruction::ScheduleCommit),
        MagicBlockInstruction::ScheduleCommitAndUndelegate => {
            Ok(MagicInstruction::ScheduleCommitAndUndelegate)
        }
        MagicBlockInstruction::ScheduleCommitFinalize {
            request_undelegation,
        } => Ok(MagicInstruction::ScheduleCommitFinalize {
            request_undelegation,
        }),
        MagicBlockInstruction::ScheduleBaseIntent(args) => schedule_base_intent(args),
        MagicBlockInstruction::ScheduleIntentBundle(args) => schedule_intent_bundle(args),
        MagicBlockInstruction::Noop(_) => Ok(MagicInstruction::Noop),
        _ => Err(InstructionError::InvalidInstructionData),
    }
}

fn schedule_base_intent(args: MagicBaseIntentArgs) -> Result<MagicInstruction, InstructionError> {
    match args {
        MagicBaseIntentArgs::Commit(commit_type)
        | MagicBaseIntentArgs::CommitFinalize(commit_type) => {
            Ok(MagicInstruction::ScheduleBaseIntent {
                committed_accounts: commit_type_indices(commit_type),
                undelegated_accounts: Vec::new(),
            })
        }
        MagicBaseIntentArgs::CommitAndUndelegate(args)
        | MagicBaseIntentArgs::CommitFinalizeAndUndelegate(args) => {
            Ok(MagicInstruction::ScheduleBaseIntent {
                committed_accounts: Vec::new(),
                undelegated_accounts: commit_and_undelegate_indices(args),
            })
        }
        MagicBaseIntentArgs::BaseActions(_) => Err(InstructionError::InvalidInstructionData),
    }
}

fn schedule_intent_bundle(
    args: MagicIntentBundleArgs,
) -> Result<MagicInstruction, InstructionError> {
    let mut committed_accounts = Vec::new();
    let mut undelegated_accounts = Vec::new();

    if let Some(commit_type) = args.commit {
        committed_accounts.extend(commit_type_indices(commit_type));
    }
    if let Some(args) = args.commit_and_undelegate {
        undelegated_accounts.extend(commit_and_undelegate_indices(args));
    }
    if let Some(commit_type) = args.commit_finalize {
        committed_accounts.extend(commit_type_indices(commit_type));
    }
    if let Some(args) = args.commit_finalize_and_undelegate {
        undelegated_accounts.extend(commit_and_undelegate_indices(args));
    }

    Ok(MagicInstruction::ScheduleIntentBundle {
        committed_accounts,
        undelegated_accounts,
    })
}

fn commit_type_indices(commit_type: CommitTypeArgs) -> Vec<u8> {
    match commit_type {
        CommitTypeArgs::Standalone(indices) => indices,
        CommitTypeArgs::WithBaseActions {
            committed_accounts, ..
        } => committed_accounts,
    }
}

fn commit_and_undelegate_indices(args: CommitAndUndelegateArgs) -> Vec<u8> {
    commit_type_indices(args.commit_type)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionTarget {
    Base,
    Ephemeral,
}

pub struct MagicSVM {
    base: LiteSVM,
    ephemeral: LiteSVM,
    validator_identity: Address,
    delegated_accounts: HashSet<Address>,
}

impl Default for MagicSVM {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for MagicSVM {
    type Target = LiteSVM;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for MagicSVM {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl MagicSVM {
    pub fn new() -> Self {
        Self::new_with_validator_identity(DEFAULT_VALIDATOR_IDENTITY)
    }

    pub fn new_with_validator_identity(validator_identity: Address) -> Self {
        let base = LiteSVM::new();
        let mut ephemeral = LiteSVM::new();
        ephemeral.add_builtin(MAGIC_PROGRAM_ID, MagicProgramEntrypoint::vm);
        ephemeral
            .set_account(
                MAGIC_CONTEXT_ID,
                Account {
                    lamports: 1,
                    owner: MAGIC_PROGRAM_ID,
                    ..Default::default()
                },
            )
            .unwrap();

        Self {
            base,
            ephemeral,
            validator_identity,
            delegated_accounts: HashSet::new(),
        }
    }

    pub fn validator_identity(&self) -> Address {
        self.validator_identity
    }

    pub fn base(&self) -> &LiteSVM {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut LiteSVM {
        &mut self.base
    }

    pub fn ephemeral(&self) -> &LiteSVM {
        &self.ephemeral
    }

    pub fn ephemeral_mut(&mut self) -> &mut LiteSVM {
        &mut self.ephemeral
    }

    pub fn get_account(&self, pubkey: &Address) -> Option<Account> {
        self.get_account_for(TransactionTarget::Base, pubkey)
    }

    pub fn get_account_for(&self, target: TransactionTarget, pubkey: &Address) -> Option<Account> {
        match target {
            TransactionTarget::Base => self.base.get_account(pubkey),
            TransactionTarget::Ephemeral => self.ephemeral.get_account(pubkey),
        }
    }

    pub fn ephemeral_account(&self, pubkey: &Address) -> Option<AccountSharedData> {
        self.ephemeral.accounts.get_account(pubkey)
    }

    pub fn set_account(&mut self, pubkey: Address, account: Account) -> Result<(), LiteSVMError> {
        self.base.set_account(pubkey, account)
    }

    pub fn get_balance(&self, pubkey: &Address) -> Option<u64> {
        self.base.get_balance(pubkey)
    }

    pub fn latest_blockhash(&self) -> Hash {
        self.base.latest_blockhash()
    }

    pub fn latest_blockhash_for(&self, target: TransactionTarget) -> Hash {
        match target {
            TransactionTarget::Base => self.base.latest_blockhash(),
            TransactionTarget::Ephemeral => self.ephemeral.latest_blockhash(),
        }
    }

    pub fn get_transaction_for(
        &self,
        target: TransactionTarget,
        signature: &Signature,
    ) -> Option<&TransactionResult> {
        match target {
            TransactionTarget::Base => self.base.get_transaction(signature),
            TransactionTarget::Ephemeral => self.ephemeral.get_transaction(signature),
        }
    }

    pub fn expire_blockhash_for(&mut self, target: TransactionTarget) {
        match target {
            TransactionTarget::Base => self.base.expire_blockhash(),
            TransactionTarget::Ephemeral => self.ephemeral.expire_blockhash(),
        }
    }

    pub fn airdrop(&mut self, pubkey: &Address, lamports: u64) -> TransactionResult {
        self.base.airdrop(pubkey, lamports)
    }

    pub fn add_program(
        &mut self,
        program_id: Address,
        program_bytes: &[u8],
    ) -> Result<(), LiteSVMError> {
        self.base.add_program(program_id, program_bytes)?;
        self.ephemeral.add_program(program_id, program_bytes)
    }

    pub fn add_program_from_file(
        &mut self,
        program_id: Address,
        path: impl AsRef<Path>,
    ) -> Result<(), LiteSVMError> {
        let program_bytes = std::fs::read(path).map_err(LiteSVMError::InvalidPath)?;
        self.add_program(program_id, &program_bytes)
    }

    pub fn add_program_with_loader(
        &mut self,
        program_id: Address,
        program_bytes: &[u8],
        loader_id: Address,
    ) -> Result<(), LiteSVMError> {
        self.base
            .add_program_with_loader(program_id, program_bytes, loader_id)?;
        self.ephemeral
            .add_program_with_loader(program_id, program_bytes, loader_id)
    }

    pub fn send_transaction(&mut self, tx: impl Into<VersionedTransaction>) -> TransactionResult {
        self.send_transaction_to(TransactionTarget::Base, tx)
    }

    pub fn send_transaction_to(
        &mut self,
        target: TransactionTarget,
        tx: impl Into<VersionedTransaction>,
    ) -> TransactionResult {
        let vtx = tx.into();
        match target {
            TransactionTarget::Base => {
                let effects = MagicTransactionEffects::from_message(&vtx.message);
                let writable_accounts =
                    MagicTransactionEffects::writable_accounts_from_message(&vtx.message);
                let result = self.base.send_transaction(vtx);
                if result.is_ok() {
                    self.apply_base_effects(effects);
                    self.apply_base_account_state(&writable_accounts);
                }
                result
            }
            TransactionTarget::Ephemeral => {
                if let Err(err) = self.check_ephemeral_writable_accounts(&vtx.message) {
                    return failed_transaction(err);
                }
                self.sync_ephemeral_fee_payer(&vtx.message);
                let message = vtx.message.clone();
                let result = self.ephemeral.send_transaction(vtx);
                if let Ok(meta) = &result {
                    let effects = MagicTransactionEffects::from_ephemeral_message_and_metadata(
                        &message, meta,
                    );
                    self.apply_base_effects(effects);
                }
                result
            }
        }
    }

    pub fn simulate_transaction_to(
        &mut self,
        target: TransactionTarget,
        tx: impl Into<VersionedTransaction>,
    ) -> std::result::Result<crate::types::SimulatedTransactionInfo, FailedTransactionMetadata>
    {
        let vtx = tx.into();
        match target {
            TransactionTarget::Base => self.base.simulate_transaction(vtx),
            TransactionTarget::Ephemeral => {
                if let Err(err) = self.check_ephemeral_writable_accounts(&vtx.message) {
                    return Err(FailedTransactionMetadata {
                        err,
                        meta: Default::default(),
                    });
                }
                self.ephemeral.simulate_transaction(vtx)
            }
        }
    }

    pub fn delegate_account_for_tests(
        &mut self,
        delegated_account: &Address,
    ) -> Result<(), TransactionError> {
        self.delegate_account(*delegated_account)
    }

    pub fn commit_account_for_tests(&mut self, delegated_account: &Address) {
        self.commit_account(*delegated_account);
    }

    fn check_ephemeral_writable_accounts(
        &self,
        message: &VersionedMessage,
    ) -> Result<(), TransactionError> {
        for (index, key) in message.static_account_keys().iter().enumerate() {
            if message.is_maybe_writable(index, None)
                && !self.is_ephemeral_writable_exception(index, key)
                && !self.delegated_accounts.contains(key)
            {
                return Err(TransactionError::InvalidWritableAccount);
            }
        }
        Ok(())
    }

    fn is_ephemeral_writable_exception(&self, account_index: usize, key: &Address) -> bool {
        account_index == 0 || *key == MAGIC_CONTEXT_ID
    }

    fn sync_ephemeral_fee_payer(&mut self, message: &VersionedMessage) {
        let Some(fee_payer) = message.static_account_keys().first() else {
            return;
        };
        if self.ephemeral.accounts.get_account(fee_payer).is_some() {
            return;
        }
        if let Some(account) = self.base.accounts.get_account(fee_payer) {
            let _ = self.ephemeral.accounts.add_account(*fee_payer, account);
        }
    }

    fn apply_base_effects(&mut self, effects: MagicTransactionEffects) {
        for account in effects.delegated_accounts {
            let _ = self.delegate_account(account);
        }
        for account in effects.committed_accounts {
            self.commit_account(account);
        }
        for account in effects.undelegated_accounts {
            self.commit_account(account);
            self.undelegate_account(account);
        }
    }

    fn apply_base_account_state(&mut self, writable_accounts: &[Address]) {
        for account in writable_accounts {
            if self.has_delegation_metadata_for(account) {
                let _ = self.delegate_account(*account);
            } else if self.delegated_accounts.contains(account)
                && self
                    .base
                    .accounts
                    .get_account(account)
                    .is_some_and(|account| *account.owner() != DELEGATION_PROGRAM_ID)
            {
                self.undelegate_account(*account);
            }
        }
    }

    fn has_delegation_metadata_for(&self, delegated_account: &Address) -> bool {
        let metadata = Address::find_program_address(
            &[b"delegation-metadata", delegated_account.as_ref()],
            &DELEGATION_PROGRAM_ID,
        )
        .0;
        self.base
            .accounts
            .get_account(&metadata)
            .is_some_and(|account| *account.owner() == DELEGATION_PROGRAM_ID)
    }

    fn delegation_record_owner(&self, delegated_account: &Address) -> Option<Address> {
        let record = Address::find_program_address(
            &[b"delegation", delegated_account.as_ref()],
            &DELEGATION_PROGRAM_ID,
        )
        .0;
        let account = self.base.accounts.get_account(&record)?;
        let owner = account.data().get(40..72)?;
        Some(Address::from(<[u8; 32]>::try_from(owner).ok()?))
    }

    fn delegate_account(&mut self, delegated_account: Address) -> Result<(), TransactionError> {
        let Some(mut base_account) = self.base.accounts.get_account(&delegated_account) else {
            return Err(TransactionError::AccountNotFound);
        };
        base_account.set_delegated(true);
        self.base
            .accounts
            .add_account(delegated_account, base_account.clone())
            .map_err(|_| TransactionError::InvalidAccountIndex)?;

        if let Some(owner) = self.delegation_record_owner(&delegated_account) {
            base_account.set_owner(owner);
        }
        base_account.set_ephemeral(true);
        self.ephemeral
            .accounts
            .add_account(delegated_account, base_account)
            .map_err(|_| TransactionError::InvalidAccountIndex)?;
        self.delegated_accounts.insert(delegated_account);
        Ok(())
    }

    fn commit_account(&mut self, delegated_account: Address) {
        let Some(mut ephemeral_account) = self.ephemeral.accounts.get_account(&delegated_account)
        else {
            return;
        };
        ephemeral_account.set_ephemeral(false);
        let _ = self
            .base
            .accounts
            .add_account(delegated_account, ephemeral_account);
    }

    fn undelegate_account(&mut self, delegated_account: Address) {
        self.delegated_accounts.remove(&delegated_account);
        if let Some(mut base_account) = self.base.accounts.get_account(&delegated_account) {
            base_account.set_delegated(false);
            base_account.set_undelegating(false);
            let _ = self
                .base
                .accounts
                .add_account(delegated_account, base_account);
        }
    }
}

fn failed_transaction(err: TransactionError) -> TransactionResult {
    Err(FailedTransactionMetadata {
        err,
        meta: Default::default(),
    })
}

#[derive(Default)]
struct MagicTransactionEffects {
    delegated_accounts: Vec<Address>,
    committed_accounts: Vec<Address>,
    undelegated_accounts: Vec<Address>,
}

impl MagicTransactionEffects {
    fn from_message(message: &VersionedMessage) -> Self {
        let account_keys = message.static_account_keys();
        let mut effects = Self::default();

        for instruction in message.instructions() {
            let Some(program_id) = account_keys.get(usize::from(instruction.program_id_index))
            else {
                continue;
            };
            if *program_id != DELEGATION_PROGRAM_ID {
                continue;
            }
            let Some(discriminator) = instruction_discriminator(&instruction.data) else {
                continue;
            };
            match discriminator {
                DlpDiscriminator::Delegate
                | DlpDiscriminator::DelegateWithAnyValidator
                | DlpDiscriminator::DelegateWithActions => {
                    if let Some(account) = instruction
                        .accounts
                        .get(1)
                        .and_then(|index| account_keys.get(usize::from(*index)))
                    {
                        effects.delegated_accounts.push(*account);
                    }
                }
                DlpDiscriminator::CommitState
                | DlpDiscriminator::Finalize
                | DlpDiscriminator::CommitStateFromBuffer
                | DlpDiscriminator::CommitDiff
                | DlpDiscriminator::CommitDiffFromBuffer
                | DlpDiscriminator::CommitFinalize
                | DlpDiscriminator::CommitFinalizeFromBuffer => {
                    if let Some(account) = instruction
                        .accounts
                        .get(1)
                        .and_then(|index| account_keys.get(usize::from(*index)))
                    {
                        effects.committed_accounts.push(*account);
                    }
                }
                DlpDiscriminator::Undelegate | DlpDiscriminator::UndelegateConfinedAccount => {
                    if let Some(account) = instruction
                        .accounts
                        .get(1)
                        .and_then(|index| account_keys.get(usize::from(*index)))
                    {
                        effects.undelegated_accounts.push(*account);
                    }
                }
                _ => {}
            }
        }

        effects
    }

    fn from_ephemeral_message(message: &VersionedMessage) -> Self {
        let account_keys = message.static_account_keys();
        let mut effects = Self::default();

        for instruction in message.instructions() {
            Self::record_magic_instruction(
                instruction.program_id_index,
                &instruction.accounts,
                &instruction.data,
                account_keys,
                &mut effects,
            );
        }

        effects
    }

    fn from_ephemeral_message_and_metadata(
        message: &VersionedMessage,
        meta: &TransactionMetadata,
    ) -> Self {
        let account_keys = message.static_account_keys();
        let mut effects = Self::from_ephemeral_message(message);

        for inner_instruction in meta.inner_instructions.iter().flatten() {
            let instruction = &inner_instruction.instruction;
            Self::record_magic_instruction(
                instruction.program_id_index,
                &instruction.accounts,
                &instruction.data,
                account_keys,
                &mut effects,
            );
        }

        effects
    }

    fn record_magic_instruction(
        program_id_index: u8,
        instruction_accounts: &[u8],
        instruction_data: &[u8],
        account_keys: &[Address],
        effects: &mut Self,
    ) {
        let Some(program_id) = account_keys.get(usize::from(program_id_index)) else {
            return;
        };
        if *program_id != MAGIC_PROGRAM_ID {
            return;
        }
        let Ok(magic_ix) = magic_instruction(instruction_data) else {
            return;
        };

        let accounts = instruction_accounts
            .iter()
            .skip(2)
            .filter_map(|index| account_keys.get(usize::from(*index)).copied());

        match magic_ix {
            MagicInstruction::ScheduleCommit
            | MagicInstruction::ScheduleCommitFinalize {
                request_undelegation: false,
            } => effects.committed_accounts.extend(accounts),
            MagicInstruction::ScheduleCommitAndUndelegate
            | MagicInstruction::ScheduleCommitFinalize {
                request_undelegation: true,
            } => effects.undelegated_accounts.extend(accounts),
            MagicInstruction::ScheduleBaseIntent {
                committed_accounts,
                undelegated_accounts,
            }
            | MagicInstruction::ScheduleIntentBundle {
                committed_accounts,
                undelegated_accounts,
            } => {
                effects
                    .committed_accounts
                    .extend(committed_accounts.into_iter().filter_map(|index| {
                        instruction_accounts
                            .get(usize::from(index))
                            .and_then(|account_index| account_keys.get(usize::from(*account_index)))
                            .copied()
                    }));
                effects
                    .undelegated_accounts
                    .extend(undelegated_accounts.into_iter().filter_map(|index| {
                        instruction_accounts
                            .get(usize::from(index))
                            .and_then(|account_index| account_keys.get(usize::from(*account_index)))
                            .copied()
                    }));
            }
            MagicInstruction::Noop => {}
        }
    }

    fn writable_accounts_from_message(message: &VersionedMessage) -> Vec<Address> {
        message
            .static_account_keys()
            .iter()
            .enumerate()
            .filter_map(|(index, key)| {
                (index != 0 && message.is_maybe_writable(index, None)).then_some(*key)
            })
            .collect()
    }
}

fn instruction_discriminator(data: &[u8]) -> Option<DlpDiscriminator> {
    let bytes = data.get(..8)?;
    let discriminator = u64::from_le_bytes(bytes.try_into().ok()?);
    u8::try_from(discriminator).ok()?.try_into().ok()
}
