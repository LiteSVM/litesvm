#![allow(clippy::result_large_err)]
use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    message::Message,
};
use solana_sdk::{
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    message::VersionedMessage,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::{Transaction, VersionedTransaction},
};

use crate::programs_bytes::HELLO_WORLD_BYTES;

mod programs_bytes;

const CHUNK_SIZE: usize = 512;

fn set_upgrade_authority(
    svm: &mut LiteSVM,
    from_keypair: &Keypair,
    program_pubkey: &Pubkey,
    current_authority_keypair: &Keypair,
    new_authority_pubkey: Option<&Pubkey>,
) -> Result<(), FailedTransactionMetadata> {
    let message = Message::new_with_blockhash(
        &[bpf_loader_upgradeable::set_upgrade_authority(
            program_pubkey,
            &current_authority_keypair.pubkey(),
            new_authority_pubkey,
        )],
        Some(&from_keypair.pubkey()),
        &svm.latest_blockhash(),
    );
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[from_keypair]).unwrap();
    svm.send_transaction(tx)?;

    Ok(())
}

fn load_upgradeable_buffer(
    svm: &mut LiteSVM,
    payer_kp: &Keypair,
    program_bytes: &[u8],
) -> Result<Pubkey, FailedTransactionMetadata> {
    let payer_pk = payer_kp.pubkey();
    let buffer_kp = Keypair::new();
    let buffer_pk = buffer_kp.pubkey();
    // loader
    let buffer_len = UpgradeableLoaderState::size_of_buffer(program_bytes.len());
    let lamports = svm.minimum_balance_for_rent_exemption(buffer_len);

    let message = Message::new_with_blockhash(
        &bpf_loader_upgradeable::create_buffer(
            &payer_pk,
            &buffer_pk,
            &payer_pk,
            lamports,
            program_bytes.len(),
        )
        .unwrap(),
        Some(&payer_pk),
        &svm.latest_blockhash(),
    );
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[payer_kp, &buffer_kp])
            .unwrap();
    svm.send_transaction(tx)?;

    let chunk_size = CHUNK_SIZE;
    let mut offset = 0;
    for chunk in program_bytes.chunks(chunk_size) {
        let message = Message::new_with_blockhash(
            &[bpf_loader_upgradeable::write(
                &buffer_pk,
                &payer_pk,
                offset,
                chunk.to_vec(),
            )],
            Some(&payer_pk),
            &svm.latest_blockhash(),
        );
        let tx =
            VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[payer_kp]).unwrap();
        svm.send_transaction(tx)?;
        offset += chunk_size as u32;
    }

    Ok(buffer_pk)
}

fn deploy_upgradeable_program(
    svm: &mut LiteSVM,
    payer_kp: &Keypair,
    program_kp: &Keypair,
    program_bytes: &[u8],
) -> Result<(), FailedTransactionMetadata> {
    let program_pk = program_kp.pubkey();
    let payer_pk = payer_kp.pubkey();
    let buffer_pk = load_upgradeable_buffer(svm, payer_kp, program_bytes)?;

    let lamports = svm.minimum_balance_for_rent_exemption(program_bytes.len());
    let message = Message::new_with_blockhash(
        &bpf_loader_upgradeable::deploy_with_max_program_len(
            &payer_pk,
            &program_pk,
            &buffer_pk,
            &payer_pk,
            lamports,
            program_bytes.len() * 2,
        )
        .unwrap(),
        Some(&payer_pk),
        &svm.latest_blockhash(),
    );
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[payer_kp, &program_kp])
            .unwrap();
    svm.send_transaction(tx)?;

    Ok(())
}

#[test]
fn hello_world_with_store() {
    let mut svm = LiteSVM::new();

    let payer = Keypair::new();
    let program_bytes = HELLO_WORLD_BYTES;

    svm.airdrop(&payer.pubkey(), 1000000000).unwrap();

    let program_kp = Keypair::new();
    let program_id = program_kp.pubkey();
    svm.add_program(program_id, program_bytes);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &[],
        vec![AccountMeta::new(payer.pubkey(), true)],
    );
    let message = Message::new(&[instruction], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], message, svm.latest_blockhash());
    let tx_result = svm.send_transaction(tx);

    assert!(tx_result.is_ok());
    assert!(tx_result
        .unwrap()
        .logs
        .contains(&"Program log: Hello world!".to_string()));
}

#[test_log::test]
fn hello_world_with_deploy_upgradeable() {
    let mut svm = LiteSVM::new();

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_bytes = HELLO_WORLD_BYTES;

    svm.airdrop(&payer_pk, 10000000000).unwrap();

    let program_keypair = Keypair::new();
    deploy_upgradeable_program(&mut svm, &payer_kp, &program_keypair, program_bytes).unwrap();
    let program_id = program_keypair.pubkey();
    let instruction =
        Instruction::new_with_bytes(program_id, &[], vec![AccountMeta::new(payer_pk, true)]);
    let message = Message::new(&[instruction], Some(&payer_pk));
    let tx = Transaction::new(&[&payer_kp], message, svm.latest_blockhash());
    let tx_result = svm.send_transaction(tx);
    assert!(tx_result
        .unwrap()
        .logs
        .contains(&"Program log: Hello world!".to_string()));
    let new_authority = Keypair::new();
    set_upgrade_authority(
        &mut svm,
        &payer_kp,
        &program_id,
        &payer_kp,
        Some(&new_authority.pubkey()),
    )
    .unwrap();
}
