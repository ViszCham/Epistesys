use lc631_substrate::codec::{
    decode_envelope, encode_frame, CodecError, SectionInput, FRAME_DOMAIN,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

use lc631_runtime::journal::{
    decode_journal_index, decode_segment, JournalIndex, JournalIndexError, JournalRecordKind,
    JournalSegmentBuilder, MAX_JOURNAL_INDEX_ENTRIES, RECORD_HEADER_BYTES,
};

const INDEX_HEADER_BYTES: usize = 104;
const INDEX_ENTRY_BYTES: usize = 48;
const INDEX_DIGEST_OFFSET: usize = 72;
const INDEX_DOMAIN: &[u8] = b"Epistesys/TLCX/JournalIndex/sha256/v1\0";
const INDEX_SECTION_TAG: u16 = 0x0201;
const FRAME_HEADER_BYTES: usize = 64;
const FRAME_DIRECTORY_ENTRY_BYTES: usize = 32;

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn segment(payloads: &[&[u8]]) -> Vec<u8> {
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

fn reseal_index_payload(bytes: &mut [u8]) {
    assert!(bytes.len() >= INDEX_HEADER_BYTES);
    let mut hash = Sha256::new();
    hash.update(INDEX_DOMAIN);
    hash.update(&bytes[..INDEX_DIGEST_OFFSET]);
    hash.update([0; 32]);
    hash.update(&bytes[INDEX_HEADER_BYTES..]);
    let digest = hash.finalize();
    bytes[INDEX_DIGEST_OFFSET..INDEX_DIGEST_OFFSET + 32].copy_from_slice(&digest);
}

fn index_payload(frame_bytes: &[u8]) -> Vec<u8> {
    let frame = decode_envelope(frame_bytes).unwrap();
    assert_eq!(frame.sections().len(), 1);
    frame.sections()[0].payload().to_vec()
}

fn wrap_index_payload(payload: &[u8], count: u32, tag: u16, version: u16, flags: u32) -> Vec<u8> {
    encode_frame(&[SectionInput {
        tag,
        version,
        flags,
        count,
        payload,
    }])
    .unwrap()
}

fn reseal_frame(bytes: &mut [u8]) {
    let section_count = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
    let payload_start = FRAME_HEADER_BYTES + section_count * FRAME_DIRECTORY_ENTRY_BYTES;
    let mut hash = Sha256::new();
    hash.update(FRAME_DOMAIN);
    hash.update(&bytes[..32]);
    hash.update([0; 32]);
    hash.update(&bytes[FRAME_HEADER_BYTES..payload_start]);
    hash.update(&bytes[payload_start..]);
    let digest = hash.finalize();
    bytes[32..64].copy_from_slice(&digest);
}

#[test]
fn tlcx07_journal_index_roundtrips_and_binds_to_a_validated_segment() {
    let bytes = segment(&[b"first", b"second"]);
    let view = decode_segment(&bytes).unwrap();
    let index = JournalIndex::from_segment(&view).unwrap();

    assert_eq!(index.entries().len(), 2);
    assert_eq!(index.entries()[0].sequence(), 1);
    assert_eq!(index.entries()[0].offset(), 0);
    assert_eq!(
        index.entries()[0].record_len(),
        u32::try_from(RECORD_HEADER_BYTES + 5).unwrap()
    );
    assert_eq!(index.entries()[1].sequence(), 2);
    assert_eq!(
        index.entries()[1].offset(),
        u32::try_from(RECORD_HEADER_BYTES + 5).unwrap()
    );
    assert_eq!(
        index.entries()[1].record_len(),
        u32::try_from(RECORD_HEADER_BYTES + 6).unwrap()
    );

    let encoded = index.encode().unwrap();
    let decoded = decode_journal_index(&encoded).unwrap();
    assert_eq!(decoded, index);
    decoded.verify_segment(&view).unwrap();
    assert!(decoded.claim_boundary().contains("not an external anchor"));

    let changed_segment = segment(&[b"first", b"changed"]);
    let changed_view = decode_segment(&changed_segment).unwrap();
    assert_eq!(
        decoded.verify_segment(&changed_view),
        Err(JournalIndexError::StaleSegment)
    );
}

#[test]
fn tlcx07_journal_index_rejects_header_integrity_and_size_failures() {
    let source = segment(&[b"entry"]);
    let view = decode_segment(&source).unwrap();
    let encoded = JournalIndex::from_segment(&view).unwrap().encode().unwrap();

    let mut bad_magic = encoded.clone();
    bad_magic[0] ^= 0x01;
    assert!(matches!(
        decode_journal_index(&bad_magic),
        Err(JournalIndexError::Frame(CodecError::InvalidMagic))
    ));

    let mut bad_version = encoded.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        decode_journal_index(&bad_version),
        Err(JournalIndexError::Frame(CodecError::UnsupportedMajor))
    ));

    let mut bad_minor = encoded.clone();
    bad_minor[10..12].copy_from_slice(&1_u16.to_le_bytes());
    reseal_frame(&mut bad_minor);
    assert!(matches!(
        decode_journal_index(&bad_minor),
        Err(JournalIndexError::Frame(CodecError::UnsupportedMinor))
    ));

    let payload = index_payload(&encoded);
    let count = u32::from_le_bytes(payload[24..28].try_into().unwrap());

    let mut bad_inner_version = payload.clone();
    bad_inner_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    let bad_inner_version = wrap_index_payload(&bad_inner_version, count, INDEX_SECTION_TAG, 1, 0);
    assert!(matches!(
        decode_journal_index(&bad_inner_version),
        Err(JournalIndexError::UnsupportedVersion)
    ));

    let mut bad_digest = payload.clone();
    bad_digest[INDEX_HEADER_BYTES] ^= 0x80;
    let bad_digest = wrap_index_payload(&bad_digest, count, INDEX_SECTION_TAG, 1, 0);
    assert!(matches!(
        decode_journal_index(&bad_digest),
        Err(JournalIndexError::IntegrityMismatch)
    ));

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(matches!(
        decode_journal_index(&trailing),
        Err(JournalIndexError::Frame(CodecError::LengthMismatch))
    ));

    let mut excessive_count = payload;
    excessive_count[24..28].copy_from_slice(
        &u32::try_from(MAX_JOURNAL_INDEX_ENTRIES + 1)
            .unwrap()
            .to_le_bytes(),
    );
    reseal_index_payload(&mut excessive_count);
    let excessive_count = wrap_index_payload(&excessive_count, count, INDEX_SECTION_TAG, 1, 0);
    assert!(matches!(
        decode_journal_index(&excessive_count),
        Err(JournalIndexError::TooManyEntries)
    ));
}

#[test]
fn tlcx07_journal_index_validates_order_and_record_bounds_before_materializing() {
    let source = segment(&[b"one", b"two"]);
    let view = decode_segment(&source).unwrap();
    let encoded = JournalIndex::from_segment(&view).unwrap().encode().unwrap();
    let payload = index_payload(&encoded);
    let count = u32::from_le_bytes(payload[24..28].try_into().unwrap());

    let second_entry = INDEX_HEADER_BYTES + INDEX_ENTRY_BYTES;
    let mut out_of_order = payload.clone();
    out_of_order[second_entry..second_entry + 8].copy_from_slice(&4_u64.to_le_bytes());
    reseal_index_payload(&mut out_of_order);
    let out_of_order = wrap_index_payload(&out_of_order, count, INDEX_SECTION_TAG, 1, 0);
    assert!(matches!(
        decode_journal_index(&out_of_order),
        Err(JournalIndexError::SequenceGap)
    ));

    let mut noncontiguous = payload.clone();
    noncontiguous[second_entry + 8..second_entry + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    reseal_index_payload(&mut noncontiguous);
    let noncontiguous = wrap_index_payload(&noncontiguous, count, INDEX_SECTION_TAG, 1, 0);
    assert!(matches!(
        decode_journal_index(&noncontiguous),
        Err(JournalIndexError::NonContiguousOffset)
    ));

    let mut oversized_record = payload;
    oversized_record[INDEX_HEADER_BYTES + 12..INDEX_HEADER_BYTES + 16]
        .copy_from_slice(&u32::MAX.to_le_bytes());
    reseal_index_payload(&mut oversized_record);
    let oversized_record = wrap_index_payload(&oversized_record, count, INDEX_SECTION_TAG, 1, 0);
    assert!(matches!(
        decode_journal_index(&oversized_record),
        Err(JournalIndexError::RecordTooLarge)
    ));
}

#[test]
fn tlcx07_resealed_index_metadata_is_not_segment_evidence() {
    let source = segment(&[b"first", b"second"]);
    let view = decode_segment(&source).unwrap();
    let encoded = JournalIndex::from_segment(&view).unwrap().encode().unwrap();
    let mut payload = index_payload(&encoded);
    let count = u32::from_le_bytes(payload[24..28].try_into().unwrap());

    // A self-consistent checksum does not authenticate a changed locator.
    payload[INDEX_HEADER_BYTES + 16] ^= 1;
    reseal_index_payload(&mut payload);
    let encoded = wrap_index_payload(&payload, count, INDEX_SECTION_TAG, 1, 0);
    let decoded = decode_journal_index(&encoded).unwrap();
    assert_eq!(
        decoded.verify_segment(&view),
        Err(JournalIndexError::StaleSegment)
    );
}

#[test]
fn tlcx07_index_requires_canonical_single_section_envelope_metadata() {
    let source = segment(&[b"first", b"second"]);
    let view = decode_segment(&source).unwrap();
    let encoded = JournalIndex::from_segment(&view).unwrap().encode().unwrap();
    let payload = index_payload(&encoded);
    let count = u32::from_le_bytes(payload[24..28].try_into().unwrap());

    let wrong_section_version = wrap_index_payload(&payload, count, INDEX_SECTION_TAG, 2, 0);
    assert!(matches!(
        decode_journal_index(&wrong_section_version),
        Err(JournalIndexError::UnsupportedSectionVersion)
    ));

    let wrong_tag = wrap_index_payload(&payload, count, INDEX_SECTION_TAG + 1, 1, 0);
    assert!(matches!(
        decode_journal_index(&wrong_tag),
        Err(JournalIndexError::WrongSectionTag)
    ));

    let wrong_count = wrap_index_payload(&payload, count + 1, INDEX_SECTION_TAG, 1, 0);
    assert!(matches!(
        decode_journal_index(&wrong_count),
        Err(JournalIndexError::SectionCountMismatch)
    ));

    let bare_index_payload = payload.clone();
    assert!(matches!(
        decode_journal_index(&bare_index_payload),
        Err(JournalIndexError::Frame(CodecError::InvalidMagic))
    ));

    let two_sections = encode_frame(&[
        SectionInput {
            tag: INDEX_SECTION_TAG,
            version: 1,
            flags: 0,
            count,
            payload: &payload,
        },
        SectionInput {
            tag: INDEX_SECTION_TAG + 1,
            version: 1,
            flags: 0,
            count: 0,
            payload: &[],
        },
    ])
    .unwrap();
    assert!(matches!(
        decode_journal_index(&two_sections),
        Err(JournalIndexError::InvalidFrameShape)
    ));

    let mut noncanonical_tags = two_sections;
    noncanonical_tags[64..66].copy_from_slice(&(INDEX_SECTION_TAG + 1).to_le_bytes());
    noncanonical_tags[96..98].copy_from_slice(&INDEX_SECTION_TAG.to_le_bytes());
    assert!(matches!(
        decode_journal_index(&noncanonical_tags),
        Err(JournalIndexError::Frame(CodecError::NonCanonicalTagOrder))
    ));

    let mut nonzero_flags = encoded;
    nonzero_flags[68..72].copy_from_slice(&1_u32.to_le_bytes());
    reseal_frame(&mut nonzero_flags);
    assert!(matches!(
        decode_journal_index(&nonzero_flags),
        Err(JournalIndexError::Frame(CodecError::UnknownCritical))
    ));
}

#[test]
fn tlcx07_index_accepts_a_valid_non_genesis_segment_without_inventing_an_anchor() {
    let first = segment(&[b"first"]);
    let first_view = decode_segment(&first).unwrap();
    let mut builder = JournalSegmentBuilder::after(first_view.cursor());
    builder
        .append_at(2, JournalRecordKind::Evidence, digest(2), b"second")
        .unwrap();
    let second = builder.finish().unwrap();
    let second_view = decode_segment(&second).unwrap();
    let index = JournalIndex::from_segment(&second_view).unwrap();
    let decoded = decode_journal_index(&index.encode().unwrap()).unwrap();
    assert_eq!(decoded.entries()[0].sequence(), 2);
    decoded.verify_segment(&second_view).unwrap();
    assert!(decoded.claim_boundary().contains("not an external anchor"));
}
