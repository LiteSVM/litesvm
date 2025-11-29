use {
    litesvm::LiteSVM,
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

#[test_log::test]
fn sysvar_accounts_are_demoted_to_readonly() {
    let payer = Keypair::new();
    let rent_key = solana_sdk_ids::sysvar::rent::id();
    let ix = Instruction {
        program_id: solana_sdk_ids::system_program::id(),
        accounts: vec![AccountMeta {
            pubkey: rent_key,
            is_signer: false,
            is_writable: true,
        }],
        data: vec![],
    };
    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&payer]).unwrap();

    let sanitized = LiteSVM::new().sanitize_transaction_for_tests(tx);

    assert!(!sanitized.message().is_writable(1));
}
