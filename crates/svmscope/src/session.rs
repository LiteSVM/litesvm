//! A replay session: one transaction's reconstructed world, replayed,
//! mutated, time-travelled and traced any number of times without RPC.

use {
    crate::{
        analyze::{AccountDiff, Explanation, FieldDiff, ReplayResult, SimulationReport},
        check::{Check, Scenario, ScenarioOutcome},
        decode,
        error::{Error, Result},
        fidelity::{AccountProvenance, AccountState, Fidelity, FidelityCertificate, Provenance},
        fixture::{Fixture, OnchainRecord},
        idl, ixname,
        mutation::Mutation,
        replay::{FeatureToggle, ReplayContext, TimeTravel},
        search::{search_threshold, Threshold},
        trace::Trace,
    },
    serde::Serialize,
    solana_address::Address,
    std::{collections::HashMap, str::FromStr},
};

/// The result of replaying a transaction against an original program and a
/// patched one — the pre-deployment "does this patch change what happened?" gate.
#[derive(Debug, Clone, Serialize)]
pub struct PatchComparison {
    /// The program whose ELF was swapped.
    pub program: String,
    /// The outcome with the original (currently-loaded) program.
    pub before: ReplayResult,
    /// The outcome with the patched program.
    pub after: ReplayResult,
}

impl PatchComparison {
    /// Whether the patch flipped success ↔ failure.
    pub fn success_changed(&self) -> bool {
        self.before.success != self.after.success
    }

    /// Whether the patch changed the (formatted) error.
    pub fn error_changed(&self) -> bool {
        self.before.error != self.after.error
    }

    /// The change in compute units (patched − original), which may be negative.
    pub fn compute_delta(&self) -> i64 {
        self.after.compute_units as i64 - self.before.compute_units as i64
    }

    /// Whether the patch changed anything observable (outcome, error, or compute).
    pub fn changed(&self) -> bool {
        self.success_changed() || self.error_changed() || self.compute_delta() != 0
    }

    /// A one-line human summary of what the patch changed.
    pub fn summary(&self) -> String {
        if !self.changed() {
            return format!("{}: no observable change", self.program);
        }
        let outcome = match (self.before.success, self.after.success) {
            (false, true) => "revert → success".to_string(),
            (true, false) => "success → revert".to_string(),
            _ => format!(
                "{:?} → {:?}",
                self.before.error.as_deref().unwrap_or("ok"),
                self.after.error.as_deref().unwrap_or("ok")
            ),
        };
        format!(
            "{}: {outcome} · compute {:+}",
            self.program,
            self.compute_delta()
        )
    }
}

/// A transaction's reconstructed world — fetched once via [`Scope::replay`](crate::Scope::replay),
/// then replayed locally any number of times. Every run builds a pristine SVM,
/// so runs are independent, repeatable, and free.
#[derive(Clone)]
pub struct Replay {
    pub(crate) ctx: ReplayContext,
    /// What actually happened on-chain (`None` for pre-flight transactions and
    /// fixtures captured before outcomes were recorded).
    pub(crate) recorded: Option<OnchainRecord>,
    pub(crate) time_travel: TimeTravel,
    pub(crate) fidelity: Fidelity,
}

impl Replay {
    pub(crate) fn set_recorded(&mut self, recorded: OnchainRecord) {
        self.recorded = Some(recorded);
    }

    pub(crate) fn set_fidelity(&mut self, fidelity: Fidelity) {
        self.fidelity = fidelity;
    }

    /// How faithful this replay's starting state is to the transaction's slot.
    pub fn fidelity(&self) -> Fidelity {
        self.fidelity
    }

    /// An honest fidelity certificate for this replay: the verdict, per-account
    /// provenance and hashes, which accounts may have drifted from the true slot,
    /// and whether there is a recorded on-chain outcome to verify against.
    pub fn certificate(&self) -> FidelityCertificate {
        let accounts: Vec<AccountProvenance> = self
            .ctx
            .loaded_info()
            .into_iter()
            .map(|i| {
                let source = match self.fidelity {
                    Fidelity::Exact { .. } => Provenance::HistoricalArchive,
                    // A balance-only account (system-owned, no data) is faithfully
                    // rewound from metadata; program ELFs and program-owned data
                    // accounts are still current-state.
                    Fidelity::Reconstructed { .. }
                        if !i.is_program && i.owner_is_system && i.data_len == 0 =>
                    {
                        Provenance::MetadataRewind
                    }
                    Fidelity::Reconstructed { .. } => Provenance::CurrentRpc,
                    Fidelity::Current => Provenance::CurrentRpc,
                };
                AccountProvenance {
                    address: i.address,
                    source,
                    is_program: i.is_program,
                    hash: i.hash,
                }
            })
            .collect();

        // In a historical replay, any account still on current-state bytes is a
        // potential drift point. A plainly-current replay isn't "drifted" — it
        // never claimed to be historical.
        let drifted = if matches!(self.fidelity, Fidelity::Current) {
            Vec::new()
        } else {
            accounts
                .iter()
                .filter(|a| a.source == Provenance::CurrentRpc)
                .map(|a| a.address.clone())
                .collect()
        };

        let verifiable = self
            .recorded
            .as_ref()
            .is_some_and(|r| r.error.as_deref() != Some("transaction metadata unavailable"));

        FidelityCertificate {
            fidelity: self.fidelity,
            clock: self.ctx.describe_clock(),
            accounts,
            drifted,
            verifiable,
        }
    }

    /// Rebuild a replay from a frozen fixture — fully offline, no RPC. A v2
    /// fixture restores the recorded on-chain outcome and captured IDLs too.
    pub fn from_fixture(fx: &Fixture) -> Result<Replay> {
        Ok(Replay {
            ctx: ReplayContext::from_fixture(fx)?,
            recorded: fx.recorded.clone(),
            time_travel: TimeTravel::default(),
            fidelity: Fidelity::Current,
        })
    }

    /// What actually happened on-chain, when known.
    pub fn recorded(&self) -> Option<&OnchainRecord> {
        self.recorded.as_ref()
    }

    // --- time travel ---------------------------------------------------------

    /// Jump forward `n` slots (additive with other jumps).
    pub fn advance_slots(&mut self, n: i64) {
        // Saturating so repeated extreme jumps clamp instead of overflow-panicking
        // in debug builds; the clock application saturates too.
        self.time_travel.slots = Some(self.time_travel.slots.unwrap_or(0).saturating_add(n));
        self.apply_tt();
    }

    /// Jump forward `n` epochs (additive with other jumps).
    pub fn advance_epochs(&mut self, n: i64) {
        self.time_travel.epochs = Some(self.time_travel.epochs.unwrap_or(0).saturating_add(n));
        self.apply_tt();
    }

    /// Jump forward `n` seconds (additive with other jumps) — vesting cliffs,
    /// cooldowns, auction deadlines.
    pub fn advance_seconds(&mut self, n: i64) {
        self.time_travel.seconds = Some(self.time_travel.seconds.unwrap_or(0).saturating_add(n));
        self.apply_tt();
    }

    /// Set the clock's slot outright (wins over relative jumps).
    pub fn warp_to_slot(&mut self, slot: u64) {
        self.time_travel.at_slot = Some(slot);
        self.apply_tt();
    }

    /// Set the clock's epoch outright (wins over relative jumps).
    pub fn warp_to_epoch(&mut self, epoch: u64) {
        self.time_travel.at_epoch = Some(epoch);
        self.apply_tt();
    }

    /// Set the clock's unix timestamp outright (wins over relative jumps).
    pub fn warp_to_timestamp(&mut self, unix_timestamp: i64) {
        self.time_travel.at_unix_timestamp = Some(unix_timestamp);
        self.apply_tt();
    }

    /// Replace the whole clock warp at once (the JSON suite format's shape).
    pub fn set_time_travel(&mut self, tt: TimeTravel) {
        self.time_travel = tt;
        self.apply_tt();
    }

    fn apply_tt(&mut self) {
        self.ctx.set_time_travel(self.time_travel.clone());
    }

    /// A human description of the (possibly warped) clock replays run at,
    /// e.g. "slot 488,863,115 · epoch 1131 · 2026-09-26 14:03 UTC".
    pub fn describe_clock(&self) -> String {
        self.ctx.describe_clock()
    }

    // --- feature gates & IDLs ------------------------------------------------

    /// Flip one runtime feature gate for subsequent runs — replay a transaction
    /// as if a not-yet-live feature were active (or an active one weren't).
    pub fn set_feature(&mut self, id: Address, active: bool) {
        self.ctx.push_feature_toggle(FeatureToggle { id, active });
    }

    /// Replace all feature toggles at once (the JSON suite format's shape).
    pub fn set_features(&mut self, toggles: Vec<FeatureToggle>) {
        self.ctx.set_feature_toggles(toggles);
    }

    /// Register a program's IDL for named-field asserts and error explanations —
    /// for programs that publish nothing on-chain (e.g. your own, in development).
    pub fn add_idl(&mut self, program: impl Into<String>, idl: serde_json::Value) {
        self.ctx.add_idl(program.into(), idl);
    }

    // --- execution -----------------------------------------------------------

    /// Replay the transaction as-is. A reverting transaction is a successful
    /// observation (`result.success == false`), never an `Err`.
    pub fn run(&self) -> Result<Replayed> {
        self.simulate(&[])
    }

    /// Replay after applying what-if `mutations` to a fresh copy of the state.
    pub fn simulate(&self, mutations: &[Mutation]) -> Result<Replayed> {
        let warped = !self.time_travel.is_noop();
        let (mut result, raw_diffs) = self.ctx.run_with_diff(mutations)?;
        let explain = (!result.success)
            .then(|| explain_error(&result, self.ctx.idl_map()))
            .flatten();
        if result.error_name.is_none() {
            result.error_name = explain.as_ref().map(|e| e.title.clone());
        }
        Ok(Replayed {
            diffs: decode_diffs(raw_diffs, self.ctx.idl_map()),
            clock: warped.then(|| self.ctx.describe_clock()),
            explain,
            result,
        })
    }

    /// Unroll the transaction into a step-by-step [`Trace`] — the debugger's
    /// view. Every top-level instruction is replayed as a prefix (`0..=k`) and
    /// diffed against the previous prefix, so each step carries the exact
    /// accounts it changed; CPIs are attached from the runtime's inner
    /// instruction list with their logs and compute. `mutations` are applied to
    /// the initial state first ("what if this field were X, step by step").
    pub fn trace(&self, mutations: &[Mutation]) -> Result<Trace> {
        use {
            crate::{
                cpi_tree::{spans_from_logs, LogSpan},
                replay::replay_result_of,
                trace::{DecodedEvent, ReturnData, Step, StepAccountState, StepError},
            },
            base64::Engine,
            solana_account::Account,
            std::collections::HashMap,
        };

        // Prefix mode replays once per instruction: bound it so a pathological 60-instruction
        // transaction can't turn one request into a minute of CPU.
        const MAX_TRACE_INSTRUCTIONS: usize = 64;
        let n_ix = self.ctx.transaction().message.instructions().len();
        if n_ix > MAX_TRACE_INSTRUCTIONS {
            return Err(Error::InvalidSpec(format!(
                "transaction has {n_ix} instructions; the debugger traces up to {MAX_TRACE_INSTRUCTIONS}"
            )));
        }
        let runs = self.ctx.trace_raw(mutations)?;
        let idls = self.ctx.idl_map();
        let keys = self.ctx.message_account_keys();
        // The transaction as mutated (instruction edits, skipped instructions):
        // step names, data and indexes must follow what actually replayed.
        let tx = self.ctx.tx_for(mutations)?;
        let top_ixs = tx.message.instructions();
        // Original position of each mutated-transaction instruction, once
        // skipped instructions are accounted for.
        let skipped: std::collections::BTreeSet<usize> = mutations
            .iter()
            .filter_map(|m| match m {
                Mutation::SkipIx { index } => Some(*index),
                _ => None,
            })
            .collect();
        // Original position of each instruction of the mutated transaction:
        // drop the skipped ones, then apply the moves in order — exactly the
        // sequence `tx_with_mutations` performs on the instruction list.
        let original_of: Vec<usize> = {
            let mut order: Vec<usize> = (0..self.ctx.transaction().message.instructions().len())
                .filter(|i| !skipped.contains(i))
                .collect();
            for m in mutations {
                if let Mutation::MoveIx { from, to } = m {
                    if *from < order.len() && *to < order.len() {
                        let v = order.remove(*from);
                        order.insert(*to, v);
                    }
                }
            }
            order
        };
        let reordered = original_of.iter().enumerate().any(|(i, &o)| i != o);
        let n = top_ixs.len();

        // The whole transaction is the last prefix. Its logs are the canonical
        // ones every step's log range indexes into: instruction k's lines sit at
        // the same positions in prefix k and in the full run (execution is
        // deterministic and later instructions can't rewrite earlier logs).
        let full = runs.last().map(|r| replay_result_of(&r.result));
        let mut result = full.clone().unwrap_or_default();
        let explain = (!result.success)
            .then(|| explain_error(&result, idls))
            .flatten();
        if result.error_name.is_none() {
            result.error_name = explain.as_ref().map(|e| e.title.clone());
        }
        let whole_succeeded = result.success;
        // Where the whole transaction failed, as a top-level instruction index
        // (`InstructionError(<idx>, ..)`), so a prefix that fails *earlier*
        // than that is known to be an artifact of running as a prefix: the
        // whole run got past it. Introspecting instructions are the usual
        // case — a flash loan's repay search cannot see instructions the
        // prefix does not contain.
        let whole_failed_at: Option<usize> = runs
            .last()
            .and_then(|r| crate::replay::failed_instruction_index(&r.result));

        // Every invocation the full run logged, pre-order. Depth-1 spans are the
        // top-level instructions in message order; the spans that follow a
        // depth-1 span until the next one are its CPIs.
        let full_spans: Vec<LogSpan> = spans_from_logs(&result.logs, 0);
        let top_span_idx: Vec<usize> = full_spans
            .iter()
            .enumerate()
            .filter(|(_, s)| s.depth == 1)
            .map(|(i, _)| i)
            .collect();
        // Precompiles (Ed25519, secp256k1, secp256r1) run natively and log
        // nothing, so the n-th depth-1 span is not instruction n once one of
        // them is in the message. Map each message index to its logged span,
        // skipping precompiles, and give a precompile no span at all.
        let is_precompile: Vec<bool> = top_ixs
            .iter()
            .map(|ix| {
                keys.get(ix.program_id_index as usize)
                    .is_some_and(|p| crate::cpi_tree::is_precompile(p))
            })
            .collect();
        let span_slot: Vec<Option<usize>> = {
            let mut next = 0usize;
            is_precompile
                .iter()
                .map(|&pre| {
                    if pre {
                        None
                    } else {
                        let s = next;
                        next += 1;
                        Some(s)
                    }
                })
                .collect()
        };
        let spans_for = |k: usize| -> &[LogSpan] {
            match span_slot
                .get(k)
                .copied()
                .flatten()
                .and_then(|s| top_span_idx.get(s).map(|&i| (s, i)))
            {
                Some((s, start)) => {
                    let end = top_span_idx.get(s + 1).copied().unwrap_or(full_spans.len());
                    &full_spans[start..end]
                }
                None => &[],
            }
        };

        let mut steps: Vec<Step> = Vec::new();
        let mut failed_step: Option<usize> = None;
        let mut prev_post: Option<HashMap<Address, Account>> = None;
        // The world the first step actually ran against: initial accounts with
        // the mutations applied, so a "before" value is the mutated one.
        let initial: HashMap<Address, Account> = {
            let prepared = self.ctx.prepare(mutations, false)?;
            keys.iter()
                .filter_map(|k| Address::from_str(k).ok())
                .filter_map(|a| prepared.svm.get_account(&a).map(|acc| (a, acc)))
                .collect()
        };
        // Which step's post-state the diffs are relative to; a prefix that
        // failed commits nothing, so the next step's diffs start from the last
        // step that did.
        let mut last_post_step: Option<usize> = None;
        let mut halted = false; // a real failure happened; later steps never ran

        for (k, run) in runs.iter().enumerate().take(n) {
            let meta = match &run.result {
                Ok(m) => m,
                Err(f) => &f.meta,
            };
            let run_failed = run.result.is_err();
            let artifact =
                run_failed && (whole_succeeded || whole_failed_at.is_some_and(|w| w > k));
            let pos = run.keep.iter().position(|&i| i == k).unwrap_or(0);

            // Nodes for this step: the top-level instruction, then its CPIs in
            // emission order (already pre-order), from the prefix run's own
            // inner-instruction list.
            let mut nodes: Vec<(
                u8,
                &solana_message::compiled_instruction::CompiledInstruction,
            )> = vec![(1, &top_ixs[k])];
            if let Some(inner) = meta.inner_instructions.get(pos) {
                nodes.extend(inner.iter().map(|ii| (ii.stack_height, &ii.instruction)));
            }

            let spans = spans_for(k);
            let aligned = spans.len() == nodes.len()
                && spans.iter().zip(nodes.iter()).all(|(s, (d, ix))| {
                    s.depth == *d
                        && keys
                            .get(ix.program_id_index as usize)
                            .map(|p| *p == s.program)
                            .unwrap_or(false)
                });
            let top_range = spans.first().map(|s| (s.start, s.end));

            // Diffs for the top-level step: this prefix's post-state vs the
            // previous prefix's (or the pre-transaction state for k == 0).
            let post: Option<HashMap<Address, Account>> =
                run.post.as_ref().map(|p| p.iter().cloned().collect());
            let mut diffs = match &post {
                Some(post) => {
                    let before_of = |a: &Address| -> Option<Account> {
                        prev_post
                            .as_ref()
                            .and_then(|m| m.get(a).cloned())
                            .or_else(|| initial.get(a).cloned())
                    };
                    diffs_of(&before_of, post, &keys, idls)
                }
                None => Vec::new(),
            };

            // Completion order of the nodes (post-order): a CPI finishes before
            // its caller, so the runtime observer's inner snapshots line up with
            // nodes in this order. Exact only when every inner node has one.
            let completion = completion_ranks(nodes.iter().map(|(d, _)| *d));
            // Exact only when every inner node has a snapshot whose program
            // and depth agree with it — otherwise no CPI borrows a snapshot.
            let inner_exact = nodes.len() > 1
                && run.inner_posts.len() == nodes.len() - 1
                && nodes.iter().enumerate().skip(1).all(|(i, (d, ix))| {
                    run.inner_posts.get(completion[i]).is_some_and(|s| {
                        s.height == *d
                            && keys
                                .get(ix.program_id_index as usize)
                                .is_some_and(|p| *p == s.program)
                    })
                });
            let inner_map = |rank: usize| -> Option<HashMap<Address, Account>> {
                run.inner_posts
                    .get(rank)
                    .map(|s| s.accounts.iter().cloned().collect())
            };
            // The state an inner instruction started from: its own entry
            // snapshot, so the caller's changes before the call are never
            // credited to the call.
            let inner_entry = |rank: usize| -> Option<HashMap<Address, Account>> {
                run.inner_posts
                    .get(rank)
                    .map(|s| s.entry.iter().cloned().collect())
            };
            let diffs_between =
                |before_of: &dyn Fn(&Address) -> Option<Account>,
                 after: &HashMap<Address, Account>,
                 addrs: &[String]|
                 -> Vec<AccountDiff> { diffs_of(before_of, after, addrs, idls) };

            // State after this prefix, for the accounts each node names.
            let changed: std::collections::HashSet<String> =
                diffs.iter().map(|d| d.address.clone()).collect();
            let state_of = |map: Option<&HashMap<Address, Account>>,
                            changed: &std::collections::HashSet<String>,
                            addr: &str,
                            role: Option<String>,
                            decode_fields: bool|
             -> StepAccountState {
                account_state(&self.ctx, idls, map, changed, addr, role, decode_fields)
            };
            let return_data = (!meta.return_data.data.is_empty()).then(|| ReturnData {
                program: meta.return_data.program_id.to_string(),
                data_base64: base64::engine::general_purpose::STANDARD
                    .encode(&meta.return_data.data),
            });

            // Emit the nodes.
            let base_index = steps.len();
            let mut paths = PathBuilder::default();
            let mut innermost_failure: Option<usize> = None;
            for (i, (depth, ix)) in nodes.iter().enumerate() {
                let program = keys
                    .get(ix.program_id_index as usize)
                    .cloned()
                    .unwrap_or_default();
                let account_indexes: Vec<usize> = ix.accounts.iter().map(|&a| a as usize).collect();
                let (name, args, accounts) =
                    ixname::enrich_offline(idls, &program, &ix.data, &account_indexes, &keys);

                let path = paths.next(k, *depth as usize);

                // A CPI with its own snapshots: state as it stood when this
                // inner instruction finished, and its changes measured from
                // the state it started with.
                let (node_post, node_diffs): (Option<HashMap<Address, Account>>, Vec<AccountDiff>) =
                    if i > 0 && inner_exact {
                        let rank = completion[i];
                        match inner_map(rank) {
                            Some(after) => {
                                let prev = inner_entry(rank);
                                let before_of = |a: &Address| -> Option<Account> {
                                    prev.as_ref()
                                        .and_then(|m| m.get(a).cloned())
                                        .or_else(|| {
                                            prev_post.as_ref().and_then(|m| m.get(a).cloned())
                                        })
                                        .or_else(|| initial.get(a).cloned())
                                };
                                let addrs: Vec<String> =
                                    accounts.iter().map(|a| a.address.clone()).collect();
                                let d = diffs_between(&before_of, &after, &addrs);
                                (Some(after), d)
                            }
                            None => (None, Vec::new()),
                        }
                    } else {
                        (None, Vec::new())
                    };
                let node_changed: std::collections::HashSet<String> =
                    node_diffs.iter().map(|d| d.address.clone()).collect();
                let (state_map, state_changed) = if node_post.is_some() {
                    (node_post.as_ref(), &node_changed)
                } else {
                    (post.as_ref(), &changed)
                };

                // Per-node state: the accounts this instruction names, capped so a
                // 40-account Jupiter route doesn't dump megabytes of fields.
                let mut seen = std::collections::HashSet::new();
                let state: Vec<StepAccountState> = accounts
                    .iter()
                    .filter(|a| seen.insert(a.address.clone()))
                    .take(96)
                    .enumerate()
                    .map(|(n, a)| {
                        state_of(state_map, state_changed, &a.address, a.name.clone(), n < 32)
                    })
                    .collect();

                let (logs, cu, success) = if aligned {
                    let s = &spans[i];
                    ((s.start, s.end), s.cu_consumed, s.success)
                } else if i == 0 {
                    let r = top_range.unwrap_or((result.logs.len(), result.logs.len()));
                    (r, spans.first().and_then(|s| s.cu_consumed), !run_failed)
                } else {
                    let r = top_range.unwrap_or((result.logs.len(), result.logs.len()));
                    (r, None, !run_failed)
                };
                if aligned && !success {
                    innermost_failure = Some(base_index + i);
                }
                // Events: `Program data: <b64>` lines inside this node's own log
                // range, decoded against its program's IDL.
                let events: Vec<DecodedEvent> = idls
                    .get(&program)
                    .map(|idl| {
                        let (a, b) = logs;
                        result.logs[a.min(result.logs.len())..b.min(result.logs.len())]
                            .iter()
                            .filter_map(|l| l.strip_prefix("Program data: "))
                            .filter_map(|b64| {
                                base64::engine::general_purpose::STANDARD
                                    .decode(b64.trim())
                                    .ok()
                            })
                            .filter_map(|bytes: Vec<u8>| idl::decode_event(idl, &bytes))
                            .map(|d| DecodedEvent {
                                name: d.type_name,
                                fields: d.fields,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                // Return data set by *this* invocation: the runtime logs
                // `Program return: <program> <base64>` inside the span. The
                // transaction-level return data is only the last one set, so it
                // is attributed to a step only when nothing in the span says
                // otherwise and the step is the last of its program.
                let node_return = result.logs[logs.0..logs.1]
                    .iter()
                    .rev()
                    .find_map(|l| {
                        let rest = l.strip_prefix("Program return: ")?;
                        let (p, b64) = rest.split_once(' ')?;
                        (p == program).then(|| ReturnData {
                            program: p.to_string(),
                            data_base64: b64.trim().to_string(),
                        })
                    })
                    .or_else(|| {
                        return_data
                            .as_ref()
                            .filter(|r| r.program == program && k + 1 == n && i == 0)
                            .cloned()
                    });
                steps.push(Step {
                    path,
                    depth: *depth,
                    index: k,
                    diffs_since: (k > 0 && last_post_step != Some(k - 1))
                        .then_some(last_post_step)
                        .flatten(),
                    data_hex: (*depth == 1)
                        .then(|| top_ixs[k].data.iter().map(|b| format!("{b:02x}")).collect()),
                    original_index: reordered.then(|| original_of.get(k).copied()).flatten(),
                    program,
                    name,
                    args,
                    accounts,
                    cu_consumed: cu,
                    logs,
                    diffs: if i == 0 {
                        std::mem::take(&mut diffs)
                    } else {
                        node_diffs
                    },
                    state_known: !halted
                        && (if i == 0 {
                            post.is_some()
                        } else {
                            node_post.is_some()
                        }),
                    success: if halted { false } else { success },
                    error: None,
                    return_data: node_return,
                    events,
                    state,
                    prefix_artifact: artifact,
                });
            }

            if run_failed && !artifact && !halted {
                // Attribute the error to the innermost failing invocation, or
                // the top-level step when logs could not be aligned.
                let target = innermost_failure.unwrap_or(base_index);
                let raw = match &run.result {
                    Err(f) => format!("{:?}", f.err),
                    Ok(_) => String::new(),
                };
                let r = replay_result_of(&run.result);
                steps[target].error = Some(StepError {
                    raw,
                    explain: explain_error(&r, idls),
                });
                steps[target].success = false;
                failed_step = Some(target);
                halted = true;
            }

            if !run_failed {
                prev_post = post;
                last_post_step = Some(k);
            }
        }

        Ok(Trace {
            signature: self.ctx.signature().to_string(),
            fee_payer: keys.first().cloned().unwrap_or_default(),
            steps,
            result,
            explain,
            failed_step,
            clock: (!self.time_travel.is_noop()).then(|| self.ctx.describe_clock()),
            fidelity: self.fidelity.label().to_string(),
            onchain_success: self.recorded.as_ref().map(|r| r.success),
            tier: None,
            tier_note: None,
            state_slot: None,
            drifted: Vec::new(),
        })
    }

    /// Replay with `mutations` and read one account's raw post-execution state.
    /// The building block of historical reconstruction (see the [`reconstruct`](crate::reconstruct)
    /// module): chain it by injecting an account's reconstructed bytes, replaying
    /// its next write, and reading it out again. `None` if the account does not
    /// exist after the replay.
    ///
    pub fn account_after(
        &self,
        mutations: &[Mutation],
        address: &str,
    ) -> Result<Option<AccountState>> {
        let (_result, acc) = self.ctx.run_and_read_account(mutations, address)?;
        Ok(acc.map(|a| AccountState {
            data: a.data,
            lamports: a.lamports,
            owner: a.owner.to_string(),
        }))
    }

    // --- counterfactual search -----------------------------------------------

    /// Binary-search a numeric knob for the value at which the outcome flips —
    /// "at what oracle price does this stop succeeding?", "what is the minimum
    /// balance that avoids the revert?". `mutate(v)` builds the mutation(s) that
    /// set the knob to candidate `v`; the search runs over the inclusive range
    /// `[lo, hi]` and returns the boundary (or `None` if the outcome is the same
    /// at both bounds). Every candidate is a fresh, independent replay, so the
    /// search never mutates shared state. Assumes a single crossing.
    ///
    /// ```no_run
    /// # use svmscope::{Mutation, Scope};
    /// # let replay = Scope::new("").replay("")?;
    /// // Lowest fee-payer balance at which the transaction still lands:
    /// let payer = "…".to_string();
    /// let boundary = replay.find_threshold(0, 5_000_000_000, |v| {
    ///     vec![Mutation::lamports(payer.clone(), v)]
    /// })?;
    /// # Ok::<(), svmscope::Error>(())
    /// ```
    pub fn find_threshold(
        &self,
        lo: u64,
        hi: u64,
        mutate: impl Fn(u64) -> Vec<Mutation>,
    ) -> Result<Option<Threshold>> {
        search_threshold(lo, hi, |v| Ok(self.simulate(&mutate(v))?.result.success))
    }

    /// Shrink a set of mutations to a minimal subset that still flips the
    /// outcome — "which of these changes actually caused the difference?".
    ///
    /// "Flips" means the subset's success differs from the un-mutated baseline.
    /// Runs a greedy delta-debugging pass: drop each mutation whose removal keeps
    /// the outcome flipped. Returns the minimal subset (input order preserved),
    /// or an empty vec if the full set doesn't change the baseline outcome at all.
    pub fn minimize_mutations(&self, mutations: &[Mutation]) -> Result<Vec<Mutation>> {
        let baseline = self.run()?.result.success;
        let target = self.simulate(mutations)?.result.success;
        if target == baseline {
            return Ok(Vec::new());
        }
        let mut keep = mutations.to_vec();
        let mut i = 0;
        while i < keep.len() {
            let mut trial = keep.clone();
            trial.remove(i);
            if self.simulate(&trial)?.result.success == target {
                keep = trial; // mutation i wasn't needed to keep the flip
            } else {
                i += 1; // it's load-bearing; keep it and move on
            }
        }
        Ok(keep)
    }

    // --- patch lab -----------------------------------------------------------

    /// Replace a program's ELF bytecode in this replay's world, for A/B patch
    /// testing. Returns the previous ELF (restore it by calling again with that).
    /// Errors if `program_id` isn't loaded as a program in this replay.
    pub fn replace_program(&mut self, program_id: &str, elf: Vec<u8>) -> Result<Vec<u8>> {
        self.ctx.replace_program(program_id, elf).ok_or_else(|| {
            Error::InvalidSpec(format!(
                "{program_id} is not a loaded program in this replay"
            ))
        })
    }

    /// Replay the transaction against the original program and against
    /// `patched_elf`, and report the difference — the pre-deployment gate "does
    /// this patch change what really happened?". `mutations` apply to both runs.
    /// `self` is left unchanged: the patch is swapped in for the comparison, then
    /// the original restored.
    pub fn compare_patch(
        &mut self,
        program_id: &str,
        patched_elf: Vec<u8>,
        mutations: &[Mutation],
    ) -> Result<PatchComparison> {
        let before = self.simulate(mutations)?.result;
        let original = self.replace_program(program_id, patched_elf)?;
        let after = self.simulate(mutations)?.result;
        // Restore the original ELF so the replay is reusable afterwards.
        self.ctx.replace_program(program_id, original);
        Ok(PatchComparison {
            program: program_id.to_string(),
            before,
            after,
        })
    }

    /// Run a suite of scenarios, each against a fresh copy of the state.
    /// Mutations across the whole suite are validated before anything executes.
    pub fn run_suite(&self, scenarios: &[Scenario]) -> Result<Vec<ScenarioOutcome>> {
        crate::replay::run_suite(&self.ctx, self.recorded.as_ref(), scenarios)
    }

    /// Run one named scenario — mutations plus the checks that must hold —
    /// and report it. Sugar over [`Replay::run_suite`] for the single case.
    pub fn verify(
        &self,
        name: impl Into<String>,
        mutations: &[Mutation],
        checks: &[Check],
    ) -> Result<ScenarioOutcome> {
        let scenario = Scenario {
            name: name.into(),
            mutations: mutations.to_vec(),
            checks: checks.to_vec(),
        };
        let mut outcomes = self.run_suite(std::slice::from_ref(&scenario))?;
        Ok(outcomes.remove(0))
    }

    /// Freeze this world into a portable [`Fixture`] for offline CI replay.
    /// Captures the loaded IDLs and the recorded on-chain outcome, so field
    /// asserts and [`Check::matches_onchain`] work offline too.
    pub fn to_fixture(&self) -> Result<Fixture> {
        let mut fx = self.ctx.to_fixture()?;
        fx.recorded = self.recorded.clone();
        Ok(fx)
    }

    /// Generate a self-contained Rust regression test that freezes this incident
    /// permanently: it loads the fixture at `fixture_path` (write
    /// `to_fixture()?.to_json()?` there next to your test), rebuilds the replay
    /// fully offline, and asserts the same outcome this replay produces now — a
    /// reverting incident also pins the exact error. Returns the test source.
    pub fn regression_test(&self, test_name: &str, fixture_path: &str) -> Result<String> {
        let outcome = self.run()?;
        Ok(crate::report::rust_regression_test(
            test_name,
            fixture_path,
            outcome.result.success,
            outcome.result.error.as_deref(),
        ))
    }
}

/// The outcome of one local replay: the result itself, what changed, and — on
/// failure — a plain-language explanation.
#[derive(Debug, Serialize)]
pub struct Replayed {
    /// The replay's outcome (success, error, logs, CU).
    pub result: ReplayResult,
    /// Every account the transaction changed, before → after, with named
    /// fields where the layout (or an IDL) is known.
    pub diffs: Vec<AccountDiff>,
    /// Where the clock was warped to, when time travel was requested.
    pub clock: Option<String>,
    /// The failure in plain language, when the replay failed.
    pub explain: Option<Explanation>,
}

impl Replayed {
    /// The wire shape the HTTP API serves (`SimulationReport`) — same JSON as v0.1.
    pub fn into_report(self) -> SimulationReport {
        SimulationReport {
            replay: self.result,
            clock: self.clock,
            explain: self.explain,
            diffs: self.diffs,
            preflight: None,
        }
    }
}

/// Turn a program error into a human explanation using the loaded IDLs.
/// (Same logic the v0.1 library ran with live RPC — now resolved offline
/// against the IDLs preloaded into the replay context.)
fn explain_error(
    r: &ReplayResult,
    idls: &HashMap<String, serde_json::Value>,
) -> Option<Explanation> {
    let raw = r.error.as_ref()?;

    // Which program failed? The last "Program <id> failed" line names it.
    let program = r
        .logs
        .iter()
        .rev()
        .find_map(|l| {
            l.strip_prefix("Program ")
                .and_then(|s| s.split(" failed").next())
        })
        .map(|s| s.trim().to_string())
        .filter(|s| Address::from_str(s).is_ok());

    // Anchor prints the resolved error itself — prefer that, it's already human.
    if let Some(line) = r.logs.iter().rev().find(|l| l.contains("Error Message:")) {
        let detail = line
            .split("Error Message:")
            .nth(1)
            .unwrap_or("")
            .trim()
            .to_string();
        let title = line
            .split("Error Code:")
            .nth(1)
            .and_then(|s| s.split('.').next())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "Program error".into());
        return Some(Explanation {
            title,
            detail,
            program,
            raw: raw.clone(),
        });
    }

    // Native programs define their errors in code, not an IDL. Name the common ones.
    if let (Some(p), Some(code)) = (
        program.as_deref(),
        raw.split("Custom(")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .and_then(|s| s.parse::<u64>().ok()),
    ) {
        if let Some((title, detail)) = native_error(p, code) {
            return Some(Explanation {
                title: title.into(),
                detail: detail.into(),
                program,
                raw: raw.clone(),
            });
        }
    }

    // Otherwise resolve the custom code against the program's IDL.
    if let Some(code) = raw
        .split("Custom(")
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.parse::<u64>().ok())
    {
        if let Some(e) = program
            .as_ref()
            .and_then(|p| idls.get(p))
            .and_then(|i| idl::error_for_code(i, code))
        {
            return Some(Explanation {
                title: e.name,
                detail: e.msg,
                program,
                raw: raw.clone(),
            });
        }
    }

    // Fall back to a friendly reading of the common runtime errors.
    let (title, detail) = if let Some(named) = runtime_error(raw) {
        named
    } else if raw.contains("AccountNotFound") {
        ("Account not found", "An account the transaction needs doesn't exist (an account with zero lamports is treated as deleted).")
    } else if raw.contains("InsufficientFunds") {
        (
            "Insufficient funds",
            "An account didn't have enough lamports for the transfer plus rent.",
        )
    } else if raw.contains("InvalidAddressLookupTableIndex") {
        ("Lookup table index invalid", "The transaction referenced an address lookup table entry that isn't active at this slot.")
    } else if let Some(code) = raw
        .split("Custom(")
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.parse::<u64>().ok())
    {
        let who = program
            .as_deref()
            .map(|p| format!("Program {p}"))
            .unwrap_or_else(|| "The program".to_string());
        let has_idl = program
            .as_ref()
            .map(|p| idls.contains_key(p))
            .unwrap_or(false);
        let detail = if has_idl {
            format!(
                "{who} returned custom error {code} (0x{code:x}), which its IDL does not define. Check the program's source for the code."
            )
        } else {
            format!(
                "{who} returned custom error {code} (0x{code:x}). It publishes no IDL, so the error has no name here; the meaning is in the program's source."
            )
        };
        return Some(Explanation {
            title: format!("Custom error {code}"),
            detail,
            program,
            raw: raw.clone(),
        });
    } else {
        (
            "Transaction failed",
            "The program returned an error. See the logs below for the failing instruction.",
        )
    };
    Some(Explanation {
        title: title.into(),
        detail: detail.into(),
        program,
        raw: raw.clone(),
    })
}

/// Plain-language readings of the runtime's own `InstructionError` /
/// `TransactionError` variants, matched on the formatted error string.
fn runtime_error(raw: &str) -> Option<(&'static str, &'static str)> {
    const TABLE: &[(&str, &str, &str)] = &[
        ("ProgramFailedToComplete", "Program failed to complete", "The program aborted: a panic, an out-of-bounds access, or an exceeded compute budget. Check the last log lines of the failing step."),
        ("ComputationalBudgetExceeded", "Compute budget exceeded", "The transaction used more compute units than it requested. Raise the compute unit limit."),
        ("InvalidAccountData", "Invalid account data", "An account's data was not what the program expected: wrong layout, uninitialized, or owned by a different program."),
        ("AccountDataTooSmall", "Account data too small", "An account is smaller than the program requires."),
        ("MissingRequiredSignature", "Missing required signature", "An account that must sign did not."),
        ("IncorrectProgramId", "Incorrect program id", "An account is owned by a different program than expected."),
        ("InvalidArgument", "Invalid argument", "The program rejected an instruction argument."),
        ("InvalidInstructionData", "Invalid instruction data", "The instruction data did not decode."),
        ("PrivilegeEscalation", "Privilege escalation", "A CPI tried to use an account as a signer or writable when the caller could not."),
        ("ExternalAccountLamportSpend", "External account lamport spend", "A program debited lamports from an account it does not own."),
        ("ReadonlyLamportChange", "Read-only lamport change", "A program changed the balance of an account passed as read-only."),
        ("ReadonlyDataModified", "Read-only data modified", "A program wrote to an account passed as read-only."),
        ("ExecutableDataModified", "Executable data modified", "A program tried to write to an executable account."),
        ("AccountBorrowFailed", "Account borrow failed", "The same account was borrowed mutably twice in one instruction."),
        ("UnbalancedInstruction", "Unbalanced instruction", "Lamports were created or destroyed: the sums before and after differ."),
        ("MaxSeedLengthExceeded", "Max seed length exceeded", "A PDA seed is longer than 32 bytes."),
        ("InvalidSeeds", "Invalid seeds", "The seeds do not derive the given program address."),
        ("InvalidRealloc", "Invalid realloc", "The account resize was rejected."),
        ("AccountAlreadyInitialized", "Account already initialized", "The account was already initialized."),
        ("UninitializedAccount", "Uninitialized account", "The account has not been initialized."),
        ("NotEnoughAccountKeys", "Not enough account keys", "The instruction was given fewer accounts than it needs."),
        ("InsufficientFundsForRent", "Insufficient funds for rent", "An account would be left below the rent-exempt minimum."),
        ("InsufficientFundsForFee", "Insufficient funds for fee", "The fee payer cannot cover the transaction fee."),
        ("BlockhashNotFound", "Blockhash not found", "The transaction's blockhash is not recent."),
        ("AlreadyProcessed", "Already processed", "This exact transaction was already executed."),
        ("TooManyAccountLocks", "Too many account locks", "The transaction references more accounts than allowed."),
        ("MaxLoadedAccountsDataSizeExceeded", "Loaded accounts too large", "The accounts loaded exceed the requested data size limit."),
        ("InvalidAddressLookupTableIndex", "Lookup table index invalid", "The transaction referenced an address lookup table entry that isn't active at this slot."),
        ("AccountNotFound", "Account not found", "An account the transaction needs doesn't exist (an account with zero lamports is treated as deleted)."),
    ];
    TABLE
        .iter()
        .find(|(needle, _, _)| raw.contains(needle))
        .map(|(_, t, d)| (*t, *d))
}

/// Error names for the native programs that have no IDL (System, SPL Token,
/// Token-2022, Associated Token). Codes follow `SystemError` / `TokenError`.
fn native_error(program: &str, code: u64) -> Option<(&'static str, &'static str)> {
    const SYSTEM: &str = "11111111111111111111111111111111";
    const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
    const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
    const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
    Some(match (program, code) {
        (SYSTEM, 0) => (
            "Account already in use",
            "The account to create already exists.",
        ),
        (SYSTEM, 1) => (
            "Insufficient funds",
            "The transfer would leave the source account with a negative balance.",
        ),
        (SYSTEM, 2) => (
            "Invalid program id",
            "The account cannot be assigned to that program.",
        ),
        (SYSTEM, 3) => (
            "Invalid account data length",
            "The requested allocation size is not allowed.",
        ),
        (SYSTEM, 4) => (
            "Max seed length exceeded",
            "A seed for a derived address is too long.",
        ),
        (SYSTEM, 5) => (
            "Address with seed mismatch",
            "The address does not derive from the given base and seed.",
        ),
        (SYSTEM, 6) => (
            "Nonce has no recent blockhashes",
            "The durable nonce could not be advanced.",
        ),
        (SYSTEM, 7) => (
            "Nonce blockhash not expired",
            "The stored nonce is still valid, so it cannot be advanced yet.",
        ),
        (SYSTEM, 8) => (
            "Unexpected nonce value",
            "The transaction's blockhash does not match the nonce account.",
        ),
        (TOKEN | TOKEN_2022, 0) => (
            "Not rent exempt",
            "The account lacks the lamports to be rent-exempt.",
        ),
        (TOKEN | TOKEN_2022, 1) => (
            "Insufficient funds",
            "The token account holds fewer tokens than the instruction moves.",
        ),
        (TOKEN | TOKEN_2022, 2) => ("Invalid mint", "The mint account is not valid."),
        (TOKEN | TOKEN_2022, 3) => (
            "Mint mismatch",
            "The token account belongs to a different mint.",
        ),
        (TOKEN | TOKEN_2022, 4) => (
            "Owner mismatch",
            "The signer is not the token account's owner or delegate.",
        ),
        (TOKEN | TOKEN_2022, 5) => ("Fixed supply", "The mint has no mint authority."),
        (TOKEN | TOKEN_2022, 6) => ("Already in use", "The account is already initialized."),
        (TOKEN | TOKEN_2022, 7) => (
            "Invalid number of provided signers",
            "A multisig received the wrong number of signers.",
        ),
        (TOKEN | TOKEN_2022, 8) => (
            "Invalid number of required signers",
            "The multisig threshold is out of range.",
        ),
        (TOKEN | TOKEN_2022, 9) => (
            "Uninitialized state",
            "The account has not been initialized.",
        ),
        (TOKEN | TOKEN_2022, 10) => (
            "Native not supported",
            "This instruction does not apply to a native (wrapped SOL) account.",
        ),
        (TOKEN | TOKEN_2022, 11) => (
            "Non-native has balance",
            "A non-native account cannot be closed while it holds tokens.",
        ),
        (TOKEN | TOKEN_2022, 12) => (
            "Invalid instruction",
            "The instruction data did not decode.",
        ),
        (TOKEN | TOKEN_2022, 13) => (
            "Invalid state",
            "The account is in a state that does not allow this operation.",
        ),
        (TOKEN | TOKEN_2022, 14) => ("Overflow", "An arithmetic operation overflowed."),
        (TOKEN | TOKEN_2022, 15) => (
            "Authority type not supported",
            "The mint or account has no such authority.",
        ),
        (TOKEN | TOKEN_2022, 16) => ("Mint cannot freeze", "The mint has no freeze authority."),
        (TOKEN | TOKEN_2022, 17) => ("Account frozen", "The token account is frozen."),
        (TOKEN | TOKEN_2022, 18) => (
            "Mint decimals mismatch",
            "The decimals passed do not match the mint.",
        ),
        (TOKEN | TOKEN_2022, 19) => (
            "Non-native not supported",
            "This instruction only applies to native (wrapped SOL) accounts.",
        ),
        (ATA, 0) => (
            "Invalid owner",
            "The associated token account's owner does not match.",
        ),
        _ => return None,
    })
}

/// Decode raw before/after bytes into named field changes, using built-in
/// layouts first and the preloaded IDLs second.
fn decode_diffs(
    raw: Vec<crate::replay::RawAccountDiff>,
    idls: &HashMap<String, serde_json::Value>,
) -> Vec<AccountDiff> {
    raw.into_iter()
        .map(|d| {
            let idl = idls.get(&d.owner);
            let decode_side = |bytes: &[u8]| -> Option<decode::DecodedAccount> {
                decode::decode_bytes(&d.owner, bytes)
                    .or_else(|| idl.and_then(|i| idl::decode_with_idl(i, bytes)))
            };
            let (before, after) = (decode_side(&d.data_before), decode_side(&d.data_after));
            let mut fields = Vec::new();
            if let (Some(b), Some(a)) = (&before, &after) {
                for (fb, fa) in b.fields.iter().zip(a.fields.iter()) {
                    if fb.value != fa.value {
                        fields.push(FieldDiff {
                            name: fa.name.clone(),
                            ty: fa.ty.clone(),
                            before: fb.value.clone(),
                            after: fa.value.clone(),
                        });
                    }
                }
            }
            let raw_data_changed = d.data_before != d.data_after && fields.is_empty();
            AccountDiff {
                address: d.address,
                owner: d.owner,
                lamports_before: d.lamports_before,
                lamports_after: d.lamports_after,
                fields,
                raw_data_changed,
            }
        })
        .collect()
}

/// Diffs of `addrs` between a before-lookup and an after-state. A newly
/// created account (no "before") is diffed against an empty account rather
/// than dropped; an account absent from `after` is treated as unchanged.
fn diffs_of(
    before_of: &dyn Fn(&Address) -> Option<solana_account::Account>,
    after: &HashMap<Address, solana_account::Account>,
    addrs: &[String],
    idls: &HashMap<String, serde_json::Value>,
) -> Vec<AccountDiff> {
    let mut raw = Vec::new();
    for key in addrs {
        let Ok(addr) = Address::from_str(key) else {
            continue;
        };
        let before = before_of(&addr);
        let after_acc = after.get(&addr).cloned().or_else(|| before.clone());
        let before = before.or_else(|| {
            after_acc
                .as_ref()
                .map(|_| solana_account::Account::default())
        });
        let (Some(b), Some(a)) = (&before, &after_acc) else {
            continue;
        };
        if b.lamports == a.lamports && b.data == a.data && b.owner == a.owner {
            continue;
        }
        raw.push(crate::replay::RawAccountDiff {
            address: key.clone(),
            owner: a.owner.to_string(),
            lamports_before: b.lamports,
            lamports_after: a.lamports,
            data_before: b.data.clone(),
            data_after: a.data.clone(),
        });
    }
    decode_diffs(raw, idls)
}

/// Post-order completion rank of each node of a pre-order tree given by
/// depth: a CPI finishes before its caller, so the runtime observer's inner
/// snapshots (recorded at completion) line up with nodes in this order.
fn completion_ranks(depths: impl Iterator<Item = u8>) -> Vec<usize> {
    let depths: Vec<u8> = depths.collect();
    let mut completion = vec![usize::MAX; depths.len()];
    let mut open: Vec<usize> = Vec::new();
    let mut rank = 0usize;
    for (i, d) in depths.iter().enumerate() {
        while open.last().is_some_and(|&t| depths[t] >= *d) {
            completion[open.pop().unwrap()] = rank;
            rank += 1;
        }
        open.push(i);
    }
    while let Some(t) = open.pop() {
        completion[t] = rank;
        rank += 1;
    }
    completion
}

#[cfg(test)]
mod trace_helper_tests {
    use super::completion_ranks;

    #[test]
    fn completion_is_post_order() {
        // top(1) → a(2) → a.0(3), a.1(3); b(2)
        assert_eq!(
            completion_ranks([1u8, 2, 3, 3, 2].into_iter()),
            vec![4, 2, 0, 1, 3]
        );
    }
}

/// Assigns step paths in pre-order: "k" for a top-level instruction, then
/// "k.c0", "k.c0.c1", … for its CPIs. `next` holds the next child index per
/// depth and `lineage` the indexes assigned to the current node's ancestors,
/// so a child of "k.0" is "k.0.0": a parent's counter advances only for its
/// own siblings.
#[derive(Default)]
struct PathBuilder {
    next: Vec<usize>,
    lineage: Vec<usize>,
}

impl PathBuilder {
    fn next(&mut self, k: usize, depth: usize) -> String {
        if depth <= 1 {
            self.next.clear();
            self.lineage.clear();
            return k.to_string();
        }
        let level = depth - 2;
        self.next.truncate(level + 1);
        while self.next.len() <= level {
            self.next.push(0);
        }
        let idx = self.next[level];
        self.next[level] += 1;
        self.lineage.truncate(level);
        self.lineage.push(idx);
        let mut p = k.to_string();
        for c in &self.lineage {
            p.push('.');
            p.push_str(&c.to_string());
        }
        p
    }
}

/// One account as a step shows it: from `map` (the state after the step)
/// when present, else the loaded pre-state; decoded through the owner's
/// layout or IDL when asked or when the step changed it.
fn account_state(
    ctx: &crate::replay::ReplayContext,
    idls: &HashMap<String, serde_json::Value>,
    map: Option<&HashMap<Address, solana_account::Account>>,
    changed: &std::collections::HashSet<String>,
    addr: &str,
    role: Option<String>,
    decode_fields: bool,
) -> crate::trace::StepAccountState {
    use crate::trace::StepAccountState;
    let acc = map
        .and_then(|m| {
            Address::from_str(addr)
                .ok()
                .and_then(|a| m.get(&a).cloned())
        })
        .or_else(|| ctx.pre_account_owned(addr));
    match acc {
        Some(a) => {
            let owner = a.owner.to_string();
            let decoded = if decode_fields || changed.contains(addr) {
                decode::decode_bytes(&owner, &a.data).or_else(|| {
                    idls.get(&owner)
                        .and_then(|i| idl::decode_with_idl(i, &a.data))
                })
            } else {
                None
            };
            StepAccountState {
                address: addr.to_string(),
                role,
                owner,
                lamports: a.lamports,
                data_len: a.data.len(),
                type_name: decoded.as_ref().map(|d| d.type_name.clone()),
                fields: decoded.map(|d| d.fields).unwrap_or_default(),
                changed: changed.contains(addr),
                exists: true,
            }
        }
        None => StepAccountState {
            address: addr.to_string(),
            role,
            owner: String::new(),
            lamports: 0,
            data_len: 0,
            type_name: None,
            fields: Vec::new(),
            changed: changed.contains(addr),
            exists: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fidelity_labels_are_honest() {
        assert_eq!(Fidelity::Current.label(), "current");
        assert_eq!(
            Fidelity::Reconstructed { slot: 442384762 }.label(),
            "reconstructed@442384762"
        );
        assert_eq!(Fidelity::Exact { slot: 100 }.label(), "exact@100");
    }

    #[test]
    fn patch_comparison_reports_what_changed() {
        let mk = |success: bool, err: Option<&str>, cu: u64| ReplayResult {
            success,
            error: err.map(String::from),
            error_name: None,
            logs: Vec::new(),
            compute_units: cu,
        };

        let fixed = PatchComparison {
            program: "P".to_string(),
            before: mk(false, Some("Custom(6001)"), 100),
            after: mk(true, None, 120),
        };
        assert!(fixed.success_changed());
        assert!(fixed.changed());
        assert_eq!(fixed.compute_delta(), 20);
        assert!(
            fixed.summary().contains("revert → success"),
            "{}",
            fixed.summary()
        );

        let unchanged = PatchComparison {
            program: "P".to_string(),
            before: mk(true, None, 100),
            after: mk(true, None, 100),
        };
        assert!(!unchanged.changed());
        assert!(unchanged.summary().contains("no observable change"));
    }

    #[test]
    fn certificate_summary_discloses_drift_and_verifiability() {
        let drifted = FidelityCertificate {
            fidelity: Fidelity::Reconstructed { slot: 442384762 },
            clock: "slot 442384762".to_string(),
            accounts: Vec::new(),
            drifted: vec!["AccA".to_string(), "AccB".to_string()],
            verifiable: true,
        };
        let s = drifted.summary();
        assert!(s.contains("reconstructed@442384762"), "{s}");
        assert!(s.contains("2 may have drifted"), "{s}");
        assert!(s.contains("verifiable against mainnet"), "{s}");

        let clean = FidelityCertificate {
            fidelity: Fidelity::Exact { slot: 5 },
            clock: String::new(),
            accounts: Vec::new(),
            drifted: Vec::new(),
            verifiable: false,
        };
        let s = clean.summary();
        assert!(s.contains("none drifted"), "{s}");
        assert!(s.contains("no recorded outcome"), "{s}");
    }
}

#[cfg(test)]
mod trace_parity_tests {
    use crate::{Fixture, Replay};

    fn check(fixture: &str) {
        let replay = Replay::from_fixture(&Fixture::from_json(fixture).unwrap()).unwrap();
        let plain = replay.run().unwrap();
        let traced = replay.trace(&[]).unwrap();
        assert_eq!(
            traced.result.success, plain.result.success,
            "verdict changed by tracing"
        );
        assert_eq!(
            traced.result.error, plain.result.error,
            "error changed by tracing"
        );
        assert_eq!(
            traced.result.compute_units, plain.result.compute_units,
            "compute changed by tracing"
        );
        // The trace's verdict must also be the transaction's: every step
        // succeeded iff the transaction did, and a failure names a step.
        assert_eq!(traced.failed_step.is_some(), !plain.result.success);
        if let Some(rec) = replay.recorded() {
            assert_eq!(
                traced.result.success, rec.success,
                "verdict differs from the recorded outcome"
            );
        }
        // Final account state: for every account the plain run reports as
        // changed, the last step that touched it in the trace must leave it
        // exactly where the plain run did.
        for pd in &plain.diffs {
            let last = traced
                .steps
                .iter()
                .flat_map(|st| st.diffs.iter())
                .rfind(|d| d.address == pd.address);
            if let Some(td) = last {
                assert_eq!(
                    td.lamports_after, pd.lamports_after,
                    "final lamports of {} differ under tracing",
                    pd.address
                );
            }
        }
    }

    #[test]
    fn traced_success_matches_plain_run() {
        check(include_str!("../tests/fixtures/counter_increment.json"));
    }

    #[test]
    fn traced_revert_matches_plain_run() {
        check(include_str!(
            "../tests/fixtures/vesting_precliff_revert.json"
        ));
    }
}
