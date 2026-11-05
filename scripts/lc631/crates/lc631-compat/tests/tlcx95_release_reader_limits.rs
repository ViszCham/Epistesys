//! Returning-read call budgets are separate from byte caps and wall deadlines.
use lc631_compat::tlcx_release_gate::{ArtifactKind, GateError, ObservedArtifact};
use lc631_compat::{hash_capped_reader, CompatError};
use std::io::{self, Read};

const CALL_LIMIT: usize = 65_536;
const ARTIFACT_BYTE_CAP: u64 = 256 * 1024 * 1024;

struct ShortReads {
    remaining: usize,
    calls: usize,
}

impl Read for ShortReads {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.remaining == 0 {
            return Ok(0);
        }
        out[0] = 42;
        self.remaining -= 1;
        Ok(1)
    }
}

struct InterruptedReads {
    remaining: usize,
    calls: usize,
}

impl Read for InterruptedReads {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.remaining == 0 {
            return Ok(0);
        }
        self.remaining -= 1;
        Err(io::ErrorKind::Interrupted.into())
    }
}

#[test]
fn tlcx95_artifact_short_reads_exhaust_calls_before_bytes() {
    // Finite even before the fix: 65,536 one-byte reads, then EOF. There is no
    // timing/watchdog oracle or truly infinite reader in this negative test.
    let mut reader = ShortReads {
        remaining: CALL_LIMIT,
        calls: 0,
    };
    let result = ObservedArtifact::read(ArtifactKind::Binary, &mut reader, 131_072);
    assert_eq!(result.err(), Some(GateError::ReadCallLimit));
    assert_eq!(reader.calls, CALL_LIMIT);
}

#[test]
fn tlcx95_baseline_interruptions_exhaust_calls_before_eof() {
    // The old implementation eventually returns EOF, not a hung test process.
    let mut reader = InterruptedReads {
        remaining: CALL_LIMIT,
        calls: 0,
    };
    let result = hash_capped_reader(&mut reader, 1024);
    assert!(matches!(result, Err(CompatError::ReadCallLimit)));
    assert_eq!(reader.calls, CALL_LIMIT);
}

#[test]
fn tlcx95_artifact_eof_on_last_call_and_interruptions_within_limits_pass() {
    let mut reader = ShortReads {
        remaining: CALL_LIMIT - 1,
        calls: 0,
    };
    let artifact = ObservedArtifact::read(ArtifactKind::Source, &mut reader, 131_072).unwrap();
    assert_eq!(artifact.length(), (CALL_LIMIT - 1) as u64);
    assert_eq!(reader.calls, CALL_LIMIT);
    let mut interrupted = InterruptedReads {
        remaining: 1024,
        calls: 0,
    };
    let empty = ObservedArtifact::read(ArtifactKind::Source, &mut interrupted, 1).unwrap();
    assert_eq!(empty.length(), 0);
    assert_eq!(interrupted.calls, 1025);
    let mut too_many = InterruptedReads {
        remaining: 1025,
        calls: 0,
    };
    assert_eq!(
        ObservedArtifact::read(ArtifactKind::Source, &mut too_many, 1).err(),
        Some(GateError::InterruptedLimit)
    );
    assert_eq!(too_many.calls, 1025);
}

#[test]
fn tlcx95_baseline_finite_interruptions_and_short_reads_pass() {
    let mut interrupted = InterruptedReads {
        remaining: 1024,
        calls: 0,
    };
    assert_eq!(
        hash_capped_reader(&mut interrupted, 1).unwrap(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(interrupted.calls, 1025);
    let mut reader = ShortReads {
        remaining: 3,
        calls: 0,
    };
    assert!(hash_capped_reader(&mut reader, 3).is_ok());
    assert_eq!(
        reader.calls, 4,
        "the exact byte cap still requires an EOF call"
    );
    let mut last_eof = ShortReads {
        remaining: CALL_LIMIT - 1,
        calls: 0,
    };
    assert!(hash_capped_reader(&mut last_eof, 131_072).is_ok());
    assert_eq!(last_eof.calls, CALL_LIMIT);
}

#[test]
fn tlcx95_baseline_progress_reads_also_consume_the_call_budget() {
    let mut reader = ShortReads {
        remaining: CALL_LIMIT,
        calls: 0,
    };
    assert!(matches!(
        hash_capped_reader(&mut reader, 131_072),
        Err(CompatError::ReadCallLimit)
    ));
    assert_eq!(reader.calls, CALL_LIMIT);
}

#[test]
fn tlcx95_artifact_full_byte_cap_regular_stream_retains_eof_sentinel() {
    // Stream the full existing 256 MiB profile without allocating/copying a
    // giant buffer. Full 8 KiB reads plus EOF require only 32,769 calls.
    let mut reader = io::repeat(42).take(ARTIFACT_BYTE_CAP);
    let artifact =
        ObservedArtifact::read(ArtifactKind::Binary, &mut reader, ARTIFACT_BYTE_CAP).unwrap();
    assert_eq!(artifact.length(), ARTIFACT_BYTE_CAP);
}
