// Run with: cargo test --test cpi_trees -- --nocapture
//
// Exercises each fixture program and prints its CPI tree for visual inspection.

use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::{address, Address},
    solana_clock::Clock,
    solana_keypair::Keypair,
    solana_message::{Instruction, Message},
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::path::PathBuf,
};

fn read_program(file: &str) -> Vec<u8> {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push(format!("test_programs/target/deploy/{file}"));
    std::fs::read(p).unwrap()
}

fn dump(label: &str, tree: String) {
    println!("\n==== {label} ====");
    print!("{tree}");
}

#[test]
fn fixture_counter() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let program_id = address!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.add_program(program_id, &read_program("counter.so"))
        .unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let counter_address = Address::new_unique();
    svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; 4],
            owner: program_id,
            ..Default::default()
        },
    )
    .unwrap();
    let tx = Transaction::new(
        &[&payer],
        Message::new_with_blockhash(
            &[Instruction {
                program_id,
                accounts: vec![solana_message::AccountMeta::new(counter_address, false)],
                data: vec![0, 0],
            }],
            Some(&payer.pubkey()),
            &svm.latest_blockhash(),
        ),
        svm.latest_blockhash(),
    );
    let meta = svm.send_transaction(tx).unwrap();
    let tree = meta.pretty_cpi_tree();
    // Real end-to-end output: header with CU totals, then the program frame.
    assert!(
        tree.starts_with("CPI Tree (") && tree.contains(" BPF CU / "),
        "missing CU header: {tree}"
    );
    assert!(
        tree.contains(&program_id.to_string()),
        "missing frame: {tree}"
    );
    dump("counter (success)", tree);
}

#[test]
fn fixture_failure() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let program_id = address!("HvrRMSshMx3itvsyWDnWg2E3cy5h57iMaR7oVxSZJDSA");
    svm.add_program(program_id, &read_program("failure.so"))
        .unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let tx = Transaction::new(
        &[&payer],
        Message::new_with_blockhash(
            &[Instruction {
                program_id,
                accounts: vec![],
                data: vec![],
            }],
            Some(&payer.pubkey()),
            &svm.latest_blockhash(),
        ),
        svm.latest_blockhash(),
    );
    let failed = svm.send_transaction(tx).unwrap_err();
    let tree = failed.meta.pretty_cpi_tree();
    // The failure must be attributed on the frame line, not lost.
    assert!(tree.contains("FAILED:"), "missing failure marker: {tree}");
    dump("failure (custom error 0)", tree);
}

#[test]
fn fixture_clock_example() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    // Program asserts clock.unix_timestamp < 100
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = 50;
    svm.set_sysvar::<Clock>(&clock);
    let program_id = address!("1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM");
    svm.add_program(program_id, &read_program("litesvm_clock_example.so"))
        .unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let tx = Transaction::new(
        &[&payer],
        Message::new_with_blockhash(
            &[Instruction {
                program_id,
                accounts: vec![],
                data: vec![],
            }],
            Some(&payer.pubkey()),
            &svm.latest_blockhash(),
        ),
        svm.latest_blockhash(),
    );
    let meta = svm.send_transaction(tx).unwrap();
    let tree = meta.pretty_cpi_tree();
    assert!(
        tree.starts_with("CPI Tree (") && tree.contains(&program_id.to_string()),
        "unexpected tree: {tree}"
    );
    dump("clock-example (success)", tree);
}
