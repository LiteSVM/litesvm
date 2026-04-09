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
    types::{FeatureSetSnapshot, LiteSvmSnapshot},
};

const STATE_VERSION: u8 = 1;
const LARGE_STACK_SIZE: usize = 64 * 1024 * 1024; // 64 MB

fn extract_snapshot(svm: &LiteSVM) -> LiteSvmSnapshot {
    LiteSvmSnapshot {
        accounts: svm
            .accounts_db()
            .inner
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect(),
        airdrop_kp: svm.airdrop_keypair_bytes().to_vec(),
        feature_set: FeatureSetSnapshot::from_feature_set(svm.get_feature_set_ref()),
        latest_blockhash: svm.latest_blockhash(),
        history: svm
            .transaction_history_entries()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect(),
        history_capacity: svm.transaction_history_capacity(),
        compute_budget: svm.get_compute_budget(),
        sigverify: svm.get_sigverify(),
        blockhash_check: svm.get_blockhash_check(),
        fee_structure: svm.get_fee_structure().clone(),
        log_bytes_limit: svm.get_log_bytes_limit(),
    }
}

fn restore_from_snapshot(snapshot: LiteSvmSnapshot) -> Result<LiteSVM, PersistenceError> {
    let feature_set = snapshot.feature_set.into_feature_set();
    let mut svm = LiteSVM::default().with_feature_set(feature_set);

    // Set scalar config
    svm = svm
        .with_sigverify(snapshot.sigverify)
        .with_blockhash_check(snapshot.blockhash_check)
        .with_log_bytes_limit(snapshot.log_bytes_limit);

    if let Some(cb) = snapshot.compute_budget {
        svm = svm.with_compute_budget(cb);
    }

    svm.set_fee_structure(snapshot.fee_structure);
    svm.set_latest_blockhash(snapshot.latest_blockhash);

    let airdrop_kp: [u8; 64] = snapshot
        .airdrop_kp
        .try_into()
        .map_err(|_| PersistenceError::Serialize(
            Box::new(bincode::ErrorKind::Custom("invalid airdrop keypair length".into())).into(),
        ))?;
    svm.set_airdrop_keypair(airdrop_kp);

    // Pass 1: insert all accounts without triggering cache updates
    for (address, account) in snapshot.accounts {
        svm.set_account_no_checks(address, account);
    }

    // Restore transaction history with original capacity
    svm.restore_transaction_history(
        snapshot.history.into_iter().collect(),
        snapshot.history_capacity,
    );

    // Pass 2: rebuild all derived caches
    svm.rebuild_caches()?;

    Ok(svm)
}

fn serialize_snapshot(snapshot: &LiteSvmSnapshot) -> Result<Vec<u8>, PersistenceError> {
    let mut buf = vec![STATE_VERSION];
    bincode::serialize_into(&mut buf, snapshot)?;
    Ok(buf)
}

fn deserialize_snapshot(bytes: &[u8]) -> Result<LiteSvmSnapshot, PersistenceError> {
    if bytes.is_empty() {
        return Err(PersistenceError::Serialize(Box::new(bincode::ErrorKind::Custom(
            "empty input".into(),
        )).into()));
    }
    let version = bytes[0];
    if version != STATE_VERSION {
        return Err(PersistenceError::UnsupportedVersion(version));
    }
    let snapshot: LiteSvmSnapshot = bincode::deserialize(&bytes[1..])?;
    Ok(snapshot)
}

/// Runs `f` on a thread with a large stack to prevent stack overflow
/// during bincode serialization/deserialization of large account maps.
fn on_large_stack<F, T>(f: F) -> Result<T, PersistenceError>
where
    F: FnOnce() -> Result<T, PersistenceError> + Send + 'static,
    T: Send + 'static,
{
    let handle = std::thread::Builder::new()
        .stack_size(LARGE_STACK_SIZE)
        .name("litesvm-persistence".into())
        .spawn(f)
        .map_err(PersistenceError::Io)?;

    handle.join().map_err(|_| PersistenceError::ThreadPanic)?
}

/// Saves the full LiteSVM state to a file.
pub fn save_to_file(svm: &LiteSVM, path: impl AsRef<Path>) -> Result<(), PersistenceError> {
    let snapshot = extract_snapshot(svm);
    let path = path.as_ref().to_path_buf();
    on_large_stack(move || {
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&[STATE_VERSION])?;
        bincode::serialize_into(&mut writer, &snapshot)?;
        writer.flush()?;
        Ok(())
    })
}

/// Loads a full LiteSVM state from a file.
pub fn load_from_file(path: impl AsRef<Path>) -> Result<LiteSVM, PersistenceError> {
    let path = path.as_ref().to_path_buf();
    on_large_stack(move || {
        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);
        let mut version = [0u8; 1];
        reader.read_exact(&mut version)?;
        if version[0] != STATE_VERSION {
            return Err(PersistenceError::UnsupportedVersion(version[0]));
        }
        let snapshot: LiteSvmSnapshot = bincode::deserialize_from(&mut reader)?;
        restore_from_snapshot(snapshot)
    })
}

/// Serializes the full LiteSVM state to bytes.
pub fn to_bytes(svm: &LiteSVM) -> Result<Vec<u8>, PersistenceError> {
    let snapshot = extract_snapshot(svm);
    on_large_stack(move || serialize_snapshot(&snapshot))
}

/// Deserializes the full LiteSVM state from bytes.
pub fn from_bytes(bytes: &[u8]) -> Result<LiteSVM, PersistenceError> {
    let bytes = bytes.to_vec();
    on_large_stack(move || {
        let snapshot = deserialize_snapshot(&bytes)?;
        restore_from_snapshot(snapshot)
    })
}
