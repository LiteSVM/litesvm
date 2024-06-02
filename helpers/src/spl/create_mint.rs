use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{
    program_pack::Pack, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_instruction::create_account, transaction::Transaction,
};
use spl_token_2022::{instruction::initialize_mint2, state::Mint};

pub fn create_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Pubkey,
    token_program_id: Option<Pubkey>,
) -> Result<Pubkey, FailedTransactionMetadata> {
    let mint_size = Mint::LEN;
    let mint_kp = Keypair::new();
    let mint_pk = mint_kp.pubkey();
    let ix1 = create_account(
        authority,
        &mint_pk,
        svm.minimum_balance_for_rent_exemption(mint_size),
        mint_size as u64,
        &spl_token_2022::ID,
    );
    let ix2 = initialize_mint2(
        &token_program_id.unwrap_or(spl_token_2022::ID),
        &mint_pk,
        authority,
        None,
        8,
    )?;

    let block_hash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix1, ix2],
        Some(&payer.pubkey()),
        &[&payer, &mint_kp],
        block_hash,
    );
    svm.send_transaction(tx)?;

    Ok(mint_pk)
}
