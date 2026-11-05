//! Narrow public facade for bounded checkpoint-journal operations.
//!
//! Returned heads are local byte-continuity summaries only. The explicit
//! legacy migration entrypoint writes through an owner after whole-chain byte
//! verification, but does not load files, refresh current authority, restore
//! completion, provide an external rollback anchor, or authenticate evidence.

pub use crate::store::journal::{
    decode_journal_index, decode_segment, inspect_bundle_root, inspect_chain,
    inspect_checkpoint_chain_resume, inspect_legacy_json_read_only,
    inspect_persisted_checkpoint_resume, inspect_persisted_segment, persist_segment,
    verify_and_persist_legacy_chain, BundleInspectionError, BundleLiveError,
    CheckpointChainResumeDecision, CheckpointChainResumeHold, JournalBundleInspection,
    JournalBundleLiveSession, JournalBundleReceipt, JournalChainDecision, JournalChainHeadState,
    JournalChainHoldReason, JournalChainLimitError, JournalChainLimits, JournalChainSummary,
    JournalCursor, JournalError, JournalHead, JournalHeadCheck, JournalIndex, JournalIndexEntry,
    JournalIndexError, JournalLiveError, JournalLiveSession, JournalPersistenceError,
    JournalRecordKind, JournalRecordView, JournalSegmentBuilder, JournalSegmentView,
    LegacyChainPersistenceDecision, LegacyChainPersistenceError, LegacyHoldReason,
    LegacyJsonMigration, LiveJournalReceipt, PersistedCheckpointResume,
    PersistedCheckpointResumeHold, PersistedJournalSegment, PersistedSegmentHold,
    PersistedSegmentInspection, RetentionDecision, JOURNAL_INDEX_ENTRY_BYTES,
    JOURNAL_INDEX_HEADER_BYTES, JOURNAL_INDEX_SECTION_TAG, JOURNAL_LOCAL_HEAD_FRAME_BYTES,
    JOURNAL_LOCAL_HEAD_PAYLOAD_BYTES, JOURNAL_LOCAL_HEAD_SECTION_TAG, JOURNAL_RECORDS_SECTION_TAG,
    MAX_JOURNAL_INDEX_BYTES, MAX_JOURNAL_INDEX_ENTRIES, MAX_JOURNAL_INDEX_FRAME_BYTES,
    MAX_JOURNAL_RECORD_BYTES, MAX_JOURNAL_SEGMENT_BYTES, RECORD_HEADER_BYTES,
};
