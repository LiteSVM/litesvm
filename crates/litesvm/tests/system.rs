use {
    deadpool_redis::redis::pipe,
    litesvm::LiteSVM,
    solana_hash::Hash,
    solana_keypair::Keypair,
    solana_message::{v0, Message, VersionedMessage},
    solana_native_token::LAMPORTS_PER_SOL,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_system_interface::instruction::{create_account, transfer},
    solana_transaction::{versioned::VersionedTransaction, Transaction},
};

#[test_log::test]
fn system_transfer() {
    let redis_urls = vec![
        "redis://default:@127.0.0.1:6379".to_string(),
        "redis://default:@127.0.0.1:6380".to_string(),
        "redis://default:@127.0.0.1:6381".to_string(),
        "redis://default:@127.0.0.1:6382".to_string(),
        "redis://default:@127.0.0.1:6383".to_string(),
        "redis://default:@127.0.0.1:6384".to_string(),
    ];
    use deadpool_redis::cluster::{Config, Runtime};
    let cfg = Config::from_urls(redis_urls);
    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
    let mut svm = LiteSVM::new()
        .with_sigverify(true)
        .with_lamports(1e9 as u64 * LAMPORTS_PER_SOL)
        .with_transaction_history(0)
        .with_blockhash_check(false);

    let count = 1e6 as u64; // 1 billion lamports
    let mut txs = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let from_keypair = Keypair::new();
        let from = from_keypair.pubkey();
        let to = Pubkey::new_unique();
        svm.airdrop(&from, 1 * LAMPORTS_PER_SOL).unwrap();
        let instruction = transfer(&from, &to, 64);
        let message =
            v0::Message::try_compile(&from, &[instruction], &[], Hash::new_unique()).unwrap();
        let versioned_message = VersionedMessage::V0(message);

        let versioned_tx =
            VersionedTransaction::try_new(versioned_message, &[from_keypair]).unwrap();
        txs.push(versioned_tx);
    }
    let now = std::time::Instant::now();
    for tx in txs {
        svm.send_transaction(tx).unwrap();
    }

    // Flush all accounts to Redis
    let all_accounts: Vec<(String, Vec<u8>)> = svm.get_all_accounts();
    let mut pipeline = pipe();
    let mut counter = 0;
    let mut payload = 0;
    for (pubkey, account) in all_accounts.iter() {
        pipeline.set(format!("{{1}}:{pubkey}"), account);
        counter += 1;
        payload += account.len();
    }
    println!(
        "Total accounts: {}, counter: {counter}, total payload: {payload} bytes",
        all_accounts.len()
    );
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let mut conn = pool.get().await.unwrap();
            pipeline.query_async::<()>(&mut conn).await.unwrap();
        });

    let elapsed = now.elapsed();
    println!(
        "Elapsed time for 1M transfers: {:?} tx per second",
        count as f64 / elapsed.as_secs_f64()
    );
}

#[test_log::test]
fn system_create_account() {
    let from_keypair = Keypair::new();
    let new_account = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    let expected_fee = 5000 * 2; // two signers
    let space = 10;
    let rent_amount = svm.minimum_balance_for_rent_exemption(space);
    let lamports = rent_amount + expected_fee;
    svm.airdrop(&from, lamports).unwrap();

    let instruction = create_account(
        &from,
        &new_account.pubkey(),
        rent_amount,
        space as u64,
        &solana_sdk_ids::system_program::id(),
    );
    let tx = Transaction::new(
        &[&from_keypair, &new_account],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    let account = svm.get_account(&new_account.pubkey()).unwrap();

    assert_eq!(account.lamports, rent_amount);
    assert_eq!(account.data.len(), space);
    assert_eq!(account.owner, solana_sdk_ids::system_program::id());
}
