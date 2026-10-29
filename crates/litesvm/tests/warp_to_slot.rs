use {litesvm::LiteSVM, solana_clock::Clock};

fn assert_warp_sets_only_slot(svm: &mut LiteSVM, before: Clock, target: u64) {
    svm.warp_to_slot(target);
    let after = svm.get_sysvar::<Clock>();

    assert_eq!(
        after,
        Clock {
            slot: target,
            ..before
        }
    );
}

#[test_log::test]
fn warp_to_slot_sets_slot_and_keeps_rest_of_clock() {
    let mut svm = LiteSVM::new();
    let before = svm.get_sysvar::<Clock>();
    let target = before.slot + 1000;
    assert_warp_sets_only_slot(&mut svm, before, target);
}

#[test_log::test]
fn warp_to_slot_can_move_backwards() {
    let mut svm = LiteSVM::new();
    let before = svm.get_sysvar::<Clock>();
    let target = before.slot - 1000;
    assert_warp_sets_only_slot(&mut svm, before, target);
}
