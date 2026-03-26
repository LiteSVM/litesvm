// use {
//     jupnet_sdk::{
//         compute_budget::ComputeBudgetInstruction,
//         hash::Hash,
//         instruction::{CompiledInstruction, Instruction},
//         message::{Message, MessageHeader},
//         pubkey::Pubkey,
//         signature::{Keypair, TypedSignature},
//         signer::Signer,
//         system_program,
//         transaction::{Transaction, TransactionError},
//     },
//     litesvm::LiteSVM,
// };

// #[test]
// fn test_account_loaded_twice() {
//     let mut svm = LiteSVM::new();
//     let payer_kp = Keypair::new();
//     let payer_pk = payer_kp.pubkey();
//     svm.airdrop(&payer_pk, 1_000_000_000).unwrap();

//     let duplicate_account = Pubkey::new_unique();

//     let data = bincode::serialize(
//         &jupnet_sdk::system_instruction::SystemInstruction::Transfer { lamports: 500_000 },
//     )
//     .unwrap();
//     let mut tx = Transaction {
//         signatures: vec![TypedSignature::default()],
//         message: Message {
//             header: MessageHeader {
//                 num_required_signatures: 1,
//                 num_readonly_signed_accounts: 0,
//                 num_readonly_unsigned_accounts: 1,
//             },
//             account_keys: vec![
//                 payer_pk,
//                 duplicate_account,
//                 duplicate_account,
//                 system_program::id(),
//             ],
//             recent_blockhash: Hash::default(),
//             instructions: vec![
//                 CompiledInstruction {
//                     program_id_index: 3,
//                     accounts: [0, 1].to_vec(),
//                     data: data.clone(),
//                 },
//                 CompiledInstruction {
//                     program_id_index: 3,
//                     accounts: [0, 2].to_vec(),
//                     data: data.clone(),
//                 },
//             ],
//         },
//     };

//     tx.sign(&[&payer_kp], svm.latest_blockhash());

//     let result = svm.send_transaction(tx);

//     assert_eq!(
//         result.unwrap_err().err,
//         TransactionError::AccountLoadedTwice,
//         "Expected AccountLoadedTwice error when same account is both writable and read-only"
//     );
// }

// #[test]
// fn test_too_many_account_locks() {
//     use jupnet_sdk::system_instruction::transfer;

//     let mut svm = LiteSVM::new();
//     let payer_kp = Keypair::new();
//     let payer_pk = payer_kp.pubkey();
//     svm.airdrop(&payer_pk, 1_000_000_000_000).unwrap();

//     let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);
//     let mut instructions: Vec<Instruction> = vec![compute_budget_ix];
//     for _ in 0..64 {
//         let recipient = Pubkey::new_unique();
//         let ix = transfer(&payer_pk, &recipient, 1_000_000);
//         instructions.push(ix);
//     }

//     let tx = Transaction::new(
//         &[&payer_kp],
//         Message::new(&instructions, Some(&payer_pk)),
//         svm.latest_blockhash(),
//     );

//     let result = svm.send_transaction(tx);

//     assert_eq!(
//         result.unwrap_err().err,
//         TransactionError::TooManyAccountLocks,
//         "Expected TooManyAccountLocks error when transaction has more than 64 accounts"
//     );
// }
