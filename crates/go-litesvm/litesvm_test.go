package litesvm

import (
	"crypto/ed25519"
	"crypto/rand"
	"os"
	"path/filepath"
	"strings"
	"testing"

	solana "github.com/gagliardetto/solana-go"
	"github.com/gagliardetto/solana-go/programs/system"
)

// programBytesDir points at the shared fixture directory used by node-litesvm.
// We reach across crates in tests only; the binding itself has no such coupling.
func programBytesDir(t *testing.T) string {
	t.Helper()
	p, err := filepath.Abs(filepath.Join("..", "node-litesvm", "program_bytes"))
	if err != nil {
		t.Fatalf("abs path: %v", err)
	}
	if _, err := os.Stat(p); err != nil {
		t.Skipf("program fixtures not available: %v", err)
	}
	return p
}

func randPubkey(t *testing.T) solana.PublicKey {
	t.Helper()
	var p solana.PublicKey
	if _, err := rand.Read(p[:]); err != nil {
		t.Fatalf("rand: %v", err)
	}
	return p
}

func TestNewAndClose(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	svm.Close()
	// second close is a no-op
	svm.Close()
}

func TestVersion(t *testing.T) {
	v := Version()
	if v == "" {
		t.Fatal("empty version")
	}
	t.Logf("wrapper version: %s", v)
}

func TestAirdropAndBalance(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	pk := randPubkey(t)

	// Unknown account -> not found.
	_, ok, err := svm.Balance(pk)
	if err != nil {
		t.Fatalf("Balance (unknown): %v", err)
	}
	if ok {
		t.Fatal("expected unknown account, got found")
	}

	const lamports = uint64(1_234_567_890)
	if err := svm.Airdrop(pk, lamports); err != nil {
		t.Fatalf("Airdrop: %v", err)
	}

	got, ok, err := svm.Balance(pk)
	if err != nil {
		t.Fatalf("Balance: %v", err)
	}
	if !ok {
		t.Fatal("expected account to exist after airdrop")
	}
	if got != lamports {
		t.Fatalf("balance = %d, want %d", got, lamports)
	}
}

func TestLatestBlockhashChangesOnExpire(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	bh1, err := svm.LatestBlockhash()
	if err != nil {
		t.Fatalf("LatestBlockhash: %v", err)
	}
	if err := svm.ExpireBlockhash(); err != nil {
		t.Fatalf("ExpireBlockhash: %v", err)
	}
	bh2, err := svm.LatestBlockhash()
	if err != nil {
		t.Fatalf("LatestBlockhash: %v", err)
	}
	if bh1.Equals(bh2) {
		t.Fatal("expected blockhash to change after ExpireBlockhash")
	}
}

func TestRentExemption(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	zero, err := svm.MinimumBalanceForRentExemption(0)
	if err != nil {
		t.Fatalf("MinimumBalanceForRentExemption(0): %v", err)
	}
	big, err := svm.MinimumBalanceForRentExemption(1024)
	if err != nil {
		t.Fatalf("MinimumBalanceForRentExemption(1024): %v", err)
	}
	if big <= zero {
		t.Fatalf("expected 1024-byte rent (%d) > 0-byte rent (%d)", big, zero)
	}
}

// pubkeyFromSeed derives the Solana public key for a 32-byte ed25519 seed.
func pubkeyFromSeed(t *testing.T, seed [32]byte) solana.PublicKey {
	t.Helper()
	priv := solana.PrivateKey(ed25519.NewKeyFromSeed(seed[:]))
	return priv.PublicKey()
}

func TestSendBadBytesProducesError(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	_, err = svm.SendLegacyTransaction([]byte{0xDE, 0xAD, 0xBE, 0xEF})
	if err == nil {
		t.Fatal("expected decode error for garbage tx bytes")
	}
	if !strings.Contains(err.Error(), "decode Transaction") {
		t.Fatalf("expected decode-error message, got: %v", err)
	}
}

func TestTransferRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	var seed [32]byte
	if _, err := rand.Read(seed[:]); err != nil {
		t.Fatalf("rand: %v", err)
	}
	payer := pubkeyFromSeed(t, seed)
	recipient := randPubkey(t)

	const payerStart = uint64(10 * 1_000_000_000) // 10 SOL
	const xfer = uint64(1_000_000)                // 0.001 SOL
	if err := svm.Airdrop(payer, payerStart); err != nil {
		t.Fatalf("Airdrop payer: %v", err)
	}

	bh, err := svm.LatestBlockhash()
	if err != nil {
		t.Fatalf("LatestBlockhash: %v", err)
	}

	txBytes, err := BuildTransferTx(seed, recipient, xfer, bh)
	if err != nil {
		t.Fatalf("BuildTransferTx: %v", err)
	}
	if len(txBytes) == 0 {
		t.Fatal("BuildTransferTx returned empty bytes")
	}

	out, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SendLegacyTransaction: %v", err)
	}
	defer out.Close()

	if !out.IsOk() {
		t.Fatalf("tx failed: %s; logs=%v", out.Error(), out.Logs())
	}
	if out.Fee() == 0 {
		t.Fatal("expected non-zero fee")
	}
	if out.ComputeUnits() == 0 {
		t.Fatal("expected non-zero compute units")
	}
	if out.Signature().IsZero() {
		t.Fatal("expected non-zero signature")
	}

	// Recipient should have exactly the transferred amount.
	rb, ok, err := svm.Balance(recipient)
	if err != nil || !ok {
		t.Fatalf("Balance(recipient): ok=%v err=%v", ok, err)
	}
	if rb != xfer {
		t.Fatalf("recipient balance = %d, want %d", rb, xfer)
	}

	// Payer should have lost transfer + fee.
	pb, ok, err := svm.Balance(payer)
	if err != nil || !ok {
		t.Fatalf("Balance(payer): ok=%v err=%v", ok, err)
	}
	if pb != payerStart-xfer-out.Fee() {
		t.Fatalf("payer balance = %d, want %d", pb, payerStart-xfer-out.Fee())
	}
}

// TestTransferViaSolanaGo exercises the pure-Go path: construct and sign a
// transaction using solana-go, marshal it with MarshalBinary, and submit it
// via SendLegacyTransaction. This validates that solana-go's legacy
// Transaction wire format is bincode-compatible with what litesvm expects.
func TestTransferViaSolanaGo(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	// Build the payer keypair using solana-go.
	priv, err := solana.NewRandomPrivateKey()
	if err != nil {
		t.Fatalf("NewRandomPrivateKey: %v", err)
	}
	payer := priv.PublicKey()
	recipient := solana.NewWallet().PublicKey()

	const payerStart = uint64(5 * 1_000_000_000)
	const xfer = uint64(2_500_000)
	if err := svm.Airdrop(payer, payerStart); err != nil {
		t.Fatalf("Airdrop: %v", err)
	}

	bh, err := svm.LatestBlockhash()
	if err != nil {
		t.Fatalf("LatestBlockhash: %v", err)
	}

	ix := system.NewTransferInstruction(xfer, payer, recipient).Build()
	tx, err := solana.NewTransaction(
		[]solana.Instruction{ix},
		bh,
		solana.TransactionPayer(payer),
	)
	if err != nil {
		t.Fatalf("NewTransaction: %v", err)
	}
	if _, err := tx.Sign(func(k solana.PublicKey) *solana.PrivateKey {
		if k.Equals(payer) {
			return &priv
		}
		return nil
	}); err != nil {
		t.Fatalf("Sign: %v", err)
	}

	txBytes, err := tx.MarshalBinary()
	if err != nil {
		t.Fatalf("MarshalBinary: %v", err)
	}

	out, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SendLegacyTransaction: %v", err)
	}
	defer out.Close()

	if !out.IsOk() {
		t.Fatalf("tx failed: %s; logs=%v", out.Error(), out.Logs())
	}

	// Signature returned by litesvm should match the one we signed with.
	want := tx.Signatures[0]
	if !out.Signature().Equals(want) {
		t.Fatalf("signature mismatch: got %s, want %s", out.Signature(), want)
	}

	rb, _, _ := svm.Balance(recipient)
	if rb != xfer {
		t.Fatalf("recipient balance = %d, want %d", rb, xfer)
	}
}

func TestTransferFailsOnInsufficientFunds(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	var seed [32]byte
	if _, err := rand.Read(seed[:]); err != nil {
		t.Fatalf("rand: %v", err)
	}
	payer := pubkeyFromSeed(t, seed)
	recipient := randPubkey(t)

	// Fund the payer, then attempt to transfer more than the balance.
	const funded = uint64(1_000_000_000) // 1 SOL
	if err := svm.Airdrop(payer, funded); err != nil {
		t.Fatalf("Airdrop payer: %v", err)
	}
	bh, err := svm.LatestBlockhash()
	if err != nil {
		t.Fatalf("LatestBlockhash: %v", err)
	}
	txBytes, err := BuildTransferTx(seed, recipient, funded*10, bh)
	if err != nil {
		t.Fatalf("BuildTransferTx: %v", err)
	}
	out, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SendLegacyTransaction: %v", err)
	}
	defer out.Close()

	if out.IsOk() {
		t.Fatal("expected tx to fail")
	}
	if out.Error() == "" {
		t.Fatal("expected a non-empty error description")
	}
	t.Logf("fail tx error: %s", out.Error())
}

// A deterministic non-system-program owner used for SetAccount tests.
var testOwner = solana.PublicKey{1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
	11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
	21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32}

func TestGetAccountMissing(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	a := svm.GetAccount(randPubkey(t))
	if a != nil {
		defer a.Close()
		t.Fatal("expected nil for non-existent account")
	}
}

func TestGetAccountAfterAirdrop(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	pk := randPubkey(t)
	const amount = uint64(5_000_000)
	if err := svm.Airdrop(pk, amount); err != nil {
		t.Fatalf("Airdrop: %v", err)
	}

	a := svm.GetAccount(pk)
	if a == nil {
		t.Fatal("expected account after airdrop")
	}
	defer a.Close()

	if got := a.Lamports(); got != amount {
		t.Fatalf("lamports = %d, want %d", got, amount)
	}
	if a.Executable() {
		t.Fatal("airdrop account should not be executable")
	}
	if data := a.Data(); len(data) != 0 {
		t.Fatalf("airdrop account data = %d bytes, want 0", len(data))
	}
}

func TestSetAccountRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	pk := randPubkey(t)
	payload := []byte("hello from go-litesvm")
	// Rent-exempt minimum + some spare, so SetAccount won't reject it.
	rent, err := svm.MinimumBalanceForRentExemption(len(payload))
	if err != nil {
		t.Fatalf("MinimumBalanceForRentExemption: %v", err)
	}

	src, err := NewAccount(rent, payload, testOwner, false, 0)
	if err != nil {
		t.Fatalf("NewAccount: %v", err)
	}
	defer src.Close()

	if err := svm.SetAccount(pk, src); err != nil {
		t.Fatalf("SetAccount: %v", err)
	}

	got := svm.GetAccount(pk)
	if got == nil {
		t.Fatal("expected account to exist after SetAccount")
	}
	defer got.Close()

	if got.Lamports() != rent {
		t.Fatalf("lamports = %d, want %d", got.Lamports(), rent)
	}
	if !got.Owner().Equals(testOwner) {
		t.Fatalf("owner = %s, want %s", got.Owner(), testOwner)
	}
	if gotData := got.Data(); string(gotData) != string(payload) {
		t.Fatalf("data = %q, want %q", gotData, payload)
	}
}

func TestAddProgramFromFile(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	dir := programBytesDir(t)
	path := filepath.Join(dir, "spl_example_logging.so")

	programID := randPubkey(t)
	if err := svm.AddProgramFromFile(programID, path); err != nil {
		t.Fatalf("AddProgramFromFile: %v", err)
	}

	acct := svm.GetAccount(programID)
	if acct == nil {
		t.Fatal("program account missing after AddProgramFromFile")
	}
	defer acct.Close()

	if !acct.Executable() {
		t.Fatal("program account should be executable")
	}
}

func TestAddProgramFromBytes(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	dir := programBytesDir(t)
	bytes, err := os.ReadFile(filepath.Join(dir, "spl_example_logging.so"))
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}

	programID := randPubkey(t)
	if err := svm.AddProgram(programID, bytes); err != nil {
		t.Fatalf("AddProgram: %v", err)
	}

	acct := svm.GetAccount(programID)
	if acct == nil {
		t.Fatal("program account missing after AddProgram")
	}
	defer acct.Close()

	if !acct.Executable() {
		t.Fatal("program account should be executable")
	}
}

func TestClockRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	want := Clock{
		Slot:                42_000,
		EpochStartTimestamp: 1_700_000_000,
		Epoch:               7,
		LeaderScheduleEpoch: 8,
		UnixTimestamp:       1_700_000_123,
	}
	if err := svm.SetClock(want); err != nil {
		t.Fatalf("SetClock: %v", err)
	}
	got, err := svm.Clock()
	if err != nil {
		t.Fatalf("Clock: %v", err)
	}
	if got != want {
		t.Fatalf("clock mismatch\n got=%+v\nwant=%+v", got, want)
	}
}

func TestRentRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	// Sanity-check defaults before overwriting.
	def, err := svm.Rent()
	if err != nil {
		t.Fatalf("Rent: %v", err)
	}
	if def.LamportsPerByteYear == 0 {
		t.Fatal("expected non-zero default lamports_per_byte_year")
	}

	want := Rent{
		LamportsPerByteYear: 123,
		ExemptionThreshold:  4.5,
		BurnPercent:         11,
	}
	if err := svm.SetRent(want); err != nil {
		t.Fatalf("SetRent: %v", err)
	}
	got, err := svm.Rent()
	if err != nil {
		t.Fatalf("Rent: %v", err)
	}
	if got != want {
		t.Fatalf("rent mismatch\n got=%+v\nwant=%+v", got, want)
	}
}

func TestEpochScheduleRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	want := EpochSchedule{
		SlotsPerEpoch:            8192,
		LeaderScheduleSlotOffset: 8192,
		Warmup:                   true,
		FirstNormalEpoch:         14,
		FirstNormalSlot:          524288,
	}
	if err := svm.SetEpochSchedule(want); err != nil {
		t.Fatalf("SetEpochSchedule: %v", err)
	}
	got, err := svm.EpochSchedule()
	if err != nil {
		t.Fatalf("EpochSchedule: %v", err)
	}
	if got != want {
		t.Fatalf("schedule mismatch\n got=%+v\nwant=%+v", got, want)
	}
}

func TestLastRestartSlotRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	const want = uint64(123456789)
	if err := svm.SetLastRestartSlot(want); err != nil {
		t.Fatalf("SetLastRestartSlot: %v", err)
	}
	got, err := svm.LastRestartSlot()
	if err != nil {
		t.Fatalf("LastRestartSlot: %v", err)
	}
	if got != want {
		t.Fatalf("last_restart_slot = %d, want %d", got, want)
	}
}

func TestWarpToSlot(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	before, err := svm.Clock()
	if err != nil {
		t.Fatalf("Clock: %v", err)
	}
	const jump = uint64(1_234_567)
	if err := svm.WarpToSlot(before.Slot + jump); err != nil {
		t.Fatalf("WarpToSlot: %v", err)
	}
	after, err := svm.Clock()
	if err != nil {
		t.Fatalf("Clock: %v", err)
	}
	if after.Slot != before.Slot+jump {
		t.Fatalf("slot = %d, want %d", after.Slot, before.Slot+jump)
	}
}

// buildSignedTransfer is a local helper that uses solana-go to construct a
// signed legacy transfer transaction and return both the marshaled bytes
// and the payer so tests can assert post-state.
func buildSignedTransfer(t *testing.T, svm *LiteSVM, fund, xfer uint64) (
	priv solana.PrivateKey,
	payer, recipient solana.PublicKey,
	txBytes []byte,
) {
	t.Helper()
	var err error
	priv, err = solana.NewRandomPrivateKey()
	if err != nil {
		t.Fatalf("NewRandomPrivateKey: %v", err)
	}
	payer = priv.PublicKey()
	recipient = solana.NewWallet().PublicKey()

	if err := svm.Airdrop(payer, fund); err != nil {
		t.Fatalf("Airdrop: %v", err)
	}
	bh, err := svm.LatestBlockhash()
	if err != nil {
		t.Fatalf("LatestBlockhash: %v", err)
	}
	ix := system.NewTransferInstruction(xfer, payer, recipient).Build()
	tx, err := solana.NewTransaction([]solana.Instruction{ix}, bh, solana.TransactionPayer(payer))
	if err != nil {
		t.Fatalf("NewTransaction: %v", err)
	}
	if _, err := tx.Sign(func(k solana.PublicKey) *solana.PrivateKey {
		if k.Equals(payer) {
			return &priv
		}
		return nil
	}); err != nil {
		t.Fatalf("Sign: %v", err)
	}
	txBytes, err = tx.MarshalBinary()
	if err != nil {
		t.Fatalf("MarshalBinary: %v", err)
	}
	return
}

func TestSimulateLegacyTransactionSuccess(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	const fund = uint64(5 * 1_000_000_000)
	const xfer = uint64(1_000_000)
	_, payer, recipient, txBytes := buildSignedTransfer(t, svm, fund, xfer)

	out, err := svm.SimulateLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SimulateLegacyTransaction: %v", err)
	}
	defer out.Close()

	if !out.IsOk() {
		t.Fatalf("simulate failed: %s", out.Error())
	}
	if out.ComputeUnits() == 0 {
		t.Fatal("expected non-zero compute units")
	}

	// A bare transfer never sets return data.
	if _, _, has := out.ReturnData(); has {
		t.Fatal("transfer should not produce return data")
	}

	// Simulate must not commit state. Recipient should still be absent.
	if _, ok, _ := svm.Balance(recipient); ok {
		t.Fatal("simulate committed state: recipient balance exists")
	}

	// Post-accounts should show what the balances *would* look like.
	posts, err := out.PostAccounts()
	if err != nil {
		t.Fatalf("PostAccounts: %v", err)
	}
	if len(posts) == 0 {
		t.Fatal("expected post_accounts")
	}
	var sawPayer, sawRecipient bool
	for _, p := range posts {
		defer p.Account.Close()
		switch {
		case p.Address.Equals(payer):
			sawPayer = true
			if p.Account.Lamports() >= fund {
				t.Fatalf("simulated payer lamports %d should be < fund %d",
					p.Account.Lamports(), fund)
			}
		case p.Address.Equals(recipient):
			sawRecipient = true
			if p.Account.Lamports() != xfer {
				t.Fatalf("simulated recipient lamports = %d, want %d",
					p.Account.Lamports(), xfer)
			}
		}
	}
	if !sawPayer || !sawRecipient {
		t.Fatalf("post_accounts missing payer or recipient: %d entries", len(posts))
	}
}

func TestSimulateLegacyTransactionFail(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	const fund = uint64(1_000_000_000)
	_, _, _, txBytes := buildSignedTransfer(t, svm, fund, fund*10)

	out, err := svm.SimulateLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SimulateLegacyTransaction: %v", err)
	}
	defer out.Close()

	if out.IsOk() {
		t.Fatal("expected failed simulation")
	}
	if out.Error() == "" {
		t.Fatal("expected non-empty error")
	}
	posts, err := out.PostAccounts()
	if err != nil {
		t.Fatalf("PostAccounts: %v", err)
	}
	if len(posts) != 0 {
		t.Fatal("failed simulation should not populate post_accounts")
	}
}

func TestSendVersionedTransactionAcceptsLegacyBytes(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	const fund = uint64(5 * 1_000_000_000)
	const xfer = uint64(2_500_000)
	_, payer, recipient, txBytes := buildSignedTransfer(t, svm, fund, xfer)

	out, err := svm.SendVersionedTransaction(txBytes)
	if err != nil {
		t.Fatalf("SendVersionedTransaction: %v", err)
	}
	defer out.Close()

	if !out.IsOk() {
		t.Fatalf("tx failed: %s", out.Error())
	}

	rb, _, _ := svm.Balance(recipient)
	if rb != xfer {
		t.Fatalf("recipient balance = %d, want %d", rb, xfer)
	}
	if _, ok, _ := svm.Balance(payer); !ok {
		t.Fatal("payer missing after send")
	}
}

func TestSimulateVersionedTransactionAcceptsLegacyBytes(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	const fund = uint64(5 * 1_000_000_000)
	const xfer = uint64(2_500_000)
	_, _, _, txBytes := buildSignedTransfer(t, svm, fund, xfer)

	out, err := svm.SimulateVersionedTransaction(txBytes)
	if err != nil {
		t.Fatalf("SimulateVersionedTransaction: %v", err)
	}
	defer out.Close()

	if !out.IsOk() {
		t.Fatalf("simulate failed: %s", out.Error())
	}
	posts, err := out.PostAccounts()
	if err != nil {
		t.Fatalf("PostAccounts: %v", err)
	}
	if len(posts) == 0 {
		t.Fatal("expected post_accounts")
	}
	for _, p := range posts {
		p.Account.Close()
	}
}

func TestGetTransaction(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	const fund = uint64(5 * 1_000_000_000)
	const xfer = uint64(1_500_000)
	_, _, _, txBytes := buildSignedTransfer(t, svm, fund, xfer)

	sent, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SendLegacyTransaction: %v", err)
	}
	if !sent.IsOk() {
		sent.Close()
		t.Fatalf("send failed: %s", sent.Error())
	}
	sig := sent.Signature()
	sent.Close()

	got := svm.GetTransaction(sig)
	if got == nil {
		t.Fatal("GetTransaction returned nil for a sent tx")
	}
	defer got.Close()

	if !got.IsOk() {
		t.Fatalf("looked-up tx reports failure: %s", got.Error())
	}
	if !got.Signature().Equals(sig) {
		t.Fatalf("signature mismatch: got %s, want %s", got.Signature(), sig)
	}

	// Unknown signature -> nil.
	var unknown solana.Signature
	if _, err := rand.Read(unknown[:]); err != nil {
		t.Fatalf("rand: %v", err)
	}
	if svm.GetTransaction(unknown) != nil {
		t.Fatal("expected nil for unknown signature")
	}
}

func TestInnerInstructionsFromTransfer(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	const fund = uint64(5 * 1_000_000_000)
	const xfer = uint64(1_000_000)
	_, _, _, txBytes := buildSignedTransfer(t, svm, fund, xfer)

	out, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SendLegacyTransaction: %v", err)
	}
	defer out.Close()
	if !out.IsOk() {
		t.Fatalf("tx failed: %s", out.Error())
	}

	inners := out.InnerInstructions()
	// A simple system transfer issues no CPIs. The outer list either contains
	// one empty entry (matching the 1 top-level ix) or is entirely empty,
	// depending on runtime behavior; either shape has 0 total inners.
	totalInner := 0
	for i, list := range inners {
		totalInner += len(list)
		for j, ix := range list {
			// Defensive: even if populated, fields shouldn't panic on access.
			_ = ix.Instruction.ProgramIDIndex
			_ = ix.Instruction.Accounts
			_ = ix.Instruction.Data
			_ = ix.StackHeight
			t.Logf("inner[%d][%d]: pid=%d stack=%d acc=%d data=%d",
				i, j, ix.Instruction.ProgramIDIndex, ix.StackHeight,
				len(ix.Instruction.Accounts), len(ix.Instruction.Data))
		}
	}
	if totalInner != 0 {
		t.Fatalf("plain transfer should have 0 inner instructions, got %d", totalInner)
	}
}

func TestComputeBudgetGetUnsetThenSet(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	// No custom budget set initially.
	_, has, err := svm.ComputeBudget()
	if err != nil {
		t.Fatalf("ComputeBudget: %v", err)
	}
	if has {
		t.Fatal("expected no custom compute budget on a fresh LiteSVM")
	}

	// Craft a budget whose distinctive fields we can round-trip.
	want := ComputeBudget{
		ComputeUnitLimit:                      2_000_000,
		Log64Units:                            999,
		CreateProgramAddressUnits:             1500,
		InvokeUnits:                           3000,
		MaxInstructionStackDepth:              7,
		MaxInstructionTraceLength:             128,
		Sha256BaseCost:                        10,
		Sha256ByteCost:                        1,
		Sha256MaxSlices:                       40,
		MaxCallDepth:                          64,
		StackFrameSize:                        4096,
		LogPubkeyUnits:                        110,
		CpiBytesPerUnit:                       1,
		SysvarBaseCost:                        100,
		Secp256k1RecoverCost:                  25_000,
		SyscallBaseCost:                       100,
		Curve25519EdwardsValidatePointCost:    160,
		Curve25519EdwardsAddCost:              450,
		Curve25519EdwardsSubtractCost:         450,
		Curve25519EdwardsMultiplyCost:         2_200,
		Curve25519EdwardsMsmBaseCost:          2_300,
		Curve25519EdwardsMsmIncrementalCost:   2_400,
		Curve25519RistrettoValidatePointCost:  170,
		Curve25519RistrettoAddCost:            520,
		Curve25519RistrettoSubtractCost:       520,
		Curve25519RistrettoMultiplyCost:       2_100,
		Curve25519RistrettoMsmBaseCost:        2_200,
		Curve25519RistrettoMsmIncrementalCost: 2_300,
		HeapSize:                              32 * 1024,
		HeapCost:                              8,
		MemOpBaseCost:                         10,
		AltBn128AdditionCost:                  334,
		AltBn128MultiplicationCost:            3_840,
		AltBn128PairingOnePairCostFirst:       36_364,
		AltBn128PairingOnePairCostOther:       12_121,
		BigModularExponentiationBaseCost:      190,
		BigModularExponentiationCostDivisor:   2,
		PoseidonCostCoefficientA:              61,
		PoseidonCostCoefficientC:              542,
		GetRemainingComputeUnitsCost:          100,
		AltBn128G1Compress:                    30,
		AltBn128G1Decompress:                  398,
		AltBn128G2Compress:                    86,
		AltBn128G2Decompress:                  13_610,
	}
	if err := svm.SetComputeBudget(want); err != nil {
		t.Fatalf("SetComputeBudget: %v", err)
	}
	got, has, err := svm.ComputeBudget()
	if err != nil {
		t.Fatalf("ComputeBudget: %v", err)
	}
	if !has {
		t.Fatal("expected custom budget after SetComputeBudget")
	}
	if got != want {
		t.Fatalf("compute_budget mismatch\n got=%+v\nwant=%+v", got, want)
	}
}

func TestFeatureSetBasics(t *testing.T) {
	fs, err := NewFeatureSet()
	if err != nil {
		t.Fatalf("NewFeatureSet: %v", err)
	}
	defer fs.Close()

	// A brand-new default feature set has inactives but no actives.
	if fs.ActiveCount() != 0 {
		t.Fatalf("expected 0 active on fresh feature set, got %d", fs.ActiveCount())
	}
	if fs.InactiveCount() == 0 {
		t.Fatal("expected >0 inactive features on fresh feature set")
	}

	// Pick one of the inactive features to flip on.
	inactive := fs.InactiveFeatures()
	if len(inactive) == 0 {
		t.Fatal("InactiveFeatures returned empty")
	}
	target := inactive[0]

	if fs.IsActive(target) {
		t.Fatalf("feature %s should start inactive", target)
	}
	if err := fs.Activate(target, 99); err != nil {
		t.Fatalf("Activate: %v", err)
	}
	if !fs.IsActive(target) {
		t.Fatalf("feature %s should be active after Activate", target)
	}
	if slot, ok := fs.ActivatedSlot(target); !ok || slot != 99 {
		t.Fatalf("ActivatedSlot = (%d, %v), want (99, true)", slot, ok)
	}

	if err := fs.Deactivate(target); err != nil {
		t.Fatalf("Deactivate: %v", err)
	}
	if fs.IsActive(target) {
		t.Fatalf("feature %s should be inactive after Deactivate", target)
	}
}

func TestFeatureSetAllEnabled(t *testing.T) {
	fs, err := NewFeatureSetAllEnabled()
	if err != nil {
		t.Fatalf("NewFeatureSetAllEnabled: %v", err)
	}
	defer fs.Close()

	if fs.ActiveCount() == 0 {
		t.Fatal("all_enabled feature set should have >0 active features")
	}
	if fs.InactiveCount() != 0 {
		t.Fatalf("all_enabled feature set should have 0 inactive, got %d",
			fs.InactiveCount())
	}
}

func TestSetFeatureSetOnSVM(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	fs, err := NewFeatureSetAllEnabled()
	if err != nil {
		t.Fatalf("NewFeatureSetAllEnabled: %v", err)
	}
	defer fs.Close()

	if err := svm.SetFeatureSet(fs); err != nil {
		t.Fatalf("SetFeatureSet: %v", err)
	}
	// Verify the SVM still works for a real tx after swapping the feature set.
	// Some all_enabled features enforce rent-exemption on transfers, so the
	// amount has to clear the minimum balance for a 0-byte account.
	const fund = uint64(1_000_000_000)
	xfer, err := svm.MinimumBalanceForRentExemption(0)
	if err != nil {
		t.Fatalf("MinimumBalanceForRentExemption: %v", err)
	}
	xfer += 1_000
	_, _, recipient, txBytes := buildSignedTransfer(t, svm, fund, xfer)
	out, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SendLegacyTransaction: %v", err)
	}
	defer out.Close()
	if !out.IsOk() {
		t.Fatalf("tx failed after SetFeatureSet: %s", out.Error())
	}
	if rb, _, _ := svm.Balance(recipient); rb != xfer {
		t.Fatalf("recipient balance = %d, want %d", rb, xfer)
	}
}

func TestEpochRewardsRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	var blockhash solana.Hash
	for i := range blockhash {
		blockhash[i] = byte(i + 1)
	}
	// A value that overflows u64 so we exercise both halves.
	want := EpochRewards{
		DistributionStartingBlockHeight: 1000,
		NumPartitions:                   4,
		ParentBlockhash:                 blockhash,
		TotalPointsLo:                   0xDEAD_BEEF_CAFE_F00D,
		TotalPointsHi:                   0x1234_5678_9ABC_DEF0,
		TotalRewards:                    9999,
		DistributedRewards:              123,
		Active:                          true,
	}
	if err := svm.SetEpochRewards(want); err != nil {
		t.Fatalf("SetEpochRewards: %v", err)
	}
	got, err := svm.EpochRewards()
	if err != nil {
		t.Fatalf("EpochRewards: %v", err)
	}
	if got != want {
		t.Fatalf("epoch_rewards mismatch\n got=%+v\nwant=%+v", got, want)
	}
}

func TestSlotHashesRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	mkhash := func(seed byte) solana.Hash {
		var h solana.Hash
		for i := range h {
			h[i] = seed + byte(i)
		}
		return h
	}
	// SlotHashes is kept in descending-slot order internally, so supply in
	// that order to make equality assertions stable.
	want := []SlotHash{
		{Slot: 42, Hash: mkhash(0x40)},
		{Slot: 41, Hash: mkhash(0x30)},
		{Slot: 40, Hash: mkhash(0x20)},
	}
	if err := svm.SetSlotHashes(want); err != nil {
		t.Fatalf("SetSlotHashes: %v", err)
	}
	got, err := svm.SlotHashes()
	if err != nil {
		t.Fatalf("SlotHashes: %v", err)
	}
	if len(got) != len(want) {
		t.Fatalf("slot_hashes len = %d, want %d", len(got), len(want))
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("slot_hashes[%d] = %+v, want %+v", i, got[i], want[i])
		}
	}
}

func TestStakeHistoryRoundtrip(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	items := []StakeHistoryItem{
		{Epoch: 100, Effective: 1_000, Activating: 200, Deactivating: 50},
		{Epoch: 101, Effective: 2_000, Activating: 100, Deactivating: 25},
	}
	if err := svm.SetStakeHistory(items); err != nil {
		t.Fatalf("SetStakeHistory: %v", err)
	}
	got, err := svm.StakeHistory()
	if err != nil {
		t.Fatalf("StakeHistory: %v", err)
	}
	// StakeHistory orders entries descending by epoch internally.
	seen := map[uint64]StakeHistoryItem{}
	for _, it := range got {
		seen[it.Epoch] = it
	}
	for _, want := range items {
		g, ok := seen[want.Epoch]
		if !ok {
			t.Fatalf("missing entry for epoch %d", want.Epoch)
		}
		if g != want {
			t.Fatalf("epoch %d: got %+v, want %+v", want.Epoch, g, want)
		}
	}
}

func TestSlotHistoryHandle(t *testing.T) {
	sh, err := NewSlotHistory()
	if err != nil {
		t.Fatalf("NewSlotHistory: %v", err)
	}
	defer sh.Close()

	// Default: next_slot 1, only slot 0 is recorded.
	if got := sh.NextSlot(); got != 1 {
		t.Fatalf("next_slot = %d, want 1", got)
	}
	if c := sh.Check(0); c != SlotHistoryFound {
		t.Fatalf("Check(0) = %d, want Found", c)
	}
	// An un-added slot below next_slot is NotFound.
	if c := sh.Check(1); c != SlotHistoryNotFound && c != SlotHistoryFuture {
		t.Fatalf("Check(1) = %d, want NotFound or Future", c)
	}

	sh.Add(100)
	sh.Add(200)
	if c := sh.Check(100); c != SlotHistoryFound {
		t.Fatalf("Check(100) = %d, want Found", c)
	}
	if c := sh.Check(101); c != SlotHistoryNotFound {
		t.Fatalf("Check(101) = %d, want NotFound", c)
	}
	if c := sh.Check(sh.NextSlot() + 1); c != SlotHistoryFuture {
		t.Fatalf("Check(future) = %d, want Future", c)
	}
}

func TestSlotHistoryGetSetFromSVM(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	sh, err := svm.SlotHistory()
	if err != nil {
		t.Fatalf("SlotHistory: %v", err)
	}
	defer sh.Close()

	// Round-trip via set + re-get.
	sh.Add(500)
	sh.Add(600)
	if err := sh.SetNextSlot(601); err != nil {
		t.Fatalf("SetNextSlot: %v", err)
	}
	if err := svm.SetSlotHistory(sh); err != nil {
		t.Fatalf("SetSlotHistory: %v", err)
	}
	got, err := svm.SlotHistory()
	if err != nil {
		t.Fatalf("SlotHistory (roundtrip): %v", err)
	}
	defer got.Close()
	if c := got.Check(500); c != SlotHistoryFound {
		t.Fatalf("after roundtrip Check(500) = %d, want Found", c)
	}
	if got.NextSlot() != 601 {
		t.Fatalf("after roundtrip next_slot = %d, want 601", got.NextSlot())
	}
}

func TestSigverifyToggle(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	on, err := svm.Sigverify()
	if err != nil {
		t.Fatalf("Sigverify: %v", err)
	}
	if !on {
		t.Fatal("expected sigverify enabled by default")
	}
	if err := svm.SetSigverify(false); err != nil {
		t.Fatalf("SetSigverify: %v", err)
	}
	if on, _ = svm.Sigverify(); on {
		t.Fatal("expected sigverify disabled after SetSigverify(false)")
	}
}

// With sigverify disabled, a transaction with garbage signatures should still
// be accepted (as long as everything else is valid).
func TestSigverifyDisabledAcceptsBadSig(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	const fund = uint64(5 * 1_000_000_000)
	const xfer = uint64(1_000_000)
	_, _, _, txBytes := buildSignedTransfer(t, svm, fund, xfer)

	// Scribble over the signature region (first bincode field is u64 sig
	// count, then each 64-byte signature). The compact-u16 len byte is 1,
	// so offsets 1..65 hold the signature.
	txBytes[1] ^= 0xFF

	// Sigverify on: must reject.
	out, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("SendLegacyTransaction: %v", err)
	}
	if out.IsOk() {
		out.Close()
		t.Fatal("expected sig failure with sigverify enabled")
	}
	out.Close()

	// Disable sigverify and try again with a fresh blockhash to avoid
	// history dedup.
	if err := svm.SetSigverify(false); err != nil {
		t.Fatalf("SetSigverify: %v", err)
	}
	if err := svm.ExpireBlockhash(); err != nil {
		t.Fatalf("ExpireBlockhash: %v", err)
	}
	_, _, _, fresh := buildSignedTransfer(t, svm, fund, xfer)
	fresh[1] ^= 0xFF // corrupt the signature again
	out, err = svm.SendLegacyTransaction(fresh)
	if err != nil {
		t.Fatalf("SendLegacyTransaction: %v", err)
	}
	defer out.Close()
	if !out.IsOk() {
		t.Fatalf("expected success with sigverify off, got: %s", out.Error())
	}
}

func TestTransactionHistoryDisabled(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	// Capacity 0 disables dedup.
	if err := svm.SetTransactionHistory(0); err != nil {
		t.Fatalf("SetTransactionHistory: %v", err)
	}

	const fund = uint64(5 * 1_000_000_000)
	const xfer = uint64(1_000_000)
	_, _, _, txBytes := buildSignedTransfer(t, svm, fund, xfer)

	first, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("first send: %v", err)
	}
	defer first.Close()
	if !first.IsOk() {
		t.Fatalf("first send failed: %s", first.Error())
	}

	second, err := svm.SendLegacyTransaction(txBytes)
	if err != nil {
		t.Fatalf("second send: %v", err)
	}
	defer second.Close()
	if !second.IsOk() {
		t.Fatalf("expected replay to succeed with history disabled, got: %s", second.Error())
	}

	// With history disabled GetTransaction returns nil.
	if got := svm.GetTransaction(first.Signature()); got != nil {
		got.Close()
		t.Fatal("GetTransaction should be nil when history is disabled")
	}
}

func TestSetLamports(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	// Setting the airdrop pool smaller than a requested airdrop should
	// cause Airdrop to fail.
	if err := svm.SetLamports(1); err != nil {
		t.Fatalf("SetLamports: %v", err)
	}
	if err := svm.Airdrop(randPubkey(t), 1_000_000_000); err == nil {
		t.Fatal("expected airdrop failure against 1-lamport pool")
	}
}

func TestSetLogBytesLimitAndSetters(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	// Just verify the no-op setters return nil; they reconfigure state
	// that's not directly observable without program invocation.
	if err := svm.SetLogBytesLimit(128); err != nil {
		t.Fatalf("SetLogBytesLimit: %v", err)
	}
	if err := svm.SetLogBytesLimit(-1); err != nil { // unlimited
		t.Fatalf("SetLogBytesLimit(-1): %v", err)
	}
	if err := svm.SetBlockhashCheck(false); err != nil {
		t.Fatalf("SetBlockhashCheck: %v", err)
	}
	if err := svm.SetSysvars(); err != nil {
		t.Fatalf("SetSysvars: %v", err)
	}
	if err := svm.SetBuiltins(); err != nil {
		t.Fatalf("SetBuiltins: %v", err)
	}
	if err := svm.SetDefaultPrograms(); err != nil {
		t.Fatalf("SetDefaultPrograms: %v", err)
	}
	if err := svm.SetPrecompiles(); err != nil {
		t.Fatalf("SetPrecompiles: %v", err)
	}
}

func TestWithNativeMints(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	// SetDefaultPrograms loads SPL Token; WithNativeMints then seeds the
	// WSOL mint account.
	if err := svm.SetDefaultPrograms(); err != nil {
		t.Fatalf("SetDefaultPrograms: %v", err)
	}
	if err := svm.WithNativeMints(); err != nil {
		t.Fatalf("WithNativeMints: %v", err)
	}

	wsol := solana.MustPublicKeyFromBase58("So11111111111111111111111111111111111111112")
	a := svm.GetAccount(wsol)
	if a == nil {
		t.Fatal("expected WSOL mint account after WithNativeMints")
	}
	defer a.Close()

	if len(a.Data()) != 82 {
		t.Fatalf("mint data len = %d, want 82", len(a.Data()))
	}
	splToken := solana.MustPublicKeyFromBase58("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
	if !a.Owner().Equals(splToken) {
		t.Fatalf("mint owner = %s, want %s", a.Owner(), splToken)
	}
}

func TestAddProgramWithLoader(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	dir := programBytesDir(t)
	bytes, err := os.ReadFile(filepath.Join(dir, "spl_example_logging.so"))
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}

	// BPFLoaderUpgradeable111... is the standard upgradeable loader.
	loader := solana.MustPublicKeyFromBase58("BPFLoaderUpgradeab1e11111111111111111111111")
	programID := randPubkey(t)
	if err := svm.AddProgramWithLoader(programID, bytes, loader); err != nil {
		t.Fatalf("AddProgramWithLoader: %v", err)
	}
	acct := svm.GetAccount(programID)
	if acct == nil {
		t.Fatal("program account missing after AddProgramWithLoader")
	}
	defer acct.Close()
	if !acct.Executable() {
		t.Fatal("program account should be executable")
	}
	if !acct.Owner().Equals(loader) {
		t.Fatalf("owner = %s, want %s", acct.Owner(), loader)
	}
}

func TestAddProgramRejectsGarbage(t *testing.T) {
	svm, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svm.Close()

	err = svm.AddProgram(randPubkey(t), []byte{0x00, 0x01, 0x02})
	if err == nil {
		t.Fatal("expected error for garbage program bytes")
	}
	if !strings.Contains(err.Error(), "add_program") {
		t.Fatalf("expected add_program error, got: %v", err)
	}
}
