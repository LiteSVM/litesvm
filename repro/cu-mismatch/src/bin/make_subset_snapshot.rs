use std::{
    fs::File,
    path::PathBuf,
    process::Command,
    thread::sleep,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use base64::Engine;
use serde_json::{Value, json};
use solana_message::VersionedMessage;
use solana_sdk::transaction::VersionedTransaction;

fn main() -> Result<()> {
    if std::env::args().len() < 3 || std::env::args().len() > 4 {
        bail!("usage: make_subset_snapshot <snapshot_dir> <keep_csv> [rpc_url]");
    }
    let snapshot_dir = PathBuf::from(std::env::args().nth(1).unwrap());
    let keep = parse_keep(std::env::args().nth(2).unwrap().as_str())?;
    let rpc_url = std::env::args().nth(3);

    let tx_path = snapshot_dir.join("tx.json");
    let mut tx_json: Value = read_json(tx_path.clone())?;
    let tx_b64 = tx_json
        .pointer("/result/transaction/0")
        .and_then(Value::as_str)
        .context("tx.json missing result.transaction[0]")?;

    let tx_bytes = base64::engine::general_purpose::STANDARD
        .decode(tx_b64)
        .context("decode tx base64")?;
    let tx: VersionedTransaction = bincode::deserialize(&tx_bytes).context("deserialize tx")?;
    let tx2 = keep_only_instructions(&tx, &keep)?;
    let tx2_b64 = base64::engine::general_purpose::STANDARD.encode(
        bincode::serialize(&tx2).context("serialize subset tx")?,
    );

    *tx_json
        .pointer_mut("/result/transaction/0")
        .context("tx.json missing mutable result.transaction[0]")? = Value::String(tx2_b64.clone());
    write_json(tx_path, &tx_json)?;

    let sim_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "simulateTransaction",
        "params": [
            tx2_b64,
            {
                "encoding": "base64",
                "replaceRecentBlockhash": true,
                "sigVerify": false,
                "commitment": "confirmed"
            }
        ]
    });
    write_json(snapshot_dir.join("simulate_req.json"), &sim_req)?;

    if let Some(url) = rpc_url {
        let sim = rpc_simulate(&url, &sim_req)?;
        write_json(snapshot_dir.join("simulate.json"), &sim)?;
        println!(
            "updated_simulate: units={} err={}",
            sim.pointer("/result/value/unitsConsumed")
                .and_then(Value::as_u64)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "null".to_string()),
            sim.pointer("/result/value/err")
                .cloned()
                .unwrap_or(Value::Null)
        );
    }

    let meta = json!({
        "kept_instruction_indices": keep,
        "updated_at_utc": chrono_like_now_utc(),
    });
    write_json(snapshot_dir.join("subset_meta.json"), &meta)?;

    println!("snapshot_dir={}", snapshot_dir.display());
    println!("kept_instruction_indices={}", meta["kept_instruction_indices"]);
    Ok(())
}

fn parse_keep(s: &str) -> Result<Vec<usize>> {
    let mut keep = Vec::new();
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        keep.push(
            p.parse::<usize>()
                .with_context(|| format!("invalid instruction index: {p}"))?,
        );
    }
    if keep.is_empty() {
        bail!("keep_csv produced empty keep list");
    }
    Ok(keep)
}

fn keep_only_instructions(tx: &VersionedTransaction, keep: &[usize]) -> Result<VersionedTransaction> {
    let mut tx2 = tx.clone();
    match &mut tx2.message {
        VersionedMessage::Legacy(m) => {
            let n = m.instructions.len();
            if keep.iter().any(|&i| i >= n) {
                bail!("keep indices out of bounds for legacy message");
            }
            m.instructions = keep.iter().map(|&i| m.instructions[i].clone()).collect();
        }
        VersionedMessage::V0(m) => {
            let n = m.instructions.len();
            if keep.iter().any(|&i| i >= n) {
                bail!("keep indices out of bounds for v0 message");
            }
            m.instructions = keep.iter().map(|&i| m.instructions[i].clone()).collect();
        }
    }
    Ok(tx2)
}

fn rpc_simulate(rpc_url: &str, request: &Value) -> Result<Value> {
    let curl_timeout = std::env::var("CURL_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(10);
    let retries = std::env::var("RPC_RETRIES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(8);

    let mut last_transport_err: Option<String> = None;
    for attempt in 1..=retries {
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

        if output.status.success() {
            let v: Value =
                serde_json::from_slice(&output.stdout).context("parse simulateTransaction JSON")?;
            return Ok(v);
        }

        last_transport_err = Some(format!(
            "status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
        if attempt < retries {
            let backoff_secs = u64::from(attempt.min(6));
            sleep(Duration::from_secs(backoff_secs));
        }
    }

    bail!(
        "curl simulateTransaction failed after {retries} attempts: {}",
        last_transport_err.unwrap_or_else(|| "unknown transport error".to_string())
    )
}

fn read_json(path: PathBuf) -> Result<Value> {
    let file = File::open(&path).with_context(|| format!("open {}", path.display()))?;
    serde_json::from_reader(file).with_context(|| format!("parse {}", path.display()))
}

fn write_json(path: PathBuf, v: &Value) -> Result<()> {
    let file = File::create(&path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(file, v).with_context(|| format!("write {}", path.display()))
}

fn chrono_like_now_utc() -> String {
    let output = Command::new("date")
        .arg("-u")
        .arg("+%FT%TZ")
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}
