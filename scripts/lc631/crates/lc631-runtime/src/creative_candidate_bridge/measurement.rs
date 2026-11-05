//! 日本語: 実生成ownerを固定finite測定へ接続し、由来を書き換えない。
//! English: Connect the actual generated owner to fixed finite measurement without rebinding provenance.
//! Given source/IR mapping and shared-checker gaps remain unchanged. This is an
//! opt-in SDK connection, not canonical/facade wiring, semantic proof or authority.
#![forbid(unsafe_code)]
use super::{CreativeCandidateBridge, CreativeError, ValidatedFormalCandidate};
use crate::formal_measurement::{self as m, owned, produced};
use lc631_solvers::typed_query::SmtStatusQuery;
use lc631_substrate::{resource::ResourceReservationView, Digest32};
use sha2::{Digest, Sha256};

/// Immutable borrowed evaluation inputs, never caller-supplied statuses/scores.
/// The nonempty plan fixes the denominator even when the dispatch is cut.
#[derive(Clone, Copy)]
pub struct MeasurementInput<'a> {
    pub source: &'a SmtStatusQuery,
    pub plan: &'a m::ProbePlan<'a>,
    pub candidate: &'a m::Candidate<'a>,
    pub schedule: &'a [produced::ScheduledProbe<'a>],
    pub budget: m::WorkBudget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Creative(CreativeError),
    SourceQueryMismatch,
    CandidateMismatch,
    ScopeMismatch,
    ProbeBindingMismatch,
    Produced(produced::ProducedError),
    Owned(owned::OwnedError),
}

/// Borrowed actual K1 reports while both owners and generated candidate remain
/// live. These are the ORIGINAL measured reports, not converted component data.
/// Reads describe the checked interval, not an atomic cancellation fence.
/// There is no detached-report constructor or deserialization entry.
///
/// ```compile_fail
/// use lc631_runtime::creative_candidate_bridge::measurement::MeasuredPair;
/// use lc631_runtime::formal_measurement::MeasuredFormalLoss;
/// fn inject<'a>(report: &'a MeasuredFormalLoss<'a>) -> MeasuredPair<'a> {
///     MeasuredPair::from_report(report)
/// }
/// ```
/// A borrowed local report cannot be returned as a static current report:
/// ```compile_fail
/// use lc631_runtime::creative_candidate_bridge::measurement::MeasuredPair;
/// use lc631_runtime::formal_measurement::MeasuredFormalLoss;
/// fn escape(pair: MeasuredPair<'_>) -> &'static MeasuredFormalLoss<'static> {
///     pair.soundness()
/// }
/// ```
pub struct MeasuredPair<'scope> {
    soundness: &'scope m::MeasuredFormalLoss<'scope>,
    completeness: &'scope m::MeasuredFormalLoss<'scope>,
    produced: usize,
    missing: usize,
}
impl<'scope> MeasuredPair<'scope> {
    pub fn soundness(&self) -> &'scope m::MeasuredFormalLoss<'scope> {
        self.soundness
    }
    pub fn completeness(&self) -> &'scope m::MeasuredFormalLoss<'scope> {
        self.completeness
    }
    pub fn produced_count(&self) -> usize {
        self.produced
    }
    pub fn missing_count(&self) -> usize {
        self.missing
    }
}

impl ValidatedFormalCandidate<'_, '_> {
    /// Content identity within this declared bridge profile, not authentication,
    /// semantic novelty or permission. Duplicate generated content has one ID.
    /// Original source/context identity is preserved; payload is not relabelled
    /// as the source revision of its existing proposed ContextRevision.
    pub fn measurement_identity_current(
        &self,
        bridge: &CreativeCandidateBridge<'_, '_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<Digest32, Error> {
        self.project_current(bridge, resources).map_err(Error::Creative)?;
        let mut hash = Sha256::new();
        hash.update(b"epistesys.generated-formal-measurement.candidate.v1\0");
        hash.update(bridge.source.id().digest().as_bytes());
        hash.update(self.prepared.parent().interpretation_digest().as_bytes());
        hash.update(self.prepared.target().interpretation_digest().as_bytes());
        hash.update(self.goal.revision.as_bytes());
        hash.update((self.payload.len() as u64).to_le_bytes());
        hash.update(&self.payload);
        let identity = Digest32::from_bytes(hash.finalize().into());
        self.project_current(bridge, resources).map_err(Error::Creative)?;
        Ok(identity)
    }

    /// Execute existing finite evaluation and both owned K1 consumers under the
    /// ACTUAL generated owner. The callback must be bounded and read-only.
    /// Its R cannot borrow local reports (the callback is higher-ranked).
    ///
    /// Check current eligibility immediately before and after consumption. A
    /// cancellation rejects the returned result; this is NOT an atomic fence,
    /// rollback, reversal of callback effects, delivery gate or mutation permit.
    /// Claims are logical payload accounting, not total RSS or fallible global OOM.
    pub fn with_formal_measurement_current<'input, 'view, R>(
        &self,
        bridge: &CreativeCandidateBridge<'_, '_>,
        input: MeasurementInput<'input>,
        resources: &'view ResourceReservationView<'_>,
        consume: impl for<'scope> FnOnce(MeasuredPair<'scope>) -> R,
    ) -> Result<R, Error> {
        self.check_measurement_input(bridge, input, resources)?;
        let produced = produced::execute(
            input.plan, input.candidate, input.schedule, input.budget, resources,
        ).map_err(Error::Produced)?;
        let observations = produced.observations_current(resources).map_err(Error::Produced)?;
        let soundness = owned::measure_owned(
            input.plan, input.candidate, m::Direction::Soundness,
            observations.as_slice(), input.budget, resources,
        ).map_err(Error::Owned)?;
        let completeness = owned::measure_owned(
            input.plan, input.candidate, m::Direction::Completeness,
            observations.as_slice(), input.budget, resources,
        ).map_err(Error::Owned)?;
        self.check_measurement_input(bridge, input, resources)?;
        observations.check_current().map_err(Error::Produced)?;
        let sound = soundness.project_current(
            input.plan, input.candidate, m::Direction::Soundness, resources,
        ).map_err(Error::Owned)?;
        let complete = completeness.project_current(
            input.plan, input.candidate, m::Direction::Completeness, resources,
        ).map_err(Error::Owned)?;
        let result = consume(MeasuredPair {
            soundness: sound, completeness: complete,
            produced: produced.produced_count(), missing: produced.missing_count(),
        });
        // Keep the original producer/observation/measurement owners alive across
        // consumption. Equal detached numbers do not substitute for these checks.
        observations.check_current().map_err(Error::Produced)?;
        soundness.project_current(
            input.plan, input.candidate, m::Direction::Soundness, resources,
        ).map_err(Error::Owned)?;
        completeness.project_current(
            input.plan, input.candidate, m::Direction::Completeness, resources,
        ).map_err(Error::Owned)?;
        self.check_measurement_input(bridge, input, resources)?;
        Ok(result)
    }

    fn check_measurement_input(
        &self,
        bridge: &CreativeCandidateBridge<'_, '_>,
        input: MeasurementInput<'_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let query = self.project_current(bridge, resources).map_err(Error::Creative)?;
        let original = bridge.contexts().original().state();
        let scope = input.plan.scope();
        if scope.context != self.prepared.parent()
            || scope.context != original.revision()
            || scope.source != scope.context.source()
            || scope.comparison != original.scope().comparison()
            || scope.probe_set != original.scope().probes()
            || self.goal.revision != original.scope().goal()
        {
            return Err(Error::ScopeMismatch);
        }
        if !std::ptr::eq(input.candidate.target_query(), query)
            || input.candidate.context() != self.prepared.target()
            || input.candidate.revision() != self.prepared.target().source()
            || input.candidate.comparison() != scope.comparison
            || input.candidate.id() != self.measurement_identity_current(bridge, resources)?
        {
            return Err(Error::CandidateMismatch);
        }
        check_source(bridge, input.source)?;
        for fixed in input.plan.fixed_probes() {
            let request = fixed.template;
            let binding = request.binding;
            if !std::ptr::eq(request.source, input.source)
                || !std::ptr::eq(request.target, query)
                || binding.source != scope.source
                || binding.source_context != self.prepared.parent()
                || binding.target != input.candidate.revision()
                || binding.target_context != self.prepared.target()
                || binding.probe_set != scope.probe_set
            {
                return Err(Error::ProbeBindingMismatch);
            }
        }
        // Exact dispatch predicates/maps/contexts are checked by the existing
        // produced::execute admission, not replaced by an approximate hash join.
        Ok(())
    }
}

fn check_source(
    bridge: &CreativeCandidateBridge<'_, '_>,
    source: &SmtStatusQuery,
) -> Result<(), Error> {
    if source.boolean_symbols() != bridge.goal.query.boolean_symbols()
        || !source.integer_symbols().is_empty()
        || source.assertions().len() != bridge.bindings.len()
    {
        return Err(Error::SourceQueryMismatch);
    }
    // Exact AST references bind the original declaration to its bridge inputs.
    // No parsing, inferred symbol correspondence or semantic equality is added.
    // Bridge admission bounds bindings to 16 and checks all source spans.
    for (assertion, binding) in source.assertions().iter().zip(bridge.bindings) {
        if !std::ptr::eq(assertion, binding.formula) {
            return Err(Error::SourceQueryMismatch);
        }
    }
    Ok(())
}
