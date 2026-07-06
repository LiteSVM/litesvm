use {
    litesvm::LiteSVM,
    solana_address::Address,
    solana_keypair::Keypair,
    solana_message::{Instruction, Message},
    solana_native_token::LAMPORTS_PER_SOL,
    solana_program_runtime::{
        declare_process_instruction, solana_sbpf::program::BuiltinFunctionDefinition,
    },
    solana_signer::Signer,
    solana_transaction::Transaction,
};

declare_process_instruction!(EmptyBuiltin, 1, |invoke_context| { Ok(()) });

impl EmptyBuiltin {
    pub const ID: Address = Address::from_str_const("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
}

#[test_log::test]
fn test_add_builtin() {
    let payer_keypair = Keypair::new();
    let payer = payer_keypair.pubkey();

    let mut svm = LiteSVM::new();

    svm.add_builtin(EmptyBuiltin::ID, EmptyBuiltin::register);

    svm.airdrop(&payer, 10 * LAMPORTS_PER_SOL).unwrap();
    let tx = Transaction::new(
        &[&payer_keypair],
        Message::new(
            &[Instruction {
                program_id: EmptyBuiltin::ID,
                accounts: vec![],
                data: vec![],
            }],
            Some(&payer),
        ),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);
    assert!(tx_res.is_ok())
}
