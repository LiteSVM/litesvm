//! The instruction tree: top-level instructions from the message, CPIs from
//! the runtime's inner-instruction list, each decoded, then joined with the
//! log-derived CPI frames for outcomes, compute and events.

use {
    crate::{errors, idl, native, DecodedAccountRef, DecodedArg, DecodedInstruction, IdlRegistry},
    base64::Engine,
    litesvm::LiteSVM,
    litesvm_cpi_tree::{CpiFrame, CpiOutcome, FrameLog},
    solana_address::Address,
    solana_message::{
        compiled_instruction::CompiledInstruction, inner_instruction::InnerInstructionsList,
        VersionedMessage,
    },
    std::fmt::Write,
};

/// Header size of an address lookup table account, before its 32-byte entries.
const LOOKUP_TABLE_META_SIZE: usize = 56;

/// The message's full account key list: its static keys followed by the
/// addresses its lookup tables resolve to (all writable ones, then all
/// read-only ones — the runtime's ordering), read from the tables' accounts
/// in `svm`. A legacy message has only static keys.
pub fn resolve_account_keys(svm: &LiteSVM, message: &VersionedMessage) -> Vec<Address> {
    let mut keys: Vec<Address> = message.static_account_keys().to_vec();
    let Some(lookups) = message.address_table_lookups() else {
        return keys;
    };
    let (mut writable, mut readonly) = (Vec::new(), Vec::new());
    for l in lookups {
        let Some(table) = svm.get_account(&l.account_key) else {
            continue;
        };
        let read = |idx: u8| -> Option<Address> {
            let off = LOOKUP_TABLE_META_SIZE + idx as usize * 32;
            let bytes: [u8; 32] = table.data.get(off..off + 32)?.try_into().ok()?;
            Some(Address::from(bytes))
        };
        writable.extend(l.writable_indexes.iter().filter_map(|&i| read(i)));
        readonly.extend(l.readonly_indexes.iter().filter_map(|&i| read(i)));
    }
    keys.extend(writable);
    keys.extend(readonly);
    keys
}

/// How many of the resolved keys past the static ones are writable: the
/// lookup-table writable entries come first in the resolved list.
fn loaded_writable_count(message: &VersionedMessage) -> usize {
    message
        .address_table_lookups()
        .map(|ls| ls.iter().map(|l| l.writable_indexes.len()).sum())
        .unwrap_or(0)
}

/// Decode every instruction of `message` — top-level ones and, from `inner`,
/// the CPIs each made — against `keys` (see [`resolve_account_keys`]) and the
/// registered IDLs. Outcomes and compute are not known here; see
/// [`crate::ScopeExt`] for the metadata-driven version that attaches them.
pub fn decode_instructions(
    message: &VersionedMessage,
    keys: &[Address],
    inner: &InnerInstructionsList,
    registry: &IdlRegistry,
) -> Vec<DecodedInstruction> {
    let n_static = message.static_account_keys().len();
    let n_writable_loaded = loaded_writable_count(message);
    let privileges = |index: usize| -> (bool, bool) {
        if index < n_static {
            (
                message.is_signer(index),
                message.is_maybe_writable_with_reserved_addresses(
                    index,
                    None::<&std::collections::HashSet<Address>>,
                ),
            )
        } else {
            (false, index - n_static < n_writable_loaded)
        }
    };
    let decode_one = |ix: &CompiledInstruction, stack_height: u8| -> DecodedInstruction {
        let program = keys
            .get(ix.program_id_index as usize)
            .copied()
            .unwrap_or_default();
        let (name, idl_name, args, role_names) =
            decode_program_instruction(registry, &program, &ix.data);
        let named_len = role_names.len();
        let is_native = native::is_native(&program.to_string());
        let accounts = ix
            .accounts
            .iter()
            .enumerate()
            .map(|(i, &ki)| {
                let index = ki as usize;
                let (signer, writable) = privileges(index);
                let name = role_names.get(i).cloned().flatten().or_else(|| {
                    (!is_native && named_len > 0 && i >= named_len)
                        .then(|| format!("Remaining Account #{}", i - named_len + 1))
                });
                DecodedAccountRef {
                    name,
                    address: keys.get(index).copied().unwrap_or_default(),
                    signer,
                    writable,
                }
            })
            .collect();
        DecodedInstruction {
            program,
            name,
            idl_name,
            args,
            accounts,
            stack_height,
            compute_units: None,
            success: None,
            error: None,
            events: Vec::new(),
            children: Vec::new(),
        }
    };

    message
        .instructions()
        .iter()
        .enumerate()
        .map(|(i, ix)| {
            let mut top = decode_one(ix, 1);
            // Nest this instruction's CPIs by stack height: a CPI at height h
            // is a child of the most recent node at height h-1. `path` is the
            // chain of child indexes from `top` to the most recent node.
            if let Some(cpis) = inner.get(i) {
                let mut path: Vec<usize> = Vec::new();
                for cpi in cpis {
                    let h = cpi.stack_height.max(2) as usize;
                    path.truncate(h - 2);
                    let parent = node_at(&mut top, &path);
                    parent.children.push(decode_one(&cpi.instruction, h as u8));
                    path.push(parent.children.len() - 1);
                }
            }
            top
        })
        .collect()
}

/// The node reached from `root` by following `path` through `children`.
fn node_at<'a>(root: &'a mut DecodedInstruction, path: &[usize]) -> &'a mut DecodedInstruction {
    path.iter().fold(root, |node, &i| &mut node.children[i])
}

/// Name, IDL spelling, args and positional role names for one instruction's
/// program and data.
fn decode_program_instruction(
    registry: &IdlRegistry,
    program: &Address,
    data: &[u8],
) -> (
    Option<String>,
    Option<String>,
    Vec<DecodedArg>,
    Vec<Option<String>>,
) {
    if data.len() >= 8 && data[..8] == native::ANCHOR_EVENT_CPI {
        return (
            Some("Emit Event".into()),
            None,
            Vec::new(),
            vec![Some("Event Authority".into())],
        );
    }
    let program_str = program.to_string();
    if let Some((name, args, roles)) = native::decode(&program_str, data) {
        return (
            name,
            None,
            args,
            roles.into_iter().map(|r| Some(r.to_string())).collect(),
        );
    }
    let Some(model) = registry.get(program) else {
        return (None, None, Vec::new(), Vec::new());
    };
    let Some(ix) = idl::find_ix(model, data) else {
        return (None, None, Vec::new(), Vec::new());
    };
    let roles = ix
        .accounts
        .iter()
        .map(|acc| acc.name.as_deref().map(native::titleize))
        .collect();
    (
        ix.name.as_deref().map(native::titleize),
        ix.name.clone(),
        idl::decode_ix_args(ix, data),
        roles,
    )
}

/// Join the decoded tree with the log-derived frames, both in pre-order:
/// outcome, compute units, the named error of a failed frame, and any
/// `Program data:` payloads decoded as events. Stops silently at the first
/// program-id mismatch rather than attach anything to the wrong node.
pub(crate) fn attach_frames(
    instructions: &mut [DecodedInstruction],
    frames: &[CpiFrame],
    registry: &IdlRegistry,
) {
    fn go(ix: &mut DecodedInstruction, frame: &CpiFrame, registry: &IdlRegistry) -> bool {
        if ix.program != frame.program_id {
            return false;
        }
        ix.compute_units = frame.compute_units.map(|c| c.consumed);
        match &frame.outcome {
            CpiOutcome::Success => ix.success = Some(true),
            CpiOutcome::Failed { message } => {
                ix.success = Some(false);
                ix.error = Some(errors::from_failure(
                    registry,
                    &ix.program,
                    message.as_deref().unwrap_or("failed"),
                    &frame.logs,
                ));
            }
            CpiOutcome::Truncated => {}
        }
        for log in &frame.logs {
            if let FrameLog::Data(payload) = log {
                // `sol_log_data` may join several base64 fields with spaces;
                // Anchor's `emit!` writes exactly one.
                for part in payload.split_whitespace() {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(part) {
                        if let Some(ev) = crate::decode_event(registry, Some(&ix.program), &bytes) {
                            ix.events.push(ev);
                        }
                    }
                }
            }
        }
        // Anchor's `emit_cpi!` carries the event in the CPI's own data.
        for (child, child_frame) in ix.children.iter_mut().zip(&frame.children) {
            if !go(child, child_frame, registry) {
                return false;
            }
        }
        true
    }
    for (ix, frame) in instructions.iter_mut().zip(frames) {
        if !go(ix, frame, registry) {
            break;
        }
    }
}

/// Render decoded instructions as box art, one line per instruction:
/// `name (program) args … ✓/✗ N CU`, with accounts indented beneath.
pub fn format_instructions(instructions: &[DecodedInstruction]) -> String {
    fn short(a: &Address) -> String {
        let s = a.to_string();
        format!("{}…{}", &s[..4], &s[s.len() - 4..])
    }
    fn go(out: &mut String, ix: &DecodedInstruction, prefix: &str, last: bool, root: bool) {
        let branch = if root {
            ""
        } else if last {
            "└─ "
        } else {
            "├─ "
        };
        let mark = match ix.success {
            Some(true) => " ✓",
            Some(false) => " ✗",
            None => "",
        };
        let cu = ix
            .compute_units
            .map(|c| format!(" {c} CU"))
            .unwrap_or_default();
        let args: Vec<String> = ix
            .args
            .iter()
            .map(|a| format!("{}={}", a.name, a.value))
            .collect();
        let _ = writeln!(
            out,
            "{prefix}{branch}{} ({}){}{}{}",
            ix.name.as_deref().unwrap_or("?"),
            short(&ix.program),
            if args.is_empty() {
                String::new()
            } else {
                format!(" {}", args.join(" "))
            },
            mark,
            cu
        );
        let child_prefix = if root {
            String::new()
        } else {
            format!("{prefix}{}", if last { "   " } else { "│  " })
        };
        if let Some(e) = &ix.error {
            let _ = writeln!(
                out,
                "{child_prefix}   ! {}{}",
                e.name,
                e.message
                    .as_deref()
                    .map(|m| format!(": {m}"))
                    .unwrap_or_default()
            );
        }
        for ev in &ix.events {
            let _ = writeln!(out, "{child_prefix}   ⚡ {}", ev.name);
        }
        let n = ix.children.len();
        for (i, c) in ix.children.iter().enumerate() {
            go(out, c, &child_prefix, i + 1 == n, false);
        }
    }
    let mut out = String::new();
    for (i, ix) in instructions.iter().enumerate() {
        let _ = write!(out, "#{i} ");
        go(&mut out, ix, "", true, true);
    }
    out
}
