use solana_sdk::{
    bpf_loader,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    loader_instruction,
    message::Message,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
};

use crate::{bank::LiteSVM, types::FailedTransactionMetadata};

const CHUNK_SIZE: usize = 512;

fn deploy_program(
    bank: &mut LiteSVM,
    payer_keypair: &Keypair,
    program_bytes: &[u8],
) -> Result<Pubkey, FailedTransactionMetadata> {
    let program_keypair = Keypair::new();
    let instruction = system_instruction::create_account(
        &payer_keypair.pubkey(),
        &program_keypair.pubkey(),
        bank.minimum_balance_for_rent_exemption(program_bytes.len()),
        program_bytes.len() as u64,
        &bpf_loader::id(),
    );
    let message = Message::new(&[instruction], Some(&payer_keypair.pubkey()));
    bank.send_message(message, &[payer_keypair, &program_keypair])?;

    let chunk_size = CHUNK_SIZE;
    let mut offset = 0;
    for chunk in program_bytes.chunks(chunk_size) {
        let instruction = loader_instruction::write(
            &program_keypair.pubkey(),
            &bpf_loader::id(),
            offset,
            chunk.to_vec(),
        );
        let message = Message::new(&[instruction], Some(&payer_keypair.pubkey()));
        bank.send_message(message, &[payer_keypair, &program_keypair])?;
        offset += chunk_size as u32;
    }
    let instruction = loader_instruction::finalize(&program_keypair.pubkey(), &bpf_loader::id());
    let message: Message = Message::new(&[instruction], Some(&payer_keypair.pubkey()));
    bank.send_message(message, &[payer_keypair, &program_keypair])?;

    Ok(program_keypair.pubkey())
}

fn set_upgrade_authority(
    bank: &mut LiteSVM,
    from_keypair: &Keypair,
    program_pubkey: &Pubkey,
    current_authority_keypair: &Keypair,
    new_authority_pubkey: Option<&Pubkey>,
) -> Result<(), FailedTransactionMetadata> {
    let message = Message::new(
        &[bpf_loader_upgradeable::set_upgrade_authority(
            program_pubkey,
            &current_authority_keypair.pubkey(),
            new_authority_pubkey,
        )],
        Some(&from_keypair.pubkey()),
    );
    bank.send_message(message, &[&from_keypair])?;

    Ok(())
}

fn load_upgradeable_buffer(
    bank: &mut LiteSVM,
    payer_kp: &Keypair,
    program_bytes: &[u8],
) -> Result<Pubkey, FailedTransactionMetadata> {
    let payer_pk = payer_kp.pubkey();
    let buffer_kp = Keypair::new();
    let buffer_pk = buffer_kp.pubkey();
    // loader
    let buffer_len = UpgradeableLoaderState::size_of_buffer(program_bytes.len());
    let lamports = bank.minimum_balance_for_rent_exemption(buffer_len);

    let message = Message::new(
        &bpf_loader_upgradeable::create_buffer(
            &payer_pk,
            &buffer_pk,
            &payer_pk,
            lamports,
            program_bytes.len(),
        )
        .unwrap(),
        Some(&payer_pk),
    );
    bank.send_message(message, &[payer_kp, &buffer_kp])?;

    let chunk_size = CHUNK_SIZE;
    let mut offset = 0;
    for chunk in program_bytes.chunks(chunk_size) {
        let message = Message::new(
            &[bpf_loader_upgradeable::write(
                &buffer_pk,
                &payer_pk,
                offset,
                chunk.to_vec(),
            )],
            Some(&payer_pk),
        );
        bank.send_message(message, &[payer_kp])?;
        offset += chunk_size as u32;
    }

    Ok(buffer_pk)
}

fn deploy_upgradeable_program(
    bank: &mut LiteSVM,
    payer_kp: &Keypair,
    program_bytes: &[u8],
) -> Result<Pubkey, FailedTransactionMetadata> {
    let program_kp = Keypair::new();
    let program_pk = program_kp.pubkey();
    let payer_pk = payer_kp.pubkey();
    let buffer_pk = load_upgradeable_buffer(bank, payer_kp, program_bytes)?;

    let lamports = bank.minimum_balance_for_rent_exemption(program_bytes.len());
    let message = Message::new(
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
    );
    bank.send_message(message, &[payer_kp, &program_kp])?;

    Ok(program_pk)
}

pub trait Loader {
    fn deploy_program(
        &mut self,
        payer_keypair: &Keypair,
        program_bytes: &[u8],
    ) -> Result<Pubkey, FailedTransactionMetadata>;

    fn set_upgrade_authority(
        &mut self,
        from_keypair: &Keypair,
        program_pubkey: &Pubkey,
        current_authority_keypair: &Keypair,
        new_authority_pubkey: Option<&Pubkey>,
    ) -> Result<(), FailedTransactionMetadata>;

    fn deploy_upgradeable_program(
        &mut self,
        payer_kp: &Keypair,
        program_bytes: &[u8],
    ) -> Result<Pubkey, FailedTransactionMetadata>;
}

impl Loader for LiteSVM {
    fn deploy_program(
        &mut self,
        payer_keypair: &Keypair,
        program_bytes: &[u8],
    ) -> Result<Pubkey, FailedTransactionMetadata> {
        deploy_program(self, payer_keypair, program_bytes)
    }

    fn deploy_upgradeable_program(
        &mut self,
        payer_kp: &Keypair,
        program_bytes: &[u8],
    ) -> Result<Pubkey, FailedTransactionMetadata> {
        deploy_upgradeable_program(self, payer_kp, program_bytes)
    }

    fn set_upgrade_authority(
        &mut self,
        from_keypair: &Keypair,
        program_pubkey: &Pubkey,
        current_authority_keypair: &Keypair,
        new_authority_pubkey: Option<&Pubkey>,
    ) -> Result<(), FailedTransactionMetadata> {
        set_upgrade_authority(
            self,
            from_keypair,
            program_pubkey,
            current_authority_keypair,
            new_authority_pubkey,
        )
    }
}
