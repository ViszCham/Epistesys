//! Read-only local bundle consistency, explicitly separate from execution resume.
use super::bundle_validation::validate_existing_bundle;
use super::{BundleLiveError, JournalChainHeadState, JournalChainLimits, JournalCursor};
use lc631_host::{HostJournalBundleReader, JournalBundleLimits};
use lc631_process_supervisor::ResourceReservationView;
use lc631_substrate::Digest32;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BundleInspectionError {
    EmptyInventory,
    Validation(BundleLiveError),
}

/// Copyable structural observations only; no restored permission or completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalBundleInspection {
    cursor: JournalCursor,
    head_frame_digest: Digest32,
    segment_count: usize,
    record_count: usize,
    segment_input_bytes: usize,
}
impl JournalBundleInspection {
    pub const fn cursor(self) -> JournalCursor {
        self.cursor
    }
    pub const fn head_frame_digest(self) -> Digest32 {
        self.head_frame_digest
    }
    pub const fn segment_count(self) -> usize {
        self.segment_count
    }
    pub const fn record_count(self) -> usize {
        self.record_count
    }
    pub const fn segment_input_bytes(self) -> usize {
        self.segment_input_bytes
    }
    pub const fn head_state(self) -> JournalChainHeadState {
        JournalChainHeadState::Unanchored
    }
    pub const fn claim_boundary(self) -> &'static str {
        "non-atomic local bundle byte consistency only; no source semantics, authentication, external anchor, rollback resistance, authority, completion, or delivery"
    }
}

/// Construct reader and validator from the same actual lease; callers cannot
/// attach a reader bound to a different reservation. No filesystem write path.
pub fn inspect_bundle_root<'a>(
    root: &Path,
    store_limits: JournalBundleLimits,
    resources: &'a ResourceReservationView<'a>,
    chain_limits: JournalChainLimits,
) -> Result<JournalBundleInspection, BundleInspectionError> {
    let mut reader = HostJournalBundleReader::open_existing(root, store_limits, resources)
        .map_err(|error| BundleInspectionError::Validation(BundleLiveError::Host(error)))?;
    // The captured inventory is already checked for complete ordered parts.
    // An empty snapshot has no decoder work and remains a Hold, never success.
    if reader
        .next_sequence()
        .map_err(|error| BundleInspectionError::Validation(BundleLiveError::Host(error)))?
        == 1
    {
        return Err(BundleInspectionError::EmptyInventory);
    }
    let state = validate_existing_bundle(&mut reader, resources, chain_limits)
        .map_err(BundleInspectionError::Validation)?;
    if state.segments == 0 {
        return Err(BundleInspectionError::EmptyInventory);
    }
    Ok(JournalBundleInspection {
        cursor: state.cursor,
        head_frame_digest: state.previous_head,
        segment_count: state.segments,
        record_count: state.total_records,
        segment_input_bytes: state.segment_bytes,
    })
}
