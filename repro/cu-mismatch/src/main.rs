use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use base64::Engine;
use litesvm::LiteSVM;
use serde::Deserialize;
use serde_json::Value;
use solana_account::Account;
use solana_epoch_schedule::EpochSchedule;
use solana_pubkey::Pubkey;
use solana_sdk::{clock::Clock, transaction::VersionedTransaction};

fn main() -> Result<()> {
    let snapshot_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_snapshot_dir);

    let tx_b64: TxB64Response = read_json(snapshot_dir.join("tx.json"))?;
    let tx_meta: TxMetaResponse = read_json(snapshot_dir.join("tx_json.json"))?;
    let account_keys = read_lines(snapshot_dir.join("account_keys.txt"))?;
    let accounts_resp: AccountsResponse = read_json(snapshot_dir.join("accounts.json"))?;
    let rpc_sim = read_rpc_sim(snapshot_dir.join("simulate.json"))?;

    let tx_result = tx_b64
        .result
        .context("tx.json missing result; check RPC response")?;
    let tx_bytes = base64::engine::general_purpose::STANDARD
        .decode(
            tx_result
                .transaction
                .first()
                .context("tx.json missing transaction payload")?,
        )
        .context("decode transaction base64")?;
    let tx: VersionedTransaction = bincode::deserialize(&tx_bytes).context("deserialize tx")?;

    let tx_meta_result = tx_meta
        .result
        .context("tx_json.json missing result; check RPC response")?;
    let onchain_cu = tx_meta_result
        .meta
        .as_ref()
        .and_then(|m| m.compute_units_consumed);

    let account_values = accounts_resp
        .result
        .context("accounts.json missing result; check RPC response")?
        .value;
    if account_keys.len() != account_values.len() {
        bail!(
            "account snapshot mismatch: keys={} values={}",
            account_keys.len(),
            account_values.len()
        );
    }

    let mut snapshot: HashMap<Pubkey, Option<Account>> = HashMap::new();
    for (key_str, account_opt) in account_keys.iter().zip(account_values.into_iter()) {
        let key: Pubkey = key_str
            .parse()
            .with_context(|| format!("invalid pubkey: {key_str}"))?;
        let account = match account_opt {
            None => None,
            Some(raw) => {
                let data_b64 = raw
                    .data
                    .first()
                    .context("account data array empty (expected base64)")?;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(data_b64)
                    .with_context(|| format!("decode account data for {key}"))?;
                let owner: Pubkey = raw
                    .owner
                    .parse()
                    .with_context(|| format!("parse account owner for {key}"))?;
                Some(Account {
                    lamports: raw.lamports,
                    data,
                    owner,
                    executable: raw.executable,
                    rent_epoch: raw.rent_epoch,
                })
            }
        };
        snapshot.insert(key, account);
    }

    let clock = make_clock(
        tx_meta_result.slot,
        tx_meta_result.block_time.unwrap_or_default(),
    );
    let sim_out = run_local_sim(snapshot_dir.join("programs"), &tx, &snapshot, &clock)?;

    let signature = tx
        .signatures
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<no-signature>".to_string());
    println!("signature={signature}");
    println!("onchain_cu={}", opt_u64_to_string(onchain_cu));
    println!(
        "rpc_sim_units={}",
        opt_u64_to_string(rpc_sim.units_consumed)
    );
    println!(
        "rpc_sim_err={}",
        rpc_sim.err.unwrap_or_else(|| "null".to_string())
    );
    println!("litesvm_cu={}", sim_out.cu);
    println!(
        "litesvm_err={}",
        sim_out.err.unwrap_or_else(|| "null".to_string())
    );

    if let Some(rpc_units) = rpc_sim.units_consumed {
        println!(
            "delta_litesvm_minus_rpc={}",
            sim_out.cu as i128 - rpc_units as i128
        );
    }
    if let Some(chain_units) = onchain_cu {
        println!(
            "delta_litesvm_minus_onchain={}",
            sim_out.cu as i128 - chain_units as i128
        );
    }

    Ok(())
}

fn default_snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("snapshots")
        .join("k13_min_2_3")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T> {
    let file = File::open(&path).with_context(|| format!("open {}", path.display()))?;
    serde_json::from_reader(file).with_context(|| format!("parse {}", path.display()))
}

fn read_lines(path: PathBuf) -> Result<Vec<String>> {
    let file = File::open(&path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if !line.is_empty() {
            lines.push(line);
        }
    }
    Ok(lines)
}

fn make_clock(slot: u64, block_time: i64) -> Clock {
    let epoch_schedule = EpochSchedule::default();
    let epoch = epoch_schedule.get_epoch(slot);
    let epoch_start_slot = epoch_schedule.get_first_slot_in_epoch(epoch);
    let slot_diff = slot.saturating_sub(epoch_start_slot);
    let epoch_start_block_time = block_time as f64 - (slot_diff as f64 / 2.5);

    Clock {
        slot,
        epoch_start_timestamp: epoch_start_block_time as i64,
        epoch,
        leader_schedule_epoch: epoch + 1,
        unix_timestamp: block_time,
    }
}

fn run_local_sim(
    programs_dir: PathBuf,
    tx: &VersionedTransaction,
    snapshot: &HashMap<Pubkey, Option<Account>>,
    clock: &Clock,
) -> Result<SimOut> {
    let mut svm = LiteSVM::new()
        .with_sigverify(false)
        .with_blockhash_check(false);
    svm.set_sysvar(clock);

    if programs_dir.exists() {
        for entry in std::fs::read_dir(&programs_dir)
            .with_context(|| format!("read_dir {}", programs_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("so") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(program_id) = stem.parse::<Pubkey>() else {
                continue;
            };
            let _ = svm.add_program_from_file(program_id, &path);
        }
    }

    let payer = tx.message.static_account_keys().first().copied();
    for (pubkey, account_opt) in snapshot {
        match account_opt {
            Some(account) if account.executable => {}
            Some(account) => {
                svm.set_account(*pubkey, account.clone())
                    .with_context(|| format!("set_account {pubkey}"))?;
            }
            None => {
                if Some(*pubkey) == payer {
                    let mut payer_account = Account::default();
                    payer_account.lamports = 1_000_000_000_000;
                    svm.set_account(*pubkey, payer_account)
                        .with_context(|| format!("set_dummy_payer {pubkey}"))?;
                } else {
                    svm.set_account(*pubkey, Account::default())
                        .with_context(|| format!("set_default_account {pubkey}"))?;
                }
            }
        }
    }

    let (cu, err) = match svm.simulate_transaction(tx.clone()) {
        Ok(info) => (info.meta.compute_units_consumed, None),
        Err(err) => (
            err.meta.compute_units_consumed,
            Some(format!("{:?}", err.err)),
        ),
    };
    Ok(SimOut { cu, err })
}

fn read_rpc_sim(path: PathBuf) -> Result<RpcSimOut> {
    if !path.exists() {
        return Ok(RpcSimOut::default());
    }

    let v: Value = read_json(path)?;
    let rpc_error = v
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let sim_err = v.pointer("/result/value/err").cloned();
    let err = match (rpc_error, sim_err) {
        (Some(e), _) => Some(e),
        (None, Some(Value::Null)) | (None, None) => None,
        (None, Some(other)) => Some(other.to_string()),
    };
    let units_consumed = v
        .pointer("/result/value/unitsConsumed")
        .and_then(Value::as_u64);

    Ok(RpcSimOut {
        units_consumed,
        err,
    })
}

fn opt_u64_to_string(v: Option<u64>) -> String {
    v.map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

struct SimOut {
    cu: u64,
    err: Option<String>,
}

#[derive(Default)]
struct RpcSimOut {
    units_consumed: Option<u64>,
    err: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TxB64Response {
    result: Option<TxB64Result>,
}

#[derive(Debug, Deserialize)]
struct TxB64Result {
    transaction: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TxMetaResponse {
    result: Option<TxMetaResult>,
}

#[derive(Debug, Deserialize)]
struct TxMetaResult {
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
    meta: Option<TxMeta>,
    slot: u64,
}

#[derive(Debug, Deserialize)]
struct TxMeta {
    #[serde(rename = "computeUnitsConsumed")]
    compute_units_consumed: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AccountsResponse {
    result: Option<AccountsResult>,
}

#[derive(Debug, Deserialize)]
struct AccountsResult {
    value: Vec<Option<RpcAccount>>,
}

#[derive(Debug, Deserialize)]
struct RpcAccount {
    data: Vec<String>,
    executable: bool,
    lamports: u64,
    owner: String,
    #[serde(rename = "rentEpoch")]
    rent_epoch: u64,
}
