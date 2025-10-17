use super::*;
use solana_pubkey::Pubkey;

#[test]
fn test_domain_separation() {
    let account = AccountState::new(Pubkey::new_unique(), 1000, 1, 1640995200);
    let leaf_hash = hash_leaf(&account);

    let internal_hash = hash_internal(&leaf_hash, &leaf_hash);

    assert_ne!(leaf_hash, internal_hash);
}

#[test]
fn test_tree_creation_and_proof() {
    let accounts = vec![
        AccountState::new(Pubkey::new_unique(), 1000, 1, 1640995200),
        AccountState::new(Pubkey::new_unique(), 2000, 2, 1640995201),
        AccountState::new(Pubkey::new_unique(), 3000, 3, 1640995202),
    ];

    let tree = SodaMerkleTree::new(accounts.clone());

    assert!(tree.height > 0);
    assert_eq!(tree.leaves.len(), 3);

    let proof = tree.generate_proof(0).unwrap();
    assert!(proof.verify());
    assert_eq!(proof.account_state, accounts[0]);
}

#[test]
fn test_proof_validation_with_wrong_height() {
    let accounts = vec![
        AccountState::new(Pubkey::new_unique(), 1000, 1, 1640995200),
    ];

    let tree = SodaMerkleTree::new(accounts);
    let proof = tree.generate_proof(0).unwrap();

    assert!(!proof.verify_with_height(5));
    assert!(proof.verify_with_height(proof.proof.len() as u32));
}

#[test]
fn test_invalid_account_index() {
    let accounts = vec![
        AccountState::new(Pubkey::new_unique(), 1000, 1, 1640995200),
    ];

    let tree = SodaMerkleTree::new(accounts);
    assert!(tree.generate_proof(5).is_none());
}

#[test]
fn test_account_update() {
    let mut accounts = vec![
        AccountState::new(Pubkey::new_unique(), 1000, 1, 1640995200),
        AccountState::new(Pubkey::new_unique(), 2000, 2, 1640995201),
    ];

    let mut tree = SodaMerkleTree::new(accounts.clone());
    let original_root = tree.root;

    accounts[0].balance = 5000;
    accounts[0].nonce = 2;
    tree.update_account(0, accounts[0]).unwrap();

    assert_ne!(original_root, tree.root);

    let proof = tree.generate_proof(0).unwrap();
    assert!(proof.verify());
    assert_eq!(proof.account_state.balance, 5000);
    assert_eq!(proof.account_state.nonce, 2);
}

#[test]
fn test_bounds_checking() {
    let accounts = vec![
        AccountState::new(Pubkey::new_unique(), 1000, 1, 1640995200),
    ];

    let tree = SodaMerkleTree::new(accounts);
    let proof = tree.generate_proof(0).unwrap();

    assert!(proof.is_valid_bounds(10));
    assert!(!proof.is_valid_bounds(0));
}