// use {
//     jupnet_sdk::{
//         hash::Hash,
//         message::Message,
//         pubkey::Pubkey,
//         rent::Rent,
//         signer::{keypair::Keypair, Signer},
//         system_instruction::transfer,
//         transaction::{Transaction, TransactionError},
//     },
//     litesvm::LiteSVM,
// };

// #[test_log::test]
// fn test_invalid_blockhash() {
//     let from_keypair = Keypair::new();
//     let from = from_keypair.pubkey();
//     let to = Pubkey::new_unique();

//     let mut svm = LiteSVM::new();

//     svm.airdrop(&from, svm.get_sysvar::<Rent>().minimum_balance(0))
//         .unwrap();
//     let instruction = transfer(&from, &to, 1);
//     let tx = Transaction::new(
//         &[&from_keypair],
//         Message::new(&[instruction], Some(&from)),
//         Hash::new_unique(),
//     );
//     let tx_res = svm.send_transaction(tx);

//     assert_eq!(tx_res.unwrap_err().err, TransactionError::BlockhashNotFound);
// }
