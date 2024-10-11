// ported from https://github.com/solana-program/stake-program/blob/master/tests/tests.rs

use {
    litesvm::LiteSVM,
    solana_sdk::{
        account::Account,
        entrypoint::ProgramResult,
        epoch_schedule::EpochSchedule,
        hash::Hash,
        instruction::Instruction,
        program_error::ProgramError,
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        signers::Signers,
        stake::{
            self,
            instruction::{self as ixn, LockupArgs},
            state::{Authorized, Delegation, Lockup, Meta, Stake, StakeAuthorize, StakeStateV2},
        },
        system_instruction, system_program,
        sysvar::{clock::Clock, rent::Rent},
        transaction::{Transaction, TransactionError},
    },
    solana_vote_program::{
        vote_instruction,
        vote_state::{self, VoteInit, VoteState, VoteStateVersions},
    },
};

fn increment_vote_account_credits(
    svm: &mut LiteSVM,
    vote_account_address: Pubkey,
    number_of_credits: u64,
) {
    // generate some vote activity for rewards
    let mut vote_account = svm.get_account(&vote_account_address).unwrap();
    let mut vote_state = vote_state::from(&vote_account).unwrap();

    let epoch = svm.get_sysvar::<Clock>().epoch;
    for _ in 0..number_of_credits {
        vote_state.increment_credits(epoch, 1);
    }
    let versioned = VoteStateVersions::new_current(vote_state);
    vote_state::to(&versioned, &mut vote_account).unwrap();
    svm.set_account(vote_account_address, vote_account).unwrap();
}

#[derive(Debug, PartialEq)]
struct Accounts {
    pub validator: Keypair,
    pub voter: Keypair,
    pub withdrawer: Keypair,
    pub vote_account: Keypair,
}

impl Accounts {
    fn initialize(&self, svm: &mut LiteSVM, payer: &Keypair) {
        let epoch_schedule = svm.get_sysvar::<EpochSchedule>();
        let slot = epoch_schedule.first_normal_slot + 1;
        svm.warp_to_slot(slot);

        create_vote(
            svm,
            payer,
            &svm.latest_blockhash(),
            &self.validator,
            &self.voter.pubkey(),
            &self.withdrawer.pubkey(),
            &self.vote_account,
        );
    }
}

impl Default for Accounts {
    fn default() -> Self {
        let vote_account = Keypair::new();

        Self {
            validator: Keypair::new(),
            voter: Keypair::new(),
            withdrawer: Keypair::new(),
            vote_account,
        }
    }
}

fn create_vote(
    svm: &mut LiteSVM,
    payer: &Keypair,
    recent_blockhash: &Hash,
    validator: &Keypair,
    voter: &Pubkey,
    withdrawer: &Pubkey,
    vote_account: &Keypair,
) {
    let rent = svm.get_sysvar::<Rent>();
    let rent_voter = rent.minimum_balance(VoteState::size_of());

    let mut instructions = vec![system_instruction::create_account(
        &payer.pubkey(),
        &validator.pubkey(),
        rent.minimum_balance(0),
        0,
        &system_program::id(),
    )];
    instructions.append(&mut vote_instruction::create_account_with_config(
        &payer.pubkey(),
        &vote_account.pubkey(),
        &VoteInit {
            node_pubkey: validator.pubkey(),
            authorized_voter: *voter,
            authorized_withdrawer: *withdrawer,
            ..VoteInit::default()
        },
        rent_voter,
        vote_instruction::CreateVoteAccountConfig {
            space: VoteStateVersions::vote_state_size_of(true) as u64,
            ..Default::default()
        },
    ));

    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[validator, vote_account, payer],
        *recent_blockhash,
    );

    // ignore errors for idempotency
    let _ = svm.send_transaction(transaction);
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

fn get_account(svm: &mut LiteSVM, pubkey: &Pubkey) -> Account {
    svm.get_account(pubkey).expect("account not found")
}

fn get_stake_account(svm: &mut LiteSVM, pubkey: &Pubkey) -> (Meta, Option<Stake>, u64) {
    let stake_account = get_account(svm, pubkey);
    let lamports = stake_account.lamports;
    match bincode::deserialize::<StakeStateV2>(&stake_account.data).unwrap() {
        StakeStateV2::Initialized(meta) => (meta, None, lamports),
        StakeStateV2::Stake(meta, stake, _) => (meta, Some(stake), lamports),
        StakeStateV2::Uninitialized => panic!("panic: uninitialized"),
        _ => unimplemented!(),
    }
}

fn get_stake_account_rent(svm: &mut LiteSVM) -> u64 {
    let rent = svm.get_sysvar::<Rent>();
    rent.minimum_balance(std::mem::size_of::<stake::state::StakeStateV2>())
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
) -> Pubkey {
    create_independent_stake_account_with_lockup(
        svm,
        authorized,
        &Lockup::default(),
        stake_amount,
        payer,
    )
}

fn create_independent_stake_account_with_lockup(
    svm: &mut LiteSVM,
    authorized: &Authorized,
    lockup: &Lockup,
    stake_amount: u64,
    payer: &Keypair,
) -> Pubkey {
    let stake = Keypair::new();
    let lamports = get_stake_account_rent(svm) + stake_amount;

    let instructions = vec![
        system_instruction::create_account(
            &payer.pubkey(),
            &stake.pubkey(),
            lamports,
            std::mem::size_of::<stake::state::StakeStateV2>() as u64,
            &solana_program::stake::program::id(),
        ),
        stake::instruction::initialize(&stake.pubkey(), authorized, lockup),
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

fn create_blank_stake_account(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    let stake = Keypair::new();
    create_blank_stake_account_from_keypair(svm, &stake, payer)
}

fn create_blank_stake_account_from_keypair(
    svm: &mut LiteSVM,
    stake: &Keypair,
    payer: &Keypair,
) -> Pubkey {
    let lamports = get_stake_account_rent(svm);

    let transaction = Transaction::new_signed_with_payer(
        &[system_instruction::create_account(
            &payer.pubkey(),
            &stake.pubkey(),
            lamports,
            StakeStateV2::size_of() as u64,
            &solana_program::stake::program::id(),
        )],
        Some(&payer.pubkey()),
        &[&payer, &stake],
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
        Transaction::new_with_payer(&[instruction.clone()], Some(&payer.pubkey()));

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
                _ => panic!("couldnt convert {:?} to ProgramError", e),
            }
        }
    }
}

fn test_instruction_with_missing_signers(
    svm: &mut LiteSVM,
    instruction: &Instruction,
    additional_signers: &Vec<&Keypair>,
    payer: &Keypair,
) {
    // remove every signer one by one and ensure we always fail
    for i in 0..instruction.accounts.len() {
        if instruction.accounts[i].is_signer {
            let mut instruction = instruction.clone();
            instruction.accounts[i].is_signer = false;
            let reduced_signers: Vec<_> = additional_signers
                .iter()
                .filter(|s| s.pubkey() != instruction.accounts[i].pubkey)
                .collect();

            let e = process_instruction(svm, &instruction, &reduced_signers, payer).unwrap_err();
            assert_eq!(e, ProgramError::MissingRequiredSignature);
        }
    }

    // now make sure the instruction succeeds
    process_instruction(svm, instruction, additional_signers, payer).unwrap();
}

#[test]
fn test_stake_checked_instructions() {
    let mut svm = LiteSVM::new();
    let accounts = Accounts::default();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000_000).unwrap();
    accounts.initialize(&mut svm, &payer);

    let staker_keypair = Keypair::new();
    let withdrawer_keypair = Keypair::new();
    let authorized_keypair = Keypair::new();
    let seed_base_keypair = Keypair::new();
    let custodian_keypair = Keypair::new();

    let staker = staker_keypair.pubkey();
    let withdrawer = withdrawer_keypair.pubkey();
    let authorized = authorized_keypair.pubkey();
    let seed_base = seed_base_keypair.pubkey();
    let custodian = custodian_keypair.pubkey();

    let seed = "test seed";
    let seeded_address = Pubkey::create_with_seed(&seed_base, seed, &system_program::id()).unwrap();

    // Test InitializeChecked with non-signing withdrawer
    let stake = create_blank_stake_account(&mut svm, &payer);
    let instruction = ixn::initialize_checked(&stake, &Authorized { staker, withdrawer });

    test_instruction_with_missing_signers(
        &mut svm,
        &instruction,
        &vec![&withdrawer_keypair],
        &payer,
    );

    // Test AuthorizeChecked with non-signing staker
    let stake =
        create_independent_stake_account(&mut svm, &Authorized { staker, withdrawer }, 0, &payer);
    let instruction =
        ixn::authorize_checked(&stake, &staker, &authorized, StakeAuthorize::Staker, None);

    test_instruction_with_missing_signers(
        &mut svm,
        &instruction,
        &vec![&staker_keypair, &authorized_keypair],
        &payer,
    );

    // Test AuthorizeChecked with non-signing withdrawer
    let stake =
        create_independent_stake_account(&mut svm, &Authorized { staker, withdrawer }, 0, &payer);
    let instruction = ixn::authorize_checked(
        &stake,
        &withdrawer,
        &authorized,
        StakeAuthorize::Withdrawer,
        None,
    );

    test_instruction_with_missing_signers(
        &mut svm,
        &instruction,
        &vec![&withdrawer_keypair, &authorized_keypair],
        &payer,
    );

    // Test AuthorizeCheckedWithSeed with non-signing authority
    for authority_type in [StakeAuthorize::Staker, StakeAuthorize::Withdrawer] {
        let stake = create_independent_stake_account(
            &mut svm,
            &Authorized::auto(&seeded_address),
            0,
            &payer,
        );
        let instruction = ixn::authorize_checked_with_seed(
            &stake,
            &seed_base,
            seed.to_string(),
            &system_program::id(),
            &authorized,
            authority_type,
            None,
        );

        test_instruction_with_missing_signers(
            &mut svm,
            &instruction,
            &vec![&seed_base_keypair, &authorized_keypair],
            &payer,
        );
    }

    // Test SetLockupChecked with non-signing lockup custodian
    let stake =
        create_independent_stake_account(&mut svm, &Authorized { staker, withdrawer }, 0, &payer);
    let instruction = ixn::set_lockup_checked(
        &stake,
        &LockupArgs {
            unix_timestamp: None,
            epoch: Some(1),
            custodian: Some(custodian),
        },
        &withdrawer,
    );

    test_instruction_with_missing_signers(
        &mut svm,
        &instruction,
        &vec![&withdrawer_keypair, &custodian_keypair],
        &payer,
    );
}

#[test]
fn test_stake_initialize() {
    let mut svm = LiteSVM::new();
    let accounts = Accounts::default();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000_000).unwrap();
    accounts.initialize(&mut svm, &payer);

    let rent_exempt_reserve = get_stake_account_rent(&mut svm);
    let no_signers: [&Keypair; 0] = [];

    let staker_keypair = Keypair::new();
    let withdrawer_keypair = Keypair::new();
    let custodian_keypair = Keypair::new();

    let staker = staker_keypair.pubkey();
    let withdrawer = withdrawer_keypair.pubkey();
    let custodian = custodian_keypair.pubkey();

    let authorized = Authorized { staker, withdrawer };

    let lockup = Lockup {
        epoch: 1,
        unix_timestamp: 0,
        custodian,
    };

    let stake = create_blank_stake_account(&mut svm, &payer);
    let instruction = ixn::initialize(&stake, &authorized, &lockup);

    // should pass
    process_instruction(&mut svm, &instruction, &no_signers, &payer).unwrap();

    // check that we see what we expect
    let account = get_account(&mut svm, &stake);
    let stake_state: StakeStateV2 = bincode::deserialize(&account.data).unwrap();
    assert_eq!(
        stake_state,
        StakeStateV2::Initialized(Meta {
            authorized,
            rent_exempt_reserve,
            lockup,
        }),
    );

    // 2nd time fails, can't move it from anything other than uninit->init
    refresh_blockhash(&mut svm);
    let e = process_instruction(&mut svm, &instruction, &no_signers, &payer).unwrap_err();
    assert_eq!(e, ProgramError::InvalidAccountData);

    // not enough balance for rent
    let stake = Pubkey::new_unique();
    let account = Account {
        lamports: rent_exempt_reserve / 2,
        data: vec![0; StakeStateV2::size_of()],
        owner: solana_program::stake::program::id(),
        executable: false,
        rent_epoch: 1000,
    };
    svm.set_account(stake, account).unwrap();

    let instruction = ixn::initialize(&stake, &authorized, &lockup);
    let e = process_instruction(&mut svm, &instruction, &no_signers, &payer).unwrap_err();
    assert_eq!(e, ProgramError::InsufficientFunds);

    // incorrect account sizes
    let stake_keypair = Keypair::new();
    let stake = stake_keypair.pubkey();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000_000).unwrap();

    let instruction = system_instruction::create_account(
        &payer.pubkey(),
        &stake,
        rent_exempt_reserve * 2,
        StakeStateV2::size_of() as u64 + 1,
        &solana_program::stake::program::id(),
    );
    process_instruction(&mut svm, &instruction, &vec![&stake_keypair], &payer).unwrap();

    let instruction = ixn::initialize(&stake, &authorized, &lockup);
    let e = process_instruction(&mut svm, &instruction, &no_signers, &payer).unwrap_err();
    assert_eq!(e, ProgramError::InvalidAccountData);

    let stake_keypair = Keypair::new();
    let stake = stake_keypair.pubkey();

    let instruction = system_instruction::create_account(
        &payer.pubkey(),
        &stake,
        rent_exempt_reserve,
        StakeStateV2::size_of() as u64 - 1,
        &solana_program::stake::program::id(),
    );
    process_instruction(&mut svm, &instruction, &vec![&stake_keypair], &payer).unwrap();

    let instruction = ixn::initialize(&stake, &authorized, &lockup);
    let e = process_instruction(&mut svm, &instruction, &no_signers, &payer).unwrap_err();
    assert_eq!(e, ProgramError::InvalidAccountData);
}

#[test]
fn test_authorize() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000_000).unwrap();
    let accounts = Accounts::default();
    accounts.initialize(&mut svm, &payer);

    let rent_exempt_reserve = get_stake_account_rent(&mut svm);
    let no_signers: [&Keypair; 0] = [];

    let stakers: [_; 3] = std::array::from_fn(|_| Keypair::new());
    let withdrawers: [_; 3] = std::array::from_fn(|_| Keypair::new());

    let stake_keypair = Keypair::new();
    let stake = create_blank_stake_account_from_keypair(&mut svm, &stake_keypair, &payer);

    // authorize uninitialized fails
    for (authority, authority_type) in [
        (&stakers[0], StakeAuthorize::Staker),
        (&withdrawers[0], StakeAuthorize::Withdrawer),
    ] {
        let instruction = ixn::authorize(&stake, &stake, &authority.pubkey(), authority_type, None);
        let e =
            process_instruction(&mut svm, &instruction, &vec![&stake_keypair], &payer).unwrap_err();
        assert_eq!(e, ProgramError::InvalidAccountData);
    }

    let authorized = Authorized {
        staker: stakers[0].pubkey(),
        withdrawer: withdrawers[0].pubkey(),
    };

    let instruction = ixn::initialize(&stake, &authorized, &Lockup::default());
    process_instruction(&mut svm, &instruction, &no_signers, &payer).unwrap();

    // changing authority works
    for (old_authority, new_authority, authority_type) in [
        (&stakers[0], &stakers[1], StakeAuthorize::Staker),
        (&withdrawers[0], &withdrawers[1], StakeAuthorize::Withdrawer),
    ] {
        let instruction = ixn::authorize(
            &stake,
            &old_authority.pubkey(),
            &new_authority.pubkey(),
            authority_type,
            None,
        );
        test_instruction_with_missing_signers(&mut svm, &instruction, &vec![old_authority], &payer);

        let (meta, _, _) = get_stake_account(&mut svm, &stake);
        let actual_authority = match authority_type {
            StakeAuthorize::Staker => meta.authorized.staker,
            StakeAuthorize::Withdrawer => meta.authorized.withdrawer,
        };
        assert_eq!(actual_authority, new_authority.pubkey());
    }

    // old authority no longer works
    for (old_authority, new_authority, authority_type) in [
        (&stakers[0], Pubkey::new_unique(), StakeAuthorize::Staker),
        (
            &withdrawers[0],
            Pubkey::new_unique(),
            StakeAuthorize::Withdrawer,
        ),
    ] {
        let instruction = ixn::authorize(
            &stake,
            &old_authority.pubkey(),
            &new_authority,
            authority_type,
            None,
        );
        let e =
            process_instruction(&mut svm, &instruction, &vec![old_authority], &payer).unwrap_err();
        assert_eq!(e, ProgramError::MissingRequiredSignature);
    }

    // changing authority again works
    for (old_authority, new_authority, authority_type) in [
        (&stakers[1], &stakers[2], StakeAuthorize::Staker),
        (&withdrawers[1], &withdrawers[2], StakeAuthorize::Withdrawer),
    ] {
        let instruction = ixn::authorize(
            &stake,
            &old_authority.pubkey(),
            &new_authority.pubkey(),
            authority_type,
            None,
        );
        test_instruction_with_missing_signers(&mut svm, &instruction, &vec![old_authority], &payer);

        let (meta, _, _) = get_stake_account(&mut svm, &stake);
        let actual_authority = match authority_type {
            StakeAuthorize::Staker => meta.authorized.staker,
            StakeAuthorize::Withdrawer => meta.authorized.withdrawer,
        };
        assert_eq!(actual_authority, new_authority.pubkey());
    }

    // changing withdrawer using staker fails
    let instruction = ixn::authorize(
        &stake,
        &stakers[2].pubkey(),
        &Pubkey::new_unique(),
        StakeAuthorize::Withdrawer,
        None,
    );
    let e = process_instruction(&mut svm, &instruction, &vec![&stakers[2]], &payer).unwrap_err();
    assert_eq!(e, ProgramError::MissingRequiredSignature);

    // changing staker using withdrawer is fine
    let instruction = ixn::authorize(
        &stake,
        &withdrawers[2].pubkey(),
        &stakers[0].pubkey(),
        StakeAuthorize::Staker,
        None,
    );
    test_instruction_with_missing_signers(&mut svm, &instruction, &vec![&withdrawers[2]], &payer);

    let (meta, _, _) = get_stake_account(&mut svm, &stake);
    assert_eq!(meta.authorized.staker, stakers[0].pubkey());

    // withdraw using staker fails
    for staker in stakers {
        let recipient = Pubkey::new_unique();
        let instruction = ixn::withdraw(
            &stake,
            &staker.pubkey(),
            &recipient,
            rent_exempt_reserve,
            None,
        );
        let e = process_instruction(&mut svm, &instruction, &vec![&staker], &payer).unwrap_err();
        assert_eq!(e, ProgramError::MissingRequiredSignature);
    }
}

#[test]
fn test_stake_delegate() {
    let mut svm = LiteSVM::new();
    let accounts = Accounts::default();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000_000).unwrap();
    accounts.initialize(&mut svm, &payer);

    let vote_account2 = Keypair::new();
    let latest_blockhash = svm.latest_blockhash();
    create_vote(
        &mut svm,
        &payer,
        &latest_blockhash,
        &Keypair::new(),
        &Pubkey::new_unique(),
        &Pubkey::new_unique(),
        &vote_account2,
    );

    let staker_keypair = Keypair::new();
    let withdrawer_keypair = Keypair::new();

    let staker = staker_keypair.pubkey();
    let withdrawer = withdrawer_keypair.pubkey();

    let authorized = Authorized { staker, withdrawer };

    let vote_state_credits = 100;
    increment_vote_account_credits(&mut svm, accounts.vote_account.pubkey(), vote_state_credits);
    let minimum_delegation = get_minimum_delegation(&mut svm, &payer);

    let stake = create_independent_stake_account(&mut svm, &authorized, minimum_delegation, &payer);
    let instruction = ixn::delegate_stake(&stake, &staker, &accounts.vote_account.pubkey());

    test_instruction_with_missing_signers(&mut svm, &instruction, &vec![&staker_keypair], &payer);

    // verify that delegate() looks right
    let clock = svm.get_sysvar::<Clock>();
    let (_, stake_data, _) = get_stake_account(&mut svm, &stake);
    assert_eq!(
        stake_data.unwrap(),
        Stake {
            delegation: Delegation {
                voter_pubkey: accounts.vote_account.pubkey(),
                stake: minimum_delegation,
                activation_epoch: clock.epoch,
                deactivation_epoch: u64::MAX,
                ..Delegation::default()
            },
            credits_observed: vote_state_credits,
        }
    );

    // verify that delegate fails as stake is active and not deactivating
    advance_epoch(&mut svm);
    let instruction = ixn::delegate_stake(&stake, &staker, &accounts.vote_account.pubkey());
    let e =
        process_instruction(&mut svm, &instruction, &vec![&staker_keypair], &payer).unwrap_err();
    // XXX TODO FIXME pr the fucking stakerror conversion this is driving me insane
    assert_eq!(e, ProgramError::Custom(3));

    // deactivate
    let instruction = ixn::deactivate_stake(&stake, &staker);
    process_instruction(&mut svm, &instruction, &vec![&staker_keypair], &payer).unwrap();

    // verify that delegate to a different vote account fails during deactivation
    let instruction = ixn::delegate_stake(&stake, &staker, &vote_account2.pubkey());
    let e =
        process_instruction(&mut svm, &instruction, &vec![&staker_keypair], &payer).unwrap_err();
    // XXX TODO FIXME pr the fucking stakerror conversion this is driving me insane
    assert_eq!(e, ProgramError::Custom(3));

    // verify that delegate succeeds to same vote account when stake is deactivating
    refresh_blockhash(&mut svm);
    let instruction = ixn::delegate_stake(&stake, &staker, &accounts.vote_account.pubkey());
    process_instruction(&mut svm, &instruction, &vec![&staker_keypair], &payer).unwrap();

    // verify that deactivation has been cleared
    let (_, stake_data, _) = get_stake_account(&mut svm, &stake);
    assert_eq!(stake_data.unwrap().delegation.deactivation_epoch, u64::MAX);

    // verify that delegate to a different vote account fails if stake is still active
    let instruction = ixn::delegate_stake(&stake, &staker, &vote_account2.pubkey());
    let e =
        process_instruction(&mut svm, &instruction, &vec![&staker_keypair], &payer).unwrap_err();
    // XXX TODO FIXME pr the fucking stakerror conversion this is driving me insane
    assert_eq!(e, ProgramError::Custom(3));

    // delegate still fails after stake is fully activated; redelegate is not supported
    advance_epoch(&mut svm);
    let instruction = ixn::delegate_stake(&stake, &staker, &vote_account2.pubkey());
    let e =
        process_instruction(&mut svm, &instruction, &vec![&staker_keypair], &payer).unwrap_err();
    // XXX TODO FIXME pr the fucking stakerror conversion this is driving me insane
    assert_eq!(e, ProgramError::Custom(3));

    // delegate to spoofed vote account fails (not owned by vote program)
    let mut fake_vote_account = get_account(&mut svm, &accounts.vote_account.pubkey());
    fake_vote_account.owner = Pubkey::new_unique();
    let fake_vote_address = Pubkey::new_unique();
    svm.set_account(fake_vote_address, fake_vote_account)
        .unwrap();

    let stake = create_independent_stake_account(&mut svm, &authorized, minimum_delegation, &payer);
    let instruction = ixn::delegate_stake(&stake, &staker, &fake_vote_address);

    let e =
        process_instruction(&mut svm, &instruction, &vec![&staker_keypair], &payer).unwrap_err();
    assert_eq!(e, ProgramError::IncorrectProgramId);

    // delegate stake program-owned non-stake account fails
    let rewards_pool_address = Pubkey::new_unique();
    let rewards_pool = Account {
        lamports: get_stake_account_rent(&mut svm),
        data: bincode::serialize(&StakeStateV2::RewardsPool)
            .unwrap()
            .to_vec(),
        owner: solana_program::stake::program::id(),
        executable: false,
        rent_epoch: u64::MAX,
    };
    svm.set_account(rewards_pool_address, rewards_pool).unwrap();

    let instruction = ixn::delegate_stake(
        &rewards_pool_address,
        &staker,
        &accounts.vote_account.pubkey(),
    );

    let e =
        process_instruction(&mut svm, &instruction, &vec![&staker_keypair], &payer).unwrap_err();
    assert_eq!(e, ProgramError::InvalidAccountData);
}
