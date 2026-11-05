//! Historical local input archives, never serialized native/current authority.
#![forbid(unsafe_code)]
mod codec;
mod context;
mod execute;
mod store;
mod types;
pub use codec::{capture_current, ScopedCapture};
pub use execute::{with_fresh_resume, with_replay_input};
pub use store::{
    persist_current, validate_store_root, CanonicalArtifactReader, OwnedReplayArtifact,
};
pub use types::{
    ArtifactKey, HistoricalView, ReplayError, ReplayFailureKind, ReplayGoal, ReplayPostWriteCause,
    ReplayProfile,
};
pub const MAX_REPLAY_BYTES: usize = 96 * 1024;
pub(super) const META_TAG: u16 = 0x0240;
pub(super) const TEXT_TAG: u16 = 0x0241;
pub(super) const META_BYTES: usize = 240;
pub(super) const FILE: &str = "segment-00000000000000000001.bin";
#[cfg(test)]
mod context_tests;
