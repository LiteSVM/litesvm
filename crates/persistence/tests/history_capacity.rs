use {
    litesvm::LiteSVM,
    litesvm_persistence::{from_bytes, to_bytes},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::Transaction,
};

#[test]
fn configured_capacity_survives_empty_and_populated_snapshots() {
    for capacity in [1, 4, 33] {
        let payer = Keypair::new();
        let recipient = Address::new_unique();
        let mut svm = LiteSVM::new().with_transaction_history(0);
        svm.airdrop(&payer.pubkey(), 10_000_000).unwrap();
        svm.airdrop(&recipient, 1_000_000).unwrap();
        svm = svm.with_transaction_history(capacity);
        for populated in [false, true] {
            let mut restored = from_bytes(&to_bytes(&svm).unwrap()).unwrap();
            assert_eq!(restored.transaction_history_capacity(), capacity);
            let mut signatures = Vec::new();
            for amount in 1..=capacity as u64 + 1 {
                let tx = Transaction::new_signed_with_payer(
                    &[transfer(&payer.pubkey(), &recipient, amount)],
                    Some(&payer.pubkey()),
                    &[&payer],
                    restored.latest_blockhash(),
                );
                signatures.push(restored.send_transaction(tx).unwrap().signature);
            }
            assert!(restored.get_transaction(&signatures[0]).is_none());
            for signature in &signatures[1..] {
                assert!(restored.get_transaction(signature).is_some());
            }
            assert_eq!(restored.transaction_history_capacity(), capacity);
            if !populated {
                svm = restored;
                svm.expire_blockhash();
            }
        }
    }
}
