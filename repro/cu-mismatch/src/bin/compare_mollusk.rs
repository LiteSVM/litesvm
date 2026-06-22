use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use base64::Engine;
use litesvm::LiteSVM;
use mollusk_svm::Mollusk;
use serde::Deserialize;
use serde_json::Value;
use solana_account::Account;
use solana_epoch_schedule::EpochSchedule;
use solana_instruction::{AccountMeta, Instruction};
use solana_loader_v3_interface::state::UpgradeableLoaderState;
use solana_message::VersionedMessage;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_sdk::{clock::Clock, transaction::VersionedTransaction};
use solana_sdk_ids::{bpf_loader, bpf_loader_upgradeable};

fn main() -> Result<()> {
    let snapshot_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_snapshot_dir);

    let tx_b64: TxB64Response = read_json(snapshot_dir.join("tx.json"))?;
    let tx_meta_resp: TxMetaResponse = read_json(snapshot_dir.join("tx_json.json"))?;
    let account_keys = read_lines(snapshot_dir.join("account_keys.txt"))?;
    let accounts_resp: AccountsResponse = read_json(snapshot_dir.join("accounts.json"))?;
    let rpc_sim = read_rpc_sim(snapshot_dir.join("simulate.json"))?;

    let tx_result = tx_b64
        .result
        .context("tx.json missing result; check snapshot")?;
    let tx_bytes = base64::engine::general_purpose::STANDARD
        .decode(
            tx_result
                .transaction
                .first()
                .context("tx.json missing transaction payload")?,
        )
        .context("decode transaction base64")?;
    let tx: VersionedTransaction = bincode::deserialize(&tx_bytes).context("deserialize tx")?;

    let tx_meta = tx_meta_resp
        .result
        .context("tx_json.json missing result; check snapshot")?;
    let onchain_cu = tx_meta
        .meta
        .as_ref()
        .and_then(|m| m.compute_units_consumed);

    let account_values = accounts_resp
        .result
        .context("accounts.json missing result; check snapshot")?
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

    let clock = make_clock(tx_meta.slot, tx_meta.block_time.unwrap_or_default());
    let programs_dir = snapshot_dir.join("programs");

    let litesvm = run_litesvm_sim(programs_dir.clone(), &tx, &snapshot, &clock)?;
    let mollusk = run_mollusk_sim(programs_dir, &tx, &tx_meta, &snapshot, &clock)?;

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

    println!("litesvm_cu={}", litesvm.cu);
    println!(
        "litesvm_err={}",
        litesvm.err.unwrap_or_else(|| "null".to_string())
    );

    println!("mollusk_cu={}", mollusk.cu);
    println!(
        "mollusk_err={}",
        mollusk.err.unwrap_or_else(|| "null".to_string())
    );

    if let Some(rpc_units) = rpc_sim.units_consumed {
        println!(
            "delta_litesvm_minus_rpc={}",
            litesvm.cu as i128 - rpc_units as i128
        );
        println!(
            "delta_mollusk_minus_rpc={}",
            mollusk.cu as i128 - rpc_units as i128
        );
    }
    if let Some(chain_units) = onchain_cu {
        println!(
            "delta_litesvm_minus_onchain={}",
            litesvm.cu as i128 - chain_units as i128
        );
        println!(
            "delta_mollusk_minus_onchain={}",
            mollusk.cu as i128 - chain_units as i128
        );
    }
    println!(
        "delta_litesvm_minus_mollusk={}",
        litesvm.cu as i128 - mollusk.cu as i128
    );
    if std::env::var("PRINT_LITESVM_LOGS")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        for line in litesvm.logs {
            println!("litesvm_log={line}");
        }
    }

    Ok(())
}

fn default_snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("snapshots")
        .join("k13_min_2_3")
}

fn run_litesvm_sim(
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
            let elf_bytes = std::fs::read(&path)
                .with_context(|| format!("read program elf {}", path.display()))?;
            let owner = snapshot
                .get(&program_id)
                .and_then(|a| a.as_ref())
                .map(|a| a.owner)
                .unwrap_or_else(bpf_loader_upgradeable::id);
            // Root-cause hypothesis:
            // If we load upgradeable programs as plain loader-v2 ELF accounts,
            // LiteSVM overcharges CU on this repro by ~90k versus saved RPC.
            // Materializing proper upgradeable Program + ProgramData accounts
            // drops the delta to low single-digit thousands.
            if owner == bpf_loader_upgradeable::id() {
                add_upgradeable_program_from_elf(&mut svm, program_id, &elf_bytes)?;
            } else {
                let _ = svm.add_program(program_id, &elf_bytes);
            }
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

    let (cu, err, logs) = match svm.simulate_transaction(tx.clone()) {
        Ok(info) => (info.meta.compute_units_consumed, None, info.meta.logs),
        Err(err) => (
            err.meta.compute_units_consumed,
            Some(format!("{:?}", err.err)),
            err.meta.logs,
        ),
    };
    Ok(SimOut { cu, err, logs })
}

fn add_upgradeable_program_from_elf(
    svm: &mut LiteSVM,
    program_id: Pubkey,
    elf_bytes: &[u8],
) -> Result<()> {
    // Important for apples-to-apples CU metering: this mirrors on-chain
    // upgradeable-loader account shape instead of raw loader-v2 ELF layout.
    let programdata_address =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id()).0;
    let programdata_metadata_len = UpgradeableLoaderState::size_of_programdata_metadata();
    let programdata_len = programdata_metadata_len + elf_bytes.len();
    let mut programdata = vec![0_u8; programdata_len];
    bincode::serialize_into(
        &mut programdata[0..programdata_metadata_len],
        &UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: None,
        },
    )
    .context("serialize upgradeable programdata metadata")?;
    programdata[programdata_metadata_len..].copy_from_slice(elf_bytes);

    let programdata_account = Account {
        lamports: Rent::default().minimum_balance(programdata.len()),
        data: programdata,
        owner: bpf_loader_upgradeable::id(),
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(programdata_address, programdata_account)
        .with_context(|| format!("set programdata account {programdata_address}"))?;

    let program_account_data = bincode::serialize(&UpgradeableLoaderState::Program {
        programdata_address,
    })
    .context("serialize upgradeable program account data")?;
    let program_account = Account {
        lamports: Rent::default().minimum_balance(program_account_data.len()),
        data: program_account_data,
        owner: bpf_loader_upgradeable::id(),
        executable: true,
        rent_epoch: 0,
    };
    svm.set_account(program_id, program_account)
        .with_context(|| format!("set program account {program_id}"))?;
    Ok(())
}

fn run_mollusk_sim(
    programs_dir: PathBuf,
    tx: &VersionedTransaction,
    tx_meta: &TxMetaResult,
    snapshot: &HashMap<Pubkey, Option<Account>>,
    clock: &Clock,
) -> Result<SimOut> {
    let model = build_tx_model(tx, tx_meta)?;
    let program_ids: HashSet<Pubkey> = model.instructions.iter().map(|ix| ix.program_id).collect();

    let mut mollusk = Mollusk::default();
    mollusk.sysvars.clock = clock.clone();
    if let Ok(limit_str) = std::env::var("MOLLUSK_COMPUTE_LIMIT") {
        if let Ok(limit) = limit_str.parse::<u64>() {
            mollusk.compute_budget.compute_unit_limit = limit;
        }
    }

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
            let elf = std::fs::read(&path)
                .with_context(|| format!("read program elf {}", path.display()))?;
            let loader = snapshot
                .get(&program_id)
                .and_then(|a| a.as_ref())
                .map(|a| loader_for_program(a.owner))
                .unwrap_or_else(bpf_loader_upgradeable::id);
            mollusk.add_program_with_loader_and_elf(&program_id, &loader, &elf);
        }
    }
    add_known_litesvm_program_elfs(&mut mollusk, snapshot)?;

    let mut accounts: Vec<(Pubkey, Account)> = Vec::new();
    let mut seen = HashSet::new();
    for key in &model.full_keys {
        if !seen.insert(*key) || program_ids.contains(key) {
            continue;
        }
        match snapshot.get(key) {
            Some(Some(account)) if account.executable && program_ids.contains(key) => {}
            Some(Some(account)) => accounts.push((*key, account.clone())),
            Some(None) => {
                if Some(*key) == model.payer {
                    let mut payer_account = Account::default();
                    payer_account.lamports = 1_000_000_000_000;
                    accounts.push((*key, payer_account));
                } else {
                    accounts.push((*key, Account::default()));
                }
            }
            None => {
                if Some(*key) == model.payer {
                    let mut payer_account = Account::default();
                    payer_account.lamports = 1_000_000_000_000;
                    accounts.push((*key, payer_account));
                } else {
                    accounts.push((*key, Account::default()));
                }
            }
        }
    }

    let result = mollusk.process_transaction_instructions(&model.instructions, &accounts);
    let err = if result.program_result.is_ok() {
        None
    } else {
        Some(format!("{:?}", result.program_result))
    };
    Ok(SimOut {
        cu: result.compute_units_consumed,
        err,
        logs: Vec::new(),
    })
}

fn add_known_litesvm_program_elfs(
    mollusk: &mut Mollusk,
    snapshot: &HashMap<Pubkey, Option<Account>>,
) -> Result<()> {
    let elf_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("crates")
        .join("litesvm")
        .join("src")
        .join("programs")
        .join("elf");

    let known = [
        (
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "spl_token-3.5.0.so",
        ),
        (
            "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
            "spl_token_2022-10.0.0.so",
        ),
        (
            "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
            "spl_associated_token_account-1.1.1.so",
        ),
        (
            "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
            "spl_memo-1.0.0.so",
        ),
        (
            "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo",
            "spl_memo-3.0.0.so",
        ),
        (
            "AddressLookupTab1e1111111111111111111111111",
            "address_lookup_table.so",
        ),
        ("Config1111111111111111111111111111111111111", "config.so"),
        (
            "Stake11111111111111111111111111111111111111",
            "core_bpf_stake-1.0.1.so",
        ),
    ];

    for (program_id_str, elf_file) in known {
        let program_id: Pubkey = program_id_str
            .parse()
            .with_context(|| format!("invalid known program id: {program_id_str}"))?;
        let elf_path = elf_dir.join(elf_file);
        if !elf_path.exists() {
            continue;
        }
        let elf = std::fs::read(&elf_path)
            .with_context(|| format!("read known elf {}", elf_path.display()))?;
        let loader = snapshot
            .get(&program_id)
            .and_then(|a| a.as_ref())
            .map(|a| loader_for_program(a.owner))
            .unwrap_or_else(bpf_loader_upgradeable::id);
        mollusk.add_program_with_loader_and_elf(&program_id, &loader, &elf);
    }

    Ok(())
}

fn loader_for_program(owner: Pubkey) -> Pubkey {
    if std::env::var("MOLLUSK_FORCE_BPF_LOADER")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        bpf_loader::id()
    } else {
        owner
    }
}

fn build_tx_model(tx: &VersionedTransaction, tx_meta: &TxMetaResult) -> Result<TxModel> {
    let (static_keys, compiled_instructions, header) = match &tx.message {
        VersionedMessage::Legacy(m) => (m.account_keys.clone(), m.instructions.clone(), m.header),
        VersionedMessage::V0(m) => (m.account_keys.clone(), m.instructions.clone(), m.header),
    };

    let loaded_writable = tx_meta
        .meta
        .as_ref()
        .and_then(|m| m.loaded_addresses.as_ref())
        .map(|l| &l.writable)
        .into_iter()
        .flatten()
        .map(|k| {
            k.parse::<Pubkey>()
                .with_context(|| format!("invalid loaded writable pubkey: {k}"))
        })
        .collect::<Result<Vec<_>>>()?;

    let loaded_readonly = tx_meta
        .meta
        .as_ref()
        .and_then(|m| m.loaded_addresses.as_ref())
        .map(|l| &l.readonly)
        .into_iter()
        .flatten()
        .map(|k| {
            k.parse::<Pubkey>()
                .with_context(|| format!("invalid loaded readonly pubkey: {k}"))
        })
        .collect::<Result<Vec<_>>>()?;

    let static_len = static_keys.len();
    let loaded_writable_len = loaded_writable.len();

    let mut full_keys = static_keys.clone();
    full_keys.extend(loaded_writable);
    full_keys.extend(loaded_readonly);

    let num_required_signatures = header.num_required_signatures as usize;
    let num_readonly_signed_accounts = header.num_readonly_signed_accounts as usize;
    let num_readonly_unsigned_accounts = header.num_readonly_unsigned_accounts as usize;
    let num_writable_signed = num_required_signatures.saturating_sub(num_readonly_signed_accounts);
    let num_unsigned_static = static_len.saturating_sub(num_required_signatures);
    let num_writable_unsigned =
        num_unsigned_static.saturating_sub(num_readonly_unsigned_accounts);

    let key_flags = |idx: usize| -> Result<(bool, bool)> {
        if idx >= full_keys.len() {
            bail!("instruction account index out of bounds: idx={idx} keys={}", full_keys.len());
        }
        if idx < static_len {
            let is_signer = idx < num_required_signatures;
            let is_writable = if is_signer {
                idx < num_writable_signed
            } else {
                let unsigned_idx = idx.saturating_sub(num_required_signatures);
                unsigned_idx < num_writable_unsigned
            };
            Ok((is_signer, is_writable))
        } else if idx < static_len + loaded_writable_len {
            Ok((false, true))
        } else {
            Ok((false, false))
        }
    };

    let mut instructions = Vec::with_capacity(compiled_instructions.len());
    for cix in compiled_instructions {
        let program_id_index = cix.program_id_index as usize;
        let program_id = *full_keys
            .get(program_id_index)
            .with_context(|| format!("program id index out of bounds: {program_id_index}"))?;

        let mut metas = Vec::with_capacity(cix.accounts.len());
        for account_idx in cix.accounts {
            let idx = account_idx as usize;
            let (is_signer, is_writable) = key_flags(idx)?;
            let pubkey = *full_keys
                .get(idx)
                .with_context(|| format!("account index out of bounds: {idx}"))?;
            metas.push(AccountMeta {
                pubkey,
                is_signer,
                is_writable,
            });
        }

        instructions.push(Instruction {
            program_id,
            accounts: metas,
            data: cix.data,
        });
    }

    Ok(TxModel {
        instructions,
        full_keys,
        payer: static_keys.first().copied(),
    })
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

struct TxModel {
    instructions: Vec<Instruction>,
    full_keys: Vec<Pubkey>,
    payer: Option<Pubkey>,
}

struct SimOut {
    cu: u64,
    err: Option<String>,
    logs: Vec<String>,
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
    #[serde(rename = "loadedAddresses")]
    loaded_addresses: Option<LoadedAddresses>,
}

#[derive(Debug, Deserialize)]
struct LoadedAddresses {
    writable: Vec<String>,
    readonly: Vec<String>,
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
