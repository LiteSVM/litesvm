//! Wire-format snapshot tests: the JSON contract with the web UI and TS SDK.
//!
//! The frontend and the TypeScript SDK depend on the exact serialized shape of
//! these types. Internal refactors (e.g. the typed IDL model, #3) must keep the
//! JSON byte-identical; a failure here means a change would silently break the
//! deployed UI. Every test builds a fully-populated instance and asserts the
//! exact JSON, plus the `skip_serializing_if` minimal shapes where the frontend
//! relies on fields being absent.

use {
    crate::{
        analyze::{
            AccountDiff, AccountOverview, Analysis, Explanation, FieldDiff, Overview, ProgramInfo,
            ReplayResult, SigInfo, SimulationReport,
        },
        check::{AssertOutcome, ScenarioOutcome},
        compute::CuUsage,
        cpi_tree::{CpiEntry, IxAccount, IxArg},
        decode::{AccountInfo, DecodedAccount, Field},
        diffs::{BalanceChange, TokenChange},
        fixture::OnchainRecord,
        idl::{IdlAccountSpec, IdlArg, IdlError, IdlInstruction, PdaSeed},
        session::Replayed,
    },
    serde_json::json,
};

fn field() -> Field {
    Field {
        name: "amount".into(),
        offset: 64,
        ty: "u64".into(),
        size: 8,
        value: "1234".into(),
        editable: true,
        note: Some("raw base units".into()),
    }
}

fn replay_result() -> ReplayResult {
    ReplayResult {
        success: false,
        error: Some("InstructionError(0, Custom(6001))".into()),
        error_name: Some("SlippageToleranceExceeded".into()),
        logs: vec!["Program log: hi".into()],
        compute_units: 42_000,
    }
}

fn explanation() -> Explanation {
    Explanation {
        title: "SlippageToleranceExceeded".into(),
        detail: "Slippage tolerance exceeded".into(),
        program: Some("Prog111".into()),
        raw: "Custom(6001)".into(),
    }
}

fn account_diff() -> AccountDiff {
    AccountDiff {
        address: "Acc111".into(),
        owner: "Owner111".into(),
        lamports_before: 10,
        lamports_after: 7,
        fields: vec![FieldDiff {
            name: "amount".into(),
            ty: "u64".into(),
            before: "5".into(),
            after: "9".into(),
        }],
        raw_data_changed: true,
    }
}

#[test]
fn replay_result_wire_shape() {
    assert_eq!(
        serde_json::to_value(replay_result()).unwrap(),
        json!({
            "success": false,
            "error": "InstructionError(0, Custom(6001))",
            "error_name": "SlippageToleranceExceeded",
            "logs": ["Program log: hi"],
            "compute_units": 42_000,
        })
    );
    // error_name is omitted (not null) when unresolved.
    let minimal = ReplayResult {
        success: true,
        error: None,
        error_name: None,
        logs: vec![],
        compute_units: 0,
    };
    assert_eq!(
        serde_json::to_value(minimal).unwrap(),
        json!({ "success": true, "error": null, "logs": [], "compute_units": 0 })
    );
}

#[test]
fn field_and_decoded_account_wire_shape() {
    assert_eq!(
        serde_json::to_value(field()).unwrap(),
        json!({
            "name": "amount",
            "offset": 64,
            "type": "u64",
            "size": 8,
            "value": "1234",
            "editable": true,
            "note": "raw base units",
        })
    );
    let dec = DecodedAccount {
        type_name: "SPL Token Account".into(),
        fields: vec![field()],
    };
    let v = serde_json::to_value(dec).unwrap();
    assert_eq!(v["type_name"], "SPL Token Account");
    assert_eq!(v["fields"][0]["type"], "u64");
}

#[test]
fn account_info_wire_shape() {
    let info = AccountInfo {
        address: "Acc111".into(),
        owner: "Owner111".into(),
        lamports: 5,
        executable: false,
        data_len: 165,
        decoded: None,
    };
    // `decoded` is omitted when absent.
    assert_eq!(
        serde_json::to_value(info).unwrap(),
        json!({
            "address": "Acc111",
            "owner": "Owner111",
            "lamports": 5,
            "executable": false,
            "data_len": 165,
        })
    );
}

#[test]
fn cpi_entry_wire_shape() {
    let full = CpiEntry {
        discriminator: None,
        introspects: false,
        compute_units: None,
        index: 0,
        program: "Prog111".into(),
        stack_height: 1,
        name: Some("Transfer".into()),
        accounts: vec![IxAccount {
            name: Some("source".into()),
            address: "Acc111".into(),
        }],
        args: vec![IxArg {
            name: "lamports".into(),
            ty: "u64".into(),
            value: "64".into(),
        }],
        data: vec![1, 2, 3],
        account_indexes: vec![0],
    };
    // `data` / `account_indexes` never serialize; `type` is the arg key.
    assert_eq!(
        serde_json::to_value(full).unwrap(),
        json!({
            "index": 0,
            "program": "Prog111",
            "stack_height": 1,
            "name": "Transfer",
            "accounts": [{ "name": "source", "address": "Acc111" }],
            "args": [{ "name": "lamports", "type": "u64", "value": "64" }],
        })
    );
    // Undecodable instruction: name/accounts/args are omitted entirely.
    let minimal = CpiEntry {
        discriminator: None,
        introspects: false,
        compute_units: None,
        index: 1,
        program: "Prog111".into(),
        stack_height: 2,
        name: None,
        accounts: vec![],
        args: vec![],
        data: vec![],
        account_indexes: vec![],
    };
    assert_eq!(
        serde_json::to_value(minimal).unwrap(),
        json!({ "index": 1, "program": "Prog111", "stack_height": 2 })
    );
}

#[test]
fn diffs_and_compute_wire_shapes() {
    assert_eq!(
        serde_json::to_value(BalanceChange {
            address: "Acc111".into(),
            delta: -5,
            post: 95,
        })
        .unwrap(),
        json!({ "address": "Acc111", "delta": -5, "post": 95 })
    );
    assert_eq!(
        serde_json::to_value(TokenChange {
            address: "Tok111".into(),
            owner: "Own111".into(),
            mint: "Mint111".into(),
            decimals: 6,
            delta_raw: "-1000".into(),
            post_raw: "9000".into(),
        })
        .unwrap(),
        json!({
            "address": "Tok111",
            "owner": "Own111",
            "mint": "Mint111",
            "decimals": 6,
            "delta_raw": "-1000",
            "post_raw": "9000",
        })
    );
    assert_eq!(
        serde_json::to_value(CuUsage {
            program: "Prog111".into(),
            cu: 150,
        })
        .unwrap(),
        json!({ "program": "Prog111", "cu": 150 })
    );
}

#[test]
fn analysis_and_overview_wire_shape() {
    let analysis = Analysis {
        signature: "Sig111".into(),
        overview: Overview {
            success: true,
            fee: 5000,
            slot: Some(100),
            compute_units: Some(150),
            fee_payer: Some("Payer111".into()),
            version: "legacy".into(),
            block_time: Some(1_700_000_000),
            recent_blockhash: Some("Hash111".into()),
            account_count: 3,
            top_programs: vec!["Prog111".into()],
        },
        cpi_tree: vec![],
        balance_change: vec![],
        token_change: vec![],
        compute: vec![],
        logs: vec!["log".into()],
        replay: None,
        accounts: vec![],
    };
    assert_eq!(
        serde_json::to_value(analysis).unwrap(),
        json!({
            "signature": "Sig111",
            "overview": {
                "success": true,
                "fee": 5000,
                "slot": 100,
                "compute_units": 150,
                "fee_payer": "Payer111",
                "version": "legacy",
                "block_time": 1_700_000_000,
                "recent_blockhash": "Hash111",
                "account_count": 3,
                "top_programs": ["Prog111"],
            },
            "cpi_tree": [],
            "balance_change": [],
            "token_change": [],
            "compute": [],
            "logs": ["log"],
            "replay": null,
            "accounts": [],
        })
    );
}

#[test]
fn simulation_report_wire_shape() {
    let report = SimulationReport {
        replay: replay_result(),
        clock: Some("slot 5 · epoch 0".into()),
        explain: Some(explanation()),
        diffs: vec![account_diff()],
        preflight: None,
    };
    let v = serde_json::to_value(report).unwrap();
    assert_eq!(v["replay"]["success"], false);
    assert_eq!(v["clock"], "slot 5 · epoch 0");
    assert_eq!(v["explain"]["title"], "SlippageToleranceExceeded");
    assert_eq!(v["diffs"][0]["fields"][0]["type"], "u64");
    assert_eq!(v["diffs"][0]["raw_data_changed"], true);
    // clock/explain are omitted when absent.
    let minimal = SimulationReport {
        replay: replay_result(),
        clock: None,
        explain: None,
        diffs: vec![],
        preflight: None,
    };
    let v = serde_json::to_value(minimal).unwrap();
    assert!(v.get("clock").is_none() && v.get("explain").is_none());
}

#[test]
fn replayed_wire_shape() {
    let replayed = Replayed {
        result: replay_result(),
        diffs: vec![account_diff()],
        clock: None,
        explain: None,
    };
    let v = serde_json::to_value(replayed).unwrap();
    assert_eq!(v["result"]["compute_units"], 42_000);
    assert_eq!(v["diffs"][0]["address"], "Acc111");
    // Replayed carries clock/explain as explicit nulls (no skip attrs).
    assert!(v.get("clock").is_some() && v["clock"].is_null());
}

#[test]
fn scenario_outcome_wire_shape() {
    let outcome = ScenarioOutcome {
        name: "drain".into(),
        expect: "reverts".into(),
        pass: true,
        actual: replay_result(),
        asserts: vec![AssertOutcome {
            description: "Acc1…111 lamports == 0".into(),
            pass: true,
        }],
    };
    assert_eq!(
        serde_json::to_value(outcome).unwrap(),
        json!({
            "name": "drain",
            "expect": "reverts",
            "pass": true,
            "actual": {
                "success": false,
                "error": "InstructionError(0, Custom(6001))",
                "error_name": "SlippageToleranceExceeded",
                "logs": ["Program log: hi"],
                "compute_units": 42_000,
            },
            "asserts": [{ "description": "Acc1…111 lamports == 0", "pass": true }],
        })
    );
}

#[test]
fn onchain_record_wire_shape_round_trips() {
    let record = OnchainRecord {
        success: false,
        error: Some("{\"InstructionError\":[0,{\"Custom\":6003}]}".into()),
        fee: 5000,
        compute_units: Some(1200),
        slot: Some(648),
        block_time: Some(1_787_952_349),
        logs: vec!["Program log: cliff".into()],
    };
    let v = serde_json::to_value(&record).unwrap();
    assert_eq!(
        v,
        json!({
            "success": false,
            "error": "{\"InstructionError\":[0,{\"Custom\":6003}]}",
            "fee": 5000,
            "compute_units": 1200,
            "slot": 648,
            "block_time": 1_787_952_349,
            "logs": ["Program log: cliff"],
        })
    );
    // Fixture round-trip: the same shape deserializes back identically.
    let back: OnchainRecord = serde_json::from_value(v).unwrap();
    assert_eq!(back, record);
}

#[test]
fn account_overview_wire_shape() {
    let overview = AccountOverview {
        address: "Prog111".into(),
        exists: true,
        owner: "BPFLoaderUpgradeab1e11111111111111111111111".into(),
        lamports: 1,
        executable: true,
        data_len: 36,
        program: Some(ProgramInfo {
            program_data: "PD111".into(),
            upgradeable: true,
            upgrade_authority: Some("Auth111".into()),
            last_deployed_slot: Some(7),
        }),
        idl_name: Some("my_program".into()),
        decoded: None,
    };
    assert_eq!(
        serde_json::to_value(overview).unwrap(),
        json!({
            "address": "Prog111",
            "exists": true,
            "owner": "BPFLoaderUpgradeab1e11111111111111111111111",
            "lamports": 1,
            "executable": true,
            "data_len": 36,
            "program": {
                "program_data": "PD111",
                "upgradeable": true,
                "upgrade_authority": "Auth111",
                "last_deployed_slot": 7,
            },
            "idl_name": "my_program",
        })
    );
}

#[test]
fn sig_info_wire_shape() {
    assert_eq!(
        serde_json::to_value(SigInfo {
            signature: "Sig111".into(),
            slot: Some(9),
            err: false,
            block_time: Some(1_700_000_000),
        })
        .unwrap(),
        json!({ "signature": "Sig111", "slot": 9, "err": false, "block_time": 1_700_000_000 })
    );
}

#[test]
fn idl_instruction_wire_shape() {
    let instruction = IdlInstruction {
        name: "createVesting".into(),
        discriminator: vec![1, 2, 3, 4, 5, 6, 7, 8],
        docs: vec!["Creates a schedule".into()],
        accounts: vec![IdlAccountSpec {
            name: "schedule".into(),
            writable: true,
            signer: false,
            pda: true,
            seeds: vec![
                PdaSeed::Const {
                    bytes: vec![118, 101],
                },
                PdaSeed::Account {
                    path: "beneficiary".into(),
                },
            ],
            address: None,
        }],
        args: vec![IdlArg {
            name: "amount".into(),
            ty: "u64".into(),
        }],
    };
    assert_eq!(
        serde_json::to_value(instruction).unwrap(),
        json!({
            "name": "createVesting",
            "discriminator": [1, 2, 3, 4, 5, 6, 7, 8],
            "docs": ["Creates a schedule"],
            "accounts": [{
                "name": "schedule",
                "writable": true,
                "signer": false,
                "pda": true,
                "seeds": [
                    { "kind": "const", "bytes": [118, 101] },
                    { "kind": "account", "path": "beneficiary" },
                ],
            }],
            "args": [{ "name": "amount", "type": "u64" }],
        })
    );
    assert_eq!(
        serde_json::to_value(IdlError {
            code: 6003,
            name: "CliffNotReached".into(),
            msg: "The vesting cliff has not been reached".into(),
        })
        .unwrap(),
        json!({ "code": 6003, "name": "CliffNotReached", "msg": "The vesting cliff has not been reached" })
    );
}
