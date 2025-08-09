use {
    chrono::Local,
    indexmap::IndexMap,
    serde::{Deserialize, Serialize},
    sha2::{Digest, Sha256},
    std::{
        fs::{read_to_string, write},
        path::Path,
    },
};

pub const MAX_HISTORICAL_CU_LOGS_TO_RECORD: usize = 5;

#[derive(Clone, Debug)]
pub struct ComputeUnitRecorder(IndexMap<String, u64>);

#[derive(Serialize, Deserialize)]
struct CuLog {
    timestamp: String,
    entries: IndexMap<String, CuEntry>,
    checksum: String,
}

#[derive(Clone, Copy, Serialize, Deserialize, Default)]
struct CuEntry {
    value: u64,
    diff: i64,
}

impl ComputeUnitRecorder {
    pub fn new() -> Self {
        ComputeUnitRecorder(IndexMap::new())
    }

    pub fn record_cus(&mut self, ix: impl ToString, cus: u64) {
        let ix = self.0.entry(ix.to_string()).or_default();
        *ix += cus;
    }

    fn hash_entries(logs: &IndexMap<String, u64>) -> String {
        let mut hasher = Sha256::new();
        for (ix, cu) in logs {
            hasher.update(format!("{}-{}", ix, cu));
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn commit_to_change_log(&self) {
        let path = Path::new("cu_logs.json");
        let checksum = Self::hash_entries(&self.0);
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // Read existing logs from file (if any)
        let mut all_logs: Vec<CuLog> = if path.exists() {
            read_to_string(path)
                .ok()
                .and_then(|data| serde_json::from_str(&data).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Skip if checksum already exists
        if all_logs.iter().any(|entry| entry.checksum == checksum) {
            return;
        }

        // Get previous entries from the most recent log (if any)
        let previous_entries: IndexMap<&String, &CuEntry> = all_logs
            .first()
            .map(|prev| prev.entries.iter().collect())
            .unwrap_or_default();

        // Build new entries with diffs
        let entries = self
            .0
            .iter()
            .map(|(ix, cu)| {
                let diff = if let Some(old) = previous_entries.get(&ix.to_string()) {
                    if old.value > 0 {
                        *cu as i64 - old.value as i64
                    } else {
                        0
                    }
                } else {
                    0
                };

                (ix.to_string(), CuEntry { value: *cu, diff })
            })
            .collect::<IndexMap<_, _>>();
        // Prepend new log entry
        all_logs.insert(
            0,
            CuLog {
                timestamp,
                checksum,
                entries,
            },
        );

        // Keep only the latest `MAX_HISTORICAL_CU_LOGS_TO_RECORD` logs
        all_logs.truncate(MAX_HISTORICAL_CU_LOGS_TO_RECORD);

        // Save back to file
        let output = serde_json::to_string_pretty(&all_logs).expect("serialization failed");
        write(path, output).expect("write failed");
    }
}
