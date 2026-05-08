use {
    litesvm::{
        MagicSVM, TransactionTarget, DELEGATION_PROGRAM_ID, MAGIC_CONTEXT_ID, MAGIC_PROGRAM_ID,
    },
    solana_account::Account,
    solana_address::{address, Address},
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_native_token::LAMPORTS_PER_SOL,
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::path::PathBuf,
};

const DELEGATED_COUNTER_PROGRAM_ID: Address =
    address!("DCntr1hZ6D66VJwY9WQ8UXux1Jdd7EavqrRztdr7RrQk");
const COUNTER_SEED: &[u8] = b"counter";

fn read_delegated_counter_program() -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/delegated_counter.so");
    std::fs::read(so_path).unwrap()
}

fn increment_counter_tx(
    counter_address: Address,
    payer: &Keypair,
    blockhash: solana_hash::Hash,
) -> Transaction {
    let instruction = Instruction {
        program_id: DELEGATED_COUNTER_PROGRAM_ID,
        accounts: vec![AccountMeta::new(counter_address, false)],
        data: vec![0],
    };
    Transaction::new(
        &[payer],
        Message::new(&[instruction], Some(&payer.pubkey())),
        blockhash,
    )
}

fn delegation_pda(seed: &[u8], delegated_account: &Address) -> Address {
    Address::find_program_address(&[seed, delegated_account.as_ref()], &DELEGATION_PROGRAM_ID).0
}

fn delegate_counter_tx(
    counter_address: Address,
    payer: &Keypair,
    blockhash: solana_hash::Hash,
) -> Transaction {
    let buffer = Address::find_program_address(
        &[b"buffer", counter_address.as_ref()],
        &DELEGATED_COUNTER_PROGRAM_ID,
    )
    .0;
    let delegation_record = delegation_pda(b"delegation", &counter_address);
    let delegation_metadata = delegation_pda(b"delegation-metadata", &counter_address);
    let instruction = Instruction {
        program_id: DELEGATED_COUNTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(counter_address, false),
            AccountMeta::new_readonly(DELEGATED_COUNTER_PROGRAM_ID, false),
            AccountMeta::new(buffer, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            AccountMeta::new_readonly(DELEGATION_PROGRAM_ID, false),
        ],
        data: vec![1],
    };
    Transaction::new(
        &[payer],
        Message::new(&[instruction], Some(&payer.pubkey())),
        blockhash,
    )
}

fn undelegate_counter_tx(
    counter_address: Address,
    payer: &Keypair,
    blockhash: solana_hash::Hash,
) -> Transaction {
    let instruction = Instruction {
        program_id: DELEGATED_COUNTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(counter_address, false),
            AccountMeta::new_readonly(MAGIC_PROGRAM_ID, false),
            AccountMeta::new(MAGIC_CONTEXT_ID, false),
        ],
        data: vec![2],
    };
    Transaction::new(
        &[payer],
        Message::new(&[instruction], Some(&payer.pubkey())),
        blockhash,
    )
}

#[test_log::test]
fn delegated_counter_increments_on_ephemeral_and_commits_to_base() {
    let payer = Keypair::new();
    let mut svm = MagicSVM::new();
    let (counter_address, _) =
        Address::find_program_address(&[COUNTER_SEED], &DELEGATED_COUNTER_PROGRAM_ID);
    svm.add_program(
        DELEGATED_COUNTER_PROGRAM_ID,
        &read_delegated_counter_program(),
    )
    .unwrap();
    svm.airdrop(&payer.pubkey(), LAMPORTS_PER_SOL).unwrap();
    svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: DELEGATED_COUNTER_PROGRAM_ID,
            ..Default::default()
        },
    )
    .unwrap();

    for _ in 0..2 {
        let tx = increment_counter_tx(
            counter_address,
            &payer,
            svm.latest_blockhash_for(TransactionTarget::Base),
        );
        svm.send_transaction_to(TransactionTarget::Base, tx)
            .unwrap();
        svm.expire_blockhash_for(TransactionTarget::Base);
    }

    let tx = delegate_counter_tx(
        counter_address,
        &payer,
        svm.latest_blockhash_for(TransactionTarget::Base),
    );
    svm.send_transaction_to(TransactionTarget::Base, tx)
        .unwrap();

    for _ in 0..2 {
        let tx = increment_counter_tx(
            counter_address,
            &payer,
            svm.latest_blockhash_for(TransactionTarget::Ephemeral),
        );
        svm.send_transaction_to(TransactionTarget::Ephemeral, tx)
            .unwrap();
        svm.expire_blockhash_for(TransactionTarget::Ephemeral);
    }

    let tx = undelegate_counter_tx(
        counter_address,
        &payer,
        svm.latest_blockhash_for(TransactionTarget::Ephemeral),
    );
    svm.send_transaction_to(TransactionTarget::Ephemeral, tx)
        .unwrap();

    assert_eq!(
        svm.get_account(&counter_address).unwrap().data,
        4u32.to_le_bytes().to_vec()
    );
}
