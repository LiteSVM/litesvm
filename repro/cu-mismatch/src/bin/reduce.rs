use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::Command,
};

use anyhow::{Context, Result, bail};
use base64::Engine;
use litesvm::LiteSVM;
use serde::Deserialize;
use serde_json::{Value, json};
use solana_account::Account;
use solana_epoch_schedule::EpochSchedule;
use solana_message::VersionedMessage;
use solana_pubkey::Pubkey;
use solana_sdk::{clock::Clock, transaction::VersionedTransaction};

fn main() -> Result<()> {
    let snapshot_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_snapshot_dir);
    let rpc_url = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let threshold = std::env::args()
        .nth(3)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1_000);
    let curl_timeout = std::env::var("CURL_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(90);

    let tx_b64: TxB64Response = read_json(snapshot_dir.join("tx.json"))?;
    let tx_meta: TxMetaResponse = read_json(snapshot_dir.join("tx_json.json"))?;
    let account_keys = read_lines(snapshot_dir.join("account_keys.txt"))?;
    let accounts_resp: AccountsResponse = read_json(snapshot_dir.join("accounts.json"))?;

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
    let original_tx: VersionedTransaction =
        bincode::deserialize(&tx_bytes).context("deserialize tx")?;
    let original_sig = original_tx
        .signatures
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<no-signature>".to_string());
    let onchain_cu = tx_meta
        .result
        .as_ref()
        .and_then(|r| r.meta.as_ref())
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

    let slot = tx_meta
        .result
        .as_ref()
        .map(|r| r.slot)
        .context("missing slot in tx_json.json")?;
    let block_time = tx_meta
        .result
        .as_ref()
        .and_then(|r| r.block_time)
        .unwrap_or_default();
    let clock = make_clock(slot, block_time);
    let programs_dir = snapshot_dir.join("programs");

    println!("signature={original_sig}");
    println!("onchain_cu={}", opt_u64_to_string(onchain_cu));
    println!("threshold={threshold}");

    let baseline = evaluate_variant(
        &original_tx,
        None,
        &programs_dir,
        &snapshot,
        &clock,
        &rpc_url,
        curl_timeout,
    )?;
    print_eval("baseline", &baseline);
    if !is_cu_mismatch(&baseline, threshold) {
        println!("baseline does not meet mismatch threshold; nothing to reduce");
        return Ok(());
    }

    let mut current_tx = original_tx.clone();
    let mut orig_ix_indices: Vec<usize> = (0..instruction_count(&original_tx)).collect();
    let mut removed: Vec<usize> = Vec::new();

    loop {
        let mut changed = false;
        let current_n = instruction_count(&current_tx);
        if current_n <= 1 {
            break;
        }

        for pos in 0..current_n {
            let candidate_tx = drop_instruction_at(&current_tx, pos)?;
            let eval = evaluate_variant(
                &candidate_tx,
                Some(pos),
                &programs_dir,
                &snapshot,
                &clock,
                &rpc_url,
                curl_timeout,
            )?;

            let original_ix = orig_ix_indices[pos];
            print_eval(
                &format!("drop current_pos={pos} original_ix={original_ix}"),
                &eval,
            );
            if is_cu_mismatch(&eval, threshold) {
                removed.push(original_ix);
                orig_ix_indices.remove(pos);
                current_tx = candidate_tx;
                changed = true;
                println!("kept mismatch after removing original_ix={original_ix}");
                break;
            }
        }

        if !changed {
            break;
        }
    }

    println!("final_instruction_count={}", instruction_count(&current_tx));
    println!("removed_original_ix={removed:?}");
    println!("remaining_original_ix={orig_ix_indices:?}");

    let final_eval = evaluate_variant(
        &current_tx,
        None,
        &programs_dir,
        &snapshot,
        &clock,
        &rpc_url,
        curl_timeout,
    )?;
    print_eval("final", &final_eval);

    Ok(())
}

fn default_snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("snapshots")
        .join("k13_min_2_3")
}

fn evaluate_variant(
    tx: &VersionedTransaction,
    dropped_pos: Option<usize>,
    programs_dir: &PathBuf,
    snapshot: &HashMap<Pubkey, Option<Account>>,
    clock: &Clock,
    rpc_url: &str,
    curl_timeout: u64,
) -> Result<EvalOut> {
    let local = run_local_sim(programs_dir, tx, snapshot, clock)?;
    let rpc = rpc_simulate(rpc_url, tx, curl_timeout)?;
    Ok(EvalOut {
        dropped_pos,
        litesvm_cu: local.cu,
        litesvm_err: local.err,
        rpc_units: rpc.units_consumed,
        rpc_err: rpc.err,
    })
}

fn rpc_simulate(rpc_url: &str, tx: &VersionedTransaction, curl_timeout: u64) -> Result<RpcSimOut> {
    let tx_bytes = bincode::serialize(tx).context("serialize tx for RPC simulate")?;
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(tx_bytes);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "simulateTransaction",
        "params": [
            tx_b64,
            {
                "encoding": "base64",
                "replaceRecentBlockhash": true,
                "sigVerify": false,
                "commitment": "confirmed"
            }
        ]
    });

    let output = Command::new("curl")
        .arg("-sS")
        .arg("-m")
        .arg(curl_timeout.to_string())
        .arg(rpc_url)
        .arg("-H")
        .arg("content-type: application/json")
        .arg("--data")
        .arg(request.to_string())
        .output()
        .context("invoke curl for simulateTransaction")?;

    if !output.status.success() {
        bail!(
            "curl simulateTransaction failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let v: Value =
        serde_json::from_slice(&output.stdout).context("parse simulateTransaction JSON")?;
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
    programs_dir: &PathBuf,
    tx: &VersionedTransaction,
    snapshot: &HashMap<Pubkey, Option<Account>>,
    clock: &Clock,
) -> Result<SimOut> {
    let mut svm = LiteSVM::new()
        .with_sigverify(false)
        .with_blockhash_check(false);
    svm.set_sysvar(clock);

    if programs_dir.exists() {
        for entry in std::fs::read_dir(programs_dir)
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

fn instruction_count(tx: &VersionedTransaction) -> usize {
    match &tx.message {
        VersionedMessage::Legacy(m) => m.instructions.len(),
        VersionedMessage::V0(m) => m.instructions.len(),
    }
}

fn drop_instruction_at(
    tx: &VersionedTransaction,
    drop_index: usize,
) -> Result<VersionedTransaction> {
    let mut tx2 = tx.clone();
    match &mut tx2.message {
        VersionedMessage::Legacy(m) => {
            if drop_index >= m.instructions.len() {
                bail!(
                    "drop index {} out of bounds for {} instructions",
                    drop_index,
                    m.instructions.len()
                );
            }
            m.instructions.remove(drop_index);
        }
        VersionedMessage::V0(m) => {
            if drop_index >= m.instructions.len() {
                bail!(
                    "drop index {} out of bounds for {} instructions",
                    drop_index,
                    m.instructions.len()
                );
            }
            m.instructions.remove(drop_index);
        }
    }
    Ok(tx2)
}

fn is_cu_mismatch(eval: &EvalOut, threshold: u64) -> bool {
    if eval.litesvm_err.is_some() || eval.rpc_err.is_some() {
        return false;
    }
    let Some(rpc_units) = eval.rpc_units else {
        return false;
    };
    eval.litesvm_cu.abs_diff(rpc_units) >= threshold
}

fn print_eval(label: &str, eval: &EvalOut) {
    let rpc_units = eval
        .rpc_units
        .map(|u| u.to_string())
        .unwrap_or_else(|| "null".to_string());
    let delta = eval
        .rpc_units
        .map(|u| (eval.litesvm_cu as i128 - u as i128).to_string())
        .unwrap_or_else(|| "null".to_string());
    println!(
        "{}: dropped_pos={} rpc_units={} rpc_err={} litesvm_cu={} litesvm_err={} delta={}",
        label,
        eval.dropped_pos
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
        rpc_units,
        eval.rpc_err.clone().unwrap_or_else(|| "null".to_string()),
        eval.litesvm_cu,
        eval.litesvm_err
            .clone()
            .unwrap_or_else(|| "null".to_string()),
        delta
    );
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

fn opt_u64_to_string(v: Option<u64>) -> String {
    v.map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

struct SimOut {
    cu: u64,
    err: Option<String>,
}

struct RpcSimOut {
    units_consumed: Option<u64>,
    err: Option<String>,
}

struct EvalOut {
    dropped_pos: Option<usize>,
    litesvm_cu: u64,
    litesvm_err: Option<String>,
    rpc_units: Option<u64>,
    rpc_err: Option<String>,
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
