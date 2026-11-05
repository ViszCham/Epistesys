use super::segment::JournalSegmentBuilder;
use super::types::{JournalError, JournalRecordKind};
use super::MAX_JOURNAL_SEGMENT_BYTES;
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

const LEGACY_V1_SCHEMA: &str = "epistesys-dgcl-checkpoint.v1";
const LEGACY_V2_SCHEMA: &str = "epistesys-dgcl-checkpoint.v2";
const LEGACY_RECORD_BYTES: usize = 16 * 1024;
const LEGACY_RECORD_COUNT: usize = 1_000;
const LEGACY_CHAIN_BYTES: usize = 3 * 1024 * 1024;

/// Legacy verification/migration states are not Authority or delivery claims.
#[derive(Clone, Eq, PartialEq)]
pub enum LegacyJsonMigration {
    Hold {
        reason: LegacyHoldReason,
    },
    /// Raw old JSON bytes are retained verbatim as checkpoint payloads. The
    /// new segment is an unanchored local conversion, not a trusted receipt.
    MigratedUnanchored {
        binary_segment: Vec<u8>,
        record_count: usize,
        final_legacy_digest: String,
    },
}

impl std::fmt::Debug for LegacyJsonMigration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hold { reason } => formatter
                .debug_struct("LegacyJsonMigration::Hold")
                .field("reason", reason)
                .finish(),
            Self::MigratedUnanchored {
                binary_segment,
                record_count,
                final_legacy_digest,
            } => formatter
                .debug_struct("LegacyJsonMigration::MigratedUnanchored")
                .field("binary_segment_bytes", &binary_segment.len())
                .field("record_count", record_count)
                .field("final_legacy_digest", final_legacy_digest)
                .finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyHoldReason {
    LegacyChainRequired,
    EmptyChain,
    InputLimitExceeded,
    UnsupportedLegacySchema,
    UnsupportedLegacyEncoding,
    DigestMismatch,
    SequenceMismatch,
    ParentDigestMismatch,
    ScopeMismatch,
    InvalidFields,
    InvalidEventState,
    MigrationBudgetExceeded,
}

impl LegacyJsonMigration {
    /// Verify an ordered in-memory list of old checkpoint records read-only,
    /// then encode each exact original record as a new `Checkpoint` payload.
    /// The caller owns file ordering/anchoring and must publish the result via
    /// a separate storage owner only after its own checks.
    pub fn verify_and_migrate(records: &[&[u8]]) -> Result<Self, JournalError> {
        if records.is_empty() {
            return Ok(hold(LegacyHoldReason::EmptyChain));
        }
        if records.len() > LEGACY_RECORD_COUNT {
            return Ok(hold(LegacyHoldReason::InputLimitExceeded));
        }
        let mut total_bytes = 0_usize;
        for record in records {
            if record.len() > LEGACY_RECORD_BYTES {
                return Ok(hold(LegacyHoldReason::InputLimitExceeded));
            }
            total_bytes = match total_bytes.checked_add(record.len()) {
                Some(total) if total <= LEGACY_CHAIN_BYTES => total,
                _ => return Ok(hold(LegacyHoldReason::InputLimitExceeded)),
            };
        }

        let mut parsed: Vec<ParsedLegacyRecord<'_>> = Vec::new();
        parsed
            .try_reserve_exact(records.len())
            .map_err(|_| JournalError::AllocationFailed)?;
        for (index, raw) in records.iter().enumerate() {
            let record = match parse_legacy_record(raw) {
                Ok(record) => record,
                Err(reason) => return Ok(hold(reason)),
            };
            if record.sequence != index as u64 {
                return Ok(hold(LegacyHoldReason::SequenceMismatch));
            }
            if index == 0 {
                if record.parent_digest.is_some() {
                    return Ok(hold(LegacyHoldReason::ParentDigestMismatch));
                }
            } else {
                let previous = &parsed[index - 1];
                if record.parent_digest != Some(previous.record_digest.as_str()) {
                    return Ok(hold(LegacyHoldReason::ParentDigestMismatch));
                }
                if record.schema != previous.schema {
                    return Ok(hold(LegacyHoldReason::UnsupportedLegacySchema));
                }
                if record.source_revision != previous.source_revision
                    || record.plan_digest != previous.plan_digest
                    || record.authority_revision != previous.authority_revision
                {
                    return Ok(hold(LegacyHoldReason::ScopeMismatch));
                }
            }
            parsed.push(record);
        }

        let mut builder = JournalSegmentBuilder::genesis();
        for (index, record) in records.iter().enumerate() {
            let sequence = match u64::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
            {
                Some(sequence) => sequence,
                None => return Ok(hold(LegacyHoldReason::MigrationBudgetExceeded)),
            };
            let object_digest = Digest32::from_bytes(Sha256::digest(record).into());
            match builder.append_at(
                sequence,
                JournalRecordKind::Checkpoint,
                object_digest,
                record,
            ) {
                Ok(()) => {}
                Err(
                    JournalError::SegmentFull
                    | JournalError::RecordTooLarge
                    | JournalError::TooManyRecords,
                ) => {
                    return Ok(hold(LegacyHoldReason::MigrationBudgetExceeded));
                }
                Err(error) => return Err(error),
            }
        }
        let binary_segment = match builder.finish() {
            Ok(segment) if segment.len() <= MAX_JOURNAL_SEGMENT_BYTES => segment,
            Ok(_) | Err(JournalError::SegmentFull) => {
                return Ok(hold(LegacyHoldReason::MigrationBudgetExceeded));
            }
            Err(error) => return Err(error),
        };
        let final_legacy_digest = parsed
            .last()
            .map(|record| record.record_digest.clone())
            .ok_or(JournalError::EmptySegment)?;
        Ok(Self::MigratedUnanchored {
            binary_segment,
            record_count: parsed.len(),
            final_legacy_digest,
        })
    }
}

/// A single record is not a chain and cannot be migrated safely without its
/// ordered parent history. Keep this compatibility probe at Hold.
pub fn inspect_legacy_json_read_only(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<LegacyJsonMigration, JournalError> {
    if bytes.len() > max_bytes {
        return Err(JournalError::InputTooLarge);
    }
    Ok(hold(LegacyHoldReason::LegacyChainRequired))
}

fn hold(reason: LegacyHoldReason) -> LegacyJsonMigration {
    LegacyJsonMigration::Hold { reason }
}

#[derive(Clone)]
struct ParsedLegacyRecord<'a> {
    schema: &'a str,
    sequence: u64,
    source_revision: &'a str,
    plan_digest: &'a str,
    authority_revision: &'a str,
    parent_digest: Option<&'a str>,
    record_digest: String,
}

fn parse_legacy_record(raw: &[u8]) -> Result<ParsedLegacyRecord<'_>, LegacyHoldReason> {
    let mut parser = CanonicalJsonCursor::new(raw);
    parser.expect(b"{\"body\":")?;
    let body_start = parser.offset;
    parser.expect(b"{\"schema_version\":")?;
    let schema = parser.string()?;
    let v1 = schema == LEGACY_V1_SCHEMA;
    let v2 = schema == LEGACY_V2_SCHEMA;
    if !v1 && !v2 {
        return Err(LegacyHoldReason::UnsupportedLegacySchema);
    }
    parser.expect(b",\"sequence\":")?;
    let sequence = parser.u64()?;
    parser.expect(b",\"source_revision\":")?;
    let source_revision = parser.string()?;
    parser.expect(b",\"plan_digest\":")?;
    let plan_digest = parser.string()?;
    parser.expect(b",\"authority_revision\":")?;
    let authority_revision = parser.string()?;
    parser.expect(b",\"action_key\":")?;
    let action_key = parser.string()?;
    if action_key.trim().is_empty()
        || action_key.len() > 1_024
        || action_key.chars().any(char::is_control)
    {
        return Err(LegacyHoldReason::InvalidFields);
    }
    let event_kind = if v2 {
        parser.expect(b",\"event_kind\":")?;
        Some(parse_event_kind(parser.string()?)?)
    } else {
        None
    };
    parser.expect(b",\"delivery\":")?;
    let delivery = parse_delivery(parser.string()?)?;
    parser.expect(b",\"result_digest\":")?;
    let result_digest = parser.optional_string()?;
    parser.expect(b",\"parent_digest\":")?;
    let parent_digest = parser.optional_string()?;
    parser.expect(b"}")?;
    let body_end = parser.offset;
    parser.expect(b",\"digest\":")?;
    let record_digest = parser.string()?.to_owned();
    parser.expect(b"}")?;
    parser.finish()?;

    if !is_digest(source_revision)
        || !is_digest(plan_digest)
        || authority_revision.is_empty()
        || authority_revision.len() > 512
        || authority_revision.chars().any(char::is_control)
        || !is_digest(&record_digest)
        || result_digest.is_some_and(|digest| !is_digest(digest))
        || parent_digest.is_some_and(|digest| !is_digest(digest))
        || (v1 && event_kind.is_some())
        || (v2 && event_kind.is_none())
    {
        return Err(LegacyHoldReason::InvalidFields);
    }
    if !event_result_is_valid(event_kind, delivery, result_digest) {
        return Err(LegacyHoldReason::InvalidEventState);
    }

    let body = std::str::from_utf8(&raw[body_start..body_end])
        .map_err(|_| LegacyHoldReason::UnsupportedLegacyEncoding)?;
    if lc631_core::stable_sha256(body) != record_digest {
        return Err(LegacyHoldReason::DigestMismatch);
    }
    Ok(ParsedLegacyRecord {
        schema,
        sequence,
        source_revision,
        plan_digest,
        authority_revision,
        parent_digest,
        record_digest,
    })
}

#[derive(Clone, Copy)]
enum LegacyDelivery {
    Unknown,
    ConfirmedFailure,
    ConfirmedSuccess,
}

#[derive(Clone, Copy)]
enum LegacyEvent {
    Unclassified,
    ActionStarted,
    ActionObserved,
    CandidatePrepared,
    CandidateSendAttempted,
    SinkDeliveryObserved,
}

fn parse_delivery(value: &str) -> Result<LegacyDelivery, LegacyHoldReason> {
    match value {
        "unknown_delivery" => Ok(LegacyDelivery::Unknown),
        "confirmed_failed" => Ok(LegacyDelivery::ConfirmedFailure),
        "confirmed_success" => Ok(LegacyDelivery::ConfirmedSuccess),
        _ => Err(LegacyHoldReason::InvalidFields),
    }
}

fn parse_event_kind(value: &str) -> Result<LegacyEvent, LegacyHoldReason> {
    match value {
        "action_started" => Ok(LegacyEvent::ActionStarted),
        "action_observed" => Ok(LegacyEvent::ActionObserved),
        "candidate_prepared" => Ok(LegacyEvent::CandidatePrepared),
        "candidate_send_attempted" => Ok(LegacyEvent::CandidateSendAttempted),
        "sink_delivery_observed" => Ok(LegacyEvent::SinkDeliveryObserved),
        _ => Err(LegacyHoldReason::InvalidFields),
    }
}

fn event_result_is_valid(
    event: Option<LegacyEvent>,
    delivery: LegacyDelivery,
    result_digest: Option<&str>,
) -> bool {
    match event.unwrap_or(LegacyEvent::Unclassified) {
        LegacyEvent::Unclassified => {
            (matches!(delivery, LegacyDelivery::Unknown) && result_digest.is_none())
                || (!matches!(delivery, LegacyDelivery::Unknown) && result_digest.is_some())
        }
        LegacyEvent::ActionStarted
        | LegacyEvent::CandidatePrepared
        | LegacyEvent::CandidateSendAttempted => {
            matches!(delivery, LegacyDelivery::Unknown) && result_digest.is_none()
        }
        LegacyEvent::ActionObserved | LegacyEvent::SinkDeliveryObserved => {
            !matches!(delivery, LegacyDelivery::Unknown) && result_digest.is_some()
        }
    }
}

fn is_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

struct CanonicalJsonCursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> CanonicalJsonCursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn expect(&mut self, token: &[u8]) -> Result<(), LegacyHoldReason> {
        let end = self
            .offset
            .checked_add(token.len())
            .ok_or(LegacyHoldReason::UnsupportedLegacyEncoding)?;
        if self.input.get(self.offset..end) != Some(token) {
            return Err(LegacyHoldReason::UnsupportedLegacyEncoding);
        }
        self.offset = end;
        Ok(())
    }

    fn string(&mut self) -> Result<&'a str, LegacyHoldReason> {
        if self.input.get(self.offset) != Some(&b'"') {
            return Err(LegacyHoldReason::UnsupportedLegacyEncoding);
        }
        self.offset += 1;
        let start = self.offset;
        while let Some(byte) = self.input.get(self.offset).copied() {
            match byte {
                b'"' => {
                    let value = std::str::from_utf8(&self.input[start..self.offset])
                        .map_err(|_| LegacyHoldReason::UnsupportedLegacyEncoding)?;
                    self.offset += 1;
                    return Ok(value);
                }
                b'\\' | 0..=0x1f => {
                    // The legacy writer emits canonical UTF-8 and escapes only
                    // exceptional strings. This verifier holds those strings
                    // instead of guessing a normalization/re-serialization.
                    return Err(LegacyHoldReason::UnsupportedLegacyEncoding);
                }
                _ => self.offset += 1,
            }
        }
        Err(LegacyHoldReason::UnsupportedLegacyEncoding)
    }

    fn optional_string(&mut self) -> Result<Option<&'a str>, LegacyHoldReason> {
        if self.input.get(self.offset..self.offset.saturating_add(4)) == Some(b"null") {
            self.offset = self
                .offset
                .checked_add(4)
                .ok_or(LegacyHoldReason::UnsupportedLegacyEncoding)?;
            Ok(None)
        } else {
            self.string().map(Some)
        }
    }

    fn u64(&mut self) -> Result<u64, LegacyHoldReason> {
        let start = self.offset;
        while self.input.get(self.offset).is_some_and(u8::is_ascii_digit) {
            self.offset += 1;
        }
        if start == self.offset || (self.offset - start > 1 && self.input.get(start) == Some(&b'0'))
        {
            return Err(LegacyHoldReason::UnsupportedLegacyEncoding);
        }
        let value = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| LegacyHoldReason::UnsupportedLegacyEncoding)?;
        value.parse().map_err(|_| LegacyHoldReason::InvalidFields)
    }

    fn finish(self) -> Result<(), LegacyHoldReason> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(LegacyHoldReason::UnsupportedLegacyEncoding)
        }
    }
}
