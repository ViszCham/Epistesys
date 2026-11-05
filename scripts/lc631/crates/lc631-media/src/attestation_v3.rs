//! Bounded v3 report preimages, streaming hashes and explicit verification.
//! No binary decoder is provided. Execution selects v3 only through explicitly
//! named additive APIs; existing v1/v2 preimages and receipts stay separate.

use std::io::{self, Write};

mod authentication;
mod clock;
mod report;
mod source_hash;
pub mod translation;
pub use authentication::{
    media_backend_payload_digest_v3, media_backend_scope_v3, verify_media_backend_attestation_v3,
    MediaAttestationV3Error, MAX_MEDIA_ATTESTATION_V3_BYTES,
};
mod current_authority;
pub(crate) use current_authority::verify_from_execution;
pub use current_authority::{
    verify_media_backend_with_current_authority_v3,
    verify_media_backend_with_current_authority_v3_with_resources,
};
pub use report::write_media_report_v3;

/// Bounded, length-delimited writes. A failed underlying write poisons this
/// writer because the sink may already contain a partial field.
pub struct CanonicalWriter<'a, W: Write> {
    sink: &'a mut W,
    limit: u64,
    written: u64,
    poisoned: bool,
}

impl<'a, W: Write> CanonicalWriter<'a, W> {
    pub fn new(sink: &'a mut W, limit: u64) -> Self {
        Self {
            sink,
            limit,
            written: 0,
            poisoned: false,
        }
    }

    fn admit(&self, bytes: u64) -> io::Result<()> {
        if self.poisoned {
            return Err(io::Error::other("canonical writer poisoned"));
        }
        let total = self
            .written
            .checked_add(bytes)
            .ok_or_else(|| io::Error::other("canonical size overflow"))?;
        if total > self.limit {
            return Err(io::Error::other("canonical size limit"));
        }
        Ok(())
    }

    fn emit(&mut self, bytes: &[u8]) -> io::Result<()> {
        let len = u64::try_from(bytes.len())
            .map_err(|_| io::Error::other("canonical length overflow"))?;
        let next = self
            .written
            .checked_add(len)
            .ok_or_else(|| io::Error::other("canonical size overflow"))?;
        if let Err(error) = self.sink.write_all(bytes) {
            self.poisoned = true;
            return Err(error);
        }
        self.written = next;
        Ok(())
    }

    fn text_size(value: &str) -> io::Result<(u64, u64)> {
        let length = u64::try_from(value.len())
            .map_err(|_| io::Error::other("canonical length overflow"))?;
        let total = length
            .checked_add(9)
            .ok_or_else(|| io::Error::other("canonical size overflow"))?;
        Ok((length, total))
    }

    fn emit_text(&mut self, value: &str, length: u64) -> io::Result<()> {
        self.emit(&[1])?;
        self.emit(&length.to_le_bytes())?;
        self.emit(value.as_bytes())
    }

    /// 0x01, UTF-8 byte length as u64 little-endian, then UTF-8 bytes.
    pub fn write_text(&mut self, value: &str) -> io::Result<()> {
        let (length, total) = Self::text_size(value)?;
        self.admit(total)?;
        self.emit_text(value, length)
    }

    /// 0x02 for absent; 0x03 followed by a text field for present.
    pub fn write_optional_text(&mut self, value: Option<&str>) -> io::Result<()> {
        match value {
            None => {
                self.admit(1)?;
                self.emit(&[2])
            }
            Some(value) => {
                let (length, total) = Self::text_size(value)?;
                let total = total
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("canonical size overflow"))?;
                self.admit(total)?;
                self.emit(&[3])?;
                self.emit_text(value, length)
            }
        }
    }

    fn write_fixed(&mut self, tag: u8, value: &[u8]) -> io::Result<()> {
        let length = u64::try_from(value.len())
            .map_err(|_| io::Error::other("canonical length overflow"))?;
        self.admit(
            length
                .checked_add(1)
                .ok_or_else(|| io::Error::other("canonical size overflow"))?,
        )?;
        self.emit(&[tag])?;
        self.emit(value)
    }

    fn write_u64(&mut self, value: u64) -> io::Result<()> {
        self.write_fixed(0x10, &value.to_le_bytes())
    }

    fn write_i64(&mut self, value: i64) -> io::Result<()> {
        self.write_fixed(0x11, &value.to_le_bytes())
    }

    fn write_u32(&mut self, value: u32) -> io::Result<()> {
        self.write_fixed(0x12, &value.to_le_bytes())
    }

    fn write_bool(&mut self, value: bool) -> io::Result<()> {
        self.write_fixed(0x13, &[u8::from(value)])
    }

    fn write_enum(&mut self, value: u32) -> io::Result<()> {
        self.write_fixed(0x14, &value.to_le_bytes())
    }

    fn write_optional_u64(&mut self, value: Option<u64>) -> io::Result<()> {
        match value {
            None => {
                self.admit(1)?;
                self.emit(&[0x15])
            }
            Some(value) => {
                self.admit(10)?;
                self.emit(&[0x16])?;
                self.write_u64(value)
            }
        }
    }

    fn write_usize(&mut self, value: usize) -> io::Result<()> {
        self.write_u64(
            u64::try_from(value).map_err(|_| io::Error::other("canonical length overflow"))?,
        )
    }

    fn write_optional_i32(&mut self, value: Option<i32>) -> io::Result<()> {
        match value {
            None => {
                self.admit(1)?;
                self.emit(&[0x17])
            }
            Some(value) => {
                self.admit(10)?;
                self.emit(&[0x18])?;
                self.write_i64(i64::from(value))
            }
        }
    }

    fn write_optional_usize(&mut self, value: Option<usize>) -> io::Result<()> {
        let converted = value
            .map(u64::try_from)
            .transpose()
            .map_err(|_| io::Error::other("canonical length overflow"))?;
        self.write_optional_u64(converted)
    }
}

fn observation_kind_code(kind: crate::ObservationKind) -> u32 {
    use crate::ObservationKind::*;
    match kind {
        ShotBoundary => 1,
        ObjectBox => 2,
        ObjectMask => 3,
        AnonymousPersonTrack => 4,
        PoseLandmarks => 5,
        FaceLandmarks => 6,
        FacialActionUnit => 7,
        ActionCandidate => 8,
        OcrTextCandidate => 9,
        RelativeDepthCandidate => 10,
        MotionCandidate => 11,
        TranscriptCandidate => 12,
        AnonymousSpeakerSegment => 13,
        AudioEventCandidate => 14,
        CrossModalRelationCandidate => 15,
        FaceRegionCandidate => 16,
        AcousticClusterCandidate => 17,
        TemporalCoPresenceCandidate => 18,
    }
}

/// Encode every public observation field in fixed v3 order. This only writes
/// bytes; it does not validate observation semantics or issue a signature.
/// An error may leave a partial object in the sink, which must be discarded.
pub fn write_observation_v3<W: Write>(
    writer: &mut CanonicalWriter<'_, W>,
    observation: &crate::MediaObservation,
) -> io::Result<()> {
    let crate::MediaObservation {
        observation_id,
        kind,
        span,
        track_id,
        label,
        backend_id,
        model_revision,
        evidence_digest,
        source_fragment_digest,
        provenance_digest,
        configuration_digest,
        candidate_only,
        identity_committed,
        internal_state_committed,
    } = observation;
    let crate::TimeSpan { start, end } = span;
    let crate::RationalTime {
        ticks: start_ticks,
        timescale: start_scale,
    } = start;
    let crate::RationalTime {
        ticks: end_ticks,
        timescale: end_scale,
    } = end;
    writer.write_text(observation_id)?;
    writer.write_enum(observation_kind_code(*kind))?;
    writer.write_i64(*start_ticks)?;
    writer.write_u32(*start_scale)?;
    writer.write_i64(*end_ticks)?;
    writer.write_u32(*end_scale)?;
    writer.write_optional_u64(*track_id)?;
    writer.write_text(label)?;
    writer.write_text(backend_id)?;
    writer.write_text(model_revision)?;
    writer.write_text(evidence_digest)?;
    writer.write_text(source_fragment_digest)?;
    writer.write_text(provenance_digest)?;
    writer.write_text(configuration_digest)?;
    writer.write_bool(*candidate_only)?;
    writer.write_bool(*identity_committed)?;
    writer.write_bool(*internal_state_committed)
}

/// Encode the plan and resource limits; this does not prove they were enforced.
pub fn write_execution_plan_v3<W: Write>(
    writer: &mut CanonicalWriter<'_, W>,
    plan: &crate::MediaExecutionPlan,
) -> io::Result<()> {
    let crate::MediaExecutionPlan {
        source_snapshot_sha256,
        source_bytes,
        python_sha256,
        backend_script_sha256,
        ffmpeg_sha256,
        model_cache_manifest_sha256,
        configuration_digest,
        resource_budget,
        network_policy,
        network_isolation_os_enforced,
    } = plan;
    let crate::MediaResourceBudget {
        max_stdout_bytes,
        max_stderr_bytes,
        max_temporary_bytes,
        max_decoded_frames,
        max_pcm_samples,
        max_observations,
    } = resource_budget;
    writer.write_text(source_snapshot_sha256)?;
    writer.write_u64(*source_bytes)?;
    writer.write_text(python_sha256)?;
    writer.write_text(backend_script_sha256)?;
    writer.write_text(ffmpeg_sha256)?;
    writer.write_text(model_cache_manifest_sha256)?;
    writer.write_text(configuration_digest)?;
    writer.write_usize(*max_stdout_bytes)?;
    writer.write_usize(*max_stderr_bytes)?;
    writer.write_u64(*max_temporary_bytes)?;
    writer.write_usize(*max_decoded_frames)?;
    writer.write_u64(*max_pcm_samples)?;
    writer.write_usize(*max_observations)?;
    writer.write_text(network_policy)?;
    writer.write_bool(*network_isolation_os_enforced)
}

/// Encode the process report without inferring that the process was genuine.
pub fn write_process_receipt_v3<W: Write>(
    writer: &mut CanonicalWriter<'_, W>,
    process: &crate::MediaProcessReceipt,
) -> io::Result<()> {
    let crate::MediaProcessReceipt {
        exit_code,
        stdout_digest,
        stderr_digest,
        stdout_truncated,
        stderr_truncated,
        process_tree_kill_attempted,
        process_tree_kill_verified,
        memory_limit_bytes,
        memory_limit_kind,
    } = process;
    writer.write_optional_i32(*exit_code)?;
    writer.write_text(stdout_digest)?;
    writer.write_text(stderr_digest)?;
    writer.write_bool(*stdout_truncated)?;
    writer.write_bool(*stderr_truncated)?;
    writer.write_bool(*process_tree_kill_attempted)?;
    writer.write_bool(*process_tree_kill_verified)?;
    writer.write_optional_usize(*memory_limit_bytes)?;
    writer.write_text(memory_limit_kind)
}
