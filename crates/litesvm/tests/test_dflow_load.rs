use {litesvm::LiteSVM, solana_address::address};

// https://github.com/LiteSVM/litesvm/issues/140
#[test]
fn test_dflow_load() {
    let mut svm = LiteSVM::new();
    let program_bytes =
        include_bytes!("../test_programs/DF1ow3DqMj3HvTj8i8J9yM2hE9hCrLLXpdbaKZu4ZPnz.so");
    svm.add_program(
        address!("DF1ow3DqMj3HvTj8i8J9yM2hE9hCrLLXpdbaKZu4ZPnz"),
        program_bytes,
    )
    .unwrap();
}
