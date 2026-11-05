//! Versioned full-report binary preimage encoder; does not sign or verify.
use super::{
    translation::write_translation_v3, write_execution_plan_v3, write_observation_v3,
    write_process_receipt_v3, CanonicalWriter,
};
use crate::{BackendFamily, BackendState, MediaBackendExecutionReceipt, MEDIA_BACKEND_SCHEMA};
use std::io::{self, Write};

fn family_code(value: BackendFamily) -> u32 {
    use BackendFamily::*;
    match value {
        Decode => 1,
        ShotBoundary => 2,
        ObjectDetection => 3,
        FaceDetection => 4,
        SegmentationTracking => 5,
        Pose => 6,
        FaceActionUnit => 7,
        ActionRecognition => 8,
        Ocr => 9,
        DepthMotion => 10,
        Asr => 11,
        Diarization => 12,
        AudioEvent => 13,
        CrossModalAlignment => 14,
        FrameDifference => 15,
        AcousticClustering => 16,
        TemporalCoPresence => 17,
    }
}
fn state_code(value: BackendState) -> u32 {
    use BackendState::*;
    match value {
        Unavailable => 1,
        ConfiguredNotExecuted => 2,
        ExecutedCandidate => 3,
        ValidatedObservation => 4,
        FailedWithDiagnostics => 5,
    }
}

/// Write a domain-separated report preimage. A failure may leave partial bytes
/// in the sink: callers must discard the entire sink and must not sign it.
/// This does not validate semantic content, prove execution, or grant authority.
pub fn write_media_report_v3<W: Write>(
    writer: &mut CanonicalWriter<'_, W>,
    report: &MediaBackendExecutionReceipt,
) -> io::Result<()> {
    let MediaBackendExecutionReceipt {
        schema_version,
        source_sha256,
        backend_id,
        backend_version,
        model_revision,
        model_cache_manifest_sha256,
        model_card_license_hint,
        backend_receipt_sha256,
        execution_plan,
        process,
        executed_family_states,
        graph,
        diagnostics,
        source_authority_preserved,
        identity_inference_committed,
        internal_state_committed,
        family_validation_count,
        authenticity_receipt_digest: _,
        authenticity_attestation: _,
        translation_envelope,
        claim_boundary,
    } = report;
    if *schema_version != MEDIA_BACKEND_SCHEMA
        || translation_envelope.schema_version != lc631_tl::TRANSLATION_ENVELOPE_V3_SCHEMA
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported report schema",
        ));
    }
    let mut previous = 0;
    for family in executed_family_states.keys() {
        let code = family_code(*family);
        if code <= previous {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "family order differs from v3",
            ));
        }
        previous = code;
    }
    writer.write_text("lc631/media-backend-report/binary-v3")?;
    writer.write_text(schema_version)?;
    writer.write_text(source_sha256)?;
    writer.write_text(backend_id)?;
    writer.write_text(backend_version)?;
    writer.write_text(model_revision)?;
    writer.write_text(model_cache_manifest_sha256)?;
    writer.write_text(model_card_license_hint)?;
    writer.write_text(backend_receipt_sha256)?;
    write_execution_plan_v3(writer, execution_plan)?;
    write_process_receipt_v3(writer, process)?;
    writer.write_usize(executed_family_states.len())?;
    for (family, state) in executed_family_states {
        writer.write_enum(family_code(*family))?;
        writer.write_enum(state_code(*state))?;
    }
    writer.write_usize(graph.observations().len())?;
    for observation in graph.observations() {
        write_observation_v3(writer, observation)?;
    }
    writer.write_usize(diagnostics.len())?;
    for diagnostic in diagnostics {
        writer.write_text(diagnostic)?;
    }
    writer.write_bool(*source_authority_preserved)?;
    writer.write_bool(*identity_inference_committed)?;
    writer.write_bool(*internal_state_committed)?;
    writer.write_usize(*family_validation_count)?;
    write_translation_v3(writer, translation_envelope)?;
    writer.write_text(claim_boundary)
}
