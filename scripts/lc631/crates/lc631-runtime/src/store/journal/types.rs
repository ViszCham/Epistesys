use lc631_substrate::codec::CodecError;
use lc631_substrate::Digest32;

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalRecordKind {
    ActionStarted = 1,
    ActionObserved = 2,
    CandidatePrepared = 3,
    SendAttempted = 4,
    SinkObserved = 5,
    Evidence = 6,
    Checkpoint = 7,
}

impl JournalRecordKind {
    pub(super) const fn wire_tag(self) -> u16 {
        match self {
            Self::ActionStarted => 1,
            Self::ActionObserved => 2,
            Self::CandidatePrepared => 3,
            Self::SendAttempted => 4,
            Self::SinkObserved => 5,
            Self::Evidence => 6,
            Self::Checkpoint => 7,
        }
    }

    pub(super) fn from_wire_tag(tag: u16) -> Result<Self, JournalError> {
        match tag {
            1 => Ok(Self::ActionStarted),
            2 => Ok(Self::ActionObserved),
            3 => Ok(Self::CandidatePrepared),
            4 => Ok(Self::SendAttempted),
            5 => Ok(Self::SinkObserved),
            6 => Ok(Self::Evidence),
            7 => Ok(Self::Checkpoint),
            _ => Err(JournalError::UnsupportedRecordKind),
        }
    }

    const fn retention_decision(self) -> RetentionDecision {
        match self {
            Self::Evidence => RetentionDecision::RetainEvidenceWithoutVerifiedArchive,
            Self::ActionStarted
            | Self::ActionObserved
            | Self::CandidatePrepared
            | Self::SendAttempted
            | Self::SinkObserved
            | Self::Checkpoint => RetentionDecision::RetainPolicyUnavailable,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalError {
    Frame(CodecError),
    InputTooLarge,
    EmptySegment,
    TooManyRecords,
    SegmentFull,
    RecordTooLarge,
    /// The all-zero digest is reserved for an absent object reference.
    InvalidObjectDigest,
    TornRecord,
    InvalidRecordMagic,
    UnsupportedRecordVersion,
    UnsupportedRecordKind,
    UnknownRecordFlags,
    NonzeroReserved,
    ZeroSequence,
    NonMonotonicSequence,
    SequenceGap,
    SequenceOverflow,
    PreviousDigestMismatch,
    RecordDigestMismatch,
    StalePredecessor,
    InvalidFrameShape,
    RecordCountMismatch,
    AllocationFailed,
}

impl From<CodecError> for JournalError {
    fn from(error: CodecError) -> Self {
        Self::Frame(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionDecision {
    RetainEvidenceWithoutVerifiedArchive,
    RetainPolicyUnavailable,
}

/// Process-local continuation cursor. Fields are private: a cursor comes from
/// genesis or a structurally validated segment, never from JSON deserialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalCursor {
    pub(super) sequence: u64,
    pub(super) record_digest: Digest32,
}

impl JournalCursor {
    pub const fn genesis() -> Self {
        Self {
            sequence: 0,
            record_digest: Digest32::from_bytes([0; 32]),
        }
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn record_digest(self) -> Digest32 {
        self.record_digest
    }
}

/// Checked record view; payload bytes remain borrowed from the admitted frame.
#[derive(Clone, Copy, Debug)]
pub struct JournalRecordView<'a> {
    pub(super) sequence: u64,
    pub(super) kind: JournalRecordKind,
    pub(super) object_digest: Digest32,
    pub(super) previous_digest: Digest32,
    pub(super) record_digest: Digest32,
    pub(super) payload: &'a [u8],
    /// Offset relative to the opaque records-section payload, not a file.
    pub(super) payload_offset: usize,
}

impl<'a> JournalRecordView<'a> {
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> JournalRecordKind {
        self.kind
    }

    pub const fn object_digest(self) -> Digest32 {
        self.object_digest
    }

    pub const fn previous_digest(self) -> Digest32 {
        self.previous_digest
    }

    pub const fn digest(self) -> Digest32 {
        self.record_digest
    }

    pub const fn payload(self) -> &'a [u8] {
        self.payload
    }

    pub const fn payload_offset(self) -> usize {
        self.payload_offset
    }

    pub const fn retention_decision(self) -> RetentionDecision {
        self.kind.retention_decision()
    }
}

/// A head derived from current bytes is a local consistency snapshot, not an
/// external monotonic anchor or authenticity proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalHead {
    pub(super) segment_digest: Digest32,
    pub(super) sequence: u64,
    pub(super) record_digest: Digest32,
}

impl JournalHead {
    pub const fn segment_digest(self) -> Digest32 {
        self.segment_digest
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn record_digest(self) -> Digest32 {
        self.record_digest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalHeadCheck {
    Unanchored,
    LocalMatchUnanchored,
    Stale,
}

/// Local index rebuilt from a validated frame. It proves byte consistency only.
#[derive(Debug)]
pub struct JournalSegmentView<'a> {
    pub(super) frame_digest: Digest32,
    pub(super) first_previous: JournalCursor,
    pub(super) cursor: JournalCursor,
    pub(super) records: Vec<JournalRecordView<'a>>,
}

impl<'a> JournalSegmentView<'a> {
    pub const fn frame_digest(&self) -> Digest32 {
        self.frame_digest
    }

    pub fn records(&self) -> &[JournalRecordView<'a>] {
        &self.records
    }

    pub const fn first_previous_cursor(&self) -> JournalCursor {
        self.first_previous
    }

    /// Byte-chain check only. It neither authenticates the prior cursor nor
    /// detects joint rollback of data and its local head.
    pub fn verify_predecessor(&self, previous: JournalCursor) -> Result<(), JournalError> {
        if self.first_previous == previous {
            Ok(())
        } else {
            Err(JournalError::StalePredecessor)
        }
    }

    pub const fn cursor(&self) -> JournalCursor {
        self.cursor
    }

    pub fn local_head(&self) -> JournalHead {
        JournalHead {
            segment_digest: self.frame_digest,
            sequence: self.cursor.sequence,
            record_digest: self.cursor.record_digest,
        }
    }

    pub fn check_local_head(&self, supplied: Option<&JournalHead>) -> JournalHeadCheck {
        let Some(supplied) = supplied else {
            return JournalHeadCheck::Unanchored;
        };
        if *supplied == self.local_head() {
            JournalHeadCheck::LocalMatchUnanchored
        } else {
            JournalHeadCheck::Stale
        }
    }

    pub fn reclaimable_indices(&self) -> Vec<usize> {
        // This primitive intentionally has no compactor or retention authority.
        Vec::new()
    }
}
