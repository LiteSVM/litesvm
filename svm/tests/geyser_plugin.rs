#![cfg(feature = "geyser-plugin")]
use std::sync::mpsc;

use agave_geyser_plugin_interface::geyser_plugin_interface::{
    GeyserPlugin, ReplicaAccountInfoVersions, Result as GeyserResult,
};
use litesvm::LiteSVM;
use solana_program::{message::Message, pubkey::Pubkey, system_instruction::transfer};
use solana_sdk::{clock::Slot, signature::Keypair, signer::Signer, transaction::Transaction};

#[derive(Clone, Debug)]
struct DummyPlugin {
    tx: mpsc::Sender<(Pubkey, u64)>,
}

impl GeyserPlugin for DummyPlugin {
    fn name(&self) -> &'static str {
        "dummy"
    }

    fn update_account(
        &self,
        account: ReplicaAccountInfoVersions,
        _slot: Slot,
        _is_startup: bool,
    ) -> GeyserResult<()> {
        let account_info = match account {
            ReplicaAccountInfoVersions::V0_0_1(_info) => {
                unreachable!("ReplicaAccountInfoVersions::V0_0_1 is not supported")
            }
            ReplicaAccountInfoVersions::V0_0_2(_info) => {
                unreachable!("ReplicaAccountInfoVersions::V0_0_2 is not supported")
            }
            ReplicaAccountInfoVersions::V0_0_3(info) => info,
        };

        let pk = Pubkey::try_from(account_info.pubkey).unwrap();
        self.tx.send((pk, account_info.lamports)).unwrap();

        Ok(())
    }
}

#[test]
pub fn test_set_geyser_plugin() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let (tx, rx) = mpsc::channel::<(Pubkey, u64)>();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, 10_000).unwrap();
    svm = svm.with_geyser_plugin(Box::new(DummyPlugin { tx }));
    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    let _ = svm.send_transaction(tx).unwrap();

    let (from_pk, from_lamports) = rx.recv().unwrap();
    assert_eq!(from_pk, from);
    assert_eq!(from_lamports, 4936);

    let (to_pk, to_lamports) = rx.recv().unwrap();
    assert_eq!(to_pk, to);
    assert_eq!(to_lamports, 64);

    let from_account = svm.get_account(&from);
    let to_account = svm.get_account(&to);
    assert_eq!(from_account.unwrap().lamports, 4936);
    assert_eq!(to_account.unwrap().lamports, 64);
}
