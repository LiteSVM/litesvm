//! The result of submitting a transaction while capturing its world.

use crate::Replay;

/// A submitted transaction together with the pre-transaction world captured
/// for deterministic local replay, mutation, and time travel.
pub struct CapturedTransaction {
    /// The landed transaction's signature, as base58.
    pub signature: String,
    /// The reconstructed pre-transaction world, ready to run locally.
    pub replay: Replay,
}

impl CapturedTransaction {
    /// Consume the capture, keeping only the replay.
    pub fn into_replay(self) -> Replay {
        self.replay
    }
}
