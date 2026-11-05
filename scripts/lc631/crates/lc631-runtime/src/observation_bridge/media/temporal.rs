//! 宣言clock上の固定時間probe比較。校正の真正性・意味真理は未検証。
//! Fixed temporal comparisons on declared clocks, not verified calibration/truth.
use super::*;
use lc631_analysis::{creativity_measurement::{weighted_assessment, MeasurementError,
    Rational, RationalInterval, SemanticValidation, WeightedProbe}, spatiotemporal_loss as time};
use lc631_media::{attestation_v3::verify_media_backend_with_current_authority_v3_with_resources,
    MediaObservation};
pub use time::TemporalProbe as TemporalRelation;

/// Each digest must bind clock/time-scale/origin/reference/precision externally.
/// A declaration is Given correspondence, not a calibration receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredMediaClockMap {
    pub baseline_clock: Digest32,
    pub candidate_clock: Digest32,
    pub offset_milliseconds: i64,
}
#[derive(Clone, Debug)]
pub struct MediaTemporalProbe {
    pub id: Digest32,
    pub baseline: [String; 2],
    pub candidate: [String; 2],
    pub relation: TemporalRelation,
    pub weight: u32,
}
/// Immutable, nonempty Given/manual plan frozen for this comparison.
/// Receipts already exist: registration before candidate capture is NeedsEvidence.
/// No per-candidate mask changes and no independent fixed-gold claim.
pub struct MediaTemporalProbePlan {
    scope: ContextScope,
    baseline: Digest32,
    candidate: Digest32,
    mapping: Option<DeclaredMediaClockMap>,
    probes: Vec<MediaTemporalProbe>,
    digest: Digest32,
    denominator: u64,
}
impl MediaTemporalProbePlan {
    pub fn declared(scope: ContextScope, baseline: &ExecutedMediaAdmission<'_>,
        candidate: &ExecutedMediaAdmission<'_>, mapping: Option<DeclaredMediaClockMap>,
        probes: Vec<MediaTemporalProbe>) -> Result<Self, MediaConnectionError> {
        if probes.is_empty() || probes.len() > 128 || scope != baseline.scope
            || scope != candidate.scope || baseline.binding.task_source != candidate.binding.task_source {
            return Err(MediaConnectionError::InvalidProbePlan);
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.media-fixed-temporal-probes.v1\0");
        for key in [scope.comparison(), scope.goal(), scope.probes(), scope.semantics(),
            baseline.receipt, candidate.receipt] { hash.update(key.as_bytes()); }
        match mapping {
            None => hash.update([0]),
            Some(clock) => {
                hash.update([1]); hash.update(clock.baseline_clock.as_bytes());
                hash.update(clock.candidate_clock.as_bytes());
                hash.update(clock.offset_milliseconds.to_le_bytes());
            }
        }
        let mut denominator = 0u64;
        for (index, probe) in probes.iter().enumerate() {
            if probe.weight == 0 || probe.weight > 1_000_000
                || probes[..index].iter().any(|p| p.id == probe.id)
                || probe.baseline[0] == probe.baseline[1] || probe.candidate[0] == probe.candidate[1]
                || probe.baseline.iter().chain(&probe.candidate).any(|id| id.is_empty() || id.len() > 256) {
                return Err(MediaConnectionError::InvalidProbePlan);
            }
            denominator = denominator.checked_add(u64::from(probe.weight))
                .ok_or(MediaConnectionError::InvalidProbePlan)?;
            hash.update(probe.id.as_bytes());
            for id in probe.baseline.iter().chain(&probe.candidate) { hash_text(&mut hash, id); }
            hash.update(probe.weight.to_le_bytes());
            hash.update([match probe.relation { TemporalRelation::Before => 1,
                TemporalRelation::Meets => 2, TemporalRelation::EqualDuration => 3,
                TemporalRelation::MappedAbsoluteStart => 4 }]);
        }
        Ok(Self { scope, baseline: baseline.receipt, candidate: candidate.receipt, mapping,
            probes, digest: Digest32::from_bytes(hash.finalize().into()), denominator })
    }
    pub const fn digest(&self) -> Digest32 { self.digest }
    pub const fn denominator(&self) -> u64 { self.denominator }
    pub const fn pre_capture_registration(&self) -> &'static str { "NeedsEvidence" }
}

/// Revalidation uses the actual source authority and a fresh funded view.
/// The owner cannot be constructed from public report fields or mutated JSON.
pub struct CurrentMedia<'a, 'view> {
    pub owner: &'a ExecutedMediaAdmission<'a>,
    pub authority: &'a MediaSourceAuthority,
    pub verifier: &'a ReceiptVerifier,
    pub resources: &'view ResourceReservationView<'view>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaTemporalGap { MissingObservation, MissingDeclaredClock, UnsupportedExtent }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalHypothesis { RelationsPreserved, RelationChanged, Unresolved }
pub struct MediaTemporalComparison {
    loss: Option<RationalInterval>,
    gap: Option<MediaTemporalGap>,
    hypothesis: TemporalHypothesis,
    plan: Digest32,
    baseline: Digest32,
    candidate: Digest32,
    denominator: u64,
}
impl MediaTemporalComparison {
    pub fn loss(&self) -> Option<&RationalInterval> { self.loss.as_ref() }
    pub const fn gap(&self) -> Option<MediaTemporalGap> { self.gap }
    pub const fn hypothesis(&self) -> TemporalHypothesis { self.hypothesis }
    pub const fn denominator(&self) -> u64 { self.denominator }
    pub const fn plan_digest(&self) -> Digest32 { self.plan }
    pub const fn receipts(&self) -> (Digest32, Digest32) { (self.baseline, self.candidate) }
    pub const fn semantic_validation(&self) -> SemanticValidation { SemanticValidation::Unassessed }
    pub const fn claim_boundary(&self) -> &'static str {
        "exact fixed-probe timestamp arithmetic on current authenticated candidates and Given clock correspondence; not semantic truth, independent calibration, TL-TOTAL, optimality, authority or completion"
    }
}

/// Caller maintains this consumer's replay guard and supplies trusted current epoch.
/// Repeated receipt consumption is rejected; resetting a guard is not durable replay.
/// The hypotheses concern protected relations only, not the truth of media labels.
pub fn compare_media_temporal(plan: &MediaTemporalProbePlan, baseline: CurrentMedia<'_, '_>,
    candidate: CurrentMedia<'_, '_>, replay: &mut ReplayGuard, now_epoch: u64)
    -> Result<MediaTemporalComparison, MediaConnectionError> {
    let started = Instant::now();
    if plan.baseline != baseline.owner.receipt || plan.candidate != candidate.owner.receipt
        || plan.scope != baseline.owner.scope || plan.scope != candidate.owner.scope {
        return Err(MediaConnectionError::BindingMismatch);
    }
    verify_current(&baseline, replay, now_epoch)?;
    let same_input = std::ptr::eq(baseline.owner, candidate.owner)
        && std::ptr::eq(baseline.authority, candidate.authority)
        && std::ptr::eq(baseline.verifier, candidate.verifier);
    if !same_input {
        verify_current(&candidate, replay, current_epoch(now_epoch, started)?)?;
    }
    else {
        candidate.resources.check()?; candidate.owner.check_live()?;
        if candidate.resources.source_revision() != candidate.owner.binding.media_source {
            return Err(MediaConnectionError::BindingMismatch);
        }
    }
    let mut report = MediaTemporalComparison { loss: None, gap: None,
        hypothesis: TemporalHypothesis::Unresolved, plan: plan.digest, baseline: plan.baseline,
        candidate: plan.candidate, denominator: plan.denominator };
    let Some(clock) = plan.mapping else {
        report.gap = Some(MediaTemporalGap::MissingDeclaredClock);
        return publish(report, &baseline, &candidate, now_epoch, started);
    };
    let _claim = baseline.resources.claim_resources(&[ResourceRequest::new(ResourceKind::Ram,
        ResourceAmount::checked(128 * 1024)?)])?;
    let mut values = Vec::new();
    let mut weights = Vec::new();
    values.try_reserve_exact(plan.probes.len()).map_err(|_| MediaConnectionError::AllocationFailed)?;
    weights.try_reserve_exact(plan.probes.len()).map_err(|_| MediaConnectionError::AllocationFailed)?;
    for probe in &plan.probes {
        let a = pair(baseline.owner, &probe.baseline, clock.baseline_clock);
        let b = pair(candidate.owner, &probe.candidate, clock.candidate_clock);
        let (a, b) = match (a, b) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(gap), _) | (_, Err(gap)) => {
                report.gap = Some(gap); return publish(report, &baseline, &candidate, now_epoch, started);
            }
        };
        let binding = time::Binding { scope: plan.scope.comparison(), probe_set: plan.digest,
            policy: plan.digest, source_revision: baseline.owner.binding.media_source,
            target_revision: candidate.owner.binding.media_source, direction: time::Direction::Forward };
        let map = time::TimeTranslation::declared(binding, clock.baseline_clock,
            clock.candidate_clock, clock.offset_milliseconds, time::TimeUnit::Millisecond);
        let comparison = time::compare_temporal(binding, probe.relation, Some(a), Some(b), Some(map))
            .map_err(MediaConnectionError::TemporalInput)?.comparison();
        let loss = match comparison {
            time::RelationComparison::MatchesDeclaredRelation => 0,
            time::RelationComparison::DiffersFromDeclaredRelation => 1,
            time::RelationComparison::NeedsEvidence(_) | time::RelationComparison::Incomparable(_) => {
                return Err(MediaConnectionError::BindingMismatch);
            }
        };
        values.push(RationalInterval::exact_ratio(loss, 1)
            .map_err(|e| MediaConnectionError::Measurement(MeasurementError::from(e)))?);
        weights.push(Rational::from_integer(i64::from(probe.weight).into()));
    }
    let mut views = Vec::new();
    views.try_reserve_exact(plan.probes.len()).map_err(|_| MediaConnectionError::AllocationFailed)?;
    for ((probe, weight), value) in plan.probes.iter().zip(&weights).zip(&values) {
        views.push(WeightedProbe { canonical_payload: probe.id.as_bytes(), weight, value });
    }
    let loss = weighted_assessment(&views).map_err(MediaConnectionError::Measurement)?;
    report.hypothesis = if values.iter().any(|v| v.lower() > &Rational::from_integer(0.into())) {
        TemporalHypothesis::RelationChanged
    } else { TemporalHypothesis::RelationsPreserved };
    report.loss = Some(loss);
    publish(report, &baseline, &candidate, now_epoch, started)
}
fn publish(report: MediaTemporalComparison, baseline: &CurrentMedia<'_, '_>,
    candidate: &CurrentMedia<'_, '_>, now_epoch: u64, started: Instant)
    -> Result<MediaTemporalComparison, MediaConnectionError> {
    baseline.resources.check()?; candidate.resources.check()?;
    baseline.owner.check_live()?; candidate.owner.check_live()?;
    let current = current_epoch(now_epoch, started)?;
    validate_current_media_authority(baseline.authority, baseline.verifier, current)
        .map_err(MediaConnectionError::Backend)?;
    validate_current_media_authority(candidate.authority, candidate.verifier, current)
        .map_err(MediaConnectionError::Backend)?;
    Ok(report)
}
fn current_epoch(base: u64, started: Instant) -> Result<u64, MediaConnectionError> {
    base.checked_add(started.elapsed().as_secs()).ok_or(MediaConnectionError::CurrentVerification)
}

fn verify_current(current: &CurrentMedia<'_, '_>, replay: &mut ReplayGuard, now_epoch: u64)
    -> Result<(), MediaConnectionError> {
    current.resources.check()?;
    current.owner.check_live()?;
    if current.resources.source_revision() != current.owner.binding.media_source {
        return Err(MediaConnectionError::BindingMismatch);
    }
    verify_media_backend_with_current_authority_v3_with_resources(&current.owner.report,
        current.authority, current.verifier, replay, now_epoch, current.resources)
        .map_err(|_| MediaConnectionError::CurrentVerification)
}
fn pair(owner: &ExecutedMediaAdmission<'_>, ids: &[String; 2], clock: Digest32)
    -> Result<(time::TimeInterval, time::TimeInterval), MediaTemporalGap> {
    let find = |id: &str| owner.report.graph.observations().iter().find(|o| o.observation_id == id)
        .ok_or(MediaTemporalGap::MissingObservation).and_then(|o| interval(o, clock));
    Ok((find(&ids[0])?, find(&ids[1])?))
}
fn interval(observation: &MediaObservation, clock: Digest32)
    -> Result<time::TimeInterval, MediaTemporalGap> {
    if !matches!(observation.time_extent(), Ok(lc631_media::time_boundary::TemporalExtent::Interval(_))) {
        return Err(MediaTemporalGap::UnsupportedExtent);
    }
    let convert = |value: lc631_media::RationalTime| {
        let numerator = i128::from(value.ticks) * 1000;
        let denominator = i128::from(value.timescale);
        if denominator == 0 || numerator % denominator != 0 { return Err(MediaTemporalGap::UnsupportedExtent); }
        i64::try_from(numerator / denominator).map_err(|_| MediaTemporalGap::UnsupportedExtent)
    };
    let start = time::Instant::declared(clock, convert(observation.span.start)?, time::TimeUnit::Millisecond);
    let end = time::Instant::declared(clock, convert(observation.span.end)?, time::TimeUnit::Millisecond);
    time::TimeInterval::checked(start, end).map_err(|_| MediaTemporalGap::UnsupportedExtent)
}
