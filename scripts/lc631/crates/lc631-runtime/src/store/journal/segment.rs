use super::record_codec::{
    decode_record_header, digest_from_record, encode_record, record_digest, DecodedRecordHeader,
};
use super::types::{
    JournalCursor, JournalError, JournalHead, JournalRecordKind, JournalRecordView,
    JournalSegmentView,
};
use super::{
    JOURNAL_RECORDS_SECTION_TAG, MAX_JOURNAL_RECORD_BYTES, MAX_JOURNAL_SEGMENT_BYTES,
    MAX_RECORDS_PER_SEGMENT, RECORD_HEADER_BYTES, SEGMENT_FRAME_OVERHEAD,
};
#[cfg(test)]
use lc631_substrate::codec::decode_envelope;
use lc631_substrate::codec::{
    decode_envelope_borrowed, encode_frame, BorrowedFrameView, SectionInput, SectionKind,
};
use lc631_substrate::Digest32;
use std::convert::TryFrom;

/// Builder owns one bounded segment's bytes only. Persistence, writer locking,
/// fsync, external-head storage and cleanup belong to the host-owned store.
pub struct JournalSegmentBuilder {
    cursor: JournalCursor,
    records_payload: Vec<u8>,
    record_count: usize,
}

impl JournalSegmentBuilder {
    pub fn genesis() -> Self {
        Self::after(JournalCursor::genesis())
    }

    pub fn after(cursor: JournalCursor) -> Self {
        Self {
            cursor,
            records_payload: Vec::new(),
            record_count: 0,
        }
    }

    pub fn append_at(
        &mut self,
        sequence: u64,
        kind: JournalRecordKind,
        object_digest: Digest32,
        payload: &[u8],
    ) -> Result<(), JournalError> {
        let expected = self
            .cursor
            .sequence
            .checked_add(1)
            .ok_or(JournalError::SequenceOverflow)?;
        if sequence == 0 {
            return Err(JournalError::ZeroSequence);
        }
        if sequence <= self.cursor.sequence {
            return Err(JournalError::NonMonotonicSequence);
        }
        if sequence != expected {
            return Err(JournalError::SequenceGap);
        }
        if self.record_count >= MAX_RECORDS_PER_SEGMENT {
            return Err(JournalError::TooManyRecords);
        }
        if object_digest.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(JournalError::InvalidObjectDigest);
        }

        let record_bytes = RECORD_HEADER_BYTES
            .checked_add(payload.len())
            .ok_or(JournalError::RecordTooLarge)?;
        if record_bytes > MAX_JOURNAL_RECORD_BYTES {
            return Err(JournalError::RecordTooLarge);
        }
        let projected_segment_bytes = SEGMENT_FRAME_OVERHEAD
            .checked_add(self.records_payload.len())
            .and_then(|bytes| bytes.checked_add(record_bytes))
            .ok_or(JournalError::SegmentFull)?;
        if projected_segment_bytes > MAX_JOURNAL_SEGMENT_BYTES {
            return Err(JournalError::SegmentFull);
        }

        let encoded = encode_record(
            sequence,
            kind,
            object_digest,
            self.cursor.record_digest,
            payload,
        )?;
        self.records_payload
            .try_reserve_exact(encoded.len())
            .map_err(|_| JournalError::AllocationFailed)?;
        let record_digest = digest_from_record(&encoded)?;
        self.records_payload.extend_from_slice(&encoded);
        self.record_count += 1;
        self.cursor = JournalCursor {
            sequence,
            record_digest,
        };
        Ok(())
    }

    pub fn finish(self) -> Result<Vec<u8>, JournalError> {
        if self.record_count == 0 {
            return Err(JournalError::EmptySegment);
        }
        let count = u32::try_from(self.record_count).map_err(|_| JournalError::TooManyRecords)?;
        let frame = encode_frame(&[SectionInput {
            tag: JOURNAL_RECORDS_SECTION_TAG,
            version: 1,
            flags: 0,
            count,
            payload: &self.records_payload,
        }])?;
        if frame.len() > MAX_JOURNAL_SEGMENT_BYTES {
            return Err(JournalError::SegmentFull);
        }
        Ok(frame)
    }
}

/// Summary produced without materializing one view per record. The head is a
/// local consistency value, not an external rollback anchor or authenticity
/// evidence. Frame sections and record headers are borrowed instead of
/// allocating metadata vectors. This does not account for callers' payload
/// buffers, checkpoint JSON parsing, or total process RSS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StreamingSegmentSummary {
    pub(super) local_head: JournalHead,
    pub(super) cursor: JournalCursor,
    pub(super) record_count: usize,
}

/// Validate a segment and its expected predecessor while streaming record
/// headers and payload spans. This avoids `decode_segment`'s per-record view
/// vector but still uses the shared bounded EPITLCX1 decoder for frame checks.
pub(super) fn validate_segment_streaming(
    bytes: &[u8],
    expected_predecessor: JournalCursor,
) -> Result<StreamingSegmentSummary, JournalError> {
    let frame = decode_segment_frame(bytes)?;
    let (payload, count) = record_payload_and_count(&frame)?;
    let mut offset = 0_usize;
    let mut record_count = 0_usize;
    let mut first_previous = None;
    let mut previous_record: Option<(u64, Digest32)> = None;

    while offset < payload.len() {
        if record_count == count {
            return Err(JournalError::RecordCountMismatch);
        }
        let header_end = offset
            .checked_add(RECORD_HEADER_BYTES)
            .ok_or(JournalError::TornRecord)?;
        let header = payload
            .get(offset..header_end)
            .ok_or(JournalError::TornRecord)?;
        let header_view = decode_record_header(header)?;
        let payload_end = header_end
            .checked_add(header_view.payload_len)
            .ok_or(JournalError::TornRecord)?;
        let record_payload = payload
            .get(header_end..payload_end)
            .ok_or(JournalError::TornRecord)?;
        verify_record_digest(header, record_payload, header_view)?;
        if let Some((previous_sequence, previous_digest)) = previous_record {
            verify_record_link(header_view, previous_sequence, previous_digest)?;
        }
        if first_previous.is_none() {
            first_previous = Some(JournalCursor {
                sequence: header_view
                    .sequence
                    .checked_sub(1)
                    .ok_or(JournalError::ZeroSequence)?,
                record_digest: header_view.previous_digest,
            });
        }

        previous_record = Some((header_view.sequence, header_view.record_digest));
        record_count += 1;
        offset = payload_end;
    }

    if record_count != count {
        return Err(JournalError::RecordCountMismatch);
    }
    let first_previous = first_previous.ok_or(JournalError::EmptySegment)?;
    if first_previous != expected_predecessor {
        return Err(JournalError::StalePredecessor);
    }
    let (sequence, record_digest) = previous_record.ok_or(JournalError::EmptySegment)?;
    let cursor = JournalCursor {
        sequence,
        record_digest,
    };
    Ok(StreamingSegmentSummary {
        local_head: JournalHead {
            segment_digest: frame.digest(),
            sequence,
            record_digest,
        },
        cursor,
        record_count,
    })
}

/// Decode a bounded EPITLCX1 segment and rebuild its local index. The caller
/// must verify the predecessor cursor; local hashes are not external anchors.
pub fn decode_segment(bytes: &[u8]) -> Result<JournalSegmentView<'_>, JournalError> {
    let frame = decode_segment_frame(bytes)?;
    let (payload, count) = record_payload_and_count(&frame)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| JournalError::AllocationFailed)?;
    let mut offset = 0_usize;
    let mut previous_record = None;
    let mut first_previous = None;

    while offset < payload.len() {
        if records.len() == count {
            return Err(JournalError::RecordCountMismatch);
        }
        let header_end = offset
            .checked_add(RECORD_HEADER_BYTES)
            .ok_or(JournalError::TornRecord)?;
        let header = payload
            .get(offset..header_end)
            .ok_or(JournalError::TornRecord)?;
        let header_view = decode_record_header(header)?;
        let payload_end = header_end
            .checked_add(header_view.payload_len)
            .ok_or(JournalError::TornRecord)?;
        let record_payload = payload
            .get(header_end..payload_end)
            .ok_or(JournalError::TornRecord)?;
        verify_record_digest(header, record_payload, header_view)?;
        verify_record_chain(header_view, previous_record)?;
        if first_previous.is_none() {
            first_previous = Some(JournalCursor {
                sequence: header_view.sequence - 1,
                record_digest: header_view.previous_digest,
            });
        }

        records.push(JournalRecordView {
            sequence: header_view.sequence,
            kind: header_view.kind,
            object_digest: header_view.object_digest,
            previous_digest: header_view.previous_digest,
            record_digest: header_view.record_digest,
            payload: record_payload,
            payload_offset: header_end,
        });
        previous_record = records.last().copied();
        offset = payload_end;
    }

    if records.len() != count {
        return Err(JournalError::RecordCountMismatch);
    }
    let last = records.last().copied().ok_or(JournalError::EmptySegment)?;
    Ok(JournalSegmentView {
        frame_digest: frame.digest(),
        first_previous: first_previous.ok_or(JournalError::EmptySegment)?,
        cursor: JournalCursor {
            sequence: last.sequence,
            record_digest: last.record_digest,
        },
        records,
    })
}

fn decode_segment_frame(bytes: &[u8]) -> Result<BorrowedFrameView<'_>, JournalError> {
    if bytes.len() > MAX_JOURNAL_SEGMENT_BYTES {
        return Err(JournalError::InputTooLarge);
    }
    let frame = decode_envelope_borrowed(bytes)?;
    if frame.section_count() != 1 {
        return Err(JournalError::InvalidFrameShape);
    }
    let section = frame.section(0)?.ok_or(JournalError::InvalidFrameShape)?;
    if section.tag() != JOURNAL_RECORDS_SECTION_TAG
        || section.version() != 1
        || section.flags() != 0
        || section.kind() != SectionKind::Opaque
    {
        return Err(JournalError::InvalidFrameShape);
    }
    Ok(frame)
}

fn record_payload_and_count<'a>(
    frame: &BorrowedFrameView<'a>,
) -> Result<(&'a [u8], usize), JournalError> {
    let section = frame.section(0)?.ok_or(JournalError::InvalidFrameShape)?;
    if section.count() == 0 {
        return Err(JournalError::EmptySegment);
    }
    let count = usize::try_from(section.count()).map_err(|_| JournalError::TooManyRecords)?;
    if count > MAX_RECORDS_PER_SEGMENT {
        return Err(JournalError::TooManyRecords);
    }
    let payload = section.payload();
    if count > payload.len() / RECORD_HEADER_BYTES {
        return Err(JournalError::RecordCountMismatch);
    }
    Ok((payload, count))
}

fn verify_record_digest(
    header: &[u8],
    payload: &[u8],
    decoded: DecodedRecordHeader,
) -> Result<(), JournalError> {
    if record_digest(header, payload)? == decoded.record_digest {
        Ok(())
    } else {
        Err(JournalError::RecordDigestMismatch)
    }
}

fn verify_record_chain(
    current: DecodedRecordHeader,
    previous: Option<JournalRecordView<'_>>,
) -> Result<(), JournalError> {
    let Some(previous) = previous else {
        return Ok(());
    };
    verify_record_link(current, previous.sequence(), previous.digest())
}

fn verify_record_link(
    current: DecodedRecordHeader,
    previous_sequence: u64,
    previous_digest: Digest32,
) -> Result<(), JournalError> {
    let expected = previous_sequence
        .checked_add(1)
        .ok_or(JournalError::SequenceOverflow)?;
    if current.sequence <= previous_sequence {
        return Err(JournalError::NonMonotonicSequence);
    }
    if current.sequence != expected {
        return Err(JournalError::SequenceGap);
    }
    if current.previous_digest != previous_digest {
        return Err(JournalError::PreviousDigestMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod streaming_validation_tests {
    use super::*;

    #[test]
    fn tlcx07_streaming_frame_uses_borrowed_section_metadata() {
        let bytes = build_segment(&[b"first", b"second"]);
        let frame: lc631_substrate::codec::BorrowedFrameView<'_> =
            decode_segment_frame(&bytes).unwrap();
        assert_eq!(frame.section_count(), 1);
        let section = frame.section(0).unwrap().unwrap();
        assert_eq!(section.tag(), JOURNAL_RECORDS_SECTION_TAG);
        assert_eq!(section.count(), 2);
        assert_eq!(frame.original_bytes().as_ptr(), bytes.as_ptr());
    }

    fn digest(byte: u8) -> Digest32 {
        Digest32::from_bytes([byte; 32])
    }

    fn build_segment(payloads: &[&[u8]]) -> Vec<u8> {
        let mut builder = JournalSegmentBuilder::genesis();
        for (index, payload) in payloads.iter().enumerate() {
            builder
                .append_at(
                    u64::try_from(index + 1).unwrap(),
                    JournalRecordKind::Evidence,
                    digest(u8::try_from(index + 1).unwrap()),
                    payload,
                )
                .unwrap();
        }
        builder.finish().unwrap()
    }

    fn decode_then_check(
        bytes: &[u8],
        expected_predecessor: JournalCursor,
    ) -> Result<StreamingSegmentSummary, JournalError> {
        let segment = decode_segment(bytes)?;
        segment.verify_predecessor(expected_predecessor)?;
        Ok(StreamingSegmentSummary {
            local_head: segment.local_head(),
            cursor: segment.cursor(),
            record_count: segment.records().len(),
        })
    }

    fn assert_differential(bytes: &[u8], expected_predecessor: JournalCursor) {
        assert_eq!(
            validate_segment_streaming(bytes, expected_predecessor),
            decode_then_check(bytes, expected_predecessor),
        );
    }

    fn assert_rejection(
        bytes: &[u8],
        expected_predecessor: JournalCursor,
        expected_error: JournalError,
    ) {
        assert_differential(bytes, expected_predecessor);
        assert_eq!(
            validate_segment_streaming(bytes, expected_predecessor),
            Err(expected_error),
        );
    }

    fn wire_record(sequence: u64, previous_digest: Digest32, payload: &[u8]) -> Vec<u8> {
        encode_record(
            sequence,
            JournalRecordKind::Evidence,
            digest(0x41),
            previous_digest,
            payload,
        )
        .unwrap()
    }

    fn reseal_record(bytes: &mut [u8]) {
        assert!(bytes.len() >= RECORD_HEADER_BYTES);
        let digest =
            record_digest(&bytes[..RECORD_HEADER_BYTES], &bytes[RECORD_HEADER_BYTES..]).unwrap();
        bytes[96..128].copy_from_slice(digest.as_bytes());
    }

    fn frame_from_records(records: &[Vec<u8>], declared_count: u32) -> Vec<u8> {
        let payload_len = records.iter().map(Vec::len).sum();
        let mut payload = Vec::with_capacity(payload_len);
        for record in records {
            payload.extend_from_slice(record);
        }
        encode_frame(&[SectionInput {
            tag: JOURNAL_RECORDS_SECTION_TAG,
            version: 1,
            flags: 0,
            count: declared_count,
            payload: &payload,
        }])
        .unwrap()
    }

    fn rewrite_records_payload(frame_bytes: &[u8], payload: &[u8]) -> Vec<u8> {
        let frame = decode_envelope(frame_bytes).unwrap();
        let section = &frame.sections()[0];
        encode_frame(&[SectionInput {
            tag: section.tag(),
            version: section.version(),
            flags: section.flags(),
            count: section.count(),
            payload,
        }])
        .unwrap()
    }

    #[test]
    fn streaming_summary_matches_decoded_segment_for_valid_input() {
        let bytes = build_segment(&[b"first", b"second"]);
        let expected = JournalCursor::genesis();
        assert_differential(&bytes, expected);

        let summary = validate_segment_streaming(&bytes, expected).unwrap();
        assert_eq!(summary.record_count, 2);
        assert_eq!(summary.cursor.sequence(), 2);
        assert_eq!(summary.local_head.sequence(), 2);
        assert_eq!(
            summary.local_head.record_digest(),
            summary.cursor.record_digest()
        );
    }

    #[test]
    fn streaming_summary_matches_empty_torn_and_tampered_decoder_errors() {
        let empty = encode_frame(&[SectionInput {
            tag: JOURNAL_RECORDS_SECTION_TAG,
            version: 1,
            flags: 0,
            count: 0,
            payload: &[],
        }])
        .unwrap();
        assert_differential(&empty, JournalCursor::genesis());

        let valid = build_segment(&[b"payload"]);
        let mut torn_frame = valid.clone();
        torn_frame.pop();
        assert_differential(&torn_frame, JournalCursor::genesis());

        let frame = decode_envelope(&valid).unwrap();
        let mut truncated_record = frame.sections()[0].payload().to_vec();
        truncated_record.pop();
        let torn_record = rewrite_records_payload(&valid, &truncated_record);
        assert_differential(&torn_record, JournalCursor::genesis());

        let mut altered_record = frame.sections()[0].payload().to_vec();
        altered_record[RECORD_HEADER_BYTES] ^= 0x80;
        let tampered = rewrite_records_payload(&valid, &altered_record);
        assert_differential(&tampered, JournalCursor::genesis());
    }

    #[test]
    fn streaming_summary_matches_stale_predecessor_result() {
        let bytes = build_segment(&[b"first", b"second"]);
        let wrong_predecessor = JournalCursor {
            sequence: 17,
            record_digest: digest(0x71),
        };
        assert_differential(&bytes, wrong_predecessor);
        assert_eq!(
            validate_segment_streaming(&bytes, wrong_predecessor),
            Err(JournalError::StalePredecessor)
        );
    }

    #[test]
    fn streaming_summary_matches_decoder_at_maximum_record_count() {
        let mut builder = JournalSegmentBuilder::genesis();
        for sequence in 1..=MAX_RECORDS_PER_SEGMENT {
            builder
                .append_at(
                    u64::try_from(sequence).unwrap(),
                    JournalRecordKind::Evidence,
                    digest(1),
                    &[],
                )
                .unwrap();
        }
        let bytes = builder.finish().unwrap();
        let expected = JournalCursor::genesis();
        assert_differential(&bytes, expected);

        let summary = validate_segment_streaming(&bytes, expected).unwrap();
        assert_eq!(summary.record_count, MAX_RECORDS_PER_SEGMENT);
        assert_eq!(
            summary.cursor.sequence(),
            u64::try_from(MAX_RECORDS_PER_SEGMENT).unwrap()
        );
    }

    #[test]
    fn streaming_validator_rejects_resealed_sequence_and_link_adversaries_exactly() {
        let genesis = JournalCursor::genesis();

        let mut zero_sequence = wire_record(1, genesis.record_digest(), b"zero");
        zero_sequence[16..24].copy_from_slice(&0_u64.to_le_bytes());
        reseal_record(&mut zero_sequence);
        let zero_sequence = frame_from_records(&[zero_sequence], 1);
        assert_rejection(&zero_sequence, genesis, JournalError::ZeroSequence);

        let first = wire_record(1, genesis.record_digest(), b"first");
        let first_digest = digest_from_record(&first).unwrap();
        let nonmonotonic = wire_record(1, first_digest, b"repeat");
        let nonmonotonic = frame_from_records(&[first.clone(), nonmonotonic], 2);
        assert_rejection(&nonmonotonic, genesis, JournalError::NonMonotonicSequence);

        let first = wire_record(1, genesis.record_digest(), b"first");
        let first_digest = digest_from_record(&first).unwrap();
        let gap = wire_record(3, first_digest, b"gap");
        let gap = frame_from_records(&[first, gap], 2);
        assert_rejection(&gap, genesis, JournalError::SequenceGap);

        let first = wire_record(1, genesis.record_digest(), b"first");
        let mismatched_previous = wire_record(2, digest(0x77), b"second");
        let mismatched_previous = frame_from_records(&[first, mismatched_previous], 2);
        assert_rejection(
            &mismatched_previous,
            genesis,
            JournalError::PreviousDigestMismatch,
        );
    }

    #[test]
    fn streaming_validator_rejects_resealed_record_flags_and_kinds_exactly() {
        let genesis = JournalCursor::genesis();

        let mut unknown_flags = wire_record(1, genesis.record_digest(), b"flags");
        unknown_flags[12..16].copy_from_slice(&1_u32.to_le_bytes());
        reseal_record(&mut unknown_flags);
        let unknown_flags = frame_from_records(&[unknown_flags], 1);
        assert_rejection(&unknown_flags, genesis, JournalError::UnknownRecordFlags);

        let mut invalid_kind = wire_record(1, genesis.record_digest(), b"kind");
        invalid_kind[10..12].copy_from_slice(&0_u16.to_le_bytes());
        reseal_record(&mut invalid_kind);
        let invalid_kind = frame_from_records(&[invalid_kind], 1);
        assert_rejection(&invalid_kind, genesis, JournalError::UnsupportedRecordKind);
    }

    #[test]
    fn streaming_validator_rejects_count_size_and_constructible_overflow_cases_exactly() {
        let genesis = JournalCursor::genesis();

        let first = wire_record(1, genesis.record_digest(), b"first");
        let first_digest = digest_from_record(&first).unwrap();
        let second = wire_record(2, first_digest, b"second");
        let wrong_count = frame_from_records(&[first, second], 1);
        assert_rejection(&wrong_count, genesis, JournalError::RecordCountMismatch);

        let too_many = frame_from_records(&[], u32::try_from(MAX_RECORDS_PER_SEGMENT + 1).unwrap());
        assert_rejection(&too_many, genesis, JournalError::TooManyRecords);

        let mut oversized_record = wire_record(1, genesis.record_digest(), b"");
        oversized_record[24..28].copy_from_slice(
            &u32::try_from(MAX_JOURNAL_RECORD_BYTES)
                .unwrap()
                .to_le_bytes(),
        );
        reseal_record(&mut oversized_record);
        let oversized_record = frame_from_records(&[oversized_record], 1);
        assert_rejection(&oversized_record, genesis, JournalError::RecordTooLarge);

        let oversized_frame = vec![0; MAX_JOURNAL_SEGMENT_BYTES + 1];
        assert_rejection(&oversized_frame, genesis, JournalError::InputTooLarge);

        let first = wire_record(u64::MAX, genesis.record_digest(), b"last-sequence");
        let first_digest = digest_from_record(&first).unwrap();
        let second = wire_record(1, first_digest, b"wrapped-sequence");
        let overflow = frame_from_records(&[first, second], 2);
        assert_rejection(&overflow, genesis, JournalError::SequenceOverflow);
    }
}
