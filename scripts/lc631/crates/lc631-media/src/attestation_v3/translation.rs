//! Explicit v3 encoding of the existing translation-envelope value.
//! Encoding does not validate target/source correspondence or prove a gate.
use super::CanonicalWriter;
use lc631_core::SourceSpan;
use lc631_tl::{
    CanonicalTranslationEnvelope, DomainTranslationPayload, PreservationState, ProjectionEdgeV3,
    ProjectionGateDecision, ProjectionStage, SourceAnchorV3, TargetClaim, VerificationReceipt,
    VerificationStatus, VerifierKind,
};
use std::io::{self, Write};

fn optional<W: Write, T>(
    writer: &mut CanonicalWriter<'_, W>,
    value: Option<&T>,
    encode: impl FnOnce(&mut CanonicalWriter<'_, W>, &T) -> io::Result<()>,
) -> io::Result<()> {
    writer.admit(1)?;
    match value {
        None => writer.emit(&[0x20]),
        Some(value) => {
            writer.emit(&[0x21])?;
            encode(writer, value)
        }
    }
}

fn span<W: Write>(writer: &mut CanonicalWriter<'_, W>, value: &SourceSpan) -> io::Result<()> {
    let SourceSpan { start, end } = value;
    writer.write_usize(*start)?;
    writer.write_usize(*end)
}

fn target<W: Write>(writer: &mut CanonicalWriter<'_, W>, value: &TargetClaim) -> io::Result<()> {
    let TargetClaim {
        id,
        artifact,
        path,
        revision,
        content_digest,
        span: target_span,
    } = value;
    writer.write_u64(id.0)?;
    writer.write_text(artifact)?;
    writer.write_text(path)?;
    writer.write_text(revision)?;
    writer.write_text(content_digest)?;
    optional(writer, target_span.as_ref(), span)
}

fn source<W: Write>(writer: &mut CanonicalWriter<'_, W>, value: &SourceAnchorV3) -> io::Result<()> {
    let SourceAnchorV3 {
        ledger_id,
        revision,
        span: source_span,
        surface_digest,
    } = value;
    writer.write_u64(ledger_id.0)?;
    writer.write_text(revision)?;
    span(writer, source_span)?;
    writer.write_text(surface_digest)
}

fn stage_code(value: ProjectionStage) -> u32 {
    use ProjectionStage::*;
    match value {
        SeedToContract => 1,
        ContractToProgram => 2,
        ProgramToCandidate => 3,
        CandidateToOutput => 4,
        AudioToTranscript => 5,
        FrameToObservation => 6,
        ObservationToEvent => 7,
    }
}
fn preservation_code(value: PreservationState) -> u32 {
    use PreservationState::*;
    match value {
        Exact => 1,
        Refinement => 2,
        Weakened => 3,
        Strengthened => 4,
        Contradicted => 5,
        Dropped => 6,
        IntroducedWithoutSource => 7,
        Unresolved => 8,
        Incomparable => 9,
    }
}
fn verifier_code(value: VerifierKind) -> u32 {
    use VerifierKind::*;
    match value {
        SourceSpan => 1,
        Schema => 2,
        TypeChecker => 3,
        Compiler => 4,
        Smt => 5,
        Roundtrip => 6,
        RuntimeObservation => 7,
        HumanReview => 8,
        ModelAdvisory => 9,
    }
}
fn status_code(value: VerificationStatus) -> u32 {
    use VerificationStatus::*;
    match value {
        Passed => 1,
        Failed => 2,
        NeedsEvidence => 3,
        Unavailable => 4,
        AdvisoryOnly => 5,
    }
}
fn gate_code(value: ProjectionGateDecision) -> u32 {
    use ProjectionGateDecision::*;
    match value {
        CandidateCommit => 1,
        Review => 2,
        Clarify => 3,
        HardHold => 4,
    }
}

fn verification<W: Write>(
    writer: &mut CanonicalWriter<'_, W>,
    value: &VerificationReceipt,
) -> io::Result<()> {
    let VerificationReceipt {
        verifier,
        status,
        revision,
        evidence_digest,
    } = value;
    writer.write_enum(verifier_code(*verifier))?;
    writer.write_enum(status_code(*status))?;
    writer.write_text(revision)?;
    writer.write_optional_text(evidence_digest.as_deref())
}

fn edge<W: Write>(writer: &mut CanonicalWriter<'_, W>, value: &ProjectionEdgeV3) -> io::Result<()> {
    let ProjectionEdgeV3 {
        id,
        obligation_id,
        source: source_anchor,
        targets,
        stage,
        state,
        rule_id,
        verifier,
    } = value;
    writer.write_u64(id.0)?;
    writer.write_optional_u64(obligation_id.map(|id| id.0))?;
    optional(writer, source_anchor.as_ref(), source)?;
    writer.write_usize(targets.len())?;
    for target in targets {
        writer.write_u64(target.0)?;
    }
    writer.write_enum(stage_code(*stage))?;
    writer.write_enum(preservation_code(*state))?;
    writer.write_text(rule_id)?;
    verification(writer, verifier)
}

fn payload<W: Write>(
    writer: &mut CanonicalWriter<'_, W>,
    value: &DomainTranslationPayload,
) -> io::Result<()> {
    let DomainTranslationPayload {
        domain,
        schema_version,
        payload_digest,
        unknowns,
    } = value;
    writer.write_text(domain)?;
    writer.write_text(schema_version)?;
    writer.write_text(payload_digest)?;
    writer.write_usize(unknowns.len())?;
    for unknown in unknowns {
        writer.write_text(unknown)?;
    }
    Ok(())
}

/// Writes only an envelope component, not a whole report or an attestation.
/// An error may leave partial bytes in the sink, which the caller must discard.
pub fn write_translation_v3<W: Write>(
    writer: &mut CanonicalWriter<'_, W>,
    envelope: &CanonicalTranslationEnvelope,
) -> io::Result<()> {
    let CanonicalTranslationEnvelope {
        schema_version,
        source_ledger_id,
        source_revision,
        source_digest,
        target_claims,
        edges,
        domain_payloads,
        gate,
        scalar_aggregate_used,
    } = envelope;
    writer.write_text(schema_version)?;
    writer.write_u64(source_ledger_id.0)?;
    writer.write_text(source_revision)?;
    writer.write_text(source_digest)?;
    writer.write_usize(target_claims.len())?;
    for claim in target_claims {
        target(writer, claim)?;
    }
    writer.write_usize(edges.len())?;
    for item in edges {
        edge(writer, item)?;
    }
    writer.write_usize(domain_payloads.len())?;
    for item in domain_payloads {
        payload(writer, item)?;
    }
    writer.write_enum(gate_code(*gate))?;
    writer.write_bool(*scalar_aggregate_used)
}
