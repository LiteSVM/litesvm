use litesvm::LiteSVM;
use litesvm_helpers::spl::create_mint;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL, program_pack::Pack, signature::Keypair, signer::Signer,
};

#[test]
fn create_mint_test() {
    let mut svm = LiteSVM::new();

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), LAMPORTS_PER_SOL * 10)
        .unwrap();

    let mint = create_mint(&mut svm, &authority, &authority.pubkey(), None).unwrap();

    let mint_acc = svm.get_account(&mint);
    let mint = spl_token_2022::state::Mint::unpack(&mint_acc.unwrap().data).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.mint_authority, Some(authority.pubkey()).into());
}
