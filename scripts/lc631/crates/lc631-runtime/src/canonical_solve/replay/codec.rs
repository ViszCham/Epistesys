//! Explicit bounded wire. Parsed historical bytes never construct native owners.
use super::super::{types::FormalInput, OneStepBudget, OwnedCanonicalDecision};
use super::{types::*, MAX_REPLAY_BYTES, META_BYTES, META_TAG, TEXT_TAG};
use crate::resource::ResourceBroker;
use lc631_substrate::{
    codec::{decode_envelope_borrowed, encode_frame, SectionInput, CRITICAL, SOURCE_UTF8},
    resource::{
        ResourceAmount, ResourceClaim, ResourceKind, ResourceRequest, ResourceReservationView,
    },
    Digest32,
};

/// Borrows the actual native/decision owner. It cannot outlive that owner or
/// become a serialized Verified token. Recheck again immediately before write.
pub struct ScopedCapture<'d, 'p, 'v> {
    pub(super) bytes: Vec<u8>,
    pub(super) key: ArtifactKey,
    decision: &'d OwnedCanonicalDecision<'p, 'v>,
    broker: &'d ResourceBroker,
    context: Digest32,
    claim: ResourceClaim<'v>,
}
impl ScopedCapture<'_, '_, '_> {
    pub const fn key(&self) -> ArtifactKey {
        self.key
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), ReplayError> {
        self.claim.check()?;
        if !self.claim.same_reservation(view) || view.source_revision() != self.key.source {
            return Err(ReplayError::Binding);
        }
        let historical = decode(&self.bytes, self.key)?;
        let rendered = self.decision.project_current(self.broker, view)?;
        if !self
            .decision
            .matches_exact(historical.source, historical.text.as_bytes())
            || rendered.text() != historical.text
            || historical.context != self.context
            || self
                .decision
                .replay_request()
                .context
                .revision()
                .interpretation_digest()
                != self.context
        {
            return Err(ReplayError::Binding);
        }
        Ok(())
    }
}

pub fn capture_current<'d, 'p, 'v>(
    decision: &'d OwnedCanonicalDecision<'p, 'v>,
    source: &str,
    profile: ReplayProfile,
    broker: &'d ResourceBroker,
    view: &'v ResourceReservationView<'_>,
) -> Result<ScopedCapture<'d, 'p, 'v>, ReplayError> {
    let rendered = decision.project_current(broker, view)?;
    let request = decision.replay_request();
    let expected_step = if profile.objective {
        Some(OneStepBudget::checked(3, 15000)?)
    } else {
        None
    };
    if request.source() != source
        || request.goal != profile.goal_policy()
        || request.context.scope() != profile.scope(source)?
        || request.evaluation.is_some() != profile.evaluation
        || request.objective_step != expected_step
        || !decision.matches_exact(source, rendered.text().as_bytes())
    {
        return Err(ReplayError::Binding);
    }
    if decision.native_model().is_none() {
        return Err(ReplayError::MissingNativeModel);
    }
    if source.is_empty()
        || source.len() > 16384
        || rendered.text().is_empty()
        || rendered.text().len() > 65536
    {
        return Err(ReplayError::Limit);
    }
    let amount = ResourceAmount::checked((MAX_REPLAY_BYTES * 2) as u64)?;
    let claim = view.claim_resources(&[
        ResourceRequest::new(ResourceKind::Ram, amount),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(MAX_REPLAY_BYTES as u64)?,
        ),
    ])?;
    let FormalInput::Problem(problem) = request.input else {
        return Err(ReplayError::Binding);
    };
    let expected = super::context::original_context(problem.source(), profile)?;
    if expected.state().revision() != request.context.revision() {
        return Err(ReplayError::Binding);
    }
    let context = expected.state().revision().interpretation_digest();
    let key = profile.key(source);
    let scope = request.context.scope();
    let mut meta = [0; META_BYTES];
    meta[..8].copy_from_slice(b"ECANR001");
    meta[8..11].copy_from_slice(&profile.tags());
    meta[16..48].copy_from_slice(key.source.as_bytes());
    meta[48..80].copy_from_slice(key.profile.as_bytes());
    meta[80..112].copy_from_slice(
        request
            .context
            .revision()
            .interpretation_digest()
            .as_bytes(),
    );
    for (slot, value) in meta[112..].as_chunks_mut::<32>().0.iter_mut().zip([
        scope.comparison(),
        scope.goal(),
        scope.probes(),
        scope.semantics(),
    ]) {
        slot.copy_from_slice(value.as_bytes());
    }
    let bytes = encode_frame(&[
        SectionInput {
            tag: SOURCE_UTF8,
            version: 1,
            flags: CRITICAL,
            count: 1,
            payload: source.as_bytes(),
        },
        SectionInput {
            tag: META_TAG,
            version: 1,
            flags: 0,
            count: 1,
            payload: &meta,
        },
        SectionInput {
            tag: TEXT_TAG,
            version: 1,
            flags: 0,
            count: 1,
            payload: rendered.text().as_bytes(),
        },
    ])?;
    if bytes.len() > MAX_REPLAY_BYTES {
        return Err(ReplayError::Limit);
    }
    let captured = ScopedCapture {
        bytes,
        key,
        decision,
        broker,
        context,
        claim,
    };
    captured.check(view)?;
    Ok(captured)
}

pub(super) fn decode(bytes: &[u8], key: ArtifactKey) -> Result<HistoricalView<'_>, ReplayError> {
    if bytes.len() > MAX_REPLAY_BYTES {
        return Err(ReplayError::Limit);
    }
    let frame = decode_envelope_borrowed(bytes)?;
    if frame.minor() != 0 || frame.section_count() != 3 {
        return Err(ReplayError::Shape);
    }
    let source = frame.section(0)?.ok_or(ReplayError::Shape)?;
    let meta = frame.section(1)?.ok_or(ReplayError::Shape)?;
    let text = frame.section(2)?.ok_or(ReplayError::Shape)?;
    for (section, tag, flags) in [
        (&source, SOURCE_UTF8, CRITICAL),
        (&meta, META_TAG, 0),
        (&text, TEXT_TAG, 0),
    ] {
        if section.tag() != tag
            || section.version() != 1
            || section.flags() != flags
            || section.count() != 1
        {
            return Err(ReplayError::Shape);
        }
    }
    let raw = meta.payload();
    if raw.len() != META_BYTES || &raw[..8] != b"ECANR001" || raw[11..16].iter().any(|b| *b != 0) {
        return Err(ReplayError::Shape);
    }
    let profile = ReplayProfile::from_tags(&raw[8..11])?;
    let source = std::str::from_utf8(source.payload()).map_err(|_| ReplayError::Shape)?;
    let text = std::str::from_utf8(text.payload()).map_err(|_| ReplayError::Shape)?;
    if source.is_empty() || source.len() > 16384 || text.is_empty() || text.len() > 65536 {
        return Err(ReplayError::Limit);
    }
    if profile.key(source) != key
        || digest(raw, 16)? != key.source
        || digest(raw, 48)? != key.profile
    {
        return Err(ReplayError::Binding);
    }
    let context = digest(raw, 80)?;
    let scope = [
        digest(raw, 112)?,
        digest(raw, 144)?,
        digest(raw, 176)?,
        digest(raw, 208)?,
    ];
    let expected = profile.scope(source)?;
    if scope
        != [
            expected.comparison(),
            expected.goal(),
            expected.probes(),
            expected.semantics(),
        ]
    {
        return Err(ReplayError::Binding);
    }
    Ok(HistoricalView {
        source,
        text,
        profile,
        context,
        scope,
    })
}
fn digest(raw: &[u8], offset: usize) -> Result<Digest32, ReplayError> {
    let bytes: [u8; 32] = raw
        .get(offset..offset + 32)
        .ok_or(ReplayError::Shape)?
        .try_into()
        .map_err(|_| ReplayError::Shape)?;
    Ok(Digest32::from_bytes(bytes))
}
