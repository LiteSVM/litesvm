// Package litesvm is a thin Go wrapper around the LiteSVM Rust library,
// accessed through a C ABI and cgo.
//
// It uses github.com/gagliardetto/solana-go for the core Solana types
// (PublicKey, Hash, Signature) so that values returned from this package
// can be passed directly into solana-go helpers (transaction construction,
// signing, base58 encoding) and vice-versa.
//
// A LiteSVM handle is not safe for concurrent use from multiple goroutines.
// Either confine it to a single goroutine, or serialize access with your
// own sync.Mutex.
//
// # Thread affinity
//
// Error messages are delivered via a thread-local slot on the Rust side.
// Go's scheduler can migrate a goroutine between OS threads between any two
// cgo calls, which would cause the error message read by one call to land
// on a different thread than the one written by the preceding call. To
// avoid this, every method that performs paired cgo calls (operation +
// error readback, or probe + copy) pins the goroutine to its current OS
// thread for the duration of the method via runtime.LockOSThread. Users
// do not need to do anything special.
package litesvm

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo darwin  LDFLAGS: -L${SRCDIR}/../../target/debug -llitesvm_go -framework Security -framework CoreFoundation
#cgo linux   LDFLAGS: -L${SRCDIR}/../../target/debug -llitesvm_go -lm -ldl -lpthread

#include <stdint.h>
#include <stdlib.h>
#include "litesvm.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime"
	"unsafe"

	"github.com/gagliardetto/solana-go"
)

// LiteSVM is an in-process Solana VM.
type LiteSVM struct {
	h *C.LiteSvmHandle
}

// New creates a fresh LiteSVM with default settings. Call Close when done
// (or let the finalizer do it, though explicit Close is preferred for
// predictable cleanup).
func New() (*LiteSVM, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	h := C.litesvm_new()
	if h == nil {
		return nil, lastError("litesvm_new returned null")
	}
	svm := &LiteSVM{h: h}
	runtime.SetFinalizer(svm, (*LiteSVM).Close)
	return svm, nil
}

// Close releases the underlying Rust handle. Safe to call more than once.
// Not safe to call concurrently with other methods on the same handle.
func (s *LiteSVM) Close() {
	if s == nil || s.h == nil {
		return
	}
	C.litesvm_free(s.h)
	s.h = nil
	runtime.SetFinalizer(s, nil)
}

// Airdrop credits lamports to pubkey.
func (s *LiteSVM) Airdrop(pubkey solana.PublicKey, lamports uint64) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_airdrop(
		s.h,
		(*C.uint8_t)(unsafe.Pointer(&pubkey[0])),
		C.uint64_t(lamports),
	)
	if rc != 0 {
		return lastError(fmt.Sprintf("airdrop rc=%d", rc))
	}
	return nil
}

// Balance returns the balance of pubkey. The bool is false if the account
// does not exist.
func (s *LiteSVM) Balance(pubkey solana.PublicKey) (uint64, bool, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var out C.uint64_t
	rc := C.litesvm_get_balance(
		s.h,
		(*C.uint8_t)(unsafe.Pointer(&pubkey[0])),
		&out,
	)
	switch rc {
	case 0:
		return uint64(out), true, nil
	case 1:
		return 0, false, nil
	default:
		return 0, false, lastError(fmt.Sprintf("get_balance rc=%d", rc))
	}
}

// LatestBlockhash returns the latest blockhash.
func (s *LiteSVM) LatestBlockhash() (solana.Hash, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var out solana.Hash
	rc := C.litesvm_latest_blockhash(
		s.h,
		(*C.uint8_t)(unsafe.Pointer(&out[0])),
	)
	if rc != 0 {
		return solana.Hash{}, lastError(fmt.Sprintf("latest_blockhash rc=%d", rc))
	}
	return out, nil
}

// ExpireBlockhash advances past the current blockhash so the next
// LatestBlockhash returns a new value.
func (s *LiteSVM) ExpireBlockhash() error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_expire_blockhash(s.h)
	if rc != 0 {
		return lastError(fmt.Sprintf("expire_blockhash rc=%d", rc))
	}
	return nil
}

// MinimumBalanceForRentExemption returns the lamports required for an
// account of the given data length to be rent-exempt.
func (s *LiteSVM) MinimumBalanceForRentExemption(dataLen int) (uint64, error) {
	if dataLen < 0 {
		return 0, errors.New("dataLen must be >= 0")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	v := C.litesvm_minimum_balance_for_rent_exemption(s.h, C.size_t(dataLen))
	// Rust side returns u64::MAX on error.
	if v == ^C.uint64_t(0) {
		return 0, lastError("minimum_balance_for_rent_exemption")
	}
	return uint64(v), nil
}

// Version returns the version string reported by the Rust crate.
func Version() string {
	// Single cgo call returning a pointer into static Rust memory; no error
	// state is consulted, so no thread pinning is required.
	p := C.litesvm_version()
	return C.GoString((*C.char)(unsafe.Pointer(p)))
}

// TxOutcome holds the result of a transaction submission. It always carries
// metadata (signature, logs, compute units, fee) regardless of whether the
// transaction succeeded; if it failed, Error returns a non-empty string.
type TxOutcome struct {
	h *C.LiteSvmTxOutcome
}

// SendLegacyTransaction submits a bincode-encoded legacy Transaction.
// solana-go's (*Transaction).MarshalBinary produces bytes in exactly this
// format, so callers can build transactions with solana-go and pass the
// result straight in.
//
// Returns a TxOutcome on both success and failure; only returns a non-nil
// error when the bytes could not be decoded or an internal error occurred.
func (s *LiteSVM) SendLegacyTransaction(txBytes []byte) (*TxOutcome, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var ptr *C.uint8_t
	if len(txBytes) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&txBytes[0]))
	}
	h := C.litesvm_send_legacy_transaction(s.h, ptr, C.size_t(len(txBytes)))
	if h == nil {
		return nil, lastError("send_legacy_transaction")
	}
	out := &TxOutcome{h: h}
	runtime.SetFinalizer(out, (*TxOutcome).Close)
	return out, nil
}

// Close releases the underlying Rust handle. Safe to call more than once.
// Not safe to call concurrently with other methods on the same handle.
func (o *TxOutcome) Close() {
	if o == nil || o.h == nil {
		return
	}
	C.litesvm_tx_outcome_free(o.h)
	o.h = nil
	runtime.SetFinalizer(o, nil)
}

// IsOk reports whether the transaction succeeded. Returns false if the
// receiver is nil or has been closed (treating an unusable outcome as "not ok").
func (o *TxOutcome) IsOk() bool {
	if o == nil || o.h == nil {
		return false
	}
	return C.litesvm_tx_outcome_is_ok(o.h) == 1
}

// Signature returns the signature of the submitted transaction. Returns the
// zero signature if the receiver is nil or has been closed.
func (o *TxOutcome) Signature() solana.Signature {
	var out solana.Signature
	if o == nil || o.h == nil {
		return out
	}
	C.litesvm_tx_outcome_signature(o.h, (*C.uint8_t)(unsafe.Pointer(&out[0])))
	return out
}

// ComputeUnits returns the number of compute units consumed.
func (o *TxOutcome) ComputeUnits() uint64 {
	if o == nil || o.h == nil {
		return 0
	}
	return uint64(C.litesvm_tx_outcome_compute_units(o.h))
}

// Fee returns the fee charged for the transaction.
func (o *TxOutcome) Fee() uint64 {
	if o == nil || o.h == nil {
		return 0
	}
	return uint64(C.litesvm_tx_outcome_fee(o.h))
}

// Logs returns the program log lines produced by the transaction.
func (o *TxOutcome) Logs() []string {
	if o == nil || o.h == nil {
		return nil
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	n := int(C.litesvm_tx_outcome_logs_count(o.h))
	if n == 0 {
		return nil
	}
	logs := make([]string, n)
	for i := range n {
		logs[i] = copyVarBuf(func(buf *C.uint8_t, bufLen C.size_t) C.size_t {
			return C.litesvm_tx_outcome_log_copy(o.h, C.size_t(i), buf, bufLen)
		})
	}
	return logs
}

// Error returns the debug-rendered transaction error, or "" if the
// transaction succeeded.
func (o *TxOutcome) Error() string {
	if o == nil || o.h == nil {
		return ""
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	return copyVarBuf(func(buf *C.uint8_t, bufLen C.size_t) C.size_t {
		return C.litesvm_tx_outcome_error_copy(o.h, buf, bufLen)
	})
}

// ReturnData returns the bytes returned by the program via set_return_data,
// along with the program that produced them. The bool is false if no
// return data was set.
func (o *TxOutcome) ReturnData() (solana.PublicKey, []byte, bool) {
	if o == nil || o.h == nil {
		return solana.PublicKey{}, nil, false
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var pid solana.PublicKey
	rc := C.litesvm_tx_outcome_return_data_program_id(o.h, (*C.uint8_t)(unsafe.Pointer(&pid[0])))
	if rc != 1 {
		return solana.PublicKey{}, nil, false
	}
	n := int(C.litesvm_tx_outcome_return_data_len(o.h))
	if n == 0 {
		return pid, nil, true
	}
	buf := make([]byte, n)
	got := int(C.litesvm_tx_outcome_return_data_copy(
		o.h,
		(*C.uint8_t)(unsafe.Pointer(&buf[0])),
		C.size_t(len(buf)),
	))
	if got > len(buf) {
		got = len(buf)
	}
	return pid, buf[:got], true
}

// PostAccount is a (address, account) pair returned by a successful
// simulation. Close the Account when done.
type PostAccount struct {
	Address solana.PublicKey
	Account *Account
}

// CompiledInstruction mirrors solana_message::compiled_instruction::CompiledInstruction.
// Accounts are account-table indices (not pubkeys); resolve via the
// transaction's account_keys.
type CompiledInstruction struct {
	ProgramIDIndex uint8
	Accounts       []byte
	Data           []byte
}

// InnerInstruction is one CPI recorded during execution.
type InnerInstruction struct {
	Instruction CompiledInstruction
	// StackHeight is the invocation stack height; 1 for a top-level ix,
	// higher for deeper CPIs.
	StackHeight uint8
}

// InnerInstructions returns the CPIs observed during execution, grouped by
// the top-level instruction that triggered them. The outer slice is indexed
// by top-level instruction index; each inner slice is the CPIs from that
// invocation. Returns nil if no inner-instruction data was recorded.
func (o *TxOutcome) InnerInstructions() [][]InnerInstruction {
	if o == nil || o.h == nil {
		return nil
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	outerN := int(C.litesvm_tx_outcome_inner_outer_count(o.h))
	if outerN == 0 {
		return nil
	}
	out := make([][]InnerInstruction, outerN)
	for outer := range outerN {
		innerN := int(C.litesvm_tx_outcome_inner_inner_count(o.h, C.size_t(outer)))
		if innerN == 0 {
			out[outer] = nil
			continue
		}
		list := make([]InnerInstruction, innerN)
		for inner := range innerN {
			pidIdx := C.litesvm_tx_outcome_inner_program_id_index(
				o.h, C.size_t(outer), C.size_t(inner))
			stack := C.litesvm_tx_outcome_inner_stack_height(
				o.h, C.size_t(outer), C.size_t(inner))
			accN := int(C.litesvm_tx_outcome_inner_accounts_len(
				o.h, C.size_t(outer), C.size_t(inner)))
			var accounts []byte
			if accN > 0 {
				accounts = make([]byte, accN)
				got := int(C.litesvm_tx_outcome_inner_accounts_copy(
					o.h, C.size_t(outer), C.size_t(inner),
					(*C.uint8_t)(unsafe.Pointer(&accounts[0])),
					C.size_t(len(accounts)),
				))
				// Pre-probed length can never exceed our buffer; guard
				// defensively and truncate if the Rust side reports more.
				if got < len(accounts) {
					accounts = accounts[:got]
				}
			}
			dataN := int(C.litesvm_tx_outcome_inner_data_len(
				o.h, C.size_t(outer), C.size_t(inner)))
			var data []byte
			if dataN > 0 {
				data = make([]byte, dataN)
				got := int(C.litesvm_tx_outcome_inner_data_copy(
					o.h, C.size_t(outer), C.size_t(inner),
					(*C.uint8_t)(unsafe.Pointer(&data[0])),
					C.size_t(len(data)),
				))
				if got < len(data) {
					data = data[:got]
				}
			}
			list[inner] = InnerInstruction{
				Instruction: CompiledInstruction{
					ProgramIDIndex: uint8(pidIdx),
					Accounts:       accounts,
					Data:           data,
				},
				StackHeight: uint8(stack),
			}
		}
		out[outer] = list
	}
	return out
}

// PostAccounts returns the accounts as they would be after executing the
// transaction. Populated only for successful simulations; returns nil for
// send_transaction outcomes and failed simulations. Returns an error only
// if the Rust side signals an internal failure while enumerating.
func (o *TxOutcome) PostAccounts() ([]PostAccount, error) {
	if o == nil || o.h == nil {
		return nil, nil
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	n := int(C.litesvm_tx_outcome_post_accounts_count(o.h))
	if n == 0 {
		return nil, nil
	}
	out := make([]PostAccount, 0, n)
	for i := range n {
		var addr solana.PublicKey
		h := C.litesvm_tx_outcome_post_account_at(
			o.h, C.size_t(i), (*C.uint8_t)(unsafe.Pointer(&addr[0])),
		)
		if h == nil {
			return nil, lastError(fmt.Sprintf("post_account_at(%d) returned null", i))
		}
		a := &Account{h: h}
		runtime.SetFinalizer(a, (*Account).Close)
		out = append(out, PostAccount{Address: addr, Account: a})
	}
	return out, nil
}

// SendVersionedTransaction submits a bincode-encoded v0+ Transaction.
// solana-go's (*Transaction).MarshalBinary produces bytes in this format
// regardless of whether the transaction is legacy or versioned.
func (s *LiteSVM) SendVersionedTransaction(txBytes []byte) (*TxOutcome, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var ptr *C.uint8_t
	if len(txBytes) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&txBytes[0]))
	}
	h := C.litesvm_send_versioned_transaction(s.h, ptr, C.size_t(len(txBytes)))
	if h == nil {
		return nil, lastError("send_versioned_transaction")
	}
	out := &TxOutcome{h: h}
	runtime.SetFinalizer(out, (*TxOutcome).Close)
	return out, nil
}

// SimulateLegacyTransaction executes the transaction without committing state.
// The returned TxOutcome has PostAccounts populated on success.
func (s *LiteSVM) SimulateLegacyTransaction(txBytes []byte) (*TxOutcome, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var ptr *C.uint8_t
	if len(txBytes) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&txBytes[0]))
	}
	h := C.litesvm_simulate_legacy_transaction(s.h, ptr, C.size_t(len(txBytes)))
	if h == nil {
		return nil, lastError("simulate_legacy_transaction")
	}
	out := &TxOutcome{h: h}
	runtime.SetFinalizer(out, (*TxOutcome).Close)
	return out, nil
}

// SimulateVersionedTransaction is the v0+ counterpart of
// SimulateLegacyTransaction.
func (s *LiteSVM) SimulateVersionedTransaction(txBytes []byte) (*TxOutcome, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var ptr *C.uint8_t
	if len(txBytes) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&txBytes[0]))
	}
	h := C.litesvm_simulate_versioned_transaction(s.h, ptr, C.size_t(len(txBytes)))
	if h == nil {
		return nil, lastError("simulate_versioned_transaction")
	}
	out := &TxOutcome{h: h}
	runtime.SetFinalizer(out, (*TxOutcome).Close)
	return out, nil
}

// WarpToSlot advances the internal clock to `slot`.
func (s *LiteSVM) WarpToSlot(slot uint64) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_warp_to_slot(s.h, C.uint64_t(slot))
	if rc != 0 {
		return lastError(fmt.Sprintf("warp_to_slot rc=%d", rc))
	}
	return nil
}

// GetTransaction looks up a transaction by signature in the transaction
// history. Returns nil if the signature is not present (either never
// submitted, or history capacity exceeded).
func (s *LiteSVM) GetTransaction(signature solana.Signature) *TxOutcome {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	h := C.litesvm_get_transaction(s.h, (*C.uint8_t)(unsafe.Pointer(&signature[0])))
	if h == nil {
		return nil
	}
	out := &TxOutcome{h: h}
	runtime.SetFinalizer(out, (*TxOutcome).Close)
	return out
}

// ---------------------------------------------------------------------------
// Configuration setters
// ---------------------------------------------------------------------------

// SetSigverify toggles transaction signature verification.
func (s *LiteSVM) SetSigverify(enabled bool) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_sigverify(s.h, C.bool(enabled))
	if rc != 0 {
		return lastError(fmt.Sprintf("set_sigverify rc=%d", rc))
	}
	return nil
}

// Sigverify reports whether transaction signature verification is enabled.
func (s *LiteSVM) Sigverify() (bool, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_get_sigverify(s.h)
	switch rc {
	case 1:
		return true, nil
	case 0:
		return false, nil
	default:
		return false, lastError(fmt.Sprintf("get_sigverify rc=%d", rc))
	}
}

// SetBlockhashCheck toggles the recent-blockhash check on submitted txs.
func (s *LiteSVM) SetBlockhashCheck(enabled bool) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_blockhash_check(s.h, C.bool(enabled))
	if rc != 0 {
		return lastError(fmt.Sprintf("set_blockhash_check rc=%d", rc))
	}
	return nil
}

// SetTransactionHistory sets the capacity of the internal transaction
// history. Pass 0 to disable dedup and allow replaying identical
// transactions.
func (s *LiteSVM) SetTransactionHistory(capacity int) error {
	if capacity < 0 {
		return errors.New("capacity must be >= 0")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_transaction_history(s.h, C.size_t(capacity))
	if rc != 0 {
		return lastError(fmt.Sprintf("set_transaction_history rc=%d", rc))
	}
	return nil
}

// SetLogBytesLimit bounds the total log bytes captured per transaction.
// Pass a negative value to remove the limit.
func (s *LiteSVM) SetLogBytesLimit(limit int) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var (
		hasLimit C.bool
		l        C.size_t
	)
	if limit >= 0 {
		hasLimit = true
		l = C.size_t(limit)
	}
	rc := C.litesvm_set_log_bytes_limit(s.h, hasLimit, l)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_log_bytes_limit rc=%d", rc))
	}
	return nil
}

// SetLamports sets the balance of the internal airdrop-pool account.
func (s *LiteSVM) SetLamports(lamports uint64) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_lamports(s.h, C.uint64_t(lamports))
	if rc != 0 {
		return lastError(fmt.Sprintf("set_lamports rc=%d", rc))
	}
	return nil
}

// SetSysvars re-initializes the built-in sysvars to their defaults.
func (s *LiteSVM) SetSysvars() error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_sysvars(s.h)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_sysvars rc=%d", rc))
	}
	return nil
}

// SetBuiltins loads the default set of built-in programs.
func (s *LiteSVM) SetBuiltins() error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_builtins(s.h)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_builtins rc=%d", rc))
	}
	return nil
}

// SetDefaultPrograms loads the standard SPL programs into the environment.
func (s *LiteSVM) SetDefaultPrograms() error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_default_programs(s.h)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_default_programs rc=%d", rc))
	}
	return nil
}

// SetPrecompiles loads the standard precompiled programs.
func (s *LiteSVM) SetPrecompiles() error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_precompiles(s.h)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_precompiles rc=%d", rc))
	}
	return nil
}

// WithNativeMints seeds the SPL Token / Token-2022 native-mint accounts
// (wrapped SOL). No-op if the matching program is not loaded; call
// SetDefaultPrograms first if you want both.
func (s *LiteSVM) WithNativeMints() error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_with_native_mints(s.h)
	if rc != 0 {
		return lastError(fmt.Sprintf("with_native_mints rc=%d", rc))
	}
	return nil
}

// AddProgramWithLoader loads an SBF program under a specific loader.
func (s *LiteSVM) AddProgramWithLoader(programID solana.PublicKey, bytes []byte, loaderID solana.PublicKey) error {
	if len(bytes) == 0 {
		return errors.New("AddProgramWithLoader: empty bytes")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_add_program_with_loader(
		s.h,
		(*C.uint8_t)(unsafe.Pointer(&programID[0])),
		(*C.uint8_t)(unsafe.Pointer(&bytes[0])),
		C.size_t(len(bytes)),
		(*C.uint8_t)(unsafe.Pointer(&loaderID[0])),
	)
	if rc != 0 {
		return lastError(fmt.Sprintf("add_program_with_loader rc=%d", rc))
	}
	return nil
}

// Account is an opaque handle around a Solana account (lamports, data,
// owner, executable, rent_epoch). Close when done.
type Account struct {
	h *C.LiteSvmAccount
}

// NewAccount constructs an Account handle.
func NewAccount(lamports uint64, data []byte, owner solana.PublicKey, executable bool, rentEpoch uint64) (*Account, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var dataPtr *C.uint8_t
	if len(data) > 0 {
		dataPtr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	h := C.litesvm_account_new(
		C.uint64_t(lamports),
		dataPtr, C.size_t(len(data)),
		(*C.uint8_t)(unsafe.Pointer(&owner[0])),
		C.bool(executable),
		C.uint64_t(rentEpoch),
	)
	if h == nil {
		return nil, lastError("account_new")
	}
	a := &Account{h: h}
	runtime.SetFinalizer(a, (*Account).Close)
	return a, nil
}

// Close releases the handle. Safe to call more than once.
// Not safe to call concurrently with other methods on the same handle.
func (a *Account) Close() {
	if a == nil || a.h == nil {
		return
	}
	C.litesvm_account_free(a.h)
	a.h = nil
	runtime.SetFinalizer(a, nil)
}

// Lamports returns the lamports of the account.
func (a *Account) Lamports() uint64 {
	if a == nil || a.h == nil {
		return 0
	}
	return uint64(C.litesvm_account_lamports(a.h))
}

// RentEpoch returns the rent epoch of the account.
func (a *Account) RentEpoch() uint64 {
	if a == nil || a.h == nil {
		return 0
	}
	return uint64(C.litesvm_account_rent_epoch(a.h))
}

// Executable reports whether the account is executable.
func (a *Account) Executable() bool {
	if a == nil || a.h == nil {
		return false
	}
	return C.litesvm_account_executable(a.h) == 1
}

// Owner returns the account owner program address.
func (a *Account) Owner() solana.PublicKey {
	var out solana.PublicKey
	if a == nil || a.h == nil {
		return out
	}
	C.litesvm_account_owner(a.h, (*C.uint8_t)(unsafe.Pointer(&out[0])))
	return out
}

// Data returns a copy of the account data.
func (a *Account) Data() []byte {
	if a == nil || a.h == nil {
		return nil
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	n := int(C.litesvm_account_data_len(a.h))
	if n == 0 {
		return nil
	}
	buf := make([]byte, n)
	got := int(C.litesvm_account_data_copy(
		a.h,
		(*C.uint8_t)(unsafe.Pointer(&buf[0])),
		C.size_t(len(buf)),
	))
	if got > len(buf) {
		got = len(buf)
	}
	return buf[:got]
}

// GetAccount returns the account at `pubkey`, or nil if it does not exist.
func (s *LiteSVM) GetAccount(pubkey solana.PublicKey) *Account {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	h := C.litesvm_get_account(s.h, (*C.uint8_t)(unsafe.Pointer(&pubkey[0])))
	if h == nil {
		return nil
	}
	a := &Account{h: h}
	runtime.SetFinalizer(a, (*Account).Close)
	return a
}

// SetAccount stores a copy of `acct` at `pubkey`. Caller retains ownership
// of `acct` and must still Close it when done.
func (s *LiteSVM) SetAccount(pubkey solana.PublicKey, acct *Account) error {
	if acct == nil {
		return errors.New("SetAccount: nil account")
	}
	if acct.h == nil {
		return errors.New("SetAccount: account is closed")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_account(
		s.h,
		(*C.uint8_t)(unsafe.Pointer(&pubkey[0])),
		acct.h,
	)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_account rc=%d", rc))
	}
	return nil
}

// AddProgram loads an SBF program from an in-memory byte buffer.
func (s *LiteSVM) AddProgram(programID solana.PublicKey, bytes []byte) error {
	if len(bytes) == 0 {
		return errors.New("AddProgram: empty bytes")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_add_program(
		s.h,
		(*C.uint8_t)(unsafe.Pointer(&programID[0])),
		(*C.uint8_t)(unsafe.Pointer(&bytes[0])),
		C.size_t(len(bytes)),
	)
	if rc != 0 {
		return lastError(fmt.Sprintf("add_program rc=%d", rc))
	}
	return nil
}

// AddProgramFromFile loads an SBF program from the file at `path`.
func (s *LiteSVM) AddProgramFromFile(programID solana.PublicKey, path string) error {
	if path == "" {
		return errors.New("AddProgramFromFile: empty path")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	pb := []byte(path)
	rc := C.litesvm_add_program_from_file(
		s.h,
		(*C.uint8_t)(unsafe.Pointer(&programID[0])),
		(*C.uint8_t)(unsafe.Pointer(&pb[0])),
		C.size_t(len(pb)),
	)
	if rc != 0 {
		return lastError(fmt.Sprintf("add_program_from_file rc=%d", rc))
	}
	return nil
}

// ---------------------------------------------------------------------------
// Sysvars
//
// solana-go does not expose struct types for Clock / Rent / EpochSchedule
// (only their well-known account pubkeys via solana.SysVarClockPubkey etc.),
// so we keep our own plain-struct mirrors that match the Solana source.
// ---------------------------------------------------------------------------

// Clock mirrors solana_clock::Clock.
type Clock struct {
	Slot                uint64
	EpochStartTimestamp int64
	Epoch               uint64
	LeaderScheduleEpoch uint64
	UnixTimestamp       int64
}

// Rent mirrors solana_rent::Rent.
type Rent struct {
	LamportsPerByteYear uint64
	ExemptionThreshold  float64
	BurnPercent         uint8
}

// EpochSchedule mirrors solana_epoch_schedule::EpochSchedule.
type EpochSchedule struct {
	SlotsPerEpoch            uint64
	LeaderScheduleSlotOffset uint64
	Warmup                   bool
	FirstNormalEpoch         uint64
	FirstNormalSlot          uint64
}

// Clock returns the current Clock sysvar.
func (s *LiteSVM) Clock() (Clock, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var c C.LiteSvmClock
	rc := C.litesvm_get_clock(s.h, &c)
	if rc != 0 {
		return Clock{}, lastError(fmt.Sprintf("get_clock rc=%d", rc))
	}
	return Clock{
		Slot:                uint64(c.slot),
		EpochStartTimestamp: int64(c.epoch_start_timestamp),
		Epoch:               uint64(c.epoch),
		LeaderScheduleEpoch: uint64(c.leader_schedule_epoch),
		UnixTimestamp:       int64(c.unix_timestamp),
	}, nil
}

// SetClock replaces the current Clock sysvar.
func (s *LiteSVM) SetClock(c Clock) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cc := C.LiteSvmClock{
		slot:                  C.uint64_t(c.Slot),
		epoch_start_timestamp: C.int64_t(c.EpochStartTimestamp),
		epoch:                 C.uint64_t(c.Epoch),
		leader_schedule_epoch: C.uint64_t(c.LeaderScheduleEpoch),
		unix_timestamp:        C.int64_t(c.UnixTimestamp),
	}
	rc := C.litesvm_set_clock(s.h, &cc)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_clock rc=%d", rc))
	}
	return nil
}

// Rent returns the current Rent sysvar.
func (s *LiteSVM) Rent() (Rent, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var r C.LiteSvmRent
	rc := C.litesvm_get_rent(s.h, &r)
	if rc != 0 {
		return Rent{}, lastError(fmt.Sprintf("get_rent rc=%d", rc))
	}
	return Rent{
		LamportsPerByteYear: uint64(r.lamports_per_byte_year),
		ExemptionThreshold:  float64(r.exemption_threshold),
		BurnPercent:         uint8(r.burn_percent),
	}, nil
}

// SetRent replaces the current Rent sysvar.
func (s *LiteSVM) SetRent(r Rent) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cr := C.LiteSvmRent{
		lamports_per_byte_year: C.uint64_t(r.LamportsPerByteYear),
		exemption_threshold:    C.double(r.ExemptionThreshold),
		burn_percent:           C.uint8_t(r.BurnPercent),
	}
	rc := C.litesvm_set_rent(s.h, &cr)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_rent rc=%d", rc))
	}
	return nil
}

// EpochSchedule returns the current EpochSchedule sysvar.
func (s *LiteSVM) EpochSchedule() (EpochSchedule, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var e C.LiteSvmEpochSchedule
	rc := C.litesvm_get_epoch_schedule(s.h, &e)
	if rc != 0 {
		return EpochSchedule{}, lastError(fmt.Sprintf("get_epoch_schedule rc=%d", rc))
	}
	return EpochSchedule{
		SlotsPerEpoch:            uint64(e.slots_per_epoch),
		LeaderScheduleSlotOffset: uint64(e.leader_schedule_slot_offset),
		Warmup:                   e.warmup != 0,
		FirstNormalEpoch:         uint64(e.first_normal_epoch),
		FirstNormalSlot:          uint64(e.first_normal_slot),
	}, nil
}

// SetEpochSchedule replaces the current EpochSchedule sysvar.
func (s *LiteSVM) SetEpochSchedule(e EpochSchedule) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	ce := C.LiteSvmEpochSchedule{
		slots_per_epoch:             C.uint64_t(e.SlotsPerEpoch),
		leader_schedule_slot_offset: C.uint64_t(e.LeaderScheduleSlotOffset),
		first_normal_epoch:          C.uint64_t(e.FirstNormalEpoch),
		first_normal_slot:           C.uint64_t(e.FirstNormalSlot),
	}
	if e.Warmup {
		ce.warmup = 1
	}
	rc := C.litesvm_set_epoch_schedule(s.h, &ce)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_epoch_schedule rc=%d", rc))
	}
	return nil
}

// LastRestartSlot returns the last-restart-slot sysvar.
func (s *LiteSVM) LastRestartSlot() (uint64, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var out C.uint64_t
	rc := C.litesvm_get_last_restart_slot(s.h, &out)
	if rc != 0 {
		return 0, lastError(fmt.Sprintf("get_last_restart_slot rc=%d", rc))
	}
	return uint64(out), nil
}

// SetLastRestartSlot replaces the last-restart-slot sysvar.
func (s *LiteSVM) SetLastRestartSlot(slot uint64) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_last_restart_slot(s.h, C.uint64_t(slot))
	if rc != 0 {
		return lastError(fmt.Sprintf("set_last_restart_slot rc=%d", rc))
	}
	return nil
}

// EpochRewards mirrors solana_epoch_rewards::EpochRewards. total_points is a
// u128 on the Solana side; we surface both halves explicitly so Go callers
// don't need a u128 library. For values that fit in 64 bits, TotalPointsHi
// is 0 and TotalPointsLo is the value.
type EpochRewards struct {
	DistributionStartingBlockHeight uint64
	NumPartitions                   uint64
	ParentBlockhash                 solana.Hash
	TotalPointsLo                   uint64
	TotalPointsHi                   uint64
	TotalRewards                    uint64
	DistributedRewards              uint64
	Active                          bool
}

// EpochRewards returns the current EpochRewards sysvar.
func (s *LiteSVM) EpochRewards() (EpochRewards, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var r C.LiteSvmEpochRewards
	rc := C.litesvm_get_epoch_rewards(s.h, &r)
	if rc != 0 {
		return EpochRewards{}, lastError(fmt.Sprintf("get_epoch_rewards rc=%d", rc))
	}
	return EpochRewards{
		DistributionStartingBlockHeight: uint64(r.distribution_starting_block_height),
		NumPartitions:                   uint64(r.num_partitions),
		// C.uint8_t and byte have the same representation on every supported
		// platform, so a layout-preserving cast replaces a 32-element loop.
		ParentBlockhash:    *(*[32]byte)(unsafe.Pointer(&r.parent_blockhash)),
		TotalPointsLo:      uint64(r.total_points_lo),
		TotalPointsHi:      uint64(r.total_points_hi),
		TotalRewards:       uint64(r.total_rewards),
		DistributedRewards: uint64(r.distributed_rewards),
		Active:             r.active != 0,
	}, nil
}

// SetEpochRewards replaces the EpochRewards sysvar.
func (s *LiteSVM) SetEpochRewards(e EpochRewards) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cr := C.LiteSvmEpochRewards{
		distribution_starting_block_height: C.uint64_t(e.DistributionStartingBlockHeight),
		num_partitions:                     C.uint64_t(e.NumPartitions),
		total_points_lo:                    C.uint64_t(e.TotalPointsLo),
		total_points_hi:                    C.uint64_t(e.TotalPointsHi),
		total_rewards:                      C.uint64_t(e.TotalRewards),
		distributed_rewards:                C.uint64_t(e.DistributedRewards),
	}
	if e.Active {
		cr.active = 1
	}
	*(*[32]byte)(unsafe.Pointer(&cr.parent_blockhash)) = e.ParentBlockhash
	rc := C.litesvm_set_epoch_rewards(s.h, &cr)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_epoch_rewards rc=%d", rc))
	}
	return nil
}

// SlotHash is one entry in the SlotHashes sysvar.
type SlotHash struct {
	Slot uint64
	Hash solana.Hash
}

// SlotHashes returns the current SlotHashes sysvar as a slice.
func (s *LiteSVM) SlotHashes() ([]SlotHash, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	n := int(C.litesvm_get_slot_hashes_count(s.h))
	if n == 0 {
		return nil, nil
	}
	buf := make([]C.LiteSvmSlotHashItem, n)
	got := int(C.litesvm_get_slot_hashes_copy(s.h, &buf[0], C.size_t(n)))
	if got > n {
		got = n
	}
	out := make([]SlotHash, got)
	for i := range got {
		out[i].Slot = uint64(buf[i].slot)
		out[i].Hash = *(*[32]byte)(unsafe.Pointer(&buf[i].hash))
	}
	return out, nil
}

// SetSlotHashes replaces the SlotHashes sysvar.
func (s *LiteSVM) SetSlotHashes(items []SlotHash) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	c := make([]C.LiteSvmSlotHashItem, len(items))
	for i, it := range items {
		c[i].slot = C.uint64_t(it.Slot)
		*(*[32]byte)(unsafe.Pointer(&c[i].hash)) = it.Hash
	}
	var ptr *C.LiteSvmSlotHashItem
	if len(c) > 0 {
		ptr = &c[0]
	}
	rc := C.litesvm_set_slot_hashes(s.h, ptr, C.size_t(len(c)))
	if rc != 0 {
		return lastError(fmt.Sprintf("set_slot_hashes rc=%d", rc))
	}
	return nil
}

// StakeHistoryItem mirrors one (epoch, StakeHistoryEntry) pair.
type StakeHistoryItem struct {
	Epoch        uint64
	Effective    uint64
	Activating   uint64
	Deactivating uint64
}

// StakeHistory returns the current StakeHistory sysvar as a slice.
func (s *LiteSVM) StakeHistory() ([]StakeHistoryItem, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	n := int(C.litesvm_get_stake_history_count(s.h))
	if n == 0 {
		return nil, nil
	}
	buf := make([]C.LiteSvmStakeHistoryItem, n)
	got := int(C.litesvm_get_stake_history_copy(s.h, &buf[0], C.size_t(n)))
	if got > n {
		got = n
	}
	out := make([]StakeHistoryItem, got)
	for i := range got {
		out[i] = StakeHistoryItem{
			Epoch:        uint64(buf[i].epoch),
			Effective:    uint64(buf[i].effective),
			Activating:   uint64(buf[i].activating),
			Deactivating: uint64(buf[i].deactivating),
		}
	}
	return out, nil
}

// SetStakeHistory replaces the StakeHistory sysvar.
func (s *LiteSVM) SetStakeHistory(items []StakeHistoryItem) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	c := make([]C.LiteSvmStakeHistoryItem, len(items))
	for i, it := range items {
		c[i] = C.LiteSvmStakeHistoryItem{
			epoch:        C.uint64_t(it.Epoch),
			effective:    C.uint64_t(it.Effective),
			activating:   C.uint64_t(it.Activating),
			deactivating: C.uint64_t(it.Deactivating),
		}
	}
	var ptr *C.LiteSvmStakeHistoryItem
	if len(c) > 0 {
		ptr = &c[0]
	}
	rc := C.litesvm_set_stake_history(s.h, ptr, C.size_t(len(c)))
	if rc != 0 {
		return lastError(fmt.Sprintf("set_stake_history rc=%d", rc))
	}
	return nil
}

// SlotHistoryCheck mirrors solana_slot_history::Check.
type SlotHistoryCheck int

const (
	SlotHistoryFuture   SlotHistoryCheck = 0
	SlotHistoryTooOld   SlotHistoryCheck = 1
	SlotHistoryFound    SlotHistoryCheck = 2
	SlotHistoryNotFound SlotHistoryCheck = 3
)

// SlotHistory is an opaque handle around solana_slot_history::SlotHistory.
// The underlying bitvec is ~128 KB so it is not passed by value across cgo.
// Always Close when done.
type SlotHistory struct {
	h *C.LiteSvmSlotHistoryHandle
}

// NewSlotHistory returns an empty (default) SlotHistory.
func NewSlotHistory() (*SlotHistory, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	h := C.litesvm_slot_history_new_default()
	if h == nil {
		return nil, lastError("slot_history_new_default")
	}
	sh := &SlotHistory{h: h}
	runtime.SetFinalizer(sh, (*SlotHistory).Close)
	return sh, nil
}

// Close releases the handle. Safe to call more than once.
// Not safe to call concurrently with other methods on the same handle.
func (sh *SlotHistory) Close() {
	if sh == nil || sh.h == nil {
		return
	}
	C.litesvm_slot_history_free(sh.h)
	sh.h = nil
	runtime.SetFinalizer(sh, nil)
}

// Add marks `slot` as observed.
func (sh *SlotHistory) Add(slot uint64) {
	if sh == nil || sh.h == nil {
		return
	}
	C.litesvm_slot_history_add(sh.h, C.uint64_t(slot))
}

// Check reports whether `slot` is in history, too old, or in the future.
func (sh *SlotHistory) Check(slot uint64) SlotHistoryCheck {
	if sh == nil || sh.h == nil {
		return SlotHistoryNotFound
	}
	return SlotHistoryCheck(C.litesvm_slot_history_check(sh.h, C.uint64_t(slot)))
}

// Oldest returns the oldest slot known to the bitvec.
// Returns 0 on a nil or closed handle (ambiguous with a legitimate slot 0).
func (sh *SlotHistory) Oldest() uint64 {
	if sh == nil || sh.h == nil {
		return 0
	}
	return uint64(C.litesvm_slot_history_oldest(sh.h))
}

// Newest returns the newest slot known to the bitvec.
// Returns 0 on a nil or closed handle.
func (sh *SlotHistory) Newest() uint64 {
	if sh == nil || sh.h == nil {
		return 0
	}
	return uint64(C.litesvm_slot_history_newest(sh.h))
}

// NextSlot returns the next_slot field of the bitvec.
// Returns 0 on a nil or closed handle.
func (sh *SlotHistory) NextSlot() uint64 {
	if sh == nil || sh.h == nil {
		return 0
	}
	return uint64(C.litesvm_slot_history_next_slot(sh.h))
}

// SetNextSlot overrides the next_slot field.
func (sh *SlotHistory) SetNextSlot(slot uint64) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_slot_history_set_next_slot(sh.h, C.uint64_t(slot))
	if rc != 0 {
		return lastError(fmt.Sprintf("slot_history_set_next_slot rc=%d", rc))
	}
	return nil
}

// SlotHistory returns the current SlotHistory sysvar as an owned handle.
// Caller must Close it.
func (s *LiteSVM) SlotHistory() (*SlotHistory, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	h := C.litesvm_get_slot_history(s.h)
	if h == nil {
		return nil, lastError("get_slot_history")
	}
	sh := &SlotHistory{h: h}
	runtime.SetFinalizer(sh, (*SlotHistory).Close)
	return sh, nil
}

// SetSlotHistory replaces the SlotHistory sysvar. Caller retains ownership
// of `history`.
func (s *LiteSVM) SetSlotHistory(history *SlotHistory) error {
	if history == nil {
		return errors.New("SetSlotHistory: nil history")
	}
	if history.h == nil {
		return errors.New("SetSlotHistory: history is closed")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_slot_history(s.h, history.h)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_slot_history rc=%d", rc))
	}
	return nil
}

// BuildTransferTx is a test helper that produces a bincode-encoded,
// signed legacy Transaction transferring `lamports` from the keypair
// derived from `payerSeed` to `to`, using `blockhash`.
//
// Prefer building transactions with solana-go directly (see the package
// README). This helper exists to bootstrap tests and for callers that
// want to avoid pulling in the full SDK.
func BuildTransferTx(payerSeed [32]byte, to solana.PublicKey, lamports uint64, blockhash solana.Hash) ([]byte, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var needed C.size_t
	// Probe for required size first (buf_len=0 never copies but reports length).
	rc := C.litesvm_build_transfer_tx(
		(*C.uint8_t)(unsafe.Pointer(&payerSeed[0])),
		(*C.uint8_t)(unsafe.Pointer(&to[0])),
		C.uint64_t(lamports),
		(*C.uint8_t)(unsafe.Pointer(&blockhash[0])),
		nil, 0, &needed,
	)
	if rc != 0 {
		return nil, lastError(fmt.Sprintf("build_transfer_tx probe rc=%d", rc))
	}
	buf := make([]byte, int(needed))
	var written C.size_t
	rc = C.litesvm_build_transfer_tx(
		(*C.uint8_t)(unsafe.Pointer(&payerSeed[0])),
		(*C.uint8_t)(unsafe.Pointer(&to[0])),
		C.uint64_t(lamports),
		(*C.uint8_t)(unsafe.Pointer(&blockhash[0])),
		(*C.uint8_t)(unsafe.Pointer(&buf[0])),
		C.size_t(len(buf)),
		&written,
	)
	if rc != 0 {
		return nil, lastError(fmt.Sprintf("build_transfer_tx rc=%d", rc))
	}
	if int(written) != len(buf) {
		return nil, fmt.Errorf("build_transfer_tx: wrote %d, expected %d", int(written), len(buf))
	}
	return buf, nil
}

// copyVarBuf is the generic "probe-then-copy" pattern used by every Rust
// extern that writes a variable-length UTF-8 string into a caller-allocated
// buffer. The callback must return the total required length (matching the
// C-side contract), copying up to bufLen bytes on call.
//
// Callers must have the OS thread locked (see package doc comment): both
// cgo calls need to land on the same Rust thread for the thread-local
// error slot to be consistent if the second call ever sets it.
func copyVarBuf(call func(buf *C.uint8_t, bufLen C.size_t) C.size_t) string {
	n := int(call(nil, 0))
	if n == 0 {
		return ""
	}
	buf := make([]byte, n)
	got := int(call(
		(*C.uint8_t)(unsafe.Pointer(&buf[0])),
		C.size_t(len(buf)),
	))
	if got > len(buf) {
		got = len(buf)
	}
	return string(buf[:got])
}

// lastError reads the thread-local Rust error string and wraps it under the
// given prefix. If the Rust side recorded no message, prefix is used as-is.
//
// Callers must have the OS thread locked (see package doc comment) — the
// message being read here was written by the preceding cgo call in the
// caller and must be read on the same OS thread.
func lastError(prefix string) error {
	// Try a generous stack buffer first: avoids a probe call in the common
	// case where the error fits, which in turn keeps the whole "operation +
	// error readback" sequence to two cgo calls total.
	var stack [512]byte
	n := int(C.litesvm_last_error_copy(
		(*C.uint8_t)(unsafe.Pointer(&stack[0])),
		C.size_t(len(stack)),
	))
	if n == 0 {
		return errors.New(prefix)
	}
	if n <= len(stack) {
		return fmt.Errorf("%s: %s", prefix, string(stack[:n]))
	}
	// Error exceeded the stack buffer; allocate and re-read.
	buf := make([]byte, n)
	got := int(C.litesvm_last_error_copy(
		(*C.uint8_t)(unsafe.Pointer(&buf[0])),
		C.size_t(len(buf)),
	))
	if got > len(buf) {
		got = len(buf)
	}
	return fmt.Errorf("%s: %s", prefix, string(buf[:got]))
}

// ---------------------------------------------------------------------------
// ComputeBudget
// ---------------------------------------------------------------------------

// ComputeBudget mirrors solana_compute_budget::compute_budget::ComputeBudget.
// Fields typed as `usize` on the Solana side are surfaced as uint64; HeapSize
// stays uint32.
type ComputeBudget struct {
	ComputeUnitLimit                      uint64
	Log64Units                            uint64
	CreateProgramAddressUnits             uint64
	InvokeUnits                           uint64
	MaxInstructionStackDepth              uint64
	MaxInstructionTraceLength             uint64
	Sha256BaseCost                        uint64
	Sha256ByteCost                        uint64
	Sha256MaxSlices                       uint64
	MaxCallDepth                          uint64
	StackFrameSize                        uint64
	LogPubkeyUnits                        uint64
	CpiBytesPerUnit                       uint64
	SysvarBaseCost                        uint64
	Secp256k1RecoverCost                  uint64
	SyscallBaseCost                       uint64
	Curve25519EdwardsValidatePointCost    uint64
	Curve25519EdwardsAddCost              uint64
	Curve25519EdwardsSubtractCost         uint64
	Curve25519EdwardsMultiplyCost         uint64
	Curve25519EdwardsMsmBaseCost          uint64
	Curve25519EdwardsMsmIncrementalCost   uint64
	Curve25519RistrettoValidatePointCost  uint64
	Curve25519RistrettoAddCost            uint64
	Curve25519RistrettoSubtractCost       uint64
	Curve25519RistrettoMultiplyCost       uint64
	Curve25519RistrettoMsmBaseCost        uint64
	Curve25519RistrettoMsmIncrementalCost uint64
	HeapSize                              uint32
	HeapCost                              uint64
	MemOpBaseCost                         uint64
	AltBn128AdditionCost                  uint64
	AltBn128MultiplicationCost            uint64
	AltBn128PairingOnePairCostFirst       uint64
	AltBn128PairingOnePairCostOther       uint64
	BigModularExponentiationBaseCost      uint64
	BigModularExponentiationCostDivisor   uint64
	PoseidonCostCoefficientA              uint64
	PoseidonCostCoefficientC              uint64
	GetRemainingComputeUnitsCost          uint64
	AltBn128G1Compress                    uint64
	AltBn128G1Decompress                  uint64
	AltBn128G2Compress                    uint64
	AltBn128G2Decompress                  uint64
}

func cbFromC(c C.LiteSvmComputeBudget) ComputeBudget {
	return ComputeBudget{
		ComputeUnitLimit:                      uint64(c.compute_unit_limit),
		Log64Units:                            uint64(c.log_64_units),
		CreateProgramAddressUnits:             uint64(c.create_program_address_units),
		InvokeUnits:                           uint64(c.invoke_units),
		MaxInstructionStackDepth:              uint64(c.max_instruction_stack_depth),
		MaxInstructionTraceLength:             uint64(c.max_instruction_trace_length),
		Sha256BaseCost:                        uint64(c.sha256_base_cost),
		Sha256ByteCost:                        uint64(c.sha256_byte_cost),
		Sha256MaxSlices:                       uint64(c.sha256_max_slices),
		MaxCallDepth:                          uint64(c.max_call_depth),
		StackFrameSize:                        uint64(c.stack_frame_size),
		LogPubkeyUnits:                        uint64(c.log_pubkey_units),
		CpiBytesPerUnit:                       uint64(c.cpi_bytes_per_unit),
		SysvarBaseCost:                        uint64(c.sysvar_base_cost),
		Secp256k1RecoverCost:                  uint64(c.secp256k1_recover_cost),
		SyscallBaseCost:                       uint64(c.syscall_base_cost),
		Curve25519EdwardsValidatePointCost:    uint64(c.curve25519_edwards_validate_point_cost),
		Curve25519EdwardsAddCost:              uint64(c.curve25519_edwards_add_cost),
		Curve25519EdwardsSubtractCost:         uint64(c.curve25519_edwards_subtract_cost),
		Curve25519EdwardsMultiplyCost:         uint64(c.curve25519_edwards_multiply_cost),
		Curve25519EdwardsMsmBaseCost:          uint64(c.curve25519_edwards_msm_base_cost),
		Curve25519EdwardsMsmIncrementalCost:   uint64(c.curve25519_edwards_msm_incremental_cost),
		Curve25519RistrettoValidatePointCost:  uint64(c.curve25519_ristretto_validate_point_cost),
		Curve25519RistrettoAddCost:            uint64(c.curve25519_ristretto_add_cost),
		Curve25519RistrettoSubtractCost:       uint64(c.curve25519_ristretto_subtract_cost),
		Curve25519RistrettoMultiplyCost:       uint64(c.curve25519_ristretto_multiply_cost),
		Curve25519RistrettoMsmBaseCost:        uint64(c.curve25519_ristretto_msm_base_cost),
		Curve25519RistrettoMsmIncrementalCost: uint64(c.curve25519_ristretto_msm_incremental_cost),
		HeapSize:                              uint32(c.heap_size),
		HeapCost:                              uint64(c.heap_cost),
		MemOpBaseCost:                         uint64(c.mem_op_base_cost),
		AltBn128AdditionCost:                  uint64(c.alt_bn128_addition_cost),
		AltBn128MultiplicationCost:            uint64(c.alt_bn128_multiplication_cost),
		AltBn128PairingOnePairCostFirst:       uint64(c.alt_bn128_pairing_one_pair_cost_first),
		AltBn128PairingOnePairCostOther:       uint64(c.alt_bn128_pairing_one_pair_cost_other),
		BigModularExponentiationBaseCost:      uint64(c.big_modular_exponentiation_base_cost),
		BigModularExponentiationCostDivisor:   uint64(c.big_modular_exponentiation_cost_divisor),
		PoseidonCostCoefficientA:              uint64(c.poseidon_cost_coefficient_a),
		PoseidonCostCoefficientC:              uint64(c.poseidon_cost_coefficient_c),
		GetRemainingComputeUnitsCost:          uint64(c.get_remaining_compute_units_cost),
		AltBn128G1Compress:                    uint64(c.alt_bn128_g1_compress),
		AltBn128G1Decompress:                  uint64(c.alt_bn128_g1_decompress),
		AltBn128G2Compress:                    uint64(c.alt_bn128_g2_compress),
		AltBn128G2Decompress:                  uint64(c.alt_bn128_g2_decompress),
	}
}

func cbToC(b ComputeBudget) C.LiteSvmComputeBudget {
	return C.LiteSvmComputeBudget{
		compute_unit_limit:                        C.uint64_t(b.ComputeUnitLimit),
		log_64_units:                              C.uint64_t(b.Log64Units),
		create_program_address_units:              C.uint64_t(b.CreateProgramAddressUnits),
		invoke_units:                              C.uint64_t(b.InvokeUnits),
		max_instruction_stack_depth:               C.uint64_t(b.MaxInstructionStackDepth),
		max_instruction_trace_length:              C.uint64_t(b.MaxInstructionTraceLength),
		sha256_base_cost:                          C.uint64_t(b.Sha256BaseCost),
		sha256_byte_cost:                          C.uint64_t(b.Sha256ByteCost),
		sha256_max_slices:                         C.uint64_t(b.Sha256MaxSlices),
		max_call_depth:                            C.uint64_t(b.MaxCallDepth),
		stack_frame_size:                          C.uint64_t(b.StackFrameSize),
		log_pubkey_units:                          C.uint64_t(b.LogPubkeyUnits),
		cpi_bytes_per_unit:                        C.uint64_t(b.CpiBytesPerUnit),
		sysvar_base_cost:                          C.uint64_t(b.SysvarBaseCost),
		secp256k1_recover_cost:                    C.uint64_t(b.Secp256k1RecoverCost),
		syscall_base_cost:                         C.uint64_t(b.SyscallBaseCost),
		curve25519_edwards_validate_point_cost:    C.uint64_t(b.Curve25519EdwardsValidatePointCost),
		curve25519_edwards_add_cost:               C.uint64_t(b.Curve25519EdwardsAddCost),
		curve25519_edwards_subtract_cost:          C.uint64_t(b.Curve25519EdwardsSubtractCost),
		curve25519_edwards_multiply_cost:          C.uint64_t(b.Curve25519EdwardsMultiplyCost),
		curve25519_edwards_msm_base_cost:          C.uint64_t(b.Curve25519EdwardsMsmBaseCost),
		curve25519_edwards_msm_incremental_cost:   C.uint64_t(b.Curve25519EdwardsMsmIncrementalCost),
		curve25519_ristretto_validate_point_cost:  C.uint64_t(b.Curve25519RistrettoValidatePointCost),
		curve25519_ristretto_add_cost:             C.uint64_t(b.Curve25519RistrettoAddCost),
		curve25519_ristretto_subtract_cost:        C.uint64_t(b.Curve25519RistrettoSubtractCost),
		curve25519_ristretto_multiply_cost:        C.uint64_t(b.Curve25519RistrettoMultiplyCost),
		curve25519_ristretto_msm_base_cost:        C.uint64_t(b.Curve25519RistrettoMsmBaseCost),
		curve25519_ristretto_msm_incremental_cost: C.uint64_t(b.Curve25519RistrettoMsmIncrementalCost),
		heap_size:                               C.uint32_t(b.HeapSize),
		heap_cost:                               C.uint64_t(b.HeapCost),
		mem_op_base_cost:                        C.uint64_t(b.MemOpBaseCost),
		alt_bn128_addition_cost:                 C.uint64_t(b.AltBn128AdditionCost),
		alt_bn128_multiplication_cost:           C.uint64_t(b.AltBn128MultiplicationCost),
		alt_bn128_pairing_one_pair_cost_first:   C.uint64_t(b.AltBn128PairingOnePairCostFirst),
		alt_bn128_pairing_one_pair_cost_other:   C.uint64_t(b.AltBn128PairingOnePairCostOther),
		big_modular_exponentiation_base_cost:    C.uint64_t(b.BigModularExponentiationBaseCost),
		big_modular_exponentiation_cost_divisor: C.uint64_t(b.BigModularExponentiationCostDivisor),
		poseidon_cost_coefficient_a:             C.uint64_t(b.PoseidonCostCoefficientA),
		poseidon_cost_coefficient_c:             C.uint64_t(b.PoseidonCostCoefficientC),
		get_remaining_compute_units_cost:        C.uint64_t(b.GetRemainingComputeUnitsCost),
		alt_bn128_g1_compress:                   C.uint64_t(b.AltBn128G1Compress),
		alt_bn128_g1_decompress:                 C.uint64_t(b.AltBn128G1Decompress),
		alt_bn128_g2_compress:                   C.uint64_t(b.AltBn128G2Compress),
		alt_bn128_g2_decompress:                 C.uint64_t(b.AltBn128G2Decompress),
	}
}

// ComputeBudget returns the current custom compute budget. The bool is
// false when no custom budget has been configured (the runtime default
// applies).
func (s *LiteSVM) ComputeBudget() (ComputeBudget, bool, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var c C.LiteSvmComputeBudget
	rc := C.litesvm_get_compute_budget(s.h, &c)
	switch rc {
	case 0:
		return cbFromC(c), true, nil
	case 1:
		return ComputeBudget{}, false, nil
	default:
		return ComputeBudget{}, false, lastError(fmt.Sprintf("get_compute_budget rc=%d", rc))
	}
}

// SetComputeBudget replaces the compute budget used for subsequent txs.
func (s *LiteSVM) SetComputeBudget(b ComputeBudget) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	c := cbToC(b)
	rc := C.litesvm_set_compute_budget(s.h, &c)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_compute_budget rc=%d", rc))
	}
	return nil
}

// ---------------------------------------------------------------------------
// FeatureSet (opaque handle)
// ---------------------------------------------------------------------------

// FeatureSet is an opaque handle around agave_feature_set::FeatureSet.
// Always Close when done.
type FeatureSet struct {
	h *C.LiteSvmFeatureSetHandle
}

// NewFeatureSet returns the default (mostly empty) feature set.
func NewFeatureSet() (*FeatureSet, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	h := C.litesvm_feature_set_new_default()
	if h == nil {
		return nil, lastError("feature_set_new_default")
	}
	fs := &FeatureSet{h: h}
	runtime.SetFinalizer(fs, (*FeatureSet).Close)
	return fs, nil
}

// NewFeatureSetAllEnabled returns a FeatureSet with every known feature
// activated at slot 0.
func NewFeatureSetAllEnabled() (*FeatureSet, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	h := C.litesvm_feature_set_new_all_enabled()
	if h == nil {
		return nil, lastError("feature_set_new_all_enabled")
	}
	fs := &FeatureSet{h: h}
	runtime.SetFinalizer(fs, (*FeatureSet).Close)
	return fs, nil
}

// Close releases the handle. Safe to call more than once.
// Not safe to call concurrently with other methods on the same handle.
func (fs *FeatureSet) Close() {
	if fs == nil || fs.h == nil {
		return
	}
	C.litesvm_feature_set_free(fs.h)
	fs.h = nil
	runtime.SetFinalizer(fs, nil)
}

// IsActive reports whether `featureID` is currently active.
func (fs *FeatureSet) IsActive(featureID solana.PublicKey) bool {
	if fs == nil || fs.h == nil {
		return false
	}
	return C.litesvm_feature_set_is_active(fs.h, (*C.uint8_t)(unsafe.Pointer(&featureID[0]))) == 1
}

// ActivatedSlot returns the slot at which `featureID` was activated.
// The bool is false if the feature is not active.
func (fs *FeatureSet) ActivatedSlot(featureID solana.PublicKey) (uint64, bool) {
	if fs == nil || fs.h == nil {
		return 0, false
	}
	var slot C.uint64_t
	rc := C.litesvm_feature_set_activated_slot(
		fs.h,
		(*C.uint8_t)(unsafe.Pointer(&featureID[0])),
		&slot,
	)
	if rc == 1 {
		return uint64(slot), true
	}
	return 0, false
}

// Activate marks `featureID` as active at `slot`.
func (fs *FeatureSet) Activate(featureID solana.PublicKey, slot uint64) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_feature_set_activate(
		fs.h,
		(*C.uint8_t)(unsafe.Pointer(&featureID[0])),
		C.uint64_t(slot),
	)
	if rc != 0 {
		return lastError(fmt.Sprintf("feature_set_activate rc=%d", rc))
	}
	return nil
}

// Deactivate marks `featureID` as inactive.
func (fs *FeatureSet) Deactivate(featureID solana.PublicKey) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_feature_set_deactivate(
		fs.h,
		(*C.uint8_t)(unsafe.Pointer(&featureID[0])),
	)
	if rc != 0 {
		return lastError(fmt.Sprintf("feature_set_deactivate rc=%d", rc))
	}
	return nil
}

// ActiveCount returns the number of active features.
func (fs *FeatureSet) ActiveCount() int {
	if fs == nil || fs.h == nil {
		return 0
	}
	return int(C.litesvm_feature_set_active_count(fs.h))
}

// InactiveCount returns the number of inactive features.
func (fs *FeatureSet) InactiveCount() int {
	if fs == nil || fs.h == nil {
		return 0
	}
	return int(C.litesvm_feature_set_inactive_count(fs.h))
}

// ActiveFeatures returns the full list of active feature pubkeys. Output is
// sorted for deterministic ordering.
func (fs *FeatureSet) ActiveFeatures() []solana.PublicKey {
	if fs == nil || fs.h == nil {
		return nil
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	n := int(C.litesvm_feature_set_active_count(fs.h))
	if n == 0 {
		return nil
	}
	buf := make([]byte, n*32)
	got := int(C.litesvm_feature_set_active_copy(
		fs.h,
		(*C.uint8_t)(unsafe.Pointer(&buf[0])),
		C.size_t(n),
	))
	if got > n {
		got = n
	}
	out := make([]solana.PublicKey, got)
	for i := range got {
		copy(out[i][:], buf[i*32:(i+1)*32])
	}
	return out
}

// InactiveFeatures returns the full list of inactive feature pubkeys. Output
// is sorted for deterministic ordering.
func (fs *FeatureSet) InactiveFeatures() []solana.PublicKey {
	if fs == nil || fs.h == nil {
		return nil
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	n := int(C.litesvm_feature_set_inactive_count(fs.h))
	if n == 0 {
		return nil
	}
	buf := make([]byte, n*32)
	got := int(C.litesvm_feature_set_inactive_copy(
		fs.h,
		(*C.uint8_t)(unsafe.Pointer(&buf[0])),
		C.size_t(n),
	))
	if got > n {
		got = n
	}
	out := make([]solana.PublicKey, got)
	for i := range got {
		copy(out[i][:], buf[i*32:(i+1)*32])
	}
	return out
}

// SetFeatureSet installs `features` as the SVM's active feature set. Caller
// retains ownership of `features` (it is cloned internally).
func (s *LiteSVM) SetFeatureSet(features *FeatureSet) error {
	if features == nil {
		return errors.New("SetFeatureSet: nil feature set")
	}
	if features.h == nil {
		return errors.New("SetFeatureSet: feature set is closed")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	rc := C.litesvm_set_feature_set(s.h, features.h)
	if rc != 0 {
		return lastError(fmt.Sprintf("set_feature_set rc=%d", rc))
	}
	return nil
}
