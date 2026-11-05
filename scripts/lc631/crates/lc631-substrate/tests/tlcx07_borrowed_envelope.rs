use lc631_substrate::codec::{
    decode_envelope, decode_envelope_borrowed, encode_frame, encode_string_map, CodecError,
    SectionInput, SectionKind, CRITICAL, FRAME_DOMAIN, SOURCE_UTF8, STRING_MAP,
};
use sha2::{Digest, Sha256};

fn rehash(bytes: &mut [u8]) {
    bytes[32..64].fill(0);
    let mut hash = Sha256::new();
    hash.update(FRAME_DOMAIN);
    hash.update(&*bytes);
    bytes[32..64].copy_from_slice(&hash.finalize());
}

fn compare(bytes: &[u8]) {
    let old = decode_envelope(bytes);
    let borrowed = decode_envelope_borrowed(bytes);
    match (old, borrowed) {
        (Ok(old), Ok(borrowed)) => {
            assert_eq!(old.minor(), borrowed.minor());
            assert_eq!(old.digest(), borrowed.digest());
            assert_eq!(borrowed.original_bytes().as_ptr(), bytes.as_ptr());
            assert_eq!(old.sections().len(), borrowed.section_count());
            for (index, expected) in old.sections().iter().enumerate() {
                let actual = borrowed.section(index).unwrap().unwrap();
                assert_eq!(actual.tag(), expected.tag());
                assert_eq!(actual.version(), expected.version());
                assert_eq!(actual.flags(), expected.flags());
                assert_eq!(actual.count(), expected.count());
                assert_eq!(actual.kind(), expected.kind());
                assert_eq!(actual.payload(), expected.payload());
                assert_eq!(actual.payload().as_ptr(), expected.payload().as_ptr());
            }
            assert!(borrowed
                .section(borrowed.section_count())
                .unwrap()
                .is_none());
            assert!(borrowed.section(usize::MAX).unwrap().is_none());
        }
        (Err(old), Err(borrowed)) => assert_eq!(old, borrowed),
        _ => panic!("owned and borrowed validation disagree"),
    }
}

#[test]
fn tlcx07_borrowed_envelope_borrows_sections_and_matches_owned() {
    let map = encode_string_map(&[("a", "first"), ("z", "last")]).unwrap();
    let bytes = encode_frame(&[
        SectionInput {
            tag: SOURCE_UTF8,
            version: 1,
            flags: CRITICAL,
            count: 1,
            payload: "日".as_bytes(),
        },
        SectionInput {
            tag: STRING_MAP,
            version: 1,
            flags: 0,
            count: 2,
            payload: &map,
        },
        SectionInput {
            tag: 0xfffe,
            version: 9,
            flags: 0,
            count: 0,
            payload: b"opaque",
        },
    ])
    .unwrap();
    compare(&bytes);
    let view = decode_envelope_borrowed(&bytes).unwrap();
    assert_eq!(
        view.section(2).unwrap().unwrap().kind(),
        SectionKind::Opaque
    );
    compare(&encode_frame(&[]).unwrap());
}

#[test]
fn tlcx07_borrowed_envelope_checks_every_section_before_returning() {
    let bytes = encode_frame(&[
        SectionInput {
            tag: 3,
            version: 1,
            flags: 0,
            count: 0,
            payload: b"first",
        },
        SectionInput {
            tag: 4,
            version: 1,
            flags: 0,
            count: 0,
            payload: b"last",
        },
    ])
    .unwrap();
    let mut critical = bytes.clone();
    critical[100..104].copy_from_slice(&CRITICAL.to_le_bytes());
    rehash(&mut critical);
    assert!(matches!(
        decode_envelope_borrowed(&critical),
        Err(CodecError::UnknownCritical)
    ));
    compare(&critical);

    let map = encode_string_map(&[("a", ""), ("z", "")]).unwrap();
    let mut late_map = encode_frame(&[
        SectionInput {
            tag: SOURCE_UTF8,
            version: 1,
            flags: CRITICAL,
            count: 1,
            payload: b"ok",
        },
        SectionInput {
            tag: STRING_MAP,
            version: 1,
            flags: 0,
            count: 2,
            payload: &map,
        },
    ])
    .unwrap();
    // Directory ends at 128; the first payload is two bytes and the second
    // one-byte map key is 13 bytes into the map payload.
    late_map[128 + 2 + 13] = b'a';
    rehash(&mut late_map);
    assert!(matches!(
        decode_envelope_borrowed(&late_map),
        Err(CodecError::NonCanonicalMap)
    ));
    compare(&late_map);
}

#[test]
fn tlcx07_borrowed_envelope_rejects_adversarial_headers_directories_and_payloads() {
    let bytes = encode_frame(&[
        SectionInput {
            tag: SOURCE_UTF8,
            version: 1,
            flags: CRITICAL,
            count: 1,
            payload: b"ok",
        },
        SectionInput {
            tag: 3,
            version: 1,
            flags: 0,
            count: 0,
            payload: b"tail",
        },
    ])
    .unwrap();
    for end in 0..bytes.len() {
        compare(&bytes[..end]);
    }
    for (offset, value) in [(0, 0), (8, 2), (12, 1), (24, 65), (28, 63), (92, 1)] {
        let mut changed = bytes.clone();
        changed[offset] = value;
        rehash(&mut changed);
        compare(&changed);
    }
    for offset in [0_u64, 64, 127, 129, u64::MAX] {
        let mut changed = bytes.clone();
        changed[72..80].copy_from_slice(&offset.to_le_bytes());
        rehash(&mut changed);
        compare(&changed);
    }
    let mut invalid_utf8 = bytes.clone();
    invalid_utf8[128] = 0xff;
    rehash(&mut invalid_utf8);
    assert!(matches!(
        decode_envelope_borrowed(&invalid_utf8),
        Err(CodecError::InvalidUtf8)
    ));
    compare(&invalid_utf8);
    let mut digest_mismatch = bytes.clone();
    digest_mismatch[128] ^= 1;
    assert!(matches!(
        decode_envelope_borrowed(&digest_mismatch),
        Err(CodecError::DigestMismatch)
    ));
    compare(&digest_mismatch);
    let mut duplicate = bytes.clone();
    duplicate[96..98].copy_from_slice(&SOURCE_UTF8.to_le_bytes());
    rehash(&mut duplicate);
    assert!(matches!(
        decode_envelope_borrowed(&duplicate),
        Err(CodecError::NonCanonicalTagOrder)
    ));
    compare(&duplicate);
}

#[test]
fn tlcx07_borrowed_envelope_eagerly_validates_map_and_full_section_limit() {
    let map = encode_string_map(&[("z", ""), ("a", "")]).unwrap();
    let mut frame = encode_frame(&[SectionInput {
        tag: STRING_MAP,
        version: 1,
        flags: 0,
        count: 2,
        payload: &map,
    }])
    .unwrap();
    // Keys are one byte each; rewrite the second to duplicate the first.
    frame[109] = b'a';
    rehash(&mut frame);
    assert!(matches!(
        decode_envelope_borrowed(&frame),
        Err(CodecError::NonCanonicalMap)
    ));
    compare(&frame);

    let inputs: Vec<_> = (0..64)
        .map(|index| SectionInput {
            tag: 100 + index,
            version: 1,
            flags: 0,
            count: 0,
            payload: b"",
        })
        .collect();
    let maximum = encode_frame(&inputs).unwrap();
    compare(&maximum);
    assert_eq!(
        decode_envelope_borrowed(&maximum).unwrap().section_count(),
        64
    );
}
