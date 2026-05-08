use {
    litesvm::{
        MagicSVM, TransactionTarget, DEFAULT_VALIDATOR_IDENTITY, DELEGATION_PROGRAM_ID,
        MAGIC_CONTEXT_ID, MAGIC_PROGRAM_ID,
    },
    solana_instruction::{account_meta::AccountMeta, error::InstructionError, Instruction},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_native_token::LAMPORTS_PER_SOL,
    solana_sdk_ids::bpf_loader_upgradeable,
    solana_signer::Signer,
    solana_system_interface::instruction::allocate,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const SCHEDULE_COMMIT: u32 = 1;
const SCHEDULE_COMMIT_AND_UNDELEGATE: u32 = 2;
const SCHEDULE_BASE_INTENT: u32 = 5;
const SCHEDULE_COMMIT_FINALIZE: u32 = 15;
const SCHEDULE_INTENT_BUNDLE: u32 = 11;
const BASE_INTENT_COMMIT: u32 = 1;
const BASE_INTENT_COMMIT_AND_UNDELEGATE: u32 = 2;
const BASE_INTENT_COMMIT_FINALIZE: u32 = 3;
const BASE_INTENT_COMMIT_FINALIZE_AND_UNDELEGATE: u32 = 4;
const COMMIT_TYPE_STANDALONE: u32 = 0;
const UNDELEGATE_TYPE_STANDALONE: u32 = 0;

fn schedule_commit_data(variant: u32, request_undelegation: Option<bool>) -> Vec<u8> {
    let mut data = variant.to_le_bytes().to_vec();
    if let Some(request_undelegation) = request_undelegation {
        data.push(u8::from(request_undelegation));
    }
    data
}

fn commit_type_standalone(account_indices: &[u8]) -> Vec<u8> {
    let mut data = COMMIT_TYPE_STANDALONE.to_le_bytes().to_vec();
    data.extend_from_slice(&(account_indices.len() as u64).to_le_bytes());
    data.extend_from_slice(account_indices);
    data
}

fn commit_and_undelegate(account_indices: &[u8]) -> Vec<u8> {
    let mut data = commit_type_standalone(account_indices);
    data.extend_from_slice(&UNDELEGATE_TYPE_STANDALONE.to_le_bytes());
    data
}

fn schedule_base_intent_data(base_intent_variant: u32, account_indices: &[u8]) -> Vec<u8> {
    let mut data = SCHEDULE_BASE_INTENT.to_le_bytes().to_vec();
    data.extend_from_slice(&base_intent_variant.to_le_bytes());
    match base_intent_variant {
        BASE_INTENT_COMMIT | BASE_INTENT_COMMIT_FINALIZE => {
            data.extend_from_slice(&commit_type_standalone(account_indices));
        }
        BASE_INTENT_COMMIT_AND_UNDELEGATE | BASE_INTENT_COMMIT_FINALIZE_AND_UNDELEGATE => {
            data.extend_from_slice(&commit_and_undelegate(account_indices));
        }
        _ => unreachable!("unsupported test base intent variant"),
    }
    data
}

fn schedule_intent_bundle_data(
    commit: Option<&[u8]>,
    commit_and_undelegate_indices: Option<&[u8]>,
    commit_finalize: Option<&[u8]>,
    commit_finalize_and_undelegate_indices: Option<&[u8]>,
) -> Vec<u8> {
    fn push_option(data: &mut Vec<u8>, indices: Option<&[u8]>, undelegate: bool) {
        match indices {
            Some(indices) => {
                data.push(1);
                if undelegate {
                    data.extend_from_slice(&commit_and_undelegate(indices));
                } else {
                    data.extend_from_slice(&commit_type_standalone(indices));
                }
            }
            None => data.push(0),
        }
    }

    let mut data = SCHEDULE_INTENT_BUNDLE.to_le_bytes().to_vec();
    push_option(&mut data, commit, false);
    push_option(&mut data, commit_and_undelegate_indices, true);
    push_option(&mut data, commit_finalize, false);
    push_option(&mut data, commit_finalize_and_undelegate_indices, true);
    data.extend_from_slice(&0_u64.to_le_bytes());
    data
}

fn schedule_commit_tx(
    payer: &Keypair,
    delegated_account: &Keypair,
    instruction_data: Vec<u8>,
    delegated_account_is_writable: bool,
    blockhash: solana_hash::Hash,
) -> Transaction {
    let delegated_meta = if delegated_account_is_writable {
        AccountMeta::new(delegated_account.pubkey(), false)
    } else {
        AccountMeta::new_readonly(delegated_account.pubkey(), false)
    };

    Transaction::new(
        &[payer],
        Message::new(
            &[Instruction {
                program_id: MAGIC_PROGRAM_ID,
                accounts: vec![
                    AccountMeta::new(payer.pubkey(), true),
                    AccountMeta::new(MAGIC_CONTEXT_ID, false),
                    delegated_meta,
                ],
                data: instruction_data,
            }],
            Some(&payer.pubkey()),
        ),
        blockhash,
    )
}

fn custom_schedule_commit_tx(
    fee_payer: &Keypair,
    accounts: Vec<AccountMeta>,
    instruction_data: Vec<u8>,
    blockhash: solana_hash::Hash,
) -> Transaction {
    Transaction::new(
        &[fee_payer],
        Message::new(
            &[Instruction {
                program_id: MAGIC_PROGRAM_ID,
                accounts,
                data: instruction_data,
            }],
            Some(&fee_payer.pubkey()),
        ),
        blockhash,
    )
}

fn allocate_ephemeral_account(svm: &mut MagicSVM, payer: &Keypair, delegated_account: &Keypair) {
    svm.send_transaction_to(
        TransactionTarget::Ephemeral,
        Transaction::new(
            &[payer, delegated_account],
            Message::new(
                &[allocate(&delegated_account.pubkey(), 8)],
                Some(&payer.pubkey()),
            ),
            svm.latest_blockhash_for(TransactionTarget::Ephemeral),
        ),
    )
    .unwrap();
    svm.expire_blockhash_for(TransactionTarget::Ephemeral);
}

#[test_log::test]
fn magic_svm_loads_delegation_program_by_default() {
    let svm = MagicSVM::new();

    let delegation_program = svm.get_account(&DELEGATION_PROGRAM_ID).unwrap();

    assert!(delegation_program.executable);
    assert_eq!(delegation_program.owner, bpf_loader_upgradeable::id());
}

#[test_log::test]
fn magic_svm_loads_magic_program_only_on_ephemeral() {
    let svm = MagicSVM::new();

    assert!(svm
        .get_account_for(TransactionTarget::Base, &MAGIC_PROGRAM_ID)
        .is_none());

    let magic_program = svm
        .get_account_for(TransactionTarget::Ephemeral, &MAGIC_PROGRAM_ID)
        .unwrap();
    assert!(magic_program.executable);
}

#[test_log::test]
fn ephemeral_magic_program_accepts_noop_and_rejects_invalid_data() {
    let payer = Keypair::new();
    let mut svm = MagicSVM::new();
    svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let mut noop_data = 10_u32.to_le_bytes().to_vec();
    noop_data.extend_from_slice(&0_u64.to_le_bytes());
    let noop = Transaction::new(
        &[&payer],
        Message::new(
            &[Instruction {
                program_id: MAGIC_PROGRAM_ID,
                accounts: vec![],
                data: noop_data,
            }],
            Some(&payer.pubkey()),
        ),
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    svm.send_transaction_to(TransactionTarget::Ephemeral, noop)
        .unwrap();

    svm.expire_blockhash_for(TransactionTarget::Ephemeral);
    let invalid = Transaction::new(
        &[&payer],
        Message::new(
            &[Instruction {
                program_id: MAGIC_PROGRAM_ID,
                accounts: vec![],
                data: vec![0xff],
            }],
            Some(&payer.pubkey()),
        ),
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    let err = svm
        .send_transaction_to(TransactionTarget::Ephemeral, invalid)
        .unwrap_err()
        .err;
    assert_eq!(
        err,
        TransactionError::InstructionError(0, InstructionError::InvalidInstructionData)
    );
}

#[test_log::test]
fn magic_svm_defaults_to_magicblock_validator_identity() {
    let svm = MagicSVM::new();

    assert_eq!(svm.validator_identity(), DEFAULT_VALIDATOR_IDENTITY);
}

#[test_log::test]
fn magic_svm_can_be_initialized_with_a_validator_identity() {
    let validator = Keypair::new();
    let svm = MagicSVM::new_with_validator_identity(validator.pubkey());

    assert_eq!(svm.validator_identity(), validator.pubkey());
}

#[test_log::test]
fn target_specific_helpers_use_the_selected_ledger() {
    let payer = Keypair::new();
    let delegated = Keypair::new();
    let mut svm = MagicSVM::new();

    let base_airdrop = svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();
    assert!(svm
        .get_transaction_for(TransactionTarget::Base, &base_airdrop.signature)
        .is_some());
    assert!(svm
        .get_transaction_for(TransactionTarget::Ephemeral, &base_airdrop.signature)
        .is_none());

    svm.airdrop(&delegated.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.delegate_account_for_tests(&delegated.pubkey()).unwrap();

    let base_blockhash = svm.latest_blockhash_for(TransactionTarget::Base);
    let ephemeral_blockhash = svm.latest_blockhash_for(TransactionTarget::Ephemeral);
    svm.expire_blockhash_for(TransactionTarget::Base);
    assert_ne!(
        svm.latest_blockhash_for(TransactionTarget::Base),
        base_blockhash
    );
    assert_eq!(
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
        ephemeral_blockhash
    );

    let allowed = Transaction::new(
        &[&payer, &delegated],
        Message::new(&[allocate(&delegated.pubkey(), 8)], Some(&payer.pubkey())),
        ephemeral_blockhash,
    );
    let ephemeral_result = svm
        .send_transaction_to(TransactionTarget::Ephemeral, allowed)
        .unwrap();
    assert!(svm
        .get_transaction_for(TransactionTarget::Ephemeral, &ephemeral_result.signature)
        .is_some());
    assert!(svm
        .get_transaction_for(TransactionTarget::Base, &ephemeral_result.signature)
        .is_none());

    assert_eq!(
        svm.get_account_for(TransactionTarget::Base, &delegated.pubkey())
            .unwrap()
            .data
            .len(),
        0
    );
    assert_eq!(
        svm.get_account_for(TransactionTarget::Ephemeral, &delegated.pubkey())
            .unwrap()
            .data
            .len(),
        8
    );
}

#[test_log::test]
fn delegated_accounts_are_mirrored_to_ephemeral_ledger() {
    let delegated = Keypair::new();
    let mut svm = MagicSVM::new();
    svm.airdrop(&delegated.pubkey(), LAMPORTS_PER_SOL).unwrap();

    svm.delegate_account_for_tests(&delegated.pubkey()).unwrap();

    let ephemeral_account = svm.ephemeral_account(&delegated.pubkey()).unwrap();
    assert!(ephemeral_account.delegated());
    assert!(ephemeral_account.ephemeral());
}

#[test_log::test]
fn ephemeral_transactions_can_only_write_delegated_accounts() {
    let payer = Keypair::new();
    let delegated = Keypair::new();
    let non_delegated = Keypair::new();
    let mut svm = MagicSVM::new();
    svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&delegated.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&non_delegated.pubkey(), LAMPORTS_PER_SOL)
        .unwrap();
    svm.delegate_account_for_tests(&delegated.pubkey()).unwrap();

    let allowed = Transaction::new(
        &[&payer, &delegated],
        Message::new(&[allocate(&delegated.pubkey(), 8)], Some(&payer.pubkey())),
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    assert!(svm
        .send_transaction_to(TransactionTarget::Ephemeral, allowed)
        .is_ok());

    let rejected = Transaction::new(
        &[&payer, &non_delegated],
        Message::new(
            &[allocate(&non_delegated.pubkey(), 8)],
            Some(&payer.pubkey()),
        ),
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    let err = svm
        .send_transaction_to(TransactionTarget::Ephemeral, rejected)
        .unwrap_err()
        .err;
    assert_eq!(err, TransactionError::InvalidWritableAccount);
}

#[test_log::test]
fn commit_finalize_copies_ephemeral_state_back_to_base() {
    let delegated = Keypair::new();
    let mut svm = MagicSVM::new();
    svm.airdrop(&delegated.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.delegate_account_for_tests(&delegated.pubkey()).unwrap();

    svm.send_transaction_to(
        TransactionTarget::Ephemeral,
        Transaction::new(
            &[&delegated],
            Message::new(
                &[allocate(&delegated.pubkey(), 8)],
                Some(&delegated.pubkey()),
            ),
            svm.latest_blockhash_for(TransactionTarget::Ephemeral),
        ),
    )
    .unwrap();

    svm.commit_account_for_tests(&delegated.pubkey());

    let base_account = svm.get_account(&delegated.pubkey()).unwrap();
    assert_eq!(base_account.data.len(), 8);
}

#[test_log::test]
fn ephemeral_schedule_commit_variants_copy_state_to_base() {
    for instruction_data in [
        schedule_commit_data(SCHEDULE_COMMIT, None),
        schedule_commit_data(SCHEDULE_COMMIT_FINALIZE, Some(false)),
        schedule_base_intent_data(BASE_INTENT_COMMIT, &[2]),
        schedule_base_intent_data(BASE_INTENT_COMMIT_FINALIZE, &[2]),
        schedule_intent_bundle_data(Some(&[2]), None, None, None),
        schedule_intent_bundle_data(None, None, Some(&[2]), None),
    ] {
        let payer = Keypair::new();
        let delegated = Keypair::new();
        let mut svm = MagicSVM::new();
        svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&delegated.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.delegate_account_for_tests(&delegated.pubkey()).unwrap();
        allocate_ephemeral_account(&mut svm, &payer, &delegated);

        svm.send_transaction_to(
            TransactionTarget::Ephemeral,
            schedule_commit_tx(
                &payer,
                &delegated,
                instruction_data,
                false,
                svm.latest_blockhash_for(TransactionTarget::Ephemeral),
            ),
        )
        .unwrap();

        let base_account = svm.get_account(&delegated.pubkey()).unwrap();
        assert_eq!(base_account.data.len(), 8);
    }
}

#[test_log::test]
fn ephemeral_schedule_commit_variants_can_undelegate() {
    for instruction_data in [
        schedule_commit_data(SCHEDULE_COMMIT_AND_UNDELEGATE, None),
        schedule_commit_data(SCHEDULE_COMMIT_FINALIZE, Some(true)),
        schedule_base_intent_data(BASE_INTENT_COMMIT_AND_UNDELEGATE, &[2]),
        schedule_base_intent_data(BASE_INTENT_COMMIT_FINALIZE_AND_UNDELEGATE, &[2]),
        schedule_intent_bundle_data(None, Some(&[2]), None, None),
        schedule_intent_bundle_data(None, None, None, Some(&[2])),
    ] {
        let payer = Keypair::new();
        let delegated = Keypair::new();
        let mut svm = MagicSVM::new();
        svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.airdrop(&delegated.pubkey(), LAMPORTS_PER_SOL).unwrap();
        svm.delegate_account_for_tests(&delegated.pubkey()).unwrap();
        allocate_ephemeral_account(&mut svm, &payer, &delegated);

        svm.send_transaction_to(
            TransactionTarget::Ephemeral,
            schedule_commit_tx(
                &payer,
                &delegated,
                instruction_data,
                true,
                svm.latest_blockhash_for(TransactionTarget::Ephemeral),
            ),
        )
        .unwrap();

        let base_account = svm.get_account(&delegated.pubkey()).unwrap();
        assert_eq!(base_account.data.len(), 8);

        svm.expire_blockhash_for(TransactionTarget::Ephemeral);
        let rejected = Transaction::new(
            &[&payer, &delegated],
            Message::new(&[allocate(&delegated.pubkey(), 16)], Some(&payer.pubkey())),
            svm.latest_blockhash_for(TransactionTarget::Ephemeral),
        );
        let err = svm
            .send_transaction_to(TransactionTarget::Ephemeral, rejected)
            .unwrap_err()
            .err;
        assert_eq!(err, TransactionError::InvalidWritableAccount);
    }
}

#[test_log::test]
fn ephemeral_magic_processors_reject_invalid_schedule_commit_accounts() {
    let payer = Keypair::new();
    let schedule_payer = Keypair::new();
    let delegated = Keypair::new();
    let wrong_context = Keypair::new();
    let mut svm = MagicSVM::new();
    svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&schedule_payer.pubkey(), LAMPORTS_PER_SOL)
        .unwrap();
    svm.airdrop(&delegated.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.delegate_account_for_tests(&delegated.pubkey()).unwrap();
    allocate_ephemeral_account(&mut svm, &payer, &delegated);

    let wrong_context_tx = custom_schedule_commit_tx(
        &payer,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(wrong_context.pubkey(), false),
            AccountMeta::new_readonly(delegated.pubkey(), false),
        ],
        schedule_commit_data(SCHEDULE_COMMIT, None),
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    let err = svm
        .send_transaction_to(TransactionTarget::Ephemeral, wrong_context_tx)
        .unwrap_err()
        .err;
    assert_eq!(
        err,
        TransactionError::InstructionError(0, InstructionError::MissingAccount)
    );

    svm.expire_blockhash_for(TransactionTarget::Ephemeral);
    let missing_signer_tx = custom_schedule_commit_tx(
        &payer,
        vec![
            AccountMeta::new_readonly(schedule_payer.pubkey(), false),
            AccountMeta::new(MAGIC_CONTEXT_ID, false),
            AccountMeta::new_readonly(delegated.pubkey(), false),
        ],
        schedule_commit_data(SCHEDULE_COMMIT, None),
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    let err = svm
        .send_transaction_to(TransactionTarget::Ephemeral, missing_signer_tx)
        .unwrap_err()
        .err;
    assert_eq!(
        err,
        TransactionError::InstructionError(0, InstructionError::MissingRequiredSignature)
    );

    svm.expire_blockhash_for(TransactionTarget::Ephemeral);
    let no_accounts_tx = custom_schedule_commit_tx(
        &payer,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(MAGIC_CONTEXT_ID, false),
        ],
        schedule_commit_data(SCHEDULE_COMMIT, None),
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    let err = svm
        .send_transaction_to(TransactionTarget::Ephemeral, no_accounts_tx)
        .unwrap_err()
        .err;
    assert_eq!(
        err,
        TransactionError::InstructionError(0, InstructionError::MissingAccount)
    );

    svm.expire_blockhash_for(TransactionTarget::Ephemeral);
    let readonly_undelegate_tx = schedule_commit_tx(
        &payer,
        &delegated,
        schedule_commit_data(SCHEDULE_COMMIT_AND_UNDELEGATE, None),
        false,
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    let err = svm
        .send_transaction_to(TransactionTarget::Ephemeral, readonly_undelegate_tx)
        .unwrap_err()
        .err;
    assert_eq!(
        err,
        TransactionError::InstructionError(0, InstructionError::ReadonlyDataModified)
    );
}
