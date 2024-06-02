use litesvm::LiteSVM;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction::create_account,
    system_program,
};

pub fn create_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
    keypair: Option<Keypair>,
    program_id: Option<Pubkey>,
) {
    let keypair = keypair.unwrap_or(Keypair::new());
    create_account(payer.pubkey(), &keypair.pubkey(), lamports, space, owner);

    // crea
}
