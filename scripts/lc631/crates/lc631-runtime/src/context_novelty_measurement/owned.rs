//! Historical reports cannot construct or reactivate an actual current owner.
use super::*;
/// Real opaque, non-Clone/non-Deserialize owner; fields are private externally.
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::OwnedAssessment;
/// use lc631_analysis::creativity_measurement::ComponentReport;
/// fn inject(report: ComponentReport) { let _ = OwnedAssessment::from_report(report); }
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::OwnedAssessment;
/// let _ = OwnedAssessment::from_scalar(0);
/// ```
/// This negative uses an EXISTING runtime dependency, not a missing serde import.
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::OwnedAssessment;
/// fn inject<'s,'a,'v>() {
///     let _: OwnedAssessment<'s,'a,'v> = serde_json::from_str("{}").unwrap();
/// }
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::OwnedAssessment;
/// fn rewrite(owner: &mut OwnedAssessment<'_,'_,'_>) { owner.reports = []; }
/// ```
/// Returning a fresh current token or report from its HRTB scope is forbidden:
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::{OwnedAssessment,ExpectedInputs,CurrentAssessment};
/// use lc631_substrate::resource::ResourceReservationView;
/// fn escape<'s,'a,'v>(owner:&OwnedAssessment<'s,'a,'v>,e:ExpectedInputs<'s,'a,'v>,v:&ResourceReservationView<'_>) -> CurrentAssessment<'static> {
///     owner.with_current(e,v,|current| current).unwrap()
/// }
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::{OwnedAssessment,ExpectedInputs};
/// use lc631_analysis::creativity_measurement::{ComponentReport,ComponentMetric};
/// use lc631_substrate::resource::ResourceReservationView;
/// fn escape<'s,'a,'v>(owner:&OwnedAssessment<'s,'a,'v>,e:ExpectedInputs<'s,'a,'v>,v:&ResourceReservationView<'_>) -> &'static ComponentReport {
///     owner.with_current(e,v,|c| c.report(ComponentMetric::Knownness).unwrap()).unwrap()
/// }
/// ```
pub struct OwnedAssessment<'s, 'a, 'v> {
    pub(super) root: &'s RootPlan<'a, 'v>,
    pub(super) candidate: &'s ValidatedFormalCandidate<'a, 'v>,
    pub(super) ast: &'s SmtStatusQuery,
    pub(super) reports: [ComponentReport; 3],
    pub(super) union_weight: Rational,
    pub(super) counts: Counts,
    pub(super) status: ArchiveStatus,
    pub(super) retained: ResourceClaim<'v>,
}
/// Borrowed current-only read surface. No detached constructor, Clone or serde.
pub struct CurrentAssessment<'scope> {
    reports: &'scope [ComponentReport; 3],
    union_weight: &'scope Rational,
    counts: Counts,
    status: ArchiveStatus,
}
impl<'s, 'a, 'v> OwnedAssessment<'s, 'a, 'v> {
    fn check(
        &self,
        expected: ExpectedInputs<'s, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.root.check(view)?;
        resources::current(&self.retained, view, self.root.state.source.id().digest())?;
        self.root.meter.debit(CURRENT_WORK)?;
        if !std::ptr::eq(self.candidate, expected.candidate)
            || !std::ptr::eq(self.ast, expected.candidate_ast)
        {
            return Err(Error::ExpectedInputMismatch);
        }
        self.root.expected(expected, view)
    }
    /// Pre/post eligibility, NOT an atomic cancellation fence or effect rollback.
    pub fn with_current<R>(
        &self,
        expected: ExpectedInputs<'s, 'a, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'scope> FnOnce(CurrentAssessment<'scope>) -> R,
    ) -> Result<R, Error> {
        self.check(expected, view)?;
        let result = consume(CurrentAssessment {
            reports: &self.reports,
            union_weight: &self.union_weight,
            counts: self.counts,
            status: self.status,
        });
        self.check(expected, view)?;
        Ok(result)
    }
    pub fn historical_report(&self, metric: ComponentMetric) -> Option<&ComponentReport> {
        self.reports.iter().find(|r| r.metric == metric)
    }
}
impl CurrentAssessment<'_> {
    pub fn report(&self, metric: ComponentMetric) -> Option<&ComponentReport> {
        self.reports.iter().find(|r| r.metric == metric)
    }
    pub fn archive_status(&self) -> ArchiveStatus {
        self.status
    }
    pub fn counts(&self) -> Counts {
        self.counts
    }
    pub fn dctx_union_weight(&self) -> &Rational {
        self.union_weight
    }
    pub fn original_status(&self, formula: OriginalFormula) -> OriginalStatus<'_> {
        use ComponentMetric as M;
        const F1: &[M] = &[
            M::DescriptionBits,
            M::SemanticConfigurationDistance,
            M::Compatibility,
        ];
        const F1_KNOWNNESS_GAP: &[M] = &[
            M::DescriptionBits,
            M::SemanticConfigurationDistance,
            M::Compatibility,
            M::Knownness,
        ];
        const F2: &[M] = &[
            M::DescriptionBits,
            M::SemanticConfigurationDistance,
            M::Compatibility,
            M::Coverage,
            M::Compression,
            M::Bridge,
            M::Prediction,
            M::Redundancy,
            M::Preserve,
            M::Rebind,
            M::Reframe,
            M::BreakCost,
            M::Explain,
            M::Design,
            M::Transfer,
            M::Solve,
            M::CounterexampleRisk,
            M::BridgeCost,
        ];
        let missing = match formula {
            OriginalFormula::F1V1 => {
                if matches!(&self.reports[2].state, ComponentState::Measured(_)) {
                    F1
                } else {
                    F1_KNOWNNESS_GAP
                }
            }
            OriginalFormula::F2V1 => F2,
        };
        OriginalStatus::NoScalar { missing }
    }
    pub fn pending_exits(&self) -> &[PendingExit] {
        &[
            PendingExit::NonzeroFramework,
            PendingExit::CrossFrameworkSemantic,
            PendingExit::CrossFrameworkCompatibility,
            PendingExit::NormativeTnc,
            PendingExit::OriginalScoreSelection,
        ]
    }
}
