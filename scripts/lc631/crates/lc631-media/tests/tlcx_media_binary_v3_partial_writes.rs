//! 日本語: 部分sinkは失敗結果であり、object全体を破棄して署名しない。
//! English: Failed partial sinks must be discarded as whole objects, never signed.
#![forbid(unsafe_code)]

use lc631_media::attestation_v3::{write_media_report_v3, write_observation_v3, CanonicalWriter};
use lc631_media::{
    MediaBackendExecutionReceipt, MediaExecutionPlan, MediaObservation, MediaProcessReceipt,
    MediaResourceBudget, MultimodalEventGraph, ObservationKind, RationalTime, TimeSpan,
    MEDIA_BACKEND_SCHEMA,
};
use lc631_tl::{CanonicalTranslationEnvelope, ProjectionDefectGraphV3, SourceLedgerId};
use std::collections::BTreeMap;
use std::io::{self, Write};

struct ShortSink(Vec<u8>);
impl Write for ShortSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(1);
        self.0.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// Succeeds through a real prefix, fails once, then would recover if called.
// WriteZero and Other must poison the writer; Interrupted must be retried.
struct PrefixFault {
    bytes: Vec<u8>,
    after: usize,
    kind: io::ErrorKind,
    fired: bool,
    calls: usize,
}
impl PrefixFault {
    fn new(after: usize, kind: io::ErrorKind) -> Self {
        Self {
            bytes: Vec::new(),
            after,
            kind,
            fired: false,
            calls: 0,
        }
    }
}
impl Write for PrefixFault {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if !self.fired && self.bytes.len() == self.after {
            self.fired = true;
            return if self.kind == io::ErrorKind::WriteZero {
                Ok(0)
            } else {
                Err(io::Error::new(self.kind, "fixture prefix failure"))
            };
        }
        let count = if self.fired {
            bytes.len()
        } else {
            bytes.len().min(self.after - self.bytes.len())
        };
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn observation() -> MediaObservation {
    MediaObservation {
        observation_id: "a".into(),
        kind: ObservationKind::ShotBoundary,
        span: TimeSpan {
            start: RationalTime {
                ticks: 0,
                timescale: 1,
            },
            end: RationalTime {
                ticks: 1,
                timescale: 1,
            },
        },
        track_id: None,
        label: String::new(),
        backend_id: String::new(),
        model_revision: String::new(),
        evidence_digest: String::new(),
        source_fragment_digest: String::new(),
        provenance_digest: String::new(),
        configuration_digest: String::new(),
        candidate_only: true,
        identity_committed: false,
        internal_state_committed: false,
    }
}

// Encoder-only fixture: empty digest strings are not authenticated media.
fn report() -> MediaBackendExecutionReceipt {
    let projection = ProjectionDefectGraphV3::new(SourceLedgerId(1), "rev", "source").unwrap();
    MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: String::new(),
        backend_id: String::new(),
        backend_version: String::new(),
        model_revision: String::new(),
        model_cache_manifest_sha256: String::new(),
        model_card_license_hint: String::new(),
        backend_receipt_sha256: String::new(),
        execution_plan: MediaExecutionPlan {
            source_snapshot_sha256: String::new(),
            source_bytes: 0,
            python_sha256: String::new(),
            backend_script_sha256: String::new(),
            ffmpeg_sha256: String::new(),
            model_cache_manifest_sha256: String::new(),
            configuration_digest: String::new(),
            resource_budget: MediaResourceBudget::default(),
            network_policy: String::new(),
            network_isolation_os_enforced: false,
        },
        process: MediaProcessReceipt {
            exit_code: None,
            stdout_digest: String::new(),
            stderr_digest: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
            process_tree_kill_attempted: false,
            process_tree_kill_verified: false,
            memory_limit_bytes: None,
            memory_limit_kind: String::new(),
        },
        executed_family_states: BTreeMap::new(),
        graph: MultimodalEventGraph::default(),
        diagnostics: vec![],
        source_authority_preserved: false,
        identity_inference_committed: false,
        internal_state_committed: false,
        family_validation_count: 0,
        authenticity_receipt_digest: None,
        authenticity_attestation: None,
        translation_envelope: CanonicalTranslationEnvelope::from_graph(&projection, vec![]),
        claim_boundary: "encoder fixture only",
    }
}

#[test]
fn successful_primitive_prefix_survives_next_field_size_failure_without_extra_bytes() {
    let mut bytes = Vec::new();
    let mut writer = CanonicalWriter::new(&mut bytes, 11);
    writer.write_text("a").unwrap();
    assert!(writer.write_text("b").is_err());
    // Capacity errors do not poison an otherwise intact primitive stream.
    // This does not authorize reuse of a failed composite object.
    writer.write_optional_text(None).unwrap();
    assert_eq!(bytes, [1, 1, 0, 0, 0, 0, 0, 0, 0, b'a', 2]);
}

#[test]
fn failed_observation_and_full_report_leave_only_discardable_partial_objects() {
    let mut observation_prefix = Vec::new();
    let result = write_observation_v3(
        &mut CanonicalWriter::new(&mut observation_prefix, 11),
        &observation(),
    );
    assert!(result.is_err());
    assert_eq!(observation_prefix, [1, 1, 0, 0, 0, 0, 0, 0, 0, b'a']);
    // The successful id field is not a successful observation.
    drop(observation_prefix);

    let report = report();
    let mut complete = Vec::new();
    write_media_report_v3(&mut CanonicalWriter::new(&mut complete, 16384), &report).unwrap();
    let limit = u64::try_from(complete.len() - 1).unwrap();
    let mut partial = Vec::new();
    assert!(
        write_media_report_v3(&mut CanonicalWriter::new(&mut partial, limit), &report).is_err()
    );
    assert!(!partial.is_empty());
    assert!(partial.len() < complete.len());
    assert_eq!(partial, complete[..partial.len()]);
    // No rollback is promised: discard the whole failed sink, never hash/sign it.
    drop(partial);
}

#[test]
fn short_writes_complete_the_exact_utf8_byte_length_vector() {
    let mut sink = ShortSink(Vec::new());
    let mut writer = CanonicalWriter::new(&mut sink, 12);
    writer.write_text("é\0").unwrap();
    assert_eq!(sink.0, [1, 3, 0, 0, 0, 0, 0, 0, 0, 0xc3, 0xa9, 0]);
}

#[test]
fn interrupted_write_after_a_prefix_is_retried_without_poisoning() {
    let mut sink = PrefixFault::new(4, io::ErrorKind::Interrupted);
    let mut writer = CanonicalWriter::new(&mut sink, 11);
    writer.write_text("a").unwrap();
    writer.write_optional_text(None).unwrap();
    assert!(sink.fired);
    assert_eq!(sink.bytes, [1, 1, 0, 0, 0, 0, 0, 0, 0, b'a', 2]);
}

#[test]
fn write_zero_after_a_prefix_poisons_even_a_recoverable_sink() {
    let mut sink = PrefixFault::new(1, io::ErrorKind::WriteZero);
    {
        let mut writer = CanonicalWriter::new(&mut sink, 64);
        assert_eq!(
            writer.write_text("a").unwrap_err().kind(),
            io::ErrorKind::WriteZero
        );
        assert!(writer.write_text("b").is_err());
        assert!(writer.write_optional_text(None).is_err());
    }
    assert!(sink.fired);
    assert_eq!(sink.calls, 2);
    assert_eq!(sink.bytes, [1]);
    // Sink recovery is real; the rejected retries above did not reach it.
    sink.write_all(b"recoverable").unwrap();
    // This fixture buffer contains failed output and must still be discarded.
    drop(sink);
}

#[test]
fn mid_length_sink_error_preserves_only_a_failed_prefix_and_poisons_retries() {
    let mut sink = PrefixFault::new(4, io::ErrorKind::BrokenPipe);
    {
        let mut writer = CanonicalWriter::new(&mut sink, 64);
        assert_eq!(
            writer.write_text("a").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert!(writer.write_text("b").is_err());
        assert!(writer.write_optional_text(Some("")).is_err());
    }
    assert!(sink.fired);
    assert_eq!(sink.calls, 3);
    assert_eq!(sink.bytes, [1, 1, 0, 0]);
    drop(sink);
}

#[test]
fn composite_sink_failure_cannot_be_returned_as_a_successful_object() {
    let mut complete = Vec::new();
    write_observation_v3(
        &mut CanonicalWriter::new(&mut complete, 4096),
        &observation(),
    )
    .unwrap();
    let mut sink = PrefixFault::new(17, io::ErrorKind::BrokenPipe);
    {
        let mut writer = CanonicalWriter::new(&mut sink, 4096);
        assert!(write_observation_v3(&mut writer, &observation()).is_err());
        assert!(write_observation_v3(&mut writer, &observation()).is_err());
    }
    assert!(sink.fired);
    assert_eq!(sink.bytes, complete[..17]);
    assert!(sink.bytes.len() < complete.len());
    // An error leaves no successful object to publish; discard the entire sink.
    drop(sink);
}
