#![forbid(unsafe_code)]

// These independent vectors define only the proposed v3 primitive encoding,
// not a complete report codec, signature, decoder, or backend execution.
use lc631_media::attestation_v3::CanonicalWriter;
use std::io::{self, Write};

#[test]
fn literal_text_pair_has_tagged_length_delimited_little_endian_bytes() {
    let mut bytes = Vec::new();
    let mut writer = CanonicalWriter::new(&mut bytes, 64);
    writer.write_text("a").unwrap();
    writer.write_text("bc").unwrap();
    assert_eq!(
        bytes,
        [1, 1, 0, 0, 0, 0, 0, 0, 0, b'a', 1, 2, 0, 0, 0, 0, 0, 0, 0, b'b', b'c',]
    );
}

#[test]
fn same_concatenation_different_fields_does_not_collide() {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut lhs = CanonicalWriter::new(&mut left, 64);
    lhs.write_text("a").unwrap();
    lhs.write_text("bc").unwrap();
    let mut rhs = CanonicalWriter::new(&mut right, 64);
    rhs.write_text("ab").unwrap();
    rhs.write_text("c").unwrap();
    assert_ne!(left, right);
}

#[test]
fn absent_and_present_empty_are_distinct() {
    let mut none = Vec::new();
    let mut empty = Vec::new();
    CanonicalWriter::new(&mut none, 64)
        .write_optional_text(None)
        .unwrap();
    CanonicalWriter::new(&mut empty, 64)
        .write_optional_text(Some(""))
        .unwrap();
    assert_eq!(none, [2]);
    assert_eq!(empty, [3, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_ne!(none, empty);
}

#[test]
fn over_limit_field_is_rejected_before_any_of_its_bytes_are_written() {
    let mut bytes = Vec::new();
    assert!(CanonicalWriter::new(&mut bytes, 10)
        .write_text("ab")
        .is_err());
    assert!(bytes.is_empty());
}

struct BrokenWriter;
impl Write for BrokenWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("synthetic sink failure"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn sink_failure_is_returned_not_turned_into_success() {
    assert!(CanonicalWriter::new(&mut BrokenWriter, 64)
        .write_text("x")
        .is_err());
}

#[test]
fn exact_limit_is_accepted_then_next_field_is_rejected_without_change() {
    let mut bytes = Vec::new();
    let mut writer = CanonicalWriter::new(&mut bytes, 10);
    writer.write_text("a").unwrap();
    assert!(writer.write_optional_text(None).is_err());
    assert_eq!(bytes, [1, 1, 0, 0, 0, 0, 0, 0, 0, b'a']);
}

struct FailsOnce(bool);
impl Write for FailsOnce {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0 {
            self.0 = false;
            Err(io::Error::other("failed"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn sink_error_poisons_writer_even_if_sink_later_recovers() {
    let mut sink = FailsOnce(true);
    let mut writer = CanonicalWriter::new(&mut sink, 64);
    assert!(writer.write_text("a").is_err());
    assert!(writer.write_text("b").is_err());
}
