//! Mainnet account dumping: the store, the RPC shape, and the resolution the
//! [`Dump`](crate::fixture::Dump) fixture drives.
//!
//! # What a dump is
//!
//! A dump copies real accounts (or a real program) from a live cluster into a
//! test world so a fixture can exercise on-chain state the harness would
//! otherwise have to hand-build. The copied bytes are cached in a committed
//! `.parallax/` store next to the consuming project so that, once warm, a test
//! is fully offline and deterministic.
//!
//! Store misses are fetched in one blocking `getMultipleAccounts` request and
//! installed at the response's observed slot.

use {
    crate::fixture::{world::Ctx, Account, Pubkey},
    base64::{engine::general_purpose::STANDARD, Engine as _},
    serde_json::{json, Value},
    solana_sdk_ids::bpf_loader_upgradeable,
    std::{
        collections::BTreeMap,
        fs,
        io::Read as _,
        path::{Path, PathBuf},
        str::FromStr,
    },
};

/// Default endpoint used when a world sets no [`rpc`](crate::fixture::CtxBuilder::rpc):
/// the public mainnet-beta RPC. This is a code-only default — there is
/// deliberately no environment-variable override.
pub(crate) const DEFAULT_RPC_URL: &str = "https://api.mainnet-beta.solana.com";

/// Magic tag beginning every dump file: "parallax dump".
const DUMP_MAGIC: [u8; 4] = *b"PLXD";

/// Dump file format version. A file written by a newer format is rejected with
/// a "re-dump" message rather than silently misread.
const DUMP_FORMAT_VERSION: u16 = 1;

/// Length of the hand-framed header (magic + little-endian version) preceding
/// the wincode body. Kept outside wincode so a bad magic or version is caught
/// before decoding — never a huge allocation from a garbage length prefix.
const DUMP_HEADER_LEN: usize = 6;

/// File extension for a dump file inside the store or shared elsewhere.
const DUMP_EXT: &str = "dump";

/// Directory (next to the project manifest) that holds the committed store.
const STORE_DIR: &str = ".parallax";

/// Mixed-slot coherence threshold: one mainnet epoch (432,000 slots, ~2–3
/// days). Entries whose observed slots span more than this are unlikely to be a
/// coherent snapshot, so combining them warns once and points at
/// [`Dump::refresh_all`](crate::fixture::Dump::refresh_all).
const EPOCH_SLOTS: u64 = 432_000;

/// Loader-v3 `ProgramData` metadata header length preceding the ELF: a 4-byte
/// enum tag, an 8-byte slot, and a 33-byte `Option<Pubkey>` upgrade authority.
/// The runtime always reserves the full 45 bytes, so the ELF starts at offset
/// 45 regardless of whether an upgrade authority is present.
const PROGRAMDATA_HEADER_LEN: usize = 45;

/// Mainnet-beta genesis creation time (2020-03-16 14:29:00 UTC), the anchor for
/// [`sync_clock`](crate::fixture::Dump::sync_clock)'s slot-derived timestamp.
const MAINNET_GENESIS_UNIX: i64 = 1_584_368_940;

/// Approximate mainnet slot time, used to derive a wall-clock from a slot.
const MS_PER_SLOT: i64 = 400;

// Role codes persisted in the dump format.
const ROLE_ACCOUNT: u8 = 0;
const ROLE_PROGRAM: u8 = 1;
const ROLE_PROGRAMDATA: u8 = 2;

fn fetch(url: &str, request_body: &[u8]) -> Result<Vec<u8>, String> {
    let response = ureq::post(url)
        .set("content-type", "application/json")
        .send_bytes(request_body)
        .map_err(|error| format!("RPC request to {url} failed: {error}"))?;
    let mut body = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|error| format!("reading RPC response from {url} failed: {error}"))?;
    Ok(body)
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// How an entry is reinstalled into a world.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Role {
    /// A plain account, installed with `set_account`.
    Account,
    /// A program's executable account; installed by loading its ELF.
    Program,
    /// A loader-v3 `ProgramData` account; carried for its program's ELF.
    ProgramData,
}

impl Role {
    fn from_code(code: u8) -> Result<Self, String> {
        match code {
            ROLE_ACCOUNT => Ok(Self::Account),
            ROLE_PROGRAM => Ok(Self::Program),
            ROLE_PROGRAMDATA => Ok(Self::ProgramData),
            other => Err(format!("dump: unknown role code {other}")),
        }
    }

    fn code(self) -> u8 {
        match self {
            Self::Account => ROLE_ACCOUNT,
            Self::Program => ROLE_PROGRAM,
            Self::ProgramData => ROLE_PROGRAMDATA,
        }
    }
}

// ---------------------------------------------------------------------------
// Dump file format (wincode) — shared by the `.parallax/` store and `Load`
// ---------------------------------------------------------------------------

/// The wincode body following the [`DUMP_MAGIC`] + version header.
#[derive(wincode::SchemaRead, wincode::SchemaWrite)]
struct DumpBodyWire {
    slot: u64,
    entries: Vec<DumpEntryWire>,
}

/// One account (or program, whose `data` is its ELF) in a dump file.
#[derive(wincode::SchemaRead, wincode::SchemaWrite)]
struct DumpEntryWire {
    address: [u8; 32],
    lamports: u64,
    owner: [u8; 32],
    executable: u8,
    role: u8,
    data: Vec<u8>,
}

/// One decoded dump entry.
#[derive(Clone, Debug)]
pub(crate) struct StoredEntry {
    address: Pubkey,
    lamports: u64,
    owner: Pubkey,
    executable: bool,
    role: Role,
    data: Vec<u8>,
}

/// A decoded dump file: the observed slot and its entries.
#[derive(Debug)]
pub(crate) struct DumpFile {
    slot: u64,
    entries: Vec<StoredEntry>,
}

/// Serialize a dump file: [`DUMP_MAGIC`] + version + wincode body. This is the
/// one format for both a store file and a `Load` input, so any file the store
/// writes is directly loadable and shareable.
fn write_dump_file(slot: u64, entries: &[StoredEntry]) -> Vec<u8> {
    let body = DumpBodyWire {
        slot,
        entries: entries
            .iter()
            .map(|entry| DumpEntryWire {
                address: entry.address.to_bytes(),
                lamports: entry.lamports,
                owner: entry.owner.to_bytes(),
                executable: u8::from(entry.executable),
                role: entry.role.code(),
                data: entry.data.clone(),
            })
            .collect(),
    };
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&DUMP_MAGIC);
    bytes.extend_from_slice(&DUMP_FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&wincode::serialize(&body).expect("dump body serializes"));
    bytes
}

/// Parse a dump file, with actionable errors that name `path` and the expected
/// format version. Shared by the `Load` path and the `.parallax/` store, so the
/// message names the file rather than either caller.
fn read_dump_file(path: &Path, bytes: &[u8]) -> Result<DumpFile, String> {
    let display = path.display();
    if bytes.len() < DUMP_HEADER_LEN {
        return Err(format!(
            "{display} is truncated — not a parallax dump file (expected format v{DUMP_FORMAT_VERSION})"
        ));
    }
    if bytes[0..4] != DUMP_MAGIC {
        return Err(format!(
            "{display} is not a parallax dump file (bad magic; expected format v{DUMP_FORMAT_VERSION})"
        ));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != DUMP_FORMAT_VERSION {
        return Err(format!(
            "{display} is dump format v{version}, but this build reads v{DUMP_FORMAT_VERSION}; re-dump the file"
        ));
    }
    let body: DumpBodyWire = wincode::deserialize_exact(&bytes[DUMP_HEADER_LEN..])
        .map_err(|error| format!("{display} is corrupt or truncated: {error:?}"))?;
    let mut entries = Vec::with_capacity(body.entries.len());
    for entry in body.entries {
        entries.push(StoredEntry {
            address: Pubkey::new_from_array(entry.address),
            lamports: entry.lamports,
            owner: Pubkey::new_from_array(entry.owner),
            executable: entry.executable != 0,
            role: Role::from_code(entry.role)?,
            data: entry.data,
        });
    }
    Ok(DumpFile {
        slot: body.slot,
        entries,
    })
}

// ---------------------------------------------------------------------------
// Store (a directory of per-primary dump files)
// ---------------------------------------------------------------------------

/// The committed `.parallax/` store: a directory of `<primary-address>.dump`
/// files, each a self-contained [`DumpFile`]. Because a store file *is* a dump
/// file, users share a dump by copying the file out of `.parallax/` and
/// [`Load`](crate::fixture::Load)-ing it by path. Only the harness reads or writes
/// the store, so the format has exactly one implementation.
pub(crate) struct DumpStore {
    dir: PathBuf,
}

impl DumpStore {
    pub(crate) fn open(project_dir: &Path) -> Self {
        Self {
            dir: project_dir.join(STORE_DIR),
        }
    }

    fn file_path(&self, primary: &Pubkey) -> PathBuf {
        self.dir.join(format!("{primary}.{DUMP_EXT}"))
    }

    /// The stored dump file for `primary`, if present.
    fn get(&self, primary: &Pubkey) -> Result<Option<DumpFile>, String> {
        let path = self.file_path(primary);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(read_dump_file(&path, &bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("dump: could not read {}: {error}", path.display())),
        }
    }

    /// Write a one-primary dump file with `entries` observed at `slot`.
    fn put(&self, primary: &Pubkey, slot: u64, entries: &[StoredEntry]) -> Result<(), String> {
        fs::create_dir_all(&self.dir)
            .map_err(|error| format!("dump: could not create {}: {error}", self.dir.display()))?;
        let path = self.file_path(primary);
        fs::write(&path, write_dump_file(slot, entries))
            .map_err(|error| format!("dump: could not write {}: {error}", path.display()))
    }

    /// Every stored file's primary address and unit kind (Account or Program),
    /// for a refresh that re-fetches everything. Sorted for determinism.
    fn stored_units(&self) -> Result<Vec<(Pubkey, Role)>, String> {
        let read = match fs::read_dir(&self.dir) {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(format!(
                    "dump: could not read {}: {error}",
                    self.dir.display()
                ))
            }
        };
        let mut units = Vec::new();
        for entry in read {
            let path = entry
                .map_err(|error| format!("dump: could not read {}: {error}", self.dir.display()))?
                .path();
            if path.extension().and_then(|ext| ext.to_str()) != Some(DUMP_EXT) {
                continue;
            }
            let Some(primary) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| Pubkey::from_str(stem).ok())
            else {
                continue;
            };
            let bytes = fs::read(&path)
                .map_err(|error| format!("dump: could not read {}: {error}", path.display()))?;
            let file = read_dump_file(&path, &bytes)?;
            let kind = if file.entries.iter().any(|entry| entry.role == Role::Program) {
                Role::Program
            } else {
                Role::Account
            };
            units.push((primary, kind));
        }
        units.sort_by_key(|(primary, _)| primary.to_bytes());
        Ok(units)
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC shape (getMultipleAccounts, base64, one observed slot)
// ---------------------------------------------------------------------------

/// One account fetched from the cluster (no slot; the slot is per-batch).
struct FetchedAccount {
    lamports: u64,
    owner: Pubkey,
    executable: bool,
    data: Vec<u8>,
}

/// Build a single batched `getMultipleAccounts` request (base64 encoding) for
/// every miss address at once — the whole array observed at one slot.
fn build_request(addresses: &[Pubkey]) -> Vec<u8> {
    let encoded: Vec<String> = addresses.iter().map(Pubkey::to_string).collect();
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getMultipleAccounts",
        "params": [encoded, { "encoding": "base64" }],
    });
    serde_json::to_vec(&body).expect("serializing a JSON-RPC request never fails")
}

/// Parse a `getMultipleAccounts` response into the observed slot and one entry
/// per requested address (`None` where the account does not exist on chain).
fn parse_response(
    addresses: &[Pubkey],
    body: &[u8],
) -> Result<(u64, Vec<Option<FetchedAccount>>), String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|error| format!("dump: RPC response was not JSON: {error}"))?;
    if let Some(error) = value.get("error") {
        return Err(format!("dump: RPC returned an error: {error}"));
    }
    let result = value
        .get("result")
        .ok_or("dump: RPC response is missing `result`")?;
    let slot = result
        .get("context")
        .and_then(|context| context.get("slot"))
        .and_then(Value::as_u64)
        .ok_or("dump: RPC response is missing the context slot")?;
    let values = result
        .get("value")
        .and_then(Value::as_array)
        .ok_or("dump: RPC response is missing the `value` array")?;
    if values.len() != addresses.len() {
        return Err(format!(
            "dump: RPC returned {} accounts for {} requested addresses",
            values.len(),
            addresses.len()
        ));
    }
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        if item.is_null() {
            out.push(None);
            continue;
        }
        let lamports = field_u64(item, "lamports")?;
        let owner = field_pubkey(item, "owner")?;
        let executable = item
            .get("executable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let data = decode_rpc_data(item.get("data").ok_or("dump: account is missing `data`")?)?;
        out.push(Some(FetchedAccount {
            lamports,
            owner,
            executable,
            data,
        }));
    }
    Ok((slot, out))
}

/// Decode `getMultipleAccounts`' base64 data field (`["<base64>", "base64"]`,
/// or a bare string for tolerance).
fn decode_rpc_data(field: &Value) -> Result<Vec<u8>, String> {
    let encoded = match field {
        Value::Array(parts) => parts
            .first()
            .and_then(Value::as_str)
            .ok_or("dump: account data array is malformed")?,
        Value::String(text) => text.as_str(),
        _ => return Err("dump: account data is neither an array nor a string".into()),
    };
    STANDARD
        .decode(encoded)
        .map_err(|error| format!("dump: account data is not valid base64: {error}"))
}

fn field_u64(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("dump: missing or non-integer `{key}`"))
}

fn field_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("dump: missing or non-string `{key}`"))
}

fn field_pubkey(value: &Value, key: &str) -> Result<Pubkey, String> {
    let text = field_str(value, key)?;
    Pubkey::from_str(text)
        .map_err(|error| format!("dump: `{key}` is not an address ({text}): {error}"))
}

/// Loader-v3 programdata address for `program_id`.
fn programdata_address(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Extract a program's ELF from its fetched account: the programdata body for
/// the upgradeable loader, or the program account itself for the older loaders.
fn program_elf(
    program_id: &Pubkey,
    program: &FetchedAccount,
    fetched: &BTreeMap<Pubkey, Option<FetchedAccount>>,
) -> Result<Vec<u8>, String> {
    if program.owner != bpf_loader_upgradeable::ID {
        return Ok(program.data.clone());
    }
    let programdata = programdata_address(program_id);
    let source = fetched
        .get(&programdata)
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            format!(
                "dump: program {program_id} is loader-v3 but its programdata {programdata} was \
                 not returned"
            )
        })?;
    Ok(source
        .data
        .get(PROGRAMDATA_HEADER_LEN..)
        .ok_or_else(|| format!("dump: programdata {programdata} is too small to hold an ELF"))?
        .to_vec())
}

// ---------------------------------------------------------------------------
// Resolution (installed into the world)
// ---------------------------------------------------------------------------

impl Ctx {
    /// Install every entry of the dump file at `path` — no store, no network.
    /// Returns the installed primary addresses (a program's id when
    /// `expect_program`). Backs [`Load`](crate::fixture::Load).
    fn load_dump(&mut self, path: &str, expect_program: bool) -> Result<Vec<Pubkey>, String> {
        let path = Path::new(path);
        let bytes = fs::read(path)
            .map_err(|error| format!("load: could not read {}: {error}", path.display()))?;
        let file = read_dump_file(path, &bytes)?;
        let program = file
            .entries
            .iter()
            .find(|entry| entry.role == Role::Program);
        if expect_program && program.is_none() {
            return Err(format!(
                "load: {} contains no program — use Load::accounts for an account file",
                path.display()
            ));
        }
        let addresses = if expect_program {
            vec![
                program
                    .expect("program present after the check above")
                    .address,
            ]
        } else {
            file.entries
                .iter()
                .filter(|entry| entry.role != Role::ProgramData)
                .map(|entry| entry.address)
                .collect()
        };
        self.install_entries(&file.entries)?;
        self.record_dumped(&file.entries, file.slot);
        self.finish_dump(false, None);
        Ok(addresses)
    }

    /// Install resolved dump entries. Accounts are set directly; a program is
    /// made executable by loading its stored ELF under the loader that owns it.
    fn install_entries(&mut self, entries: &[StoredEntry]) -> Result<(), String> {
        for entry in entries {
            match entry.role {
                Role::Account => self.backend.set_account(Account {
                    address: entry.address,
                    lamports: entry.lamports,
                    data: entry.data.clone(),
                    owner: entry.owner,
                    executable: entry.executable,
                }),
                Role::Program => {
                    self.backend.load_program_with_loader(
                        &entry.address,
                        &entry.data,
                        entry.owner,
                    )?;
                }
                // Never stored; a programdata account is folded into its program.
                Role::ProgramData => {}
            }
        }
        Ok(())
    }

    /// Record installed dump entries for coherence tracking and guided errors.
    fn record_dumped(&mut self, entries: &[StoredEntry], slot: u64) {
        for entry in entries {
            self.dumped_addresses.push(entry.address);
            self.dumped_slots.push(slot);
        }
    }

    /// Apply `sync_clock` (from the most recent touched slot) and emit the
    /// mixed-slot coherence warning at most once per world.
    fn finish_dump(&mut self, sync_clock: bool, slot: Option<u64>) {
        if sync_clock {
            if let Some(slot) = slot {
                let timestamp = MAINNET_GENESIS_UNIX + (slot as i64) * MS_PER_SLOT / 1000;
                self.backend.sync_clock(slot, timestamp);
            }
        }
        if self.dump_warned {
            return;
        }
        if let Some(warning) = coherence_warning(&self.dumped_slots, EPOCH_SLOTS) {
            self.dump_warned = true;
            eprintln!("{warning}");
        }
    }

    /// Whether this world has any dumped accounts (drives guided errors).
    pub(crate) fn has_dumps(&self) -> bool {
        !self.dumped_addresses.is_empty()
    }

    /// Dump accounts, using the store before fetching any misses.
    pub(crate) fn dump_accounts_native(&mut self, addresses: &[Pubkey], sync_clock: bool) {
        let targets: Vec<(Pubkey, Role)> = addresses
            .iter()
            .map(|address| (*address, Role::Account))
            .collect();
        self.run_dump_native(&targets, sync_clock, false);
    }

    /// Dump a program and its loader-v3 programdata coherently.
    pub(crate) fn dump_program_native(&mut self, program_id: Pubkey, sync_clock: bool) {
        self.run_dump_native(&[(program_id, Role::Program)], sync_clock, false);
    }

    /// Re-fetch every known store entry in one coherent batch.
    pub(crate) fn refresh_all_native(&mut self) -> Vec<Pubkey> {
        self.run_dump_native(&[], false, true)
    }

    /// Native path for `Load`: install a dump file by path, panicking with an
    /// actionable message on failure, as the rest of fixture setup does.
    pub(crate) fn load_file_native(&mut self, path: &Path, expect_program: bool) -> Vec<Pubkey> {
        let path_str = path
            .to_str()
            .unwrap_or_else(|| panic!("load: path is not valid UTF-8: {}", path.display()));
        self.load_dump(path_str, expect_program)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn run_dump_native(
        &mut self,
        targets: &[(Pubkey, Role)],
        sync_clock: bool,
        refresh: bool,
    ) -> Vec<Pubkey> {
        let dir = self.project_dir_string();
        let store = DumpStore::open(Path::new(&dir));
        let units = if refresh {
            store
                .stored_units()
                .unwrap_or_else(|error| panic!("{error}"))
        } else {
            targets.to_vec()
        };
        let resolved = units.iter().map(|(address, _)| *address).collect();

        let mut misses = Vec::new();
        let mut hit_slot = None;
        for (primary, role) in units {
            if !refresh {
                if let Some(file) = store
                    .get(&primary)
                    .unwrap_or_else(|error| panic!("{error}"))
                {
                    self.install_entries(&file.entries)
                        .unwrap_or_else(|error| panic!("{error}"));
                    self.record_dumped(&file.entries, file.slot);
                    hit_slot = Some(hit_slot.map_or(file.slot, |slot: u64| slot.max(file.slot)));
                    continue;
                }
            }
            misses.push((primary, role));
            if role == Role::Program {
                misses.push((programdata_address(&primary), Role::ProgramData));
            }
        }

        if misses.is_empty() {
            self.finish_dump(sync_clock, hit_slot);
            return resolved;
        }

        let addresses: Vec<Pubkey> = misses.iter().map(|(address, _)| *address).collect();
        let response = fetch(&self.rpc_url, &build_request(&addresses))
            .unwrap_or_else(|error| panic!("parallax dump: {error}"));
        let (slot, values) = parse_response(&addresses, &response)
            .unwrap_or_else(|error| panic!("parallax dump: {error}"));

        let fetched: BTreeMap<_, _> = addresses.into_iter().zip(values).collect();
        let mut installed = Vec::new();
        for (address, role) in misses {
            let entry = match role {
                Role::Account => match fetched.get(&address).and_then(Option::as_ref) {
                    Some(account) => StoredEntry {
                        address,
                        lamports: account.lamports,
                        owner: account.owner,
                        executable: account.executable,
                        role,
                        data: account.data.clone(),
                    },
                    None => {
                        eprintln!(
                            "parallax dump: account {address} does not exist on chain; \
                             skipped (dump only real addresses)"
                        );
                        continue;
                    }
                },
                Role::Program => {
                    let Some(program) = fetched.get(&address).and_then(Option::as_ref) else {
                        eprintln!(
                            "parallax dump: program {address} does not exist on chain; skipped"
                        );
                        continue;
                    };
                    StoredEntry {
                        address,
                        lamports: program.lamports,
                        owner: program.owner,
                        executable: true,
                        role,
                        data: program_elf(&address, program, &fetched)
                            .unwrap_or_else(|error| panic!("parallax dump: {error}")),
                    }
                }
                Role::ProgramData => continue,
            };
            store
                .put(&address, slot, std::slice::from_ref(&entry))
                .unwrap_or_else(|error| panic!("parallax dump: {error}"));
            installed.push(entry);
        }

        self.install_entries(&installed)
            .unwrap_or_else(|error| panic!("parallax dump: {error}"));
        self.record_dumped(&installed, slot);
        eprintln!(
            "parallax: dumped {} account(s) @ slot {slot}",
            installed.len()
        );
        self.finish_dump(sync_clock, Some(slot));
        resolved
    }

    /// Resolve the project directory whose `.parallax/` store this world uses:
    /// the builder-set directory (the `#[parallax_test]` macro passes
    /// `CARGO_MANIFEST_DIR`), then the `CARGO_MANIFEST_DIR` runtime variable,
    /// then the nearest ancestor of the working directory that has a
    /// `Cargo.toml`.
    fn project_dir_string(&self) -> String {
        if let Some(dir) = &self.project_dir {
            return dir.clone();
        }
        if let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
            return dir.to_string_lossy().into_owned();
        }
        if let Ok(cwd) = std::env::current_dir() {
            for ancestor in cwd.ancestors() {
                if ancestor.join("Cargo.toml").is_file() {
                    return ancestor.to_string_lossy().into_owned();
                }
            }
            return cwd.to_string_lossy().into_owned();
        }
        ".".into()
    }
}

/// The mixed-slot coherence warning for a set of observed slots, or `None` when
/// they fall within `threshold` of each other. Pure so it can be tested without
/// capturing stderr.
fn coherence_warning(slots: &[u64], threshold: u64) -> Option<String> {
    let min = slots.iter().min()?;
    let max = slots.iter().max()?;
    (max - min > threshold).then(|| {
        format!(
            "parallax dump: combining accounts across a {}-slot range ({min}..={max}, more than \
             one epoch) — the world may not be a coherent snapshot; call Dump::refresh_all() to \
             re-fetch every entry at one slot",
            max - min
        )
    })
}

/// The guided-error hint appended when a transaction fails on an account the
/// world never installed, in a world that has dumped accounts. Returns `None`
/// unless `has_dumps` and a named read-only account is genuinely absent.
pub(crate) fn missing_account_hint(missing: Option<Pubkey>) -> Option<String> {
    missing.map(|address| {
        format!(
            "missing account {address}: if it exists on mainnet, add it to your \
             dump accounts fixture"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_dump_installs_without_network() {
        let root = std::env::temp_dir().join(format!(
            "litesvm-fixture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let address = Pubkey::new_from_array([7; 32]);
        DumpStore::open(&root)
            .put(
                &address,
                42,
                &[StoredEntry {
                    address,
                    lamports: 9,
                    owner: Pubkey::new_from_array([8; 32]),
                    executable: false,
                    role: Role::Account,
                    data: vec![1, 2, 3],
                }],
            )
            .unwrap();
        let mut ctx = Ctx::builder(Pubkey::new_from_array([9; 32]))
            .no_program()
            .project_dir(root.to_string_lossy())
            .rpc("http://127.0.0.1:0")
            .build()
            .unwrap();

        ctx.dump_accounts_native(&[address], false);

        assert_eq!(ctx.account(address).unwrap().data, [1, 2, 3]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
