mod error;
mod types;

pub use error::PersistenceError;

use {
    litesvm::LiteSVM,
    std::{
        fs::File,
        io::{BufReader, BufWriter, Read, Write},
        path::Path,
    },
    types::{FeatureSetSnapshot, LiteSvmSnapshot, TxResult},
    wincode::{Deserialize, DeserializeOwned, Serialize},
};

const STATE_VERSION: u8 = 1;

fn extract_snapshot(svm: &LiteSVM) -> LiteSvmSnapshot {
    LiteSvmSnapshot {
        // AccountSharedData::clone is an Arc bump — no underlying data copy.
        // The actual data bytes are written once during serialization via AccountSchema.
        accounts: svm
            .accounts_db()
            .inner
            .iter()
            .map(|(k, v)| (*k, v.clone()))
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
    }
}

fn restore_from_snapshot(snapshot: LiteSvmSnapshot) -> Result<LiteSVM, PersistenceError> {
    let feature_set = snapshot.feature_set.into_feature_set();
    let mut svm = LiteSVM::default().with_feature_set(feature_set);

    svm = svm
        .with_sigverify(snapshot.sigverify)
        .with_blockhash_check(snapshot.blockhash_check)
        .with_log_bytes_limit(snapshot.log_bytes_limit.map(|v| v as usize));

    if let Some(cb) = snapshot.compute_budget {
        svm = svm.with_compute_budget(cb);
    }

    svm.set_fee_structure(snapshot.fee_structure);
    svm.set_latest_blockhash(snapshot.latest_blockhash);
    svm.set_airdrop_keypair(snapshot.airdrop_kp);

    for (address, account) in snapshot.accounts {
        svm.set_account_no_checks(address, account);
    }

    svm.restore_transaction_history(
        snapshot
            .history
            .into_iter()
            .map(|(k, v)| (k, v.into_result()))
            .collect(),
        snapshot.history_capacity as usize,
    );

    svm.rebuild_caches()?;

    Ok(svm)
}

/// Saves the full LiteSVM state to a file. Streams directly to disk via `BufWriter`
/// without materializing the full snapshot in memory first.
pub fn save_to_file(svm: &LiteSVM, path: impl AsRef<Path>) -> Result<(), PersistenceError> {
    let snapshot = extract_snapshot(svm);
    let mut writer = BufWriter::new(File::create(path)?);
    writer.write_all(&[STATE_VERSION])?;
    LiteSvmSnapshot::serialize_into(&mut writer, &snapshot)?;
    writer.flush()?;
    Ok(())
}

/// Loads a full LiteSVM state from a file. Reads directly from disk via `BufReader`
/// without buffering the full file in memory first.
pub fn load_from_file(path: impl AsRef<Path>) -> Result<LiteSVM, PersistenceError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut version = [0u8; 1];
    reader.read_exact(&mut version)?;
    if version[0] != STATE_VERSION {
        return Err(PersistenceError::UnsupportedVersion(version[0]));
    }
    let snapshot: LiteSvmSnapshot = LiteSvmSnapshot::deserialize_from(reader)?;
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
    if *version != STATE_VERSION {
        return Err(PersistenceError::UnsupportedVersion(*version));
    }
    let snapshot: LiteSvmSnapshot = LiteSvmSnapshot::deserialize(rest)?;
    restore_from_snapshot(snapshot)
}
