//! Bounded append-only journal primitives over the shared EPITLCX1 codec.
//!
//! The facade keeps four method worlds explicit: typed events/retention,
//! binary record validation, segment/index handling, and legacy migration.
//! A single owner-backed segment write bridge is present; persisted index/head
//! recovery, normal resume and external anchoring remain separate closures.
#![forbid(unsafe_code)]

#[path = "journal/bundle_head.rs"]
mod bundle_head;
#[path = "journal/bundle_inspection.rs"]
mod bundle_inspection;
#[path = "journal/bundle_live.rs"]
mod bundle_live;
#[path = "journal/bundle_validation.rs"]
mod bundle_validation;
#[path = "journal/chain.rs"]
mod chain;
#[path = "journal/checkpoint_chain_resume.rs"]
mod checkpoint_chain_resume;
#[path = "journal/index_codec.rs"]
mod index_codec;
#[path = "journal/legacy.rs"]
mod legacy;
#[path = "journal/live_session.rs"]
mod live_session;
#[path = "journal/migration.rs"]
mod migration;
#[path = "journal/persistence.rs"]
mod persistence;
#[path = "journal/record_codec.rs"]
mod record_codec;
#[path = "journal/recovery.rs"]
mod recovery;
#[path = "journal/segment.rs"]
mod segment;
#[path = "journal/types.rs"]
mod types;

// The public `runtime::journal` facade exports only bounded inspection and
// encoding types; it does not restore a completion or authorize a resume.
#[allow(unused_imports)]
pub use bundle_head::{
    JOURNAL_LOCAL_HEAD_FRAME_BYTES, JOURNAL_LOCAL_HEAD_PAYLOAD_BYTES,
    JOURNAL_LOCAL_HEAD_SECTION_TAG,
};
#[allow(unused_imports)]
pub use bundle_live::{BundleLiveError, JournalBundleLiveSession, JournalBundleReceipt};

pub use bundle_inspection::{inspect_bundle_root, BundleInspectionError, JournalBundleInspection};

#[allow(unused_imports)]
pub use chain::{
    inspect_chain, JournalChainDecision, JournalChainHeadState, JournalChainHoldReason,
    JournalChainLimitError, JournalChainLimits, JournalChainSummary,
};
#[allow(unused_imports)]
pub use checkpoint_chain_resume::{
    inspect_checkpoint_chain_resume, CheckpointChainResumeDecision, CheckpointChainResumeHold,
};
#[allow(unused_imports)]
pub use index_codec::{
    decode_journal_index, JournalIndex, JournalIndexEntry, JournalIndexError,
    JOURNAL_INDEX_ENTRY_BYTES, JOURNAL_INDEX_HEADER_BYTES, JOURNAL_INDEX_SECTION_TAG,
    MAX_JOURNAL_INDEX_BYTES, MAX_JOURNAL_INDEX_ENTRIES, MAX_JOURNAL_INDEX_FRAME_BYTES,
};
#[allow(unused_imports)]
pub use legacy::{inspect_legacy_json_read_only, LegacyHoldReason, LegacyJsonMigration};
#[allow(unused_imports)]
pub use live_session::{JournalLiveError, JournalLiveSession, LiveJournalReceipt};
#[allow(unused_imports)]
pub use migration::{
    verify_and_persist_legacy_chain, LegacyChainPersistenceDecision, LegacyChainPersistenceError,
};
#[allow(unused_imports)]
pub use persistence::{persist_segment, JournalPersistenceError, PersistedJournalSegment};
#[allow(unused_imports)]
pub use recovery::{
    inspect_persisted_checkpoint_resume, inspect_persisted_segment, PersistedCheckpointResume,
    PersistedCheckpointResumeHold, PersistedSegmentHold, PersistedSegmentInspection,
};
#[allow(unused_imports)]
pub use segment::{decode_segment, JournalSegmentBuilder};
#[allow(unused_imports)]
pub use types::{
    JournalCursor, JournalError, JournalHead, JournalHeadCheck, JournalRecordKind,
    JournalRecordView, JournalSegmentView, RetentionDecision,
};

const RECORD_MAGIC: &[u8; 8] = b"EJREC001";
const RECORD_VERSION: u16 = 1;
const RECORD_DIGEST_RANGE: std::ops::Range<usize> = 96..128;
const RECORD_DOMAIN: &[u8] = b"Epistesys/TLCX/JournalRecord/sha256/v1\0";
const SEGMENT_FRAME_OVERHEAD: usize = 64 + 32;
const MAX_RECORDS_PER_SEGMENT: usize = 16_384;

/// Maximum bytes for one logical record, including its 128-byte header.
pub const MAX_JOURNAL_RECORD_BYTES: usize = 1024 * 1024;
/// Maximum bytes for one EPITLCX1 segment, including frame header/directory.
pub const MAX_JOURNAL_SEGMENT_BYTES: usize = 4 * 1024 * 1024;
/// Fixed binary record header size. No native Rust layout is persisted.
pub const RECORD_HEADER_BYTES: usize = 128;
/// Optional EPITLCX1 extension tag carrying canonical journal-record bytes.
pub const JOURNAL_RECORDS_SECTION_TAG: u16 = 0x0200;
