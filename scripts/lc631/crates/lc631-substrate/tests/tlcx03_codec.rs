use lc631_substrate::codec::{
    decode_envelope, encode_frame, encode_string_map, read_frame, CodecError, SectionInput,
    SectionKind, CRITICAL, FRAME_DOMAIN, MAX_FRAME_BYTES, MAX_MAP_ENTRIES, MAX_STRING_BYTES,
    SOURCE_UTF8, STRING_MAP,
};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::sync::Arc;

const G1: &str = concat!(
    "455049544c435831010000000000000003000000000000000100000040000000",
    "da5f58ecbb5dd2a8e91e7807841ad5a126ffc2a7cdea5019daa6c27cd4900623",
    "feff010000000000600000000000000003000000000000000100000000000000e697a5"
);

fn unhex(text: &str) -> Vec<u8> {
    let (pairs, remainder) = text.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

// Independent fixture construction: never calls the production codec/hash helper.
fn fixture(minor: u16, records: &[(u16, u16, u32, u32, &[u8])]) -> Vec<u8> {
    let start = 64 + records.len() * 32;
    let mut bytes = vec![0; start];
    bytes[..8].copy_from_slice(b"EPITLCX1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&minor.to_le_bytes());
    bytes[24..28].copy_from_slice(&(records.len() as u32).to_le_bytes());
    bytes[28..32].copy_from_slice(&64_u32.to_le_bytes());
    for (index, (tag, version, flags, count, payload)) in records.iter().enumerate() {
        let offset = bytes.len() as u64;
        let entry = 64 + index * 32;
        bytes[entry..entry + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[entry + 2..entry + 4].copy_from_slice(&version.to_le_bytes());
        bytes[entry + 4..entry + 8].copy_from_slice(&flags.to_le_bytes());
        bytes[entry + 8..entry + 16].copy_from_slice(&offset.to_le_bytes());
        bytes[entry + 16..entry + 24].copy_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes[entry + 24..entry + 28].copy_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(payload);
    }
    let length = (bytes.len() - start) as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    rehash(&mut bytes);
    bytes
}

fn rehash(bytes: &mut [u8]) {
    bytes[32..64].fill(0);
    let mut hash = Sha256::new();
    hash.update(b"Epistesys/TLCX/FrameDigest/sha256/v1\0");
    hash.update(&*bytes);
    bytes[32..64].copy_from_slice(&hash.finalize());
}

fn source_frame(source: &[u8]) -> Vec<u8> {
    fixture(0, &[(SOURCE_UTF8, 1, CRITICAL, 1, source)])
}

#[test]
fn tlcx_03_corruption_and_every_truncated_prefix_are_rejected() {
    let original = unhex(G1);
    for end in 0..original.len() {
        assert!(decode_envelope(&original[..end]).is_err(), "prefix {end}");
    }
    for index in 0..original.len() {
        let mut altered = original.clone();
        altered[index] ^= 1;
        assert!(decode_envelope(&altered).is_err(), "byte {index}");
    }
}

#[test]
fn tlcx_03_independent_golden_preserves_opaque_bytes_exactly() {
    assert_eq!(FRAME_DOMAIN, b"Epistesys/TLCX/FrameDigest/sha256/v1\0");
    let bytes = unhex(G1);
    assert_eq!(bytes.len(), 99);
    let frame = decode_envelope(&bytes).unwrap();
    assert_eq!(frame.original_bytes(), bytes);
    assert_eq!(frame.minor(), 0);
    assert_eq!(frame.sections().len(), 1);
    assert_eq!(frame.sections()[0].kind(), SectionKind::Opaque);
    assert_eq!(frame.sections()[0].payload(), "日".as_bytes());
    assert!(matches!(
        frame.source_profile(),
        Err(CodecError::MissingSource)
    ));
    let encoded = encode_frame(&[SectionInput {
        tag: 0xfffe,
        version: 1,
        flags: 0,
        count: 1,
        payload: "日".as_bytes(),
    }])
    .unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn tlcx_03_header_versions_flags_counts_and_reserved_bytes_fail_closed() {
    let original = unhex(G1);
    for (offset, replacement) in [(0, 0), (8, 2), (12, 1), (24, 65), (28, 63), (92, 1)] {
        let mut bytes = original.clone();
        bytes[offset] = replacement;
        rehash(&mut bytes);
        assert!(decode_envelope(&bytes).is_err(), "offset {offset}");
    }
    let mut bytes = original;
    bytes[68..72].copy_from_slice(&2_u32.to_le_bytes());
    rehash(&mut bytes);
    assert!(matches!(
        decode_envelope(&bytes),
        Err(CodecError::UnknownFlags)
    ));
}

#[test]
fn tlcx_03_directory_rejects_order_duplicates_gaps_overlap_and_overflow() {
    for records in [
        vec![(9, 1, 0, 0, &b""[..]), (8, 1, 0, 0, &b""[..])],
        vec![(9, 1, 0, 0, &b""[..]), (9, 1, 0, 0, &b""[..])],
    ] {
        assert!(matches!(
            decode_envelope(&fixture(0, &records)),
            Err(CodecError::NonCanonicalTagOrder)
        ));
    }
    for offset in [0_u64, 64, 95, 97, u64::MAX] {
        let mut bytes = unhex(G1);
        bytes[72..80].copy_from_slice(&offset.to_le_bytes());
        rehash(&mut bytes);
        assert!(decode_envelope(&bytes).is_err());
    }
    let mut bytes = unhex(G1);
    bytes[80..88].copy_from_slice(&u64::MAX.to_le_bytes());
    rehash(&mut bytes);
    assert!(decode_envelope(&bytes).is_err());
    let mut trailing = unhex(G1);
    trailing.push(0);
    assert!(matches!(
        decode_envelope(&trailing),
        Err(CodecError::LengthMismatch)
    ));
}

#[test]
fn tlcx_03_unknown_critical_and_unsupported_versions_are_not_typed_values() {
    let critical = fixture(0, &[(0xfffe, 1, CRITICAL, 1, b"opaque")]);
    assert!(matches!(
        decode_envelope(&critical),
        Err(CodecError::UnknownCritical)
    ));
    let critical_version = fixture(0, &[(SOURCE_UTF8, 2, CRITICAL, 1, b"future")]);
    assert!(matches!(
        decode_envelope(&critical_version),
        Err(CodecError::UnsupportedSectionVersion)
    ));
    let optional = fixture(0, &[(SOURCE_UTF8, 2, 0, 1, &[0xff])]);
    let frame = decode_envelope(&optional).unwrap();
    assert_eq!(frame.sections()[0].kind(), SectionKind::Opaque);
    assert!(frame.source_profile().is_err());
}

#[test]
fn tlcx_03_source_profile_preserves_raw_utf8_and_requires_critical_minor_zero() {
    let source = "line\r\nあ\0🦀";
    let bytes = source_frame(source.as_bytes());
    assert_eq!(
        decode_envelope(&bytes)
            .unwrap()
            .source_profile()
            .unwrap()
            .source(),
        source
    );
    let empty = source_frame(b"");
    assert_eq!(
        decode_envelope(&empty)
            .unwrap()
            .source_profile()
            .unwrap()
            .source(),
        ""
    );
    let optional = fixture(0, &[(SOURCE_UTF8, 1, 0, 1, b"source")]);
    assert!(matches!(
        decode_envelope(&optional).unwrap().source_profile(),
        Err(CodecError::SourceNotCritical)
    ));
    let future = fixture(7, &[(SOURCE_UTF8, 1, CRITICAL, 1, b"source")]);
    let view = decode_envelope(&future).unwrap();
    assert_eq!(view.minor(), 7);
    assert_eq!(view.original_bytes(), future);
    assert!(matches!(
        view.source_profile(),
        Err(CodecError::UnsupportedMinor)
    ));
}

#[test]
fn tlcx_03_known_source_rejects_invalid_utf8_count_and_size() {
    let invalid = source_frame(&[0xff]);
    assert!(matches!(
        decode_envelope(&invalid),
        Err(CodecError::InvalidUtf8)
    ));
    let count = fixture(0, &[(SOURCE_UTF8, 1, CRITICAL, 0, b"")]);
    assert!(matches!(
        decode_envelope(&count),
        Err(CodecError::CountMismatch)
    ));
    let oversized = vec![b'a'; MAX_STRING_BYTES + 1];
    assert!(decode_envelope(&source_frame(&oversized)).is_err());
}

#[test]
fn tlcx_03_map_writer_sorts_byte_keys_and_decoder_checks_count_and_order() {
    let first = encode_string_map(&[("z", "last"), ("a", "first")]).unwrap();
    let second = encode_string_map(&[("a", "first"), ("z", "last")]).unwrap();
    assert_eq!(first, second);
    assert!(encode_string_map(&[("a", "one"), ("a", "two")]).is_err());
    let bytes = fixture(0, &[(STRING_MAP, 1, 0, 2, &first)]);
    let view = decode_envelope(&bytes).unwrap();
    assert_eq!(
        view.sections()[0].string_map().unwrap(),
        vec![("a", "first"), ("z", "last")]
    );
    for count in [0, 1, 3, MAX_MAP_ENTRIES as u32 + 1] {
        let wrong = fixture(0, &[(STRING_MAP, 1, 0, count, &first)]);
        assert!(decode_envelope(&wrong).is_err());
    }
    let one = encode_string_map(&[("z", "last")]).unwrap();
    let two = encode_string_map(&[("a", "first")]).unwrap();
    let reversed = [one, two].concat();
    let wrong = fixture(0, &[(STRING_MAP, 1, 0, 2, &reversed)]);
    assert!(matches!(
        decode_envelope(&wrong),
        Err(CodecError::NonCanonicalMap)
    ));
}

#[test]
fn tlcx_03_map_string_lengths_and_invalid_utf8_are_rejected_without_defaulting() {
    let oversized = "x".repeat(MAX_STRING_BYTES + 1);
    assert!(encode_string_map(&[("a", oversized.as_str())]).is_err());
    for payload in [&[1, 0, 0, 0, 0xff, 0, 0, 0, 0][..], &[0xff; 8][..]] {
        let wrong = fixture(0, &[(STRING_MAP, 1, 0, 1, payload)]);
        assert!(decode_envelope(&wrong).is_err());
    }
}

#[test]
fn tlcx_03_digest_domain_cannot_be_replaced_by_a_plain_payload_hash() {
    let mut bytes = unhex(G1);
    bytes[32..64].fill(0);
    let plain = Sha256::digest(&bytes);
    bytes[32..64].copy_from_slice(&plain);
    assert!(matches!(
        decode_envelope(&bytes),
        Err(CodecError::DigestMismatch)
    ));
}

#[test]
fn tlcx_03_encoder_checks_sections_and_total_frame_before_publication() {
    let sections = vec![
        SectionInput {
            tag: 3,
            version: 1,
            flags: 0,
            count: 0,
            payload: b""
        };
        65
    ];
    assert!(encode_frame(&sections).is_err());
    let huge = vec![0; MAX_FRAME_BYTES];
    assert!(encode_frame(&[SectionInput {
        tag: 3,
        version: 1,
        flags: 0,
        count: 0,
        payload: &huge
    }])
    .is_err());
    let empty = encode_frame(&[]).unwrap();
    let view = decode_envelope(&empty).unwrap();
    assert!(view.sections().is_empty());
    assert!(matches!(
        view.source_profile(),
        Err(CodecError::MissingSource)
    ));
}

#[test]
fn tlcx_03_reader_rejects_oversize_header_before_reading_a_body() {
    struct HeaderOnly {
        bytes: Cursor<Vec<u8>>,
        read: usize,
    }
    impl Read for HeaderOnly {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let count = self.bytes.read(output)?;
            self.read += count;
            Ok(count)
        }
    }
    let mut header = unhex(G1)[..64].to_vec();
    header[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut reader = HeaderOnly {
        bytes: Cursor::new(header),
        read: 0,
    };
    assert!(read_frame(&mut reader, MAX_FRAME_BYTES).is_err());
    assert_eq!(reader.read, 64);
}

#[test]
fn tlcx_03_reader_consumes_exactly_one_frame_and_never_publishes_a_partial() {
    let one = unhex(G1);
    let mut reader = Cursor::new([one.as_slice(), one.as_slice()].concat());
    assert_eq!(read_frame(&mut reader, MAX_FRAME_BYTES).unwrap(), one);
    assert_eq!(reader.position(), 99);
    assert_eq!(read_frame(&mut reader, MAX_FRAME_BYTES).unwrap(), one);
    for end in [0, 32, 64, 98] {
        assert!(read_frame(&mut Cursor::new(&one[..end]), MAX_FRAME_BYTES).is_err());
    }
}

#[test]
fn tlcx_03_small_adversarial_byte_inputs_do_not_panic() {
    let mut state = 0x1234_5678_u32;
    for length in 0..256 {
        let bytes = (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect::<Vec<_>>();
        assert!(decode_envelope(&bytes).is_err());
    }
}

#[test]
fn tlcx_03_bounded_reader_source_profile_reaches_the_same_source_store() {
    use lc631_substrate::source::{SourceSpan, SourceStore};

    let source = "aあ🦀z\r\n\0";
    let frame = source_frame(source.as_bytes());
    let owned = read_frame(&mut Cursor::new(frame), MAX_FRAME_BYTES).unwrap();
    let view = decode_envelope(&owned).unwrap();
    let profile = view.source_profile().unwrap();
    let store = SourceStore::new(Arc::from(profile.source()), MAX_STRING_BYTES).unwrap();
    assert_eq!(profile.source_id(), store.id());
    let span = SourceSpan::checked(&store, 1, 8).unwrap();
    assert_eq!(store.slice(span), Ok("あ🦀"));
    assert_eq!(store.text().as_bytes(), source.as_bytes());
}

#[test]
fn tlcx_03_string_map_preserves_unicode_and_nul_without_normalization() {
    let entries = [("é", "\r\n\0"), ("e\u{301}", "different canonical bytes")];
    let payload = encode_string_map(&entries).unwrap();
    let bytes = fixture(0, &[(STRING_MAP, 1, 0, 2, &payload)]);
    let view = decode_envelope(&bytes).unwrap();
    assert_eq!(
        view.sections()[0].string_map().unwrap(),
        vec![("e\u{301}", "different canonical bytes"), ("é", "\r\n\0")]
    );
}

#[test]
fn tlcx_03_exact_string_and_entry_caps_remain_admissible() {
    let source = vec![b'x'; MAX_STRING_BYTES];
    let bytes = source_frame(&source);
    assert_eq!(
        decode_envelope(&bytes)
            .unwrap()
            .source_profile()
            .unwrap()
            .source()
            .len(),
        MAX_STRING_BYTES,
    );
    let keys = (0..MAX_MAP_ENTRIES)
        .map(|index| format!("{index:06}"))
        .collect::<Vec<_>>();
    let entries = keys
        .iter()
        .map(|key| (key.as_str(), ""))
        .collect::<Vec<_>>();
    let payload = encode_string_map(&entries).unwrap();
    let bytes = fixture(0, &[(STRING_MAP, 1, 0, MAX_MAP_ENTRIES as u32, &payload)]);
    let view = decode_envelope(&bytes).unwrap();
    assert_eq!(
        view.sections()[0].string_map().unwrap().len(),
        MAX_MAP_ENTRIES
    );
}

#[test]
fn tlcx_03_exact_total_frame_cap_counts_header_and_directory() {
    let payload = vec![0; MAX_FRAME_BYTES - 96];
    let bytes = encode_frame(&[SectionInput {
        tag: 3,
        version: 1,
        flags: 0,
        count: 0,
        payload: &payload,
    }])
    .unwrap();
    assert_eq!(bytes.len(), MAX_FRAME_BYTES);
    assert_eq!(
        decode_envelope(&bytes).unwrap().sections()[0]
            .payload()
            .len(),
        payload.len()
    );
    assert_eq!(
        read_frame(&mut Cursor::new(bytes), MAX_FRAME_BYTES)
            .unwrap()
            .len(),
        MAX_FRAME_BYTES
    );
}
