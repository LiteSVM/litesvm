use {
    bincode::{deserialize, serialize},
    jupnet_sdk::{
        account::Account,
        bpf_loader_upgradeable::{self, get_program_data_address, UpgradeableLoaderState},
        clock::Clock,
        instruction::{AccountMeta, Instruction},
        loader_upgradeable_instruction::UpgradeableLoaderInstruction,
        message::Message,
        native_token::MOTES_PER_JUP,
        pubkey::Pubkey,
        signer::{keypair::Keypair, Signer},
        transaction::Transaction,
    },
    litesvm::LiteSVM,
    std::path::PathBuf,
};

fn read_counter_program() -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/counter.so");
    std::fs::read(so_path).unwrap()
}

fn set_program_upgrade_authority(
    svm: &mut LiteSVM,
    program_id: Pubkey,
    authority: Pubkey,
) -> Pubkey {
    let programdata_address = get_program_data_address(&program_id);
    let mut programdata_account = svm.get_account(&programdata_address).unwrap();
    let metadata_len = UpgradeableLoaderState::size_of_programdata_metadata();
    let metadata =
        deserialize::<UpgradeableLoaderState>(&programdata_account.data[..metadata_len]).unwrap();
    let slot = match metadata {
        UpgradeableLoaderState::ProgramData { slot, .. } => slot,
        other => panic!("expected ProgramData account, got {other:?}"),
    };

    let mut data = bincode::serialize(&UpgradeableLoaderState::ProgramData {
        slot,
        upgrade_authority_address: Some(authority),
    })
    .unwrap();
    data.extend_from_slice(&programdata_account.data[metadata_len..]);
    programdata_account.data = data;

    svm.set_account(programdata_address, programdata_account)
        .unwrap();
    programdata_address
}

fn invoke_counter(
    svm: &mut LiteSVM,
    program_id: Pubkey,
    counter_address: Pubkey,
    payer: &Keypair,
    deduper: u8,
) {
    let payer_address = payer.pubkey();
    let tx = Transaction::new(
        &[payer],
        Message::new_with_blockhash(
            &[Instruction {
                program_id,
                accounts: vec![AccountMeta::new(counter_address, false)],
                data: vec![0, deduper],
            }],
            Some(&payer_address),
            &svm.latest_blockhash(),
        ),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
}

#[test_log::test]
fn close_upgradeable_program_keeps_vm_usable() {
    let authority_kp = Keypair::new();
    let authority = authority_kp.pubkey();
    let program_id = Pubkey::from_str_const("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    let counter_address = Pubkey::from_str_const("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");

    let mut svm = LiteSVM::new();
    svm.airdrop(&authority, MOTES_PER_JUP).unwrap();
    svm.add_program(program_id, &read_counter_program())
        .unwrap();

    let programdata_address = set_program_upgrade_authority(&mut svm, program_id, authority);
    let original_program_account = svm.get_account(&program_id).unwrap();
    let original_programdata_account = svm.get_account(&programdata_address).unwrap();

    // confirm invoking program works at start
    {
        svm.set_account(
            counter_address,
            Account {
                lamports: 5,
                data: vec![0_u8; std::mem::size_of::<u32>()],
                owner: program_id,
                ..Default::default()
            },
        )
        .unwrap();

        invoke_counter(&mut svm, program_id, counter_address, &authority_kp, 0);
        assert_eq!(
            svm.get_account(&counter_address).unwrap().data,
            1u32.to_le_bytes().to_vec()
        );
    }

    let current_slot = svm.get_sysvar::<Clock>().slot;
    svm.warp_to_slot(current_slot + 1);

    // verify we can close the program
    {
        let close_ix = Instruction::new_with_bytes(
            bpf_loader_upgradeable::id(),
            &serialize(&UpgradeableLoaderInstruction::Close).unwrap(),
            vec![
                AccountMeta::new(programdata_address, false),
                AccountMeta::new(authority, false),
                AccountMeta::new_readonly(authority, true),
                AccountMeta::new(program_id, false),
            ],
        );
        let close_tx = Transaction::new(
            &[&authority_kp],
            Message::new_with_blockhash(&[close_ix], Some(&authority), &svm.latest_blockhash()),
            svm.latest_blockhash(),
        );
        svm.send_transaction(close_tx).unwrap();

        assert!(svm.get_account(&programdata_address).is_none());
    }

    // verify that if we directly write to the program data address again we can still invoke the program
    {
        svm.set_account(programdata_address, original_programdata_account)
            .unwrap();
        svm.set_account(program_id, original_program_account)
            .unwrap();

        invoke_counter(&mut svm, program_id, counter_address, &authority_kp, 1);
        assert_eq!(
            svm.get_account(&counter_address).unwrap().data,
            2u32.to_le_bytes().to_vec()
        );
    }
}
