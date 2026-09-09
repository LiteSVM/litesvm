//! Compute profiler: where every BPF instruction of a transaction went.
//!
//! LiteSVM's register tracing records every instruction each program frame
//! executes, including CPIs. Walking that trace with the executable's static
//! analysis (function boundaries from the call graph, syscall names from the
//! loader) attributes instructions to functions and to syscalls, and folds the
//! call stacks into flamegraph input.
//!
//! Counts are *executed BPF instructions*, which cost one compute unit each.
//! Syscalls cost extra per call (their published prices); they are counted
//! separately so the two can be combined by the caller.
//!
//! Mainnet programs are stripped: only `entrypoint` and `custom_panic` keep
//! names, every other function is `function_<pc>`. Function *boundaries* are
//! still exact, so the profile's shape is exact; names come from an unstripped
//! build of the same program when the caller has one (see
//! [`Profile::symbolize`]).

use {
    litesvm::{InvocationInspectCallback, LiteSVM},
    serde::Serialize,
    solana_program_runtime::{
        invoke_context::{Executable, InvokeContext, RegisterTrace},
        solana_sbpf::ebpf,
    },
    solana_transaction::sanitized::SanitizedTransaction,
    solana_transaction_context::{instruction::InstructionContext, IndexOfAccount},
    std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    },
};

/// One function's share of a frame.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionProfile {
    /// `entrypoint`, a symbol name, or `function_<pc>` for a stripped program.
    pub name: String,
    /// What the function *does*, derived from the trace when its name is
    /// anonymous: `Buy handler` (it logged `Instruction: Buy`), `CPI → Token
    /// Program: Transfer`, `PDA derivation`, `emits event`, `hashing`,
    /// `error: SlippageExceeded`, … `None` when nothing in the trace says.
    pub label: Option<String>,
    /// The function's first instruction.
    pub pc: usize,
    /// Instructions executed inside this function itself.
    pub self_insns: u64,
    /// Instructions executed inside this function and everything it called.
    pub total_insns: u64,
    /// Times it was entered.
    pub calls: u64,
    /// Estimated compute units: the frame's measured CU split across its
    /// functions in proportion to `self_insns`. Exact when the frame made no
    /// syscalls (one CU per instruction); otherwise the syscall overhead is
    /// spread proportionally. `None` until [`Profile`] has frame compute.
    pub compute_units: Option<u64>,
    /// The function's code shape (see [`Shape`]), for matching it to the same
    /// function in another build of the same source. Not serialized.
    #[serde(skip)]
    pub shape: Shape,
}

/// A build-independent fingerprint of one function's code: its instruction
/// sequence with everything that moves between builds normalised away —
/// jump offsets, call targets, 64-bit immediates (addresses of data) — and
/// its length. Two builds of the same source produce the same shape for the
/// same function far more often than they produce the same address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shape {
    /// FNV-1a over the normalised instruction stream.
    pub full: u64,
    /// FNV-1a over opcodes only — the fallback when `full` finds no match.
    pub opcodes: u64,
    /// Instruction count.
    pub len: usize,
    /// Opcode histogram (indexed by opcode byte), for the near-miss tier:
    /// two builds of one function differ in a few instructions, not in what
    /// kinds of instructions they are made of.
    pub histogram: [u16; 256],
}

impl Shape {
    /// Similarity of two opcode histograms in `[0, 1]`: shared opcode mass
    /// over total mass. 1.0 means identical multisets of opcodes.
    pub fn similarity(&self, other: &Shape) -> f64 {
        let (mut shared, mut total) = (0u32, 0u32);
        for i in 0..256 {
            let (a, b) = (self.histogram[i] as u32, other.histogram[i] as u32);
            shared += a.min(b);
            total += a.max(b);
        }
        if total == 0 {
            1.0
        } else {
            shared as f64 / total as f64
        }
    }
}

impl Shape {
    /// Shape of the instructions `[start, end)` in `text` (sBPF, 8-byte slots).
    pub fn of(text: &[u8], start: usize, end: usize) -> Shape {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0100_0000_01b3;
        let mut full = FNV_OFFSET;
        let mut opcodes = FNV_OFFSET;
        let mix = |h: &mut u64, bytes: &[u8]| {
            for &b in bytes {
                *h ^= b as u64;
                *h = h.wrapping_mul(FNV_PRIME);
            }
        };
        let end = end.min(text.len() / ebpf::INSN_SIZE);
        let mut histogram = [0u16; 256];
        let mut pc = start;
        while pc < end {
            let insn = ebpf::get_insn_unchecked(text, pc);
            histogram[insn.opc as usize] = histogram[insn.opc as usize].saturating_add(1);
            let is_lddw = insn.opc == ebpf::LD_DW_IMM;
            let is_jump = insn.opc & 0x07 == 0x05 || insn.opc & 0x07 == 0x06;
            let is_call = insn.opc == ebpf::CALL_IMM || insn.opc == ebpf::CALL_REG;
            mix(&mut opcodes, &[insn.opc]);
            mix(&mut full, &[insn.opc, insn.dst, insn.src]);
            if !is_jump {
                mix(&mut full, &insn.off.to_le_bytes());
            }
            if !is_lddw && !is_call {
                mix(&mut full, &(insn.imm as i32).to_le_bytes());
            }
            pc += if is_lddw { 2 } else { 1 };
        }
        Shape {
            full,
            opcodes,
            len: end.saturating_sub(start),
            histogram,
        }
    }
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            full: 0,
            opcodes: 0,
            len: 0,
            histogram: [0; 256],
        }
    }
}

/// One line of a shape corpus: a function's shape and its name, from a build
/// with symbols. Library code (`core`, `alloc`, borsh, `anchor_lang`,
/// `solana_program`) compiles to the same shape in every program built with
/// the same toolchain, so a corpus from open-source programs names the same
/// functions inside stripped ones.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct CorpusEntry {
    /// [`Shape::full`].
    pub full: u64,
    /// [`Shape::len`].
    pub len: usize,
    /// Demangled, hash-stripped symbol name.
    pub name: String,
    /// [`Shape::opcodes`]: the opcode-only hash, for the approximate tier
    /// (same instruction kinds in the same order, registers and constants
    /// ignored). Absent in entries from older corpus dumps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opcodes: Option<u64>,
}

/// Shortest function the corpus will name: below this, shapes are generic
/// (a two-slot getter, a thunk) and identical across unrelated programs.
pub const MIN_CORPUS_LEN: usize = 8;

/// Dump every named function of a build (its `.so` for code, its `.debug` for
/// symbols) as corpus entries. Functions whose shape is shared by several
/// names in this build are dropped as ambiguous.
pub fn corpus_from_build(so: &[u8], debug: &[u8]) -> crate::Result<Vec<CorpusEntry>> {
    let (text, symbols) = elf_parse(so)
        .and_then(|(text, _)| elf_parse(debug).map(|(_, syms)| (text, syms)))
        .ok_or_else(|| {
            crate::Error::InvalidSpec("expected an ELF .so and a .debug with a symbol table".into())
        })?;
    let mut by_full: BTreeMap<u64, Option<CorpusEntry>> = BTreeMap::new();
    for (&pc, (name, size)) in &symbols {
        if *size < MIN_CORPUS_LEN {
            // Tiny functions (a getter is `ldxdw; exit`) are the same bytes in
            // every program; naming them from a corpus mislabels far more than
            // it names.
            continue;
        }
        let sh = Shape::of(&text, pc, pc + size);
        let pretty = strip_hash(&rustc_demangle::demangle(name).to_string());
        by_full
            .entry(sh.full)
            .and_modify(|v| {
                if v.as_ref().is_some_and(|e| e.name != pretty) {
                    *v = None;
                }
            })
            .or_insert(Some(CorpusEntry {
                full: sh.full,
                len: sh.len,
                name: pretty,
                opcodes: Some(sh.opcodes),
            }));
    }
    Ok(by_full.into_values().flatten().collect())
}

/// A program's complete symbol table from a build byte-identical to what is
/// on chain: `pc → demangled name`, guarded by the stripped sha256 of the ELF
/// (the same "executable hash" `solana-verify` compares).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExactSymbols {
    /// Program id the map belongs to.
    pub program: String,
    /// [`stripped_sha256`] of the build's `.so`; must equal the on-chain ELF's.
    pub elf_sha256: String,
    /// `pc → demangled function name` from the build's `.debug`.
    pub symbols: BTreeMap<usize, String>,
}

/// sha256 of an ELF with its zero padding removed — how on-chain program data
/// (padded to the account size) is compared with a local build.
pub fn stripped_sha256(elf: &[u8]) -> String {
    use sha2::Digest;
    let end = elf.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
    format!("{:x}", sha2::Sha256::digest(&elf[..end]))
}

/// Build an [`ExactSymbols`] from a build's `.so` (hashed) and its `.debug`
/// (symbol table). Fails when the two are not the same build.
pub fn exact_from_build(program: &str, so: &[u8], debug: &[u8]) -> crate::Result<ExactSymbols> {
    let symbols_sized: FunctionSymbols = elf_parse(debug)
        .map(|(_, s)| s)
        .ok_or_else(|| crate::Error::InvalidSpec("not an ELF with a symbol table".into()))?;
    let symbols: BTreeMap<usize, String> = symbols_sized
        .iter()
        .map(|(pc, (name, _))| (*pc, name.clone()))
        .collect();
    if !symbols.values().any(|n| n == "entrypoint") {
        return Err(crate::Error::InvalidSpec(
            "the .debug ELF has no entrypoint symbol".into(),
        ));
    }
    let (_, so_syms) =
        elf_parse(so).ok_or_else(|| crate::Error::InvalidSpec("not an ELF".into()))?;
    // The stripped .so keeps `entrypoint`; it must sit at the same address
    // and compile to the same code, or the two are not one build.
    let (so_text, _) =
        elf_parse(so).ok_or_else(|| crate::Error::InvalidSpec("not an ELF".into()))?;
    let (dbg_text, _) =
        elf_parse(debug).ok_or_else(|| crate::Error::InvalidSpec("not an ELF".into()))?;
    let so_entry = so_syms
        .iter()
        .find(|(_, (n, _))| n == "entrypoint")
        .map(|(pc, _)| *pc);
    let dbg_entry = symbols
        .iter()
        .find(|(_, n)| *n == "entrypoint")
        .map(|(pc, _)| *pc);
    if so_entry.is_some() && so_entry != dbg_entry {
        return Err(crate::Error::InvalidSpec(
            ".so and .debug are not the same build".into(),
        ));
    }
    if let (Some(pc), Some((_, size))) = (dbg_entry, symbols_sized.get(&dbg_entry.unwrap_or(0))) {
        let a = Shape::of(&so_text, pc, pc + size);
        let b = Shape::of(&dbg_text, pc, pc + size);
        if a.full != b.full {
            return Err(crate::Error::InvalidSpec(
                ".so and .debug are not the same build (entrypoint differs)".into(),
            ));
        }
    }
    Ok(ExactSymbols {
        program: program.to_string(),
        elf_sha256: stripped_sha256(so),
        symbols: symbols
            .into_iter()
            .map(|(pc, sym)| (pc, strip_hash(&rustc_demangle::demangle(&sym).to_string())))
            .collect(),
    })
}

/// Exact symbol maps shipped with the crate (`symbols/exact.jsonl.gz`, one
/// JSON map per line), one per verified mainnet program rebuilt byte-for-byte
/// with symbols. Decoded once per process.
pub fn builtin_exact() -> &'static [ExactSymbols] {
    static EXACT: std::sync::LazyLock<Vec<ExactSymbols>> = std::sync::LazyLock::new(|| {
        use std::io::Read;
        const GZ: &[u8] = include_bytes!("../symbols/exact.jsonl.gz");
        let mut text = String::new();
        if GZ.is_empty()
            || flate2::read::GzDecoder::new(GZ)
                .read_to_string(&mut text)
                .is_err()
        {
            return Vec::new();
        }
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    });
    &EXACT
}

/// The corpus shipped with the crate (`symbols/corpus.jsonl.gz`): shapes of
/// library and open-source program functions across platform-tools
/// versions. Decoded once per process.
pub fn builtin_corpus() -> &'static [CorpusEntry] {
    static CORPUS: std::sync::LazyLock<Vec<CorpusEntry>> = std::sync::LazyLock::new(|| {
        use std::io::Read;
        const GZ: &[u8] = include_bytes!("../symbols/corpus.jsonl.gz");
        if GZ.is_empty() {
            return Vec::new();
        }
        let mut text = String::new();
        if flate2::read::GzDecoder::new(GZ)
            .read_to_string(&mut text)
            .is_err()
        {
            return Vec::new();
        }
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    });
    &CORPUS
}

/// What [`Profile::symbolize_from_build`] managed to name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SymbolizeReport {
    /// Functions named by an exact code-shape match.
    pub exact: usize,
    /// Functions named by an opcode-only shape match (unique on both sides).
    pub by_opcodes: usize,
    /// Functions named as the unique near-miss: same length within 10% and
    /// an opcode histogram at least 90% similar, with no runner-up close by.
    pub by_similarity: usize,
    /// Functions in the profiled frames left unnamed.
    pub unmatched: usize,
    /// True when the build turned out to be the very same one (mapped by address).
    pub same_build: bool,
}

/// One program frame (a top-level instruction or a CPI) of the transaction.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FrameProfile {
    /// The instruction this frame ran, decoded the same way the Analyze tree
    /// names it ("Swap", "Mint Levercoin Lst"); `None` when it could not be
    /// decoded. Attached by [`Profile::attach_names`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Compute this instruction consumed *on chain*, from the transaction's
    /// own logs — to compare with `compute_units`, which is what the replay
    /// charged. Attached by [`Profile::attach_names`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onchain_compute_units: Option<u64>,
    /// The program that ran.
    pub program: String,
    /// Total BPF instructions the frame executed.
    pub instructions: u64,
    /// Compute units the runtime charged this frame itself: its `consumed`
    /// log line minus the CPIs it made. `None` when the logs did not report it.
    pub compute_units: Option<u64>,
    /// `compute_units - instructions`: what syscalls, CPI overhead and
    /// account serialization cost beyond one CU per BPF instruction.
    pub syscall_overhead: Option<u64>,
    /// Per-function breakdown, largest `self_insns` first.
    pub functions: Vec<FunctionProfile>,
    /// Syscall name → number of calls.
    pub syscalls: Vec<(String, u64)>,
    /// Folded stacks (`a;b;c`) → instructions, the flamegraph input.
    pub stacks: Vec<(String, u64)>,
    /// Every syscall in trace order with the function (start pc) that made
    /// it — the evidence the frame's behavioural labels are derived from.
    #[serde(skip)]
    pub events: Vec<(usize, String)>,
}

/// Where a syscall's price comes from: the runtime's fixed per-call charge
/// (`solana-program-runtime` execution budget defaults). Calls that also
/// charge per byte (memory ops, CPI account data, hashing, return data) are
/// floored at their base, so an estimate built from these is a *lower bound*.
fn syscall_base_cost(name: &str) -> u64 {
    match name {
        // `DEFAULT_INVOCATION_COST` in solana-program-runtime 4.2.
        "sol_invoke_signed_rust" | "sol_invoke_signed_c" => 946,
        "sol_create_program_address" | "sol_try_find_program_address" => 1_500,
        "sol_secp256k1_recover" => 25_000,
        "sol_sha256" | "sol_keccak256" | "sol_blake3" | "sol_poseidon" => 85,
        "sol_memcpy_" | "sol_memmove_" | "sol_memcmp_" | "sol_memset_" => 10,
        _ => 100, // sol_log_*, sysvar getters, return data, everything else
    }
}

impl FrameProfile {
    /// A lower-bound itemisation of `syscall_overhead`: each syscall's calls
    /// times its fixed charge. The gap between the sum and the measured
    /// overhead is data-size dependent cost (bytes copied, compared, hashed
    /// or passed to CPIs) the trace cannot see.
    pub fn syscall_estimate(&self) -> Vec<(String, u64, u64)> {
        let mut v: Vec<(String, u64, u64)> = self
            .syscalls
            .iter()
            .map(|(name, calls)| (name.clone(), *calls, calls * syscall_base_cost(name)))
            .collect();
        v.sort_by_key(|(_, _, cu)| std::cmp::Reverse(*cu));
        v
    }
}

/// The whole transaction's profile: one entry per program frame, in
/// execution order.
#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    /// One entry per program frame, in execution order.
    pub frames: Vec<FrameProfile>,
    /// How many program frames the transaction ran on chain (instructions that
    /// entered the VM). Compared with `frames.len()` it shows how far a
    /// diverged replay got. Attached by [`Profile::attach_names`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onchain_frames: Option<usize>,
}

impl Profile {
    /// Attach the runtime's measured compute to each frame from the
    /// transaction's logs, *exclusive* of the CPIs the frame made: a
    /// `Program X consumed N` line counts the whole subtree, so each direct
    /// child's figure is subtracted. The runtime records a frame's trace when
    /// the frame finishes and logs its `consumed` line at the same moment, so
    /// spans are matched to frames in completion order. Builtins log no
    /// `consumed` line and leave no trace, so mismatched program ids are
    /// skipped.
    /// Name every frame with the instruction it ran. The replay's logs give
    /// one span per invocation in the on-chain order, so span *k* is tree
    /// entry *k* for as long as the programs agree (the replay stops at its
    /// failure; nothing after it ran). Frames are then paired with spans the
    /// way `attach_compute` does: in completion order, skipping the
    /// builtins that run outside the VM and produce no frame.
    pub fn attach_names(&mut self, tree: &[crate::CpiEntry], logs: &[String]) -> usize {
        let spans = crate::cpi_tree::spans_from_logs(logs, 0);
        let mut aligned = true;
        let mut onchain: Vec<Option<u64>> = Vec::with_capacity(spans.len());
        let names: Vec<Option<String>> = spans
            .iter()
            .enumerate()
            .map(|(k, span)| {
                let entry = tree.get(k).filter(|e| e.program == span.program);
                aligned &= entry.is_some();
                onchain.push(if aligned {
                    entry.and_then(|e| e.compute_units)
                } else {
                    None
                });
                if aligned {
                    entry.and_then(|e| e.name.clone())
                } else {
                    None
                }
            })
            .collect();
        let mut order: Vec<usize> = (0..spans.len()).collect();
        order.sort_by_key(|&i| spans[i].end);
        let (mut next, mut named) = (0usize, 0usize);
        for i in order {
            if spans[i].cu_consumed.is_none() {
                continue; // builtin: no frame
            }
            let Some(frame) = self.frames.get_mut(next) else {
                break;
            };
            if frame.program != spans[i].program {
                continue;
            }
            frame.name = names[i].clone();
            frame.onchain_compute_units = onchain[i];
            named += frame.name.is_some() as usize;
            next += 1;
        }
        self.onchain_frames = Some(tree.iter().filter(|e| e.compute_units.is_some()).count());
        named
    }

    pub(crate) fn attach_compute(&mut self, logs: &[String]) {
        let spans = crate::cpi_tree::spans_from_logs(logs, 0);
        let mut exclusive: Vec<Option<u64>> = spans.iter().map(|s| s.cu_consumed).collect();
        let mut stack: Vec<usize> = Vec::new();
        for (i, span) in spans.iter().enumerate() {
            while stack.last().is_some_and(|&p| spans[p].depth >= span.depth) {
                stack.pop();
            }
            if let (Some(&parent), Some(cu)) = (stack.last(), span.cu_consumed) {
                if let Some(pcu) = exclusive[parent].as_mut() {
                    *pcu = pcu.saturating_sub(cu);
                }
            }
            stack.push(i);
        }
        // Each span's own log lines (not its children's), and its direct
        // children as (program, instruction name from the child's logs).
        let own_lines = |i: usize| -> Vec<String> {
            let s = &spans[i];
            let mut out = Vec::new();
            for (li, line) in logs.iter().enumerate().take(s.end).skip(s.start) {
                let inside_child = spans
                    .iter()
                    .any(|c| c.depth > s.depth && c.start > s.start && c.start <= li && li < c.end);
                if inside_child {
                    continue;
                }
                if let Some(text) = line.strip_prefix("Program log: ") {
                    out.push(text.to_string());
                }
            }
            out
        };
        let children = |i: usize| -> Vec<(String, Option<String>)> {
            let s = &spans[i];
            spans
                .iter()
                .enumerate()
                .filter(|(_, c)| c.depth == s.depth + 1 && c.start > s.start && c.start < s.end)
                .map(|(ci, c)| {
                    let name = own_lines(ci).iter().find_map(|l| {
                        l.strip_prefix("Instruction: ")
                            .map(|n| n.trim().to_string())
                    });
                    (c.program.clone(), name)
                })
                .collect()
        };
        let mut order: Vec<usize> = (0..spans.len()).collect();
        order.sort_by_key(|&i| spans[i].end);
        let mut next = 0usize;
        for i in order {
            let Some(cu) = exclusive[i] else { continue };
            let Some(frame) = self.frames.get_mut(next) else {
                break;
            };
            if frame.program != spans[i].program {
                continue;
            }
            frame.derive_labels(&own_lines(i), &children(i));
            frame.compute_units = Some(cu);
            frame.syscall_overhead = Some(cu.saturating_sub(frame.instructions));
            let insns = frame.instructions.max(1);
            for f in &mut frame.functions {
                f.compute_units = Some(cu * f.self_insns / insns);
            }
            next += 1;
        }
    }

    /// Name a program's functions from the unstripped ELF of the *same* build
    /// — the `.debug` file `cargo build-sbf --debug` writes next to the `.so`.
    /// Symbol addresses are checked against the program's actual entrypoint
    /// so a mismatched build is refused rather than mislabelled.
    ///
    /// Renames every `function_<pc>` in that program's frames, functions and
    /// folded stacks; Rust symbols are demangled.
    pub fn symbolize(&mut self, program: &str, elf_with_symbols: &[u8]) -> crate::Result<usize> {
        let symbols = elf_function_symbols(elf_with_symbols)
            .ok_or_else(|| crate::Error::InvalidSpec("not an ELF with a symbol table".into()))?;
        let symbols = symbols
            .into_iter()
            .map(|(pc, sym)| (pc, strip_hash(&rustc_demangle::demangle(&sym).to_string())))
            .collect();
        self.symbolize_map(program, &symbols)
    }

    /// Apply every bundled exact symbol map whose ELF hash equals the program
    /// loaded in this replay — byte-identical builds, so names map by address
    /// with nothing inferred. Returns the number of functions renamed.
    pub fn apply_exact<'a>(&mut self, program_elf: impl Fn(&str) -> Option<&'a [u8]>) -> usize {
        let programs: std::collections::BTreeSet<String> =
            self.frames.iter().map(|f| f.program.clone()).collect();
        let mut renamed = 0;
        for program in programs {
            let Some(exact) = builtin_exact().iter().find(|e| e.program == program) else {
                continue;
            };
            let Some(elf) = program_elf(&program) else {
                continue;
            };
            if stripped_sha256(elf) != exact.elf_sha256 {
                continue;
            }
            renamed += self.symbolize_map(&program, &exact.symbols).unwrap_or(0);
        }
        renamed
    }

    /// Rename `function_<pc>` entries of `program`'s frames from a `pc → name`
    /// map (names already demangled). The map's `entrypoint` must sit where the
    /// program's entrypoint actually ran, or the map is refused.
    fn symbolize_map(
        &mut self,
        program: &str,
        symbols: &BTreeMap<usize, String>,
    ) -> crate::Result<usize> {
        let mut renamed = 0usize;
        for frame in self.frames.iter_mut().filter(|f| f.program == program) {
            // Self-check: the ELF's `entrypoint` must sit where this program's
            // actual entrypoint ran.
            if let Some(entry) = frame.functions.iter().find(|f| f.name == "entrypoint") {
                match symbols.get(&entry.pc) {
                    Some(n) if n == "entrypoint" => {}
                    _ => {
                        return Err(crate::Error::InvalidSpec(format!(
                            "symbols do not match program {program}: its entrypoint runs at pc {} but the ELF's entrypoint symbol does not",
                            entry.pc
                        )))
                    }
                }
            }
            let mut rename: BTreeMap<String, String> = BTreeMap::new();
            for f in &mut frame.functions {
                if let Some(pretty) = symbols.get(&f.pc).cloned() {
                    if pretty != f.name {
                        rename.insert(f.name.clone(), pretty.clone());
                        f.name = pretty;
                        renamed += 1;
                    }
                }
            }
            for (stack, _) in &mut frame.stacks {
                *stack = stack
                    .split(';')
                    .map(|s| rename.get(s).cloned().unwrap_or_else(|| s.to_string()))
                    .collect::<Vec<_>>()
                    .join(";");
            }
        }
        Ok(renamed)
    }

    /// Name a program's functions from *another build of the same source*:
    /// `debug` is that build's `.debug` file (symbols and function sizes),
    /// `so` is the `.so` from the same build (the code those symbols describe).
    /// Functions are matched by code shape, so a plain release deployment can
    /// be named from a `--debug` build, and a verified build rebuilt with
    /// symbols can name what is on chain. When the two turn out to be the same
    /// build, names map by address instead.
    pub fn symbolize_from_build(
        &mut self,
        program: &str,
        so: &[u8],
        debug: &[u8],
    ) -> crate::Result<SymbolizeReport> {
        let (text, symbols) = elf_parse(so)
            .and_then(|(text, _)| elf_parse(debug).map(|(_, syms)| (text, syms)))
            .ok_or_else(|| {
                crate::Error::InvalidSpec(
                    "expected an ELF .so and a .debug with a symbol table".into(),
                )
            })?;
        // Same build? Then the entrypoint sits at the same pc with the same shape.
        let same_build = self
            .frames
            .iter()
            .filter(|f| f.program == program)
            .flat_map(|f| f.functions.iter())
            .find(|f| f.name == "entrypoint")
            .and_then(|entry| {
                let (name, size) = symbols.get(&entry.pc)?;
                Some(
                    name == "entrypoint"
                        && Shape::of(&text, entry.pc, entry.pc + size) == entry.shape,
                )
            })
            .unwrap_or(false);
        if same_build {
            let renamed = self.symbolize(program, debug)?;
            return Ok(SymbolizeReport {
                exact: renamed,
                by_opcodes: 0,
                by_similarity: 0,
                unmatched: 0,
                same_build: true,
            });
        }
        // Shapes of the build's symbols; drop shapes shared by several symbols
        // (identical bodies) as ambiguous.
        let mut by_full: std::collections::HashMap<u64, Option<String>> =
            std::collections::HashMap::new();
        let mut by_ops: std::collections::HashMap<(u64, usize), Option<String>> =
            std::collections::HashMap::new();
        let mut candidates: Vec<(Shape, String)> = Vec::new();
        for (&pc, (name, size)) in &symbols {
            if *size == 0 {
                continue;
            }
            let sh = Shape::of(&text, pc, pc + size);
            let pretty = strip_hash(&rustc_demangle::demangle(name).to_string());
            by_full
                .entry(sh.full)
                .and_modify(|v| *v = None)
                .or_insert(Some(pretty.clone()));
            by_ops
                .entry((sh.opcodes, sh.len))
                .and_modify(|v| *v = None)
                .or_insert(Some(pretty.clone()));
            candidates.push((sh, pretty));
        }
        // Near-miss tier: for a function nothing matched exactly, the unique
        // symbol of about the same length whose opcode histogram is ≥ 90%
        // similar — and clearly better than the next candidate.
        let near_miss = |shape: &Shape| -> Option<String> {
            let (mut best, mut second): (Option<(f64, &String)>, f64) = (None, 0.0);
            for (sh, name) in &candidates {
                let len_ok =
                    (sh.len as f64 - shape.len as f64).abs() <= (shape.len as f64 * 0.10).max(2.0);
                if !len_ok {
                    continue;
                }
                let sim = shape.similarity(sh);
                match best {
                    Some((b, _)) if sim <= b => second = second.max(sim),
                    Some((b, _)) => {
                        second = b;
                        best = Some((sim, name));
                    }
                    None => best = Some((sim, name)),
                }
            }
            match best {
                Some((sim, name)) if sim >= 0.90 && sim - second >= 0.05 => Some(name.clone()),
                _ => None,
            }
        };
        let mut report = SymbolizeReport::default();
        for frame in self.frames.iter_mut().filter(|f| f.program == program) {
            let mut rename: BTreeMap<String, String> = BTreeMap::new();
            for f in &mut frame.functions {
                if !f.name.starts_with("function_") && f.name != "entrypoint" {
                    continue;
                }
                let hit = match by_full.get(&f.shape.full) {
                    Some(Some(n)) => {
                        report.exact += 1;
                        Some(n.clone())
                    }
                    _ => match by_ops.get(&(f.shape.opcodes, f.shape.len)) {
                        Some(Some(n)) => {
                            report.by_opcodes += 1;
                            Some(n.clone())
                        }
                        _ => match near_miss(&f.shape) {
                            Some(n) => {
                                report.by_similarity += 1;
                                Some(n)
                            }
                            None => None,
                        },
                    },
                };
                match hit {
                    Some(n) if n != f.name => {
                        rename.insert(f.name.clone(), n.clone());
                        f.name = n;
                    }
                    Some(_) => {}
                    None => report.unmatched += 1,
                }
            }
            for (stack, _) in &mut frame.stacks {
                *stack = stack
                    .split(';')
                    .map(|s| rename.get(s).cloned().unwrap_or_else(|| s.to_string()))
                    .collect::<Vec<_>>()
                    .join(";");
            }
        }
        Ok(report)
    }

    /// Name anonymous functions from a shape corpus (see [`CorpusEntry`]):
    /// exact shape matches only, across every frame. Returns how many were
    /// named. Real symbols and behavioural labels are left alone; a corpus
    /// name lands in `name`, so it shows everywhere a symbol would.
    pub fn symbolize_from_corpus(&mut self, corpus: &[CorpusEntry]) -> usize {
        // Exact tier: a shape hash carried by two different names (shapes are
        // register-and-immediate exact but drop call targets, so thunks can
        // collide across builds) is ambiguous and never applied; nor is any
        // entry shorter than the corpus floor, whatever the file says.
        let mut index: std::collections::HashMap<u64, Option<&CorpusEntry>> =
            std::collections::HashMap::new();
        for e in corpus.iter().filter(|e| e.len >= MIN_CORPUS_LEN) {
            index
                .entry(e.full)
                .and_modify(|v| {
                    if v.is_some_and(|x| x.name != e.name) {
                        *v = None;
                    }
                })
                .or_insert(Some(e));
        }
        // Approximate tier: opcode sequence + length. A hash shared by two
        // different names is ambiguous and never used.
        let mut by_opcodes: std::collections::HashMap<(u64, usize), Option<&str>> =
            std::collections::HashMap::new();
        for e in corpus {
            if let Some(op) = e.opcodes {
                by_opcodes
                    .entry((op, e.len))
                    .and_modify(|v| {
                        if v.is_some_and(|n| n != e.name) {
                            *v = None;
                        }
                    })
                    .or_insert(Some(e.name.as_str()));
            }
        }
        let mut renamed = 0usize;
        for frame in &mut self.frames {
            let mut rename: BTreeMap<String, String> = BTreeMap::new();
            for f in &mut frame.functions {
                if !f.name.starts_with("function_") {
                    continue;
                }
                let exact = index
                    .get(&f.shape.full)
                    .copied()
                    .flatten()
                    .filter(|e| e.len == f.shape.len);
                let new_name = match exact {
                    Some(e) => Some(e.name.clone()),
                    None => by_opcodes
                        .get(&(f.shape.opcodes, f.shape.len))
                        .copied()
                        .flatten()
                        .map(|n| format!("≈ {n}")),
                };
                if let Some(n) = new_name {
                    rename.insert(f.name.clone(), n.clone());
                    f.name = n;
                    renamed += 1;
                }
            }
            if rename.is_empty() {
                continue;
            }
            for (stack, _) in &mut frame.stacks {
                *stack = stack
                    .split(';')
                    .map(|s| rename.get(s).cloned().unwrap_or_else(|| s.to_string()))
                    .collect::<Vec<_>>()
                    .join(";");
            }
        }
        renamed
    }

    /// Total BPF instructions across every frame.
    pub fn instructions(&self) -> u64 {
        self.frames.iter().map(|f| f.instructions).sum()
    }

    /// Compute per program, summed across that program's frames: the measured
    /// (CPI-exclusive) compute units where the logs reported them, else the
    /// frame's instruction count.
    pub fn by_program(&self) -> Vec<(String, u64)> {
        let mut m: BTreeMap<String, u64> = BTreeMap::new();
        for f in &self.frames {
            *m.entry(f.program.clone()).or_default() += f.compute_units.unwrap_or(f.instructions);
        }
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        v
    }
}

/// `FUNC` symbols of an ELF64 (its `.symtab`, falling back to `.dynsym`),
/// keyed by sBPF program counter: `(st_value - .text address) / 8`.
fn elf_function_symbols(elf: &[u8]) -> Option<BTreeMap<usize, String>> {
    elf_parse(elf).map(|(_, syms)| syms.into_iter().map(|(pc, (name, _))| (pc, name)).collect())
}

/// `pc → (symbol name, instruction count)` for an ELF's `FUNC` symbols.
type FunctionSymbols = BTreeMap<usize, (String, usize)>;

/// The `.text` bytes of an ELF64 and its `FUNC` symbols.
fn elf_parse(elf: &[u8]) -> Option<(Vec<u8>, FunctionSymbols)> {
    if elf.get(0..4)? != b"\x7fELF" {
        return None;
    }
    let u16_at =
        |o: usize| -> Option<u16> { Some(u16::from_le_bytes(elf.get(o..o + 2)?.try_into().ok()?)) };
    let u32_at =
        |o: usize| -> Option<u32> { Some(u32::from_le_bytes(elf.get(o..o + 4)?.try_into().ok()?)) };
    let u64_at =
        |o: usize| -> Option<u64> { Some(u64::from_le_bytes(elf.get(o..o + 8)?.try_into().ok()?)) };
    let shoff = u64_at(0x28)? as usize;
    let shentsize = u16_at(0x3a)? as usize;
    let shnum = u16_at(0x3c)? as usize;
    let shstrndx = u16_at(0x3e)? as usize;
    // (name, type, addr, offset, size, link, entsize)
    let section = |i: usize| -> Option<(u32, u32, u64, usize, usize, usize, usize)> {
        let o = shoff.checked_add(i.checked_mul(shentsize)?)?;
        Some((
            u32_at(o)?,
            u32_at(o + 4)?,
            u64_at(o + 16)?,
            u64_at(o + 24)? as usize,
            u64_at(o + 32)? as usize,
            u32_at(o + 40)? as usize,
            u64_at(o + 56)? as usize,
        ))
    };
    let cstr = |base: usize, off: usize| -> Option<String> {
        let start = base.checked_add(off)?;
        let end = start + elf.get(start..)?.iter().position(|&b| b == 0)?;
        Some(String::from_utf8_lossy(&elf[start..end]).to_string())
    };
    let (_, _, _, shstr_off, _, _, _) = section(shstrndx)?;
    let mut text = None;
    for i in 0..shnum {
        let (name, _, addr, off, size, _, _) = section(i)?;
        if cstr(shstr_off, name as usize)? == ".text" {
            text = Some((
                addr,
                elf.get(off..off + size)
                    .map(|b| b.to_vec())
                    .unwrap_or_default(),
            ));
        }
    }
    let (text_addr, text_bytes) = text?;
    const SHT_SYMTAB: u32 = 2;
    const SHT_DYNSYM: u32 = 11;
    const STT_FUNC: u8 = 2;
    let mut out = BTreeMap::new();
    for wanted in [SHT_SYMTAB, SHT_DYNSYM] {
        for i in 0..shnum {
            let (_, typ, _, off, size, link, entsize) = section(i)?;
            if typ != wanted || entsize == 0 {
                continue;
            }
            let (_, _, _, str_off, _, _, _) = section(link)?;
            for j in 0..size / entsize {
                let e = off + j * entsize;
                let st_name = u32_at(e)? as usize;
                let st_info = *elf.get(e + 4)?;
                let st_value = u64_at(e + 8)?;
                let st_size = u64_at(e + 16)? as usize;
                if st_info & 0xf != STT_FUNC || st_name == 0 || st_value < text_addr {
                    continue;
                }
                let pc = ((st_value - text_addr) / 8) as usize;
                out.entry(pc)
                    .or_insert((cstr(str_off, st_name)?, st_size / 8));
            }
        }
        if !out.is_empty() {
            break;
        }
    }
    Some((text_bytes, out))
}

/// `foo::bar::h9a99872dbe52d553` → `foo::bar`.
fn strip_hash(name: &str) -> String {
    // `__rustc[95bceff0ff0a01a5]::__rust_alloc`: the compiler's own crate
    // disambiguator on allocator shims — noise, not a name.
    let name = match name.strip_prefix("__rustc[") {
        Some(rest) => rest.split_once("]::").map_or(name, |(_, tail)| tail),
        None => name,
    };
    match name.rsplit_once("::h") {
        Some((head, hash)) if hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()) => {
            head.to_string()
        }
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod strip_hash_tests {
    #[test]
    fn strips_rustc_disambiguator_and_symbol_hash() {
        assert_eq!(
            super::strip_hash("__rustc[95bceff0ff0a01a5]::__rust_alloc"),
            "__rust_alloc"
        );
        assert_eq!(
            super::strip_hash("core::fmt::write::h0123456789abcdef"),
            "core::fmt::write"
        );
        assert_eq!(super::strip_hash("memcpy"), "memcpy");
    }
}

/// Captures every frame's register trace as the transaction executes.
struct Collector {
    frames: Arc<Mutex<Vec<FrameProfile>>>,
}

impl InvocationInspectCallback for Collector {
    fn before_invocation(
        &self,
        _svm: &LiteSVM,
        _tx: &SanitizedTransaction,
        _program_indices: &[IndexOfAccount],
        _invoke_context: &mut InvokeContext,
        _enable_register_tracing: bool,
    ) {
    }

    fn after_invocation(
        &self,
        _svm: &LiteSVM,
        _tx: &SanitizedTransaction,
        _program_indices: &[IndexOfAccount],
        invoke_context: &InvokeContext,
        enable_register_tracing: bool,
    ) {
        if !enable_register_tracing {
            return;
        }
        invoke_context.iterate_vm_traces(
            &|ictx: InstructionContext, exe: &Executable, trace: RegisterTrace| {
                let program = ictx
                    .get_program_key()
                    .map(|k| k.to_string())
                    .unwrap_or_default();
                if let Some(frame) = profile_frame(program, exe, &trace) {
                    self.frames.lock().unwrap().push(frame);
                }
            },
        );
    }
}

/// Function boundaries of a program: every internal call target plus the
/// registered (named) functions, keyed by first pc. One linear pass over the
/// text, cached per program: the sBPF static analysis computes the same set
/// but also builds a full control-flow graph, which on a 10 MB program costs
/// more than the trace it serves.
type FunctionMap = Arc<BTreeMap<usize, String>>;

fn function_map(exe: &Executable) -> FunctionMap {
    static CACHE: std::sync::LazyLock<Mutex<std::collections::HashMap<u64, FunctionMap>>> =
        std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
    let (_, text) = exe.get_text_bytes();
    // The whole text, hashed: two programs that differ anywhere get different
    // maps. FNV over a few MB is well under a millisecond, and the map is
    // built once per program per process anyway.
    let key = {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in text {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h ^ (text.len() as u64)
    };
    if let Some(m) = CACHE.lock().unwrap().get(&key) {
        return Arc::clone(m);
    }
    const CACHE_MAX: usize = 256;
    let mut functions: BTreeMap<usize, String> = BTreeMap::new();
    for (_, (name, pc)) in exe.get_function_registry().iter() {
        functions.insert(pc, String::from_utf8_lossy(name).to_string());
    }
    if exe.get_sbpf_version().static_syscalls() {
        let n = text.len() / ebpf::INSN_SIZE;
        for pc in 0..n {
            let insn = ebpf::get_insn_unchecked(text, pc);
            if insn.opc == ebpf::CALL_IMM && insn.src == 1 {
                let target = pc as i64 + 1 + insn.imm;
                if target >= 0 && (target as usize) < n {
                    functions
                        .entry(target as usize)
                        .or_insert_with(|| format!("function_{target}"));
                }
            }
        }
    }
    let map = Arc::new(functions);
    {
        let mut c = CACHE.lock().unwrap();
        if c.len() >= CACHE_MAX {
            c.clear();
        }
        c.insert(key, Arc::clone(&map));
    }
    map
}

/// Attribute one frame's trace to functions, syscalls and folded stacks.
fn profile_frame(program: String, exe: &Executable, trace: &RegisterTrace) -> Option<FrameProfile> {
    if trace.is_empty() {
        return None;
    }
    let functions = function_map(exe);
    let (_, text) = exe.get_text_bytes();
    let static_syscalls = exe.get_sbpf_version().static_syscalls();
    let loader = exe.get_loader();
    let syscall_registry = loader.get_function_registry();
    let program_registry = exe.get_function_registry();

    let enclosing = |pc: usize| -> usize {
        functions
            .range(..=pc)
            .next_back()
            .map(|(s, _)| *s)
            .unwrap_or(0)
    };
    let name_of = |start: usize| -> String {
        functions
            .get(&start)
            .cloned()
            .unwrap_or_else(|| format!("function_{start}"))
    };

    let mut self_insns: BTreeMap<usize, u64> = BTreeMap::new();
    let mut total_insns: BTreeMap<usize, u64> = BTreeMap::new();
    let mut calls: BTreeMap<usize, u64> = BTreeMap::new();
    let mut syscalls: BTreeMap<String, u64> = BTreeMap::new();
    // Folded stacks keyed on function *starts*; the names are rendered once
    // per distinct stack at the end, not once per executed instruction.
    let mut stacks: BTreeMap<Vec<usize>, u64> = BTreeMap::new();
    let mut events: Vec<(usize, String)> = Vec::new();

    let first_pc = trace[0][11] as usize;
    let mut stack: Vec<usize> = vec![enclosing(first_pc)];
    *calls.entry(stack[0]).or_default() += 1;

    for regs in trace.iter() {
        let pc = regs[11] as usize;
        // Keep the stack honest against the trace: after a `callx` or an
        // unmatched exit, the enclosing function of the current pc wins.
        let here = enclosing(pc);
        match stack.last() {
            Some(&top) if top == here => {}
            _ => {
                if let Some(pos) = stack.iter().rposition(|&f| f == here) {
                    stack.truncate(pos + 1);
                } else {
                    stack.push(here);
                    *calls.entry(here).or_default() += 1;
                }
            }
        }
        *self_insns.entry(here).or_default() += 1;
        for &f in &stack {
            *total_insns.entry(f).or_default() += 1;
        }
        *stacks.entry(stack.clone()).or_default() += 1;

        let insn = ebpf::get_insn_unchecked(text, pc);
        match insn.opc {
            ebpf::CALL_IMM => {
                // Same resolution order as the interpreter: syscall first.
                if !static_syscalls || insn.src == 0 {
                    if let Some((name, _)) = syscall_registry.lookup_by_key(insn.imm as u32) {
                        let name = String::from_utf8_lossy(name).to_string();
                        *syscalls.entry(name.clone()).or_default() += 1;
                        events.push((here, name));
                        continue;
                    }
                }
                let target = if static_syscalls {
                    (insn.src == 1).then(|| (pc as i64 + 1 + insn.imm) as usize)
                } else {
                    program_registry
                        .lookup_by_key(insn.imm as u32)
                        .map(|(_, t)| t)
                };
                if let Some(t) = target {
                    stack.push(t);
                    *calls.entry(t).or_default() += 1;
                }
            }
            ebpf::EXIT => {
                stack.pop();
            }
            _ => {}
        }
    }

    let n_insns = text.len() / ebpf::INSN_SIZE;
    let end_of = |start: usize| -> usize {
        functions
            .range(start + 1..)
            .next()
            .map(|(s, _)| *s)
            .unwrap_or(n_insns)
    };
    let mut fns: Vec<FunctionProfile> = self_insns
        .iter()
        .map(|(&pc, &s)| FunctionProfile {
            name: name_of(pc),
            pc,
            self_insns: s,
            total_insns: *total_insns.get(&pc).unwrap_or(&s),
            calls: *calls.get(&pc).unwrap_or(&0),
            compute_units: None,
            shape: Shape::of(text, pc, end_of(pc)),
            label: None,
        })
        .collect();
    fns.sort_by_key(|f| std::cmp::Reverse(f.self_insns));
    let mut sys: Vec<_> = syscalls.into_iter().collect();
    sys.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let mut folded: Vec<(String, u64)> = stacks
        .into_iter()
        .map(|(starts, n)| {
            (
                starts
                    .iter()
                    .map(|&f| name_of(f))
                    .collect::<Vec<_>>()
                    .join(";"),
                n,
            )
        })
        .collect();
    folded.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    Some(FrameProfile {
        name: None,
        onchain_compute_units: None,
        program,
        instructions: trace.len() as u64,
        compute_units: None,
        syscall_overhead: None,
        functions: fns,
        syscalls: sys,
        stacks: folded,
        events,
    })
}

/// A readable name for a program id: the well-known natives by name, anything
/// else shortened.
fn program_label(id: &str) -> String {
    match id {
        "11111111111111111111111111111111" => "System Program".into(),
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" => "Token Program".into(),
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb" => "Token-2022".into(),
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL" => "Associated Token".into(),
        "ComputeBudget111111111111111111111111111111" => "Compute Budget".into(),
        "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr" => "Memo".into(),
        other if other.len() > 12 => format!("{}…{}", &other[..4], &other[other.len() - 4..]),
        other => other.to_string(),
    }
}

/// Label priority: a stronger piece of evidence overrides a weaker one.
fn label_rank(label: &str) -> u8 {
    if label.ends_with(" handler") {
        6
    } else if label.starts_with("error: ") {
        5
    } else if label.starts_with("CPI → ") {
        4
    } else if label == "PDA derivation" {
        3
    } else if label == "emits event" || label == "hashing" || label == "sets return data" {
        2
    } else {
        1
    }
}

impl FrameProfile {
    /// Derive [`FunctionProfile::label`]s from the frame's syscall events and
    /// its own log lines (`own_logs`, in order) and its child invocations
    /// (`children`: `(program id, instruction name)` in order). The k-th
    /// logging syscall wrote the k-th log line; the k-th `sol_invoke_signed`
    /// made the k-th child call.
    fn derive_labels(&mut self, own_logs: &[String], children: &[(String, Option<String>)]) {
        let mut labels: BTreeMap<usize, String> = BTreeMap::new();
        let mut propose = |pc: usize, label: String| {
            let better = labels
                .get(&pc)
                .map(|cur| label_rank(&label) > label_rank(cur))
                .unwrap_or(true);
            if better {
                labels.insert(pc, label);
            }
        };
        let (mut log_i, mut cpi_i) = (0usize, 0usize);
        for (pc, sys) in &self.events {
            match sys.as_str() {
                "sol_log_" | "sol_log_64_" | "sol_log_pubkey" => {
                    if let Some(line) = own_logs.get(log_i) {
                        if let Some(name) = line.strip_prefix("Instruction: ") {
                            propose(*pc, format!("{} handler", name.trim()));
                        } else if line.contains("AnchorError") {
                            let name = line
                                .split("Error Code: ")
                                .nth(1)
                                .and_then(|r| r.split('.').next())
                                .unwrap_or("AnchorError");
                            propose(*pc, format!("error: {name}"));
                        } else {
                            propose(*pc, "logging".into());
                        }
                    }
                    log_i += 1;
                }
                "sol_invoke_signed_rust" | "sol_invoke_signed_c" => {
                    let target = match children.get(cpi_i) {
                        Some((program, Some(ix))) => {
                            format!("CPI → {}: {ix}", program_label(program))
                        }
                        Some((program, None)) => format!("CPI → {}", program_label(program)),
                        None => "CPI".into(),
                    };
                    propose(*pc, target);
                    cpi_i += 1;
                }
                "sol_try_find_program_address" | "sol_create_program_address" => {
                    propose(*pc, "PDA derivation".into())
                }
                "sol_log_data" => propose(*pc, "emits event".into()),
                "sol_sha256" | "sol_keccak256" | "sol_blake3" | "sol_poseidon" => {
                    propose(*pc, "hashing".into())
                }
                "sol_set_return_data" => propose(*pc, "sets return data".into()),
                "sol_get_return_data" => propose(*pc, "reads return data".into()),
                s if s.starts_with("sol_get_") && s.ends_with("_sysvar") => {
                    propose(*pc, "reads sysvar".into())
                }
                "sol_memcpy_" | "sol_memmove_" | "sol_memset_" => {
                    propose(*pc, "memory copy".into())
                }
                "sol_memcmp_" => propose(*pc, "memory compare".into()),
                _ => {}
            }
        }
        // Structure: on any stack that reaches a handler, the functions between
        // the entrypoint and the handler are the instruction dispatch.
        let handler_names: Vec<String> = self
            .functions
            .iter()
            .filter(|f| labels.get(&f.pc).is_some_and(|l| l.ends_with(" handler")))
            .map(|f| f.name.clone())
            .collect();
        let mut dispatch: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (stack, _) in &self.stacks {
            let parts: Vec<&str> = stack.split(';').collect();
            if let Some(h) = parts
                .iter()
                .position(|p| handler_names.iter().any(|n| n == p))
            {
                for p in parts.iter().take(h).skip(1) {
                    dispatch.insert((*p).to_string());
                }
            }
        }
        for f in &mut self.functions {
            if f.name == "entrypoint" {
                f.label = Some("entrypoint".into());
            } else if let Some(l) = labels.get(&f.pc) {
                f.label = Some(l.clone());
            } else if dispatch.contains(&f.name) {
                f.label = Some("instruction dispatch".into());
            }
        }
    }
}

impl crate::Replay {
    /// Profile the transaction: trace every BPF instruction it executes and
    /// attribute them to functions, syscalls and call stacks, per program
    /// frame. `mutations` apply first, as in [`Self::simulate`].
    pub fn profile(
        &self,
        mutations: &[crate::Mutation],
    ) -> crate::Result<(crate::ReplayResult, Profile)> {
        let crate::replay::Prepared { mut svm, tx, .. } = self.ctx.prepare(mutations, true)?;
        let frames = Arc::new(Mutex::new(Vec::new()));
        svm.set_invocation_inspect_callback(Collector {
            frames: Arc::clone(&frames),
        });
        let result = crate::replay::replay_result_of(&svm.send_transaction(tx));
        let frames = std::mem::take(&mut *frames.lock().unwrap());
        let mut profile = Profile {
            frames,
            onchain_frames: None,
        };
        profile.attach_compute(&result.logs);
        // Exact names first: a bundled symbol map for a program whose on-chain
        // ELF hash matches names every function by address.
        profile.apply_exact(|p| self.ctx.program_elf(p));
        // Names for free: the bundled corpus names every library function
        // whose shape it knows, in any program, before anyone uploads anything.
        profile.symbolize_from_corpus(builtin_corpus());
        Ok((result, profile))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal ELF64 with a `.text` at 0x120 and a `.symtab` holding two
    /// FUNC symbols, built by hand so the parser is tested without fixtures.
    fn tiny_elf() -> Vec<u8> {
        let mut elf = vec![0u8; 0x40];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2; // 64-bit
        elf[5] = 1; // little-endian
                    // Layout: [header 0x40][.text 0x120..0x160][.strtab][.symtab][.shstrtab][section headers]
        elf.resize(0x120, 0);
        elf.extend_from_slice(&[0u8; 64]); // 8 instructions of .text at addr 0x120
        let strtab_off = elf.len();
        elf.extend_from_slice(b"\0entrypoint\0_ZN3foo3bar17h9a99872dbe52d553E\0");
        let symtab_off = elf.len();
        let mut sym = |name: u32, value: u64| {
            elf.extend_from_slice(&name.to_le_bytes());
            elf.push(2); // STT_FUNC
            elf.push(0);
            elf.extend_from_slice(&1u16.to_le_bytes());
            elf.extend_from_slice(&value.to_le_bytes());
            elf.extend_from_slice(&8u64.to_le_bytes());
        };
        sym(0, 0); // null symbol
        sym(1, 0x120); // entrypoint at pc 0
        sym(12, 0x120 + 3 * 8); // foo::bar at pc 3
        let shstr_off = elf.len();
        elf.extend_from_slice(b"\0.text\0.strtab\0.symtab\0.shstrtab\0");
        let shoff = elf.len();
        let mut sh =
            |name: u32, typ: u32, addr: u64, off: u64, size: u64, link: u32, entsize: u64| {
                elf.extend_from_slice(&name.to_le_bytes());
                elf.extend_from_slice(&typ.to_le_bytes());
                elf.extend_from_slice(&0u64.to_le_bytes()); // flags
                elf.extend_from_slice(&addr.to_le_bytes());
                elf.extend_from_slice(&off.to_le_bytes());
                elf.extend_from_slice(&size.to_le_bytes());
                elf.extend_from_slice(&link.to_le_bytes());
                elf.extend_from_slice(&0u32.to_le_bytes()); // info
                elf.extend_from_slice(&0u64.to_le_bytes()); // align
                elf.extend_from_slice(&entsize.to_le_bytes());
            };
        sh(0, 0, 0, 0, 0, 0, 0);
        sh(1, 1, 0x120, 0x120, 64, 0, 0); // .text
        sh(
            7,
            3,
            0,
            strtab_off as u64,
            (symtab_off - strtab_off) as u64,
            0,
            0,
        ); // .strtab
        sh(
            15,
            2,
            0,
            symtab_off as u64,
            (shstr_off - symtab_off) as u64,
            2,
            24,
        ); // .symtab → link 2
        sh(23, 3, 0, shstr_off as u64, (shoff - shstr_off) as u64, 0, 0); // .shstrtab
        elf[0x28..0x30].copy_from_slice(&(shoff as u64).to_le_bytes());
        elf[0x3a..0x3c].copy_from_slice(&64u16.to_le_bytes());
        elf[0x3c..0x3e].copy_from_slice(&5u16.to_le_bytes());
        elf[0x3e..0x40].copy_from_slice(&4u16.to_le_bytes());
        elf
    }

    fn frame(program: &str, fns: &[(usize, &str, u64)]) -> FrameProfile {
        FrameProfile {
            name: None,
            onchain_compute_units: None,
            program: program.into(),
            instructions: fns.iter().map(|f| f.2).sum(),
            compute_units: None,
            syscall_overhead: None,
            functions: fns
                .iter()
                .map(|&(pc, name, n)| FunctionProfile {
                    name: name.into(),
                    pc,
                    self_insns: n,
                    total_insns: n,
                    calls: 1,
                    compute_units: None,
                    shape: Shape::default(),
                    label: None,
                })
                .collect(),
            syscalls: vec![],
            stacks: vec![(fns.iter().map(|f| f.1).collect::<Vec<_>>().join(";"), 1)],
            events: vec![],
        }
    }

    #[test]
    fn labels_come_from_logs_and_syscalls_in_trace_order() {
        let mut f = frame(
            "Amm",
            &[
                (0, "entrypoint", 10),
                (50, "function_50", 5),
                (100, "function_100", 50),
                (200, "function_200", 30),
                (300, "function_300", 20),
            ],
        );
        f.stacks = vec![
            (
                "entrypoint;function_50;function_100;function_200".into(),
                10,
            ),
            ("entrypoint;function_300".into(), 1),
        ];
        f.events = vec![
            (100, "sol_log_".into()), // "Instruction: Buy"
            (200, "sol_try_find_program_address".into()),
            (200, "sol_invoke_signed_rust".into()), // → Token Program: Transfer
            (300, "sol_log_".into()),               // AnchorError …
            (300, "sol_memcpy_".into()),
        ];
        let logs = vec![
            "Instruction: Buy".to_string(),
            "AnchorError thrown in x. Error Code: SlippageExceeded. Error Number: 6001."
                .to_string(),
        ];
        let children = vec![(
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            Some("Transfer".to_string()),
        )];
        f.derive_labels(&logs, &children);
        let label = |pc: usize| {
            f.functions
                .iter()
                .find(|x| x.pc == pc)
                .unwrap()
                .label
                .clone()
        };
        assert_eq!(label(0).as_deref(), Some("entrypoint"));
        assert_eq!(label(100).as_deref(), Some("Buy handler"));
        assert_eq!(
            label(200).as_deref(),
            Some("CPI → Token Program: Transfer"),
            "CPI outranks PDA derivation"
        );
        assert_eq!(
            label(300).as_deref(),
            Some("error: SlippageExceeded"),
            "error outranks memory copy"
        );
        assert_eq!(
            label(50).as_deref(),
            Some("instruction dispatch"),
            "between entrypoint and the handler"
        );
    }

    #[test]
    fn elf_symbols_map_to_program_counters() {
        let syms = elf_function_symbols(&tiny_elf()).unwrap();
        assert_eq!(syms.get(&0).map(String::as_str), Some("entrypoint"));
        assert_eq!(
            syms.get(&3).map(String::as_str),
            Some("_ZN3foo3bar17h9a99872dbe52d553E")
        );
        assert!(elf_function_symbols(b"not an elf").is_none());
    }

    #[test]
    fn symbolize_renames_functions_and_stacks_and_refuses_a_mismatch() {
        let mut p = Profile {
            frames: vec![frame("P", &[(0, "entrypoint", 10), (3, "function_3", 90)])],
            onchain_frames: None,
        };
        let renamed = p.symbolize("P", &tiny_elf()).unwrap();
        assert_eq!(renamed, 1);
        assert_eq!(p.frames[0].functions[1].name, "foo::bar");
        assert_eq!(p.frames[0].stacks[0].0, "entrypoint;foo::bar");
        // Entrypoint at a different pc than the ELF says: refused, untouched.
        let mut q = Profile {
            frames: vec![frame("P", &[(5, "entrypoint", 10), (3, "function_3", 90)])],
            onchain_frames: None,
        };
        assert!(q.symbolize("P", &tiny_elf()).is_err());
        assert_eq!(q.frames[0].functions[1].name, "function_3");
        assert_eq!(strip_hash("a::b::h0123456789abcdef"), "a::b");
        assert_eq!(strip_hash("a::b::hxyz"), "a::b::hxyz");
    }

    /// One sBPF instruction: opc, dst/src nibbles, off, imm.
    fn insn(opc: u8, dst: u8, src: u8, off: i16, imm: i32) -> [u8; 8] {
        let mut b = [0u8; 8];
        b[0] = opc;
        b[1] = (src << 4) | dst;
        b[2..4].copy_from_slice(&off.to_le_bytes());
        b[4..8].copy_from_slice(&imm.to_le_bytes());
        b
    }

    #[test]
    fn shape_ignores_what_moves_between_builds_and_keeps_what_does_not() {
        // add64 r1 += 5 ; jeq r1, 0, +off ; call +target ; exit
        let body = |off: i16, target: i32, imm: i32| -> Vec<u8> {
            let mut t = Vec::new();
            t.extend_from_slice(&insn(0x07, 1, 0, 0, imm));
            t.extend_from_slice(&insn(0x15, 1, 0, off, 0));
            t.extend_from_slice(&insn(ebpf::CALL_IMM, 0, 1, 0, target));
            t.extend_from_slice(&insn(ebpf::EXIT, 0, 0, 0, 0));
            t
        };
        let a = Shape::of(&body(3, 100, 5), 0, 4);
        let b = Shape::of(&body(-7, 2_000, 5), 0, 4); // different jump offset and call target
        let c = Shape::of(&body(3, 100, 6), 0, 4); // different constant
        assert_eq!(a, b, "offsets and call targets are normalised away");
        assert_ne!(a.full, c.full, "a real constant is part of the shape");
        assert_eq!(a.opcodes, c.opcodes, "opcode-only shape still agrees");
        assert_eq!(a.len, 4);
    }

    #[test]
    fn corpus_names_exact_shapes_only() {
        let mut p = Profile {
            frames: vec![frame(
                "P",
                &[
                    (0, "entrypoint", 10),
                    (7, "function_7", 30),
                    (9, "function_9", 30),
                ],
            )],
            onchain_frames: None,
        };
        p.frames[0].functions[1].shape = Shape {
            full: 42,
            opcodes: 1,
            len: 30,
            histogram: [0; 256],
        };
        p.frames[0].functions[2].shape = Shape {
            full: 42,
            opcodes: 1,
            len: 31,
            histogram: [0; 256],
        };
        let corpus = vec![CorpusEntry {
            opcodes: None,
            full: 42,
            len: 30,
            name: "core::fmt::write".into(),
        }];
        assert_eq!(p.symbolize_from_corpus(&corpus), 1);
        assert_eq!(p.frames[0].functions[1].name, "core::fmt::write");
        assert_eq!(
            p.frames[0].functions[2].name, "function_9",
            "same hash, different length: not renamed"
        );
        assert!(!builtin_corpus().is_empty(), "the bundled corpus decodes");
        assert!(builtin_corpus()
            .iter()
            .any(|e| e.name.starts_with("core::") || e.name.starts_with("alloc::")));
    }

    #[test]
    fn syscall_estimate_is_a_lower_bound_by_fixed_charge() {
        let mut f = frame("P", &[(0, "entrypoint", 10)]);
        f.syscalls = vec![
            ("sol_memcpy_".into(), 100),
            ("sol_invoke_signed_rust".into(), 3),
            ("sol_try_find_program_address".into(), 2),
            ("sol_log_".into(), 4),
        ];
        let est = f.syscall_estimate();
        assert_eq!(est[0], ("sol_try_find_program_address".into(), 2, 3_000));
        assert_eq!(est[1], ("sol_invoke_signed_rust".into(), 3, 2_838));
        assert_eq!(est[2], ("sol_memcpy_".into(), 100, 1_000));
        assert_eq!(est[3], ("sol_log_".into(), 4, 400));
    }

    #[test]
    fn compute_attaches_by_invocation_order_skipping_builtins() {
        let mut p = Profile {
            frames: vec![
                frame("Tok", &[(0, "entrypoint", 100)]),
                frame("Amm", &[(0, "entrypoint", 300), (9, "function_9", 700)]),
            ],
            onchain_frames: None,
        };
        // Amm calls Tok: the runtime logs Tok's `consumed` first (it finishes
        // first) and records Tok's trace first for the same reason.
        let logs: Vec<String> = [
            "Program 11111111111111111111111111111111 invoke [1]",
            "Program 11111111111111111111111111111111 success",
            "Program Amm invoke [1]",
            "Program Tok invoke [2]",
            "Program Tok consumed 150 of 200 compute units",
            "Program Tok success",
            "Program Amm consumed 2000 of 10000 compute units",
            "Program Amm success",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        p.attach_compute(&logs);
        assert_eq!(p.frames[0].compute_units, Some(150));
        assert_eq!(p.frames[0].syscall_overhead, Some(50));
        // Amm's 2000 includes Tok's 150: its own share is 1850.
        assert_eq!(p.frames[1].compute_units, Some(1850));
        assert_eq!(
            p.frames[1].functions[1].compute_units,
            Some(1295),
            "700 of 1000 insns → 70% of 1850 CU"
        );
    }
}

#[cfg(test)]
mod frame_name_tests {
    use super::*;

    fn entry(program: &str, name: &str, h: u64) -> crate::CpiEntry {
        crate::CpiEntry {
            discriminator: None,
            introspects: false,
            compute_units: None,
            index: 0,
            program: program.into(),
            stack_height: h,
            name: Some(name.into()),
            accounts: vec![],
            args: vec![],
            data: vec![],
            account_indexes: vec![],
        }
    }

    fn frame(program: &str) -> FrameProfile {
        FrameProfile {
            name: None,
            onchain_compute_units: None,
            program: program.into(),
            instructions: 1,
            compute_units: None,
            syscall_overhead: None,
            functions: vec![],
            syscalls: vec![],
            stacks: vec![],
            ..Default::default()
        }
    }

    #[test]
    fn names_follow_the_replay_logs_and_skip_builtins_and_unrun_steps() {
        const SYS: &str = "11111111111111111111111111111111";
        const CB: &str = "ComputeBudget111111111111111111111111111111";
        let tree = vec![
            entry(CB, "Set Limit", 1),
            entry("ATA", "Create", 1),
            entry("TOK", "Get Size", 2),
            entry(SYS, "Create Account", 2),
            entry("JUP", "Route", 1),
            entry("HYX", "Mint", 2),
            entry("TOK", "Transfer Checked", 3), // never ran: HYX failed first
            entry("TOK", "Close Account", 1),    // never ran: after the failure
        ];
        let logs: Vec<String> = [
            format!("Program {CB} invoke [1]"),
            format!("Program {CB} success"),
            "Program ATA invoke [1]".into(),
            "Program TOK invoke [2]".into(),
            "Program TOK consumed 5 of 100 compute units".into(),
            "Program TOK success".into(),
            format!("Program {SYS} invoke [2]"),
            format!("Program {SYS} success"),
            "Program ATA consumed 20 of 100 compute units".into(),
            "Program ATA success".into(),
            "Program JUP invoke [1]".into(),
            "Program HYX invoke [2]".into(),
            "Program HYX consumed 30 of 100 compute units".into(),
            "Program HYX failed: custom program error: 0x1".into(),
            "Program JUP consumed 40 of 100 compute units".into(),
            "Program JUP failed: custom program error: 0x1".into(),
        ]
        .into_iter()
        .collect();
        let mut p = Profile {
            frames: ["TOK", "ATA", "HYX", "JUP"]
                .into_iter()
                .map(frame)
                .collect(),
            onchain_frames: None,
        };
        assert_eq!(p.attach_names(&tree, &logs), 4);
        let names: Vec<_> = p
            .frames
            .iter()
            .map(|f| f.name.as_deref().unwrap())
            .collect();
        assert_eq!(names, ["Get Size", "Create", "Mint", "Route"]);
    }
}

#[cfg(test)]
mod exact_bundle_tests {
    #[test]
    fn every_bundled_exact_map_parses_and_has_an_entrypoint() {
        use std::io::Read;
        let gz: &[u8] = include_bytes!("../symbols/exact.jsonl.gz");
        let mut text = String::new();
        flate2::read::GzDecoder::new(gz)
            .read_to_string(&mut text)
            .unwrap();
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(
            super::builtin_exact().len(),
            lines.len(),
            "a bundled map failed to parse"
        );
        for m in super::builtin_exact() {
            assert_eq!(m.elf_sha256.len(), 64, "{}: bad hash", m.program);
            assert!(
                m.symbols.values().any(|n| n == "entrypoint"),
                "{}: no entrypoint",
                m.program
            );
        }
    }
}
