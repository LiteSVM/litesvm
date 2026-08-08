#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 3 ]]; then
  echo "usage: $0 <signature> [rpc_url] [snapshot_dir]"
  echo "example: $0 k13QNvPB5WAZQDKmcMzPdiCv1kmP9Eb3RfNBuZfXLK5N2Qi2qXiSdfFrTngjWp54p72NrTuXAzMLxW6RQbodnrS"
  exit 1
fi

SIG="$1"
RPC_URL="${2:-https://api.mainnet-beta.solana.com}"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SNAPSHOT_DIR="${3:-$SCRIPT_DIR/snapshots/$SIG}"
PROGRAMS_DIR="$SNAPSHOT_DIR/programs"
CURL_TIMEOUT="${CURL_TIMEOUT:-90}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1"
    exit 1
  }
}

need_cmd curl
need_cmd jq
need_cmd solana

mkdir -p "$PROGRAMS_DIR"
cd "$SNAPSHOT_DIR"

# 1) Fetch transaction in base64 + json encodings.
curl -sS -m "$CURL_TIMEOUT" "$RPC_URL" \
  -H "content-type: application/json" \
  --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTransaction\",\"params\":[\"$SIG\",{\"encoding\":\"base64\",\"maxSupportedTransactionVersion\":0,\"commitment\":\"confirmed\"}]}" \
  > tx.json

curl -sS -m "$CURL_TIMEOUT" "$RPC_URL" \
  -H "content-type: application/json" \
  --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTransaction\",\"params\":[\"$SIG\",{\"encoding\":\"json\",\"maxSupportedTransactionVersion\":0,\"commitment\":\"confirmed\"}]}" \
  > tx_json.json

jq -e '.result != null and .result.meta.err == null' tx_json.json >/dev/null || {
  echo "transaction missing or failed onchain for signature: $SIG"
  jq -c '.result.meta.err // .error // "unknown_error"' tx_json.json
  exit 1
}

# 2) Snapshot all required accounts, including lookup table accounts.
jq -r \
  '.result.transaction.message.accountKeys[],
   .result.meta.loadedAddresses.readonly[]?,
   .result.meta.loadedAddresses.writable[]?,
   .result.transaction.message.addressTableLookups[]?.accountKey' \
  tx_json.json | sort -u > account_keys.txt

keys_json="$(jq -Rsc 'split("\n") | map(select(length > 0))' account_keys.txt)"
jq -n \
  --argjson keys "$keys_json" \
  '{"jsonrpc":"2.0","id":1,"method":"getMultipleAccounts","params":[ $keys, {"encoding":"base64","commitment":"confirmed"}]}' \
  > get_multiple_accounts_req.json

curl -sS -m "$CURL_TIMEOUT" "$RPC_URL" \
  -H "content-type: application/json" \
  --data @get_multiple_accounts_req.json \
  > accounts.json

# 3) Dump executable programs referenced by the snapshot (except common builtins/defaults).
mapfile -t KEYS < account_keys.txt
mapfile -t EXEC_IDXS < <(
  jq -r '.result.value | to_entries[] | select(.value != null and .value.executable == true) | .key' accounts.json
)

for idx in "${EXEC_IDXS[@]}"; do
  key="${KEYS[$idx]}"
  [[ -f "$PROGRAMS_DIR/$key.so" ]] && continue
  case "$key" in
    11111111111111111111111111111111|\
    ComputeBudget111111111111111111111111111111|\
    TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA|\
    TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb|\
    MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr|\
    Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo|\
    ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL|\
    AddressLookupTab1e1111111111111111111111111|\
    Config1111111111111111111111111111111111111|\
    Stake11111111111111111111111111111111111111)
      continue
      ;;
  esac
  solana program dump "$key" "$PROGRAMS_DIR/$key.so" >/dev/null 2>&1 || true
done

# 4) Save RPC simulation result for the exact captured transaction.
tx_b64="$(jq -r '.result.transaction[0]' tx.json)"
jq -n \
  --arg tx "$tx_b64" \
  '{"jsonrpc":"2.0","id":1,"method":"simulateTransaction","params":[ $tx, {"encoding":"base64","replaceRecentBlockhash":true,"sigVerify":false,"commitment":"confirmed"}]}' \
  > simulate_req.json

curl -sS -m "$CURL_TIMEOUT" "$RPC_URL" \
  -H "content-type: application/json" \
  --data @simulate_req.json \
  > simulate.json

jq -n \
  --arg signature "$SIG" \
  --arg rpc_url "$RPC_URL" \
  --arg captured_at_utc "$(date -u +%FT%TZ)" \
  '{"signature":$signature,"rpc_url":$rpc_url,"captured_at_utc":$captured_at_utc}' \
  > snapshot_meta.json

echo "signature=$SIG"
echo "snapshot_dir=$SNAPSHOT_DIR"
echo "onchain_cu=$(jq -r '.result.meta.computeUnitsConsumed // "null"' tx_json.json)"
echo "rpc_sim_units=$(jq -r '.result.value.unitsConsumed // "null"' simulate.json)"
echo "rpc_sim_err=$(jq -c '.result.value.err // .error // null' simulate.json)"
