//! How faithfully a replay's world matches the chain: per-account
//! provenance and the fidelity certificate built from it.

use serde::Serialize;

/// How faithful a replay's starting state is to the transaction's real slot —
/// the honest label on every replay, so a convincing-but-drifted run is never
/// mistaken for an exact one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Fidelity {
    /// Current-state replay ([`Scope::replay`](crate::Scope::replay)) — no historical anchoring.
    Current,
    /// Balances rewound to their pre-transaction values from the transaction's
    /// own metadata, clock set to the transaction's slot; account *data* is still
    /// current-state. Free — no archive needed.
    Reconstructed {
        /// The transaction's own slot, that the clock is anchored to.
        slot: u64,
    },
    /// Every account and program ELF loaded at the true slot from an archive.
    Exact {
        /// The transaction's own slot, that state was loaded at.
        slot: u64,
    },
}

impl Fidelity {
    /// A short human label: `current`, `reconstructed@<slot>`, `exact@<slot>`.
    pub fn label(&self) -> String {
        match self {
            Fidelity::Current => "current".to_string(),
            Fidelity::Reconstructed { slot } => format!("reconstructed@{slot}"),
            Fidelity::Exact { slot } => format!("exact@{slot}"),
        }
    }
}

/// A reconstructed account's raw state — its data bytes, lamports, and owner.
#[derive(Debug, Clone, Serialize)]
pub struct AccountState {
    /// The account's raw data bytes.
    pub data: Vec<u8>,
    /// The account's lamport balance.
    pub lamports: u64,
    /// The account's owner program, base58.
    pub owner: String,
}

/// Where an account's loaded bytes came from — the honest per-account source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Provenance {
    /// A frozen fixture — offline, content-addressed.
    Fixture,
    /// A historical archive, at the transaction's true slot.
    HistoricalArchive,
    /// Reconstructed from the transaction's own metadata (a pre-tx balance, or a
    /// since-closed account rebuilt from `preTokenBalances`).
    MetadataRewind,
    /// Loaded at current state from a normal RPC — may differ from the true slot.
    CurrentRpc,
}

/// One account's provenance within a replay.
#[derive(Debug, Clone, Serialize)]
pub struct AccountProvenance {
    /// The account address.
    pub address: String,
    /// Where its loaded bytes came from.
    pub source: Provenance,
    /// Whether it was loaded as an executable program (its ELF) rather than data.
    pub is_program: bool,
    /// A blake3 hash of the exact bytes loaded, for content addressing.
    pub hash: String,
}

/// An honest report of how faithful a replay's starting state is: the verdict,
/// where every account's bytes came from, which accounts may have drifted from
/// the transaction's true slot, and whether there is a recorded on-chain outcome
/// to check the replay against. Trust is a product feature — the crate should
/// never silently hand back a convincing but historically inaccurate replay.
#[derive(Debug, Clone, Serialize)]
pub struct FidelityCertificate {
    /// The overall fidelity tier of this replay.
    pub fidelity: Fidelity,
    /// The (possibly warped) clock the replay runs at, human-readable.
    pub clock: String,
    /// Provenance and hash of every loaded account.
    pub accounts: Vec<AccountProvenance>,
    /// Addresses whose bytes are current-state in a historical replay, so their
    /// data may differ from the true slot. Empty for an exact or current replay.
    pub drifted: Vec<String>,
    /// Whether a recorded on-chain outcome exists to verify the replay against.
    pub verifiable: bool,
}

impl FidelityCertificate {
    /// A one-line human summary of the certificate.
    pub fn summary(&self) -> String {
        let n = self.accounts.len();
        let verify = if self.verifiable {
            "verifiable against mainnet"
        } else {
            "no recorded outcome"
        };
        match self.drifted.len() {
            0 => format!(
                "{} · {n} accounts, none drifted · {verify}",
                self.fidelity.label()
            ),
            d => format!(
                "{} · {n} accounts, {d} may have drifted · {verify}",
                self.fidelity.label()
            ),
        }
    }
}
