#![cfg(feature = "invocation-inspect-callback")]

use {
    litesvm::{InvocationInspectCallback, LiteSVM},
    solana_account::ReadableAccount,
    solana_address::Address,
    solana_instruction_error::InstructionError,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_program_runtime::invoke_context::InvokeContext,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::{sanitized::SanitizedTransaction, Transaction},
    solana_transaction_context::IndexOfAccount,
    std::sync::{Arc, Mutex},
};

/// `(instruction index, fee payer lamports after it, succeeded, compute units)`.
type Observation = (usize, u64, bool, u64);

/// Records the fee payer's lamports after every top-level instruction.
struct BalanceAfterEachInstruction {
    seen: Arc<Mutex<Vec<Observation>>>,
}

impl InvocationInspectCallback for BalanceAfterEachInstruction {
    fn before_invocation(
        &self,
        _: &LiteSVM,
        _: &SanitizedTransaction,
        _: &[IndexOfAccount],
        _: &mut InvokeContext,
        _: bool,
    ) {
    }

    fn after_invocation(
        &self,
        _: &LiteSVM,
        _: &SanitizedTransaction,
        _: &[IndexOfAccount],
        _: &InvokeContext,
        _: bool,
    ) {
    }

    fn after_instruction(
        &self,
        _: &LiteSVM,
        _: &SanitizedTransaction,
        instruction_index: usize,
        invoke_context: &InvokeContext,
        result: &Result<(), InstructionError>,
        compute_units_consumed: u64,
    ) {
        // Account 0 is the fee payer in this test's message.
        let lamports = invoke_context
            .transaction_context
            .accounts()
            .try_borrow(0)
            .unwrap()
            .lamports();
        self.seen.lock().unwrap().push((
            instruction_index,
            lamports,
            result.is_ok(),
            compute_units_consumed,
        ));
    }
}

#[test]
fn after_instruction_sees_state_between_instructions() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let a = Address::new_unique();
    let b = Address::new_unique();
    let start = 1_000_000_000u64;
    svm.airdrop(&payer.pubkey(), start).unwrap();

    // Install the hook after the airdrop, which is itself a transaction.
    let seen = Arc::new(Mutex::new(Vec::new()));
    svm.set_invocation_inspect_callback(BalanceAfterEachInstruction {
        seen: Arc::clone(&seen),
    });

    // Two transfers in one transaction. The hook must see the balance after
    // the first one and before the second one, which no post-transaction
    // view can show.
    let (first, second) = (100_000_000u64, 250_000_000u64);
    let msg = Message::new(
        &[
            transfer(&payer.pubkey(), &a, first),
            transfer(&payer.pubkey(), &b, second),
        ],
        Some(&payer.pubkey()),
    );
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());
    let meta = svm.send_transaction(tx).unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "one observation per top-level instruction");
    let fee = meta.fee;
    let (i0, after_first, ok0, _) = seen[0];
    let (i1, after_second, ok1, _) = seen[1];
    assert_eq!((i0, i1), (0, 1));
    assert!(ok0 && ok1);
    assert_eq!(after_first, start - fee - first);
    assert_eq!(after_second, start - fee - first - second);
    assert_eq!(svm.get_balance(&payer.pubkey()).unwrap(), after_second);
}

#[test]
fn after_instruction_reports_the_failing_instruction() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let a = Address::new_unique();
    svm.airdrop(&payer.pubkey(), 10_000_000).unwrap();

    let seen = Arc::new(Mutex::new(Vec::new()));
    svm.set_invocation_inspect_callback(BalanceAfterEachInstruction {
        seen: Arc::clone(&seen),
    });

    // Second transfer exceeds the balance: instruction 1 fails, 0 succeeded.
    let msg = Message::new(
        &[
            transfer(&payer.pubkey(), &a, 1_000_000),
            transfer(&payer.pubkey(), &a, 1_000_000_000),
        ],
        Some(&payer.pubkey()),
    );
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());
    let failed = svm.send_transaction(tx).unwrap_err();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen[0].2, "first instruction succeeded");
    assert!(!seen[1].2, "second instruction failed");
    // The hook saw the first transfer applied mid-transaction…
    assert_eq!(seen[0].1, 10_000_000 - failed.meta.fee - 1_000_000);
    // …but the failed transaction commits nothing except the fee.
    assert_eq!(
        svm.get_balance(&payer.pubkey()).unwrap(),
        10_000_000 - failed.meta.fee
    );
}
