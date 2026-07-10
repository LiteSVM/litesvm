use {
    litesvm::LiteSVM,
    solana_account::{Account, WritableAccount},
    solana_address::Address,
    solana_clock::Clock,
    solana_epoch_schedule::EpochSchedule,
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_program_error::{ProgramError, ProgramResult},
    solana_signer::{signers::Signers, Signer},
    solana_stake_interface::{
        self as stake,
        instruction as ixn,
        state::{Authorized, Lockup},
    },
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
    solana_vote_interface::{
        authorized_voters::AuthorizedVoters,
        state::{VoteStateV4, VoteStateVersions},
    },
};

// utility function, used by Stakes, tests
fn to<T: WritableAccount>(versioned: &VoteStateVersions, account: &mut T) -> Option<()> {
    VoteStateV4::serialize(versioned, account.data_as_mut_slice()).ok()
}

fn advance_epoch(svm: &mut LiteSVM) {
    refresh_blockhash(svm);
    let old_clock = svm.get_sysvar::<Clock>();
    let root_slot = old_clock.slot;
    let slots_per_epoch = svm.get_sysvar::<EpochSchedule>().slots_per_epoch;
    svm.warp_to_slot(root_slot + slots_per_epoch);
    let mut new_clock = old_clock;
    new_clock.epoch += 1;
    svm.set_sysvar::<Clock>(&new_clock)
}

fn refresh_blockhash(svm: &mut LiteSVM) {
    svm.expire_blockhash()
}

fn get_stake_account_rent(svm: &mut LiteSVM) -> u64 {
    svm.minimum_balance_for_rent_exemption(std::mem::size_of::<stake::state::StakeStateV2>())
}

fn get_minimum_delegation(svm: &mut LiteSVM, payer: &Keypair) -> u64 {
    let transaction = Transaction::new_signed_with_payer(
        &[stake::instruction::get_minimum_delegation()],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );
    let mut data = svm
        .simulate_transaction(transaction)
        .unwrap()
        .meta
        .return_data
        .data;
    data.resize(8, 0);

    data.try_into().map(u64::from_le_bytes).unwrap()
}

fn create_independent_stake_account(
    svm: &mut LiteSVM,
    authorized: &Authorized,
    stake_amount: u64,
    payer: &Keypair,
) -> Address {
    let stake = Keypair::new();
    let lamports = get_stake_account_rent(svm) + stake_amount;

    let instructions = vec![
        solana_system_interface::instruction::create_account(
            &payer.pubkey(),
            &stake.pubkey(),
            lamports,
            std::mem::size_of::<stake::state::StakeStateV2>() as u64,
            &solana_sdk_ids::stake::id(),
        ),
        stake::instruction::initialize(&stake.pubkey(), authorized, &Lockup::default()),
    ];

    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[payer, &stake],
        svm.latest_blockhash(),
    );

    svm.send_transaction(transaction).unwrap();

    stake.pubkey()
}

fn process_instruction<T: Signers + ?Sized>(
    svm: &mut LiteSVM,
    instruction: &Instruction,
    additional_signers: &T,
    payer: &Keypair,
) -> ProgramResult {
    let mut transaction =
        Transaction::new_with_payer(std::slice::from_ref(instruction), Some(&payer.pubkey()));

    transaction.partial_sign(&[&payer], svm.latest_blockhash());
    transaction.sign(additional_signers, svm.latest_blockhash());

    match svm.send_transaction(transaction) {
        Ok(_) => Ok(()),
        Err(e) => {
            // banks client error -> transaction error -> instruction error -> program error
            match e.err {
                TransactionError::InstructionError(_, e) => Err(e.try_into().unwrap()),
                TransactionError::InsufficientFundsForRent { .. } => {
                    Err(ProgramError::InsufficientFunds)
                }
                _ => panic!("couldnt convert {e:?} to ProgramError"),
            }
        }
    }
}

#[test]
fn test_stake_surfpool_605() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    // Bug 3: StakeConfig must be present at the well-known address after LiteSVM::new().
    #[allow(deprecated)]
    let stake_config_id = solana_sdk_ids::stake::config::id();
    assert!(
        svm.get_account(&stake_config_id).is_some(),
        "StakeConfig account not found — was it removed from set_sysvars()?"
    );

    // Build a V4 vote account directly, bypassing the vote program.
    // This simulates the way surfpool (and similar fork-from-mainnet tools) inject live accounts:
    // they serialise the RPC state straight into LiteSVM without re-creating it via instructions.
    let voter = Keypair::new();
    let vote_account_address = Address::new_unique();
    let vote_state = VoteStateV4 {
        node_pubkey: Keypair::new().pubkey(),
        authorized_withdrawer: Keypair::new().pubkey(),
        authorized_voters: AuthorizedVoters::new(0, voter.pubkey()),
        ..VoteStateV4::default()
    };
    let mut vote_account = Account {
        lamports: svm.minimum_balance_for_rent_exemption(VoteStateV4::size_of()),
        data: vec![0u8; VoteStateV4::size_of()],
        owner: solana_sdk_ids::vote::id(),
        executable: false,
        rent_epoch: u64::MAX,
    };
    to(
        &VoteStateVersions::V4(Box::new(vote_state)),
        &mut vote_account,
    )
    .unwrap();
    svm.set_account(vote_account_address, vote_account).unwrap();

    // Bug 1: DelegateStake to the V4 vote account.
    // The old stake ELF (v1.0.1) returned InvalidAccountData for V4 vote accounts.
    let staker = Keypair::new();
    let withdrawer = Keypair::new();
    let minimum_delegation = get_minimum_delegation(&mut svm, &payer);
    let stake = create_independent_stake_account(
        &mut svm,
        &Authorized {
            staker: staker.pubkey(),
            withdrawer: withdrawer.pubkey(),
        },
        minimum_delegation * 2,
        &payer,
    );
    process_instruction(
        &mut svm,
        &ixn::delegate_stake(&stake, &staker.pubkey(), &vote_account_address),
        &[&staker],
        &payer,
    )
    .unwrap();

    // Bug 2: Perform a stake operation after epoch 2.
    // The old 16 KiB zero-padded StakeHistory caused the Stake BPF program to panic when it read
    // an entry via sol_get_sysvar and the zero bytes made the epoch-index assertion fire.
    advance_epoch(&mut svm);
    advance_epoch(&mut svm);

    process_instruction(
        &mut svm,
        &ixn::deactivate_stake(&stake, &staker.pubkey()),
        &[&staker],
        &payer,
    )
    .unwrap();
}
