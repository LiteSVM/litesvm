#!/usr/bin/env python3
"""Regenerate MAINNET_ACTIVE_FEATURES in crates/litesvm/src/features.rs.

Queries mainnet RPC directly for each feature ID known to the pinned
agave-feature-set crate and emits the active ones in upstream source order.
"""
import base64
import json
import re
import subprocess
import sys
import urllib.request
from datetime import date
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "crates" / "litesvm" / "src" / "features.rs"
RPC = "https://api.mainnet-beta.solana.com"
FEATURE_PROGRAM = "Feature111111111111111111111111111111111111"


def run(*cmd):
    return subprocess.check_output(cmd, cwd=REPO)


def agave_src():
    meta = json.loads(run("cargo", "metadata", "--format-version", "1"))
    for pkg in meta["packages"]:
        if pkg["name"] == "agave-feature-set":
            return Path(pkg["manifest_path"]).parent / "src" / "lib.rs"
    sys.exit("agave-feature-set not found in cargo metadata")


def parse_modules(src):
    head = src.split("pub static FEATURE_NAMES", 1)[0]
    result = {}
    stack = []
    mod_re = re.compile(r"pub mod ([A-Za-z_][A-Za-z0-9_]*)\s*\{")
    id_re = re.compile(r'declare_id!\("([1-9A-HJ-NP-Za-km-z]+)"\)')
    i = 0
    while i < len(head):
        m = mod_re.match(head, i)
        if m:
            stack.append(m.group(1))
            i = m.end()
            continue
        m = id_re.match(head, i)
        if m:
            named = [n for n in stack if n is not None]
            if named:
                result[m.group(1)] = "::".join(named)
            i = m.end()
            continue
        c = head[i]
        if c == "{":
            stack.append(None)
        elif c == "}" and stack:
            stack.pop()
        i += 1
    return result


def rpc(method, params):
    req = urllib.request.Request(
        RPC,
        data=json.dumps({"jsonrpc": "2.0", "id": 1,
                         "method": method, "params": params}).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        body = json.loads(r.read())
    if "error" in body:
        sys.exit(f"RPC error: {body['error']}")
    return body["result"]


def active_mainnet_features(feature_ids):
    """Return {feature_id: activation_slot} for features active on mainnet.

    The `Feature` account is bincode `Option<u64>`: a leading tag byte (1 for
    `Some`) followed by the activation slot as a little-endian u64.
    """
    activation_slots = {}
    for i in range(0, len(feature_ids), 100):  # RPC caps at 100 per call
        chunk = feature_ids[i:i + 100]
        result = rpc("getMultipleAccounts",
                     [chunk, {"encoding": "base64"}])
        for fid, acc in zip(chunk, result["value"]):
            if not acc or acc["owner"] != FEATURE_PROGRAM:
                continue
            data = base64.b64decode(acc["data"][0])
            if len(data) >= 9 and data[0] == 1:  # Some(activation_slot)
                activation_slots[fid] = int.from_bytes(data[1:9], "little")
    return activation_slots


def main():
    pubkey_to_path = parse_modules(agave_src().read_text())
    active = active_mainnet_features(list(pubkey_to_path))
    in_source_order = [p for p in pubkey_to_path if p in active]

    lines = [
        "use solana_address::Address;",
        "",
        "/// Feature gates currently activated on Solana mainnet-beta, paired with their",
        "/// activation slot, sourced from the cluster on "
        f"{date.today().isoformat()}.",
        "pub const MAINNET_ACTIVE_FEATURES: &[(Address, u64)] = &[",
    ]
    for pk in in_source_order:
        lines.append(
            f"    (agave_feature_set::{pubkey_to_path[pk]}::ID, {active[pk]}),")
    lines += ["];", ""]
    OUT.write_text("\n".join(lines))
    print(f"wrote {len(in_source_order)} features to "
          f"{OUT.relative_to(REPO)}")


if __name__ == "__main__":
    main()
