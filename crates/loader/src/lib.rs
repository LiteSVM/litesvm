use {
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_loader_v3_interface::{
        instruction as bpf_loader_upgradeable, state::UpgradeableLoaderState,
    },
    solana_signer::Signer,
    solana_transaction::Transaction,
};

const CHUNK_SIZE: usize = 512;

pub fn set_upgrade_authority(
    svm: &mut LiteSVM,
    from_keypair: &Keypair,
    program_address: &Address,
    current_authority_keypair: &Keypair,
    new_authority_address: Option<&Address>,
) -> Result<(), FailedTransactionMetadata> {
    let tx = Transaction::new_signed_with_payer(
        &[bpf_loader_upgradeable::set_upgrade_authority(
            program_address,
            &current_authority_keypair.pubkey(),
            new_authority_address,
        )],
        Some(&from_keypair.pubkey()),
        &[&from_keypair],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)?;

    Ok(())
}

fn load_upgradeable_buffer(
    svm: &mut LiteSVM,
    payer_kp: &Keypair,
    program_bytes: &[u8],
) -> Result<Address, FailedTransactionMetadata> {
    let payer_pk = payer_kp.pubkey();
    let buffer_kp = Keypair::new();
    let buffer_pk = buffer_kp.pubkey();
    // loader
    let buffer_len = UpgradeableLoaderState::size_of_buffer(program_bytes.len());
    let lamports = svm.minimum_balance_for_rent_exemption(buffer_len);

    let tx = Transaction::new_signed_with_payer(
        &bpf_loader_upgradeable::create_buffer(
            &payer_pk,
            &buffer_pk,
            &payer_pk,
            lamports,
            program_bytes.len(),
        )
        .unwrap(),
        Some(&payer_pk),
        &[payer_kp, &buffer_kp],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)?;

    let chunk_size = CHUNK_SIZE;
    let mut offset = 0;
    for chunk in program_bytes.chunks(chunk_size) {
        let tx = Transaction::new_signed_with_payer(
            &[bpf_loader_upgradeable::write(
                &buffer_pk,
                &payer_pk,
                offset,
                chunk.to_vec(),
            )],
            Some(&payer_pk),
            &[payer_kp],
            svm.latest_blockhash(),
        );

        svm.send_transaction(tx)?;
        offset += chunk_size as u32;
    }

    Ok(buffer_pk)
}

pub fn deploy_upgradeable_program(
    svm: &mut LiteSVM,
    payer_kp: &Keypair,
    program_kp: &Keypair,
    program_bytes: &[u8],
) -> Result<(), FailedTransactionMetadata> {
    let program_pk = program_kp.pubkey();
    let payer_pk = payer_kp.pubkey();
    let buffer_pk = load_upgradeable_buffer(svm, payer_kp, program_bytes)?;

    let lamports = svm.minimum_balance_for_rent_exemption(program_bytes.len());
    #[allow(deprecated)]
    let tx = Transaction::new_signed_with_payer(
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
        &[&payer_kp, &program_kp],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)?;

    Ok(())
}
