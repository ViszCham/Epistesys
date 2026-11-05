//! Only actual execution creates result owners. History never renews current use.
use super::*;

pub struct OwnedLegacyBaseline<'a, 'v> {
    pub(super) root: &'a RootDefinition<'a, 'v>,
    pub(super) actual: execute::Actual,
    retained: ResourceClaim<'v>,
}
impl OwnedLegacyBaseline<'_, '_> {
    pub fn historical_report(&self) -> &ComponentReport {
        &self.actual.report
    }
    pub(super) fn check(
        &self,
        root: &RootDefinition<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.root.check(view)?;
        resources::current(&self.retained, view)?;
        if !same(self.root, root) {
            return Err(Error::BindingMismatch);
        }
        resources::current(&self.retained, view)
    }
}
/// Private actual outputs/outcomes, not a report factory.
/// ```compile_fail
/// use lc631_analysis::{breakcost_measurement::OwnedBreakCost,creativity_measurement::ComponentReport};
/// fn inject(report: ComponentReport) { OwnedBreakCost::from_report(report); }
/// ```
/// ```compile_fail
/// use lc631_analysis::breakcost_measurement::OwnedBreakCost;
/// fn inject() { OwnedBreakCost::from_scalar(0); }
/// ```
/// ```compile_fail
/// use lc631_analysis::breakcost_measurement::OwnedBreakCost;
/// let _: OwnedBreakCost<'_, '_> = serde_json::from_str("{}").unwrap();
/// ```
/// ```compile_fail
/// use lc631_analysis::breakcost_measurement::OwnedBreakCost;
/// fn overwrite(owner: &mut OwnedBreakCost<'_, '_>) { owner.report = panic!("not a factory"); }
/// ```
/// ```compile_fail
/// use lc631_analysis::breakcost_measurement::{observe_breakcost_current,RootDefinition,OwnedLegacyBaseline,AdapterCandidate};
/// use lc631_substrate::resource::ResourceReservationView;
/// fn escape<'a,'v>(root: &'a RootDefinition<'a,'v>,base: &'a OwnedLegacyBaseline<'a,'v>,
///     candidate: &'a AdapterCandidate<'a,'v>,view: &'v ResourceReservationView<'_>) {
///     let owner=observe_breakcost_current(root,base,candidate,view).unwrap();
///     let report=owner.historical_report(); drop(owner); let _=report.metric;
/// }
/// ```
pub struct OwnedBreakCost<'a, 'v> {
    root: &'a RootDefinition<'a, 'v>,
    baseline: &'a OwnedLegacyBaseline<'a, 'v>,
    candidate: &'a AdapterCandidate<'a, 'v>,
    actual: execute::Actual,
    retained: ResourceClaim<'v>,
}
impl<'a, 'v> OwnedBreakCost<'a, 'v> {
    pub fn historical_report(&self) -> &ComponentReport {
        &self.actual.report
    }
    pub fn historical_outputs(&self) -> &[Vec<u8>] {
        &self.actual.outputs
    }
    pub fn historical_hard_status(&self) -> LocalHardStatus {
        self.actual.hard
    }
    pub fn historical_denominator(&self) -> &Rational {
        &self.root.denominator
    }
    pub fn historical_counters(&self) -> ExecutionCounters {
        self.actual.counters
    }
    pub fn historical_property(
        &self,
        case: usize,
        property: LegacyProperty,
    ) -> Option<PropertyOutcome> {
        let rule = self
            .root
            .rules
            .iter()
            .position(|r| r.property == property)?;
        self.actual.outcomes.get(case)?.get(rule).copied()
    }
    pub fn measurement_binding(&self) -> Digest32 {
        self.actual.binding
    }
    pub fn report_current(
        &self,
        expected: ExpectedInputs<'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.root.check(view)?;
        self.baseline.check(self.root, view)?;
        self.candidate.check(self.root, view)?;
        resources::current(&self.retained, view)?;
        if !same(self.root, expected.root)
            || !same(self.root.source, expected.source)
            || !std::ptr::eq(self.root.cases, expected.cases)
            || !same(self.baseline, expected.baseline)
            || !same(self.candidate, expected.candidate)
        {
            return Err(Error::BindingMismatch);
        }
        resources::current(&self.retained, view)?;
        Ok(&self.actual.report)
    }
}
pub fn observe_baseline_current<'a, 'v>(
    root: &'a RootDefinition<'a, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedLegacyBaseline<'a, 'v>, Error> {
    root.check(view)?;
    let retained = resources::claim(view, root.budget.max_output_bytes_per_run as u64)?;
    let modes = resources::vector(root.cases.len())?;
    let actual = execute::run(root, &modes, None, None, LocalHardStatus::NotRejected, view)?;
    root.check(view)?;
    resources::current(&retained, view)?;
    Ok(OwnedLegacyBaseline {
        root,
        actual,
        retained,
    })
}
pub fn observe_breakcost_current<'a, 'v>(
    root: &'a RootDefinition<'a, 'v>,
    base: &'a OwnedLegacyBaseline<'a, 'v>,
    candidate: &'a AdapterCandidate<'a, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedBreakCost<'a, 'v>, Error> {
    base.check(root, view)?;
    candidate.check(root, view)?;
    let retained = resources::claim(view, root.budget.max_output_bytes_per_run as u64)?;
    let actual = execute::run(
        root,
        &candidate.kinds,
        Some(candidate.binding),
        Some(base.actual.binding),
        base.actual.hard,
        view,
    )?;
    base.check(root, view)?;
    candidate.check(root, view)?;
    resources::current(&retained, view)?;
    Ok(OwnedBreakCost {
        root,
        baseline: base,
        candidate,
        actual,
        retained,
    })
}
