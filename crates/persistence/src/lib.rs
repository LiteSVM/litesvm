mod error;
mod types;

pub use error::PersistenceError;
use {
    litesvm::LiteSVM,
    std::{
        fs::File,
        io::{BufWriter, Read, Write},
        marker::PhantomData,
        path::Path,
    },
    types::{
        AccountEntryWire, ComputeBudgetV2Wire, FeatureSetSnapshot, LiteSvmSnapshot, LiteSvmState,
        TxResult,
    },
    wincode::{Deserialize, Serialize},
};

const V1_STATE_VERSION: u8 = 1;
const V2_STATE_VERSION: u8 = 2;
const V3_STATE_VERSION: u8 = 3;
const STATE_VERSION: u8 = 4;

fn extract_state(svm: &LiteSVM) -> LiteSvmState {
    LiteSvmState {
        // AccountSharedData::clone is an Arc bump — no underlying data copy.
        // The actual data bytes are written once during serialization via AccountSchema.
        accounts: svm
            .accounts_db()
            .inner
            .iter()
            .map(|(k, v)| AccountEntryWire::from((*k, v.clone())))
            .collect(),
        airdrop_kp: *svm.airdrop_keypair_bytes(),
        feature_set: FeatureSetSnapshot::from_feature_set(svm.get_feature_set_ref()),
        latest_blockhash: svm.latest_blockhash(),
        history: svm
            .transaction_history_entries()
            .iter()
            .map(|(k, v)| (*k, TxResult::from_result(v.clone())))
            .collect(),
        history_capacity: svm.transaction_history_capacity() as u64,
        compute_budget: svm.get_compute_budget(),
        sigverify: svm.get_sigverify(),
        blockhash_check: svm.get_blockhash_check(),
        fee_structure: svm.get_fee_structure().clone(),
        log_bytes_limit: svm.get_log_bytes_limit().map(|v| v as u64),
        budget_layout: PhantomData,
    }
}

fn extract_snapshot(svm: &LiteSVM) -> LiteSvmSnapshot {
    let mut epoch_vote_stakes: Vec<_> = svm
        .epoch_vote_stakes()
        .map(|(vote_account, stake)| (*vote_account, *stake))
        .collect();
    epoch_vote_stakes.sort_unstable_by_key(|(vote_account, _)| *vote_account);
    LiteSvmSnapshot {
        state: extract_state(svm),
        epoch_vote_stakes,
    }
}

fn restore_from_snapshot(snapshot: LiteSvmSnapshot) -> Result<LiteSVM, PersistenceError> {
    let LiteSvmSnapshot {
        state,
        mut epoch_vote_stakes,
    } = snapshot;
    let feature_set = state.feature_set.into_feature_set();
    let mut svm = LiteSVM::default().with_feature_set(feature_set);

    svm = svm
        .with_sigverify(state.sigverify)
        .with_blockhash_check(state.blockhash_check)
        .with_log_bytes_limit(state.log_bytes_limit.map(|v| v as usize));

    if let Some(cb) = state.compute_budget {
        svm = svm.with_compute_budget(cb);
    }

    svm.set_fee_structure(state.fee_structure);
    svm.set_latest_blockhash(state.latest_blockhash);
    svm.set_airdrop_keypair(state.airdrop_kp);
    epoch_vote_stakes.sort_unstable_by_key(|(vote_account, _)| *vote_account);
    if let Some(entries) = epoch_vote_stakes
        .windows(2)
        .find(|entries| entries[0].0 == entries[1].0)
    {
        return Err(PersistenceError::DuplicateEpochStake(entries[0].0));
    }
    svm.set_epoch_stakes(epoch_vote_stakes)
        .map_err(PersistenceError::InvalidEpochStakes)?;

    for (address, account) in state.accounts.into_iter().map(Into::into) {
        svm.set_account_no_checks(address, account);
    }

    svm.restore_transaction_history(
        state
            .history
            .into_iter()
            .map(|(k, v)| (k, v.into_result()))
            .collect(),
        state.history_capacity as usize,
    );

    svm.rebuild_caches()?;

    Ok(svm)
}

fn deserialize_snapshot(version: u8, bytes: &[u8]) -> Result<LiteSvmSnapshot, PersistenceError> {
    match version {
        V1_STATE_VERSION => Ok(LiteSvmState::deserialize(bytes)?.into()),
        V2_STATE_VERSION => Ok(LiteSvmState::<ComputeBudgetV2Wire>::deserialize(bytes)?
            .with_layout()
            .into()),
        V3_STATE_VERSION => {
            Ok(LiteSvmSnapshot::<ComputeBudgetV2Wire>::deserialize(bytes)?.with_layout())
        }
        STATE_VERSION => Ok(LiteSvmSnapshot::deserialize(bytes)?),
        version => Err(PersistenceError::UnsupportedVersion(version)),
    }
}

/// Saves the full LiteSVM state to a file.
pub fn save_to_file(svm: &LiteSVM, path: impl AsRef<Path>) -> Result<(), PersistenceError> {
    let snapshot = extract_snapshot(svm);
    let mut writer = BufWriter::new(File::create(path)?);
    let payload_size = LiteSvmSnapshot::serialized_size(&snapshot)? as usize;
    let mut payload = Vec::with_capacity(payload_size);
    LiteSvmSnapshot::serialize_into(&mut payload, &snapshot)?;
    writer.write_all(&[STATE_VERSION])?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Loads a full LiteSVM state from a file.
pub fn load_from_file(path: impl AsRef<Path>) -> Result<LiteSVM, PersistenceError> {
    let mut reader = File::open(path)?;
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let (version, rest) = bytes.split_first().ok_or(PersistenceError::EmptyInput)?;
    let snapshot = deserialize_snapshot(*version, rest)?;
    restore_from_snapshot(snapshot)
}

/// Serializes the full LiteSVM state to bytes.
pub fn to_bytes(svm: &LiteSVM) -> Result<Vec<u8>, PersistenceError> {
    let snapshot = extract_snapshot(svm);
    let payload_size = LiteSvmSnapshot::serialized_size(&snapshot)? as usize;
    let mut buf = Vec::with_capacity(1 + payload_size);
    buf.push(STATE_VERSION);
    LiteSvmSnapshot::serialize_into(&mut buf, &snapshot)?;
    Ok(buf)
}

/// Deserializes the full LiteSVM state from bytes.
pub fn from_bytes(bytes: &[u8]) -> Result<LiteSVM, PersistenceError> {
    let (version, rest) = bytes.split_first().ok_or(PersistenceError::EmptyInput)?;
    let snapshot = deserialize_snapshot(*version, rest)?;
    restore_from_snapshot(snapshot)
}

#[cfg(test)]
mod tests {
    use {super::*, solana_compute_budget::compute_budget::ComputeBudget};

    fn serialize<T: Serialize<Src = T>>(version: u8, snapshot: &T) -> Vec<u8> {
        let mut bytes = vec![version];
        T::serialize_into(&mut bytes, snapshot).unwrap();
        bytes
    }

    fn custom_budget() -> ComputeBudget {
        ComputeBudget {
            compute_unit_limit: 123,
            big_modular_exponentiation_base_cost: 7,
            ..ComputeBudget::new_with_defaults(false)
        }
    }

    #[test]
    fn version_two_snapshot_is_still_loadable() {
        let mut svm = LiteSVM::new().with_compute_budget(custom_budget());
        svm.set_epoch_stake(solana_address::Address::new_unique(), 456)
            .unwrap();

        let state = extract_state(&svm).with_layout::<ComputeBudgetV2Wire>();
        let restored = from_bytes(&serialize(V2_STATE_VERSION, &state)).unwrap();
        assert_eq!(restored.epoch_total_stake(), 0);
        // Version 2 predates the modexp costs: they come back as defaults.
        let budget = restored.get_compute_budget().unwrap();
        assert_eq!(budget.compute_unit_limit, 123);
        assert_eq!(
            budget.big_modular_exponentiation_base_cost,
            ComputeBudget::new_with_defaults(false).big_modular_exponentiation_base_cost
        );
    }

    #[test]
    fn version_three_snapshot_is_still_loadable() {
        let mut svm = LiteSVM::new().with_compute_budget(custom_budget());
        svm.set_epoch_stake(solana_address::Address::new_unique(), 456)
            .unwrap();

        let snapshot = extract_snapshot(&svm).with_layout::<ComputeBudgetV2Wire>();
        let restored = from_bytes(&serialize(V3_STATE_VERSION, &snapshot)).unwrap();
        assert_eq!(restored.epoch_total_stake(), 456);
        assert_eq!(
            restored.get_compute_budget().unwrap().compute_unit_limit,
            123
        );
    }

    #[test]
    fn modexp_costs_round_trip() {
        let svm = LiteSVM::new().with_compute_budget(custom_budget());
        let restored = from_bytes(&to_bytes(&svm).unwrap()).unwrap();
        assert_eq!(
            restored
                .get_compute_budget()
                .unwrap()
                .big_modular_exponentiation_base_cost,
            7
        );
    }

    #[test]
    fn duplicate_epoch_stakes_are_rejected() {
        let vote_account = solana_address::Address::new_unique();
        let snapshot = LiteSvmSnapshot {
            state: extract_state(&LiteSVM::new()),
            epoch_vote_stakes: vec![(vote_account, 100), (vote_account, 200)],
        };

        assert!(matches!(
            from_bytes(&serialize(STATE_VERSION, &snapshot)),
            Err(PersistenceError::DuplicateEpochStake(address)) if address == vote_account
        ));
    }

    #[test]
    fn overflowing_unique_epoch_stakes_are_rejected() {
        let snapshot = LiteSvmSnapshot {
            state: extract_state(&LiteSVM::new()),
            epoch_vote_stakes: vec![
                (solana_address::Address::new_unique(), u64::MAX),
                (solana_address::Address::new_unique(), 1),
            ],
        };

        assert!(matches!(
            from_bytes(&serialize(STATE_VERSION, &snapshot)),
            Err(PersistenceError::InvalidEpochStakes(
                litesvm::error::LiteSVMError::EpochStakeOverflow
            ))
        ));
    }
}
