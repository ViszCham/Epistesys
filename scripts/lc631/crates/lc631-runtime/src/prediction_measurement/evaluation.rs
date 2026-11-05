//! 日本語: 凍結後に新規storeが実labelを計算する。Given traceは世界の正解でない。
//! English: A fresh store computes labels after freeze; Given traces are not world gold.
use super::*;
use crate::finite_results::{FiniteResultError, FiniteResultId, FiniteResultStore};
use lc631_solvers::temporal::{EvaluationBudget, Truth};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LabelGap {
    UnknownTruth,
    EvaluationBudget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LabelState {
    Observed(Truth),
    Unknown(LabelGap),
}
struct Label {
    state: LabelState,
    id: Option<FiniteResultId>,
}
/// The report is an export view, not a certificate. No public constructor, fields,
/// mutable accessor or deserialize entry can replace the computed private result.
pub struct OwnedPredictionGain<'a, 'v> {
    report: ComponentReport,
    frozen: &'a FrozenPredictions<'v>,
    store: FiniteResultStore<'a, 'v>,
    records: Vec<Label>,
    labels: Vec<LabelState>,
    brier0: Option<Rational>,
    brierz: Option<Rational>,
    result: Digest32,
    retained: ResourceClaim<'v>,
}
impl OwnedPredictionGain<'_, '_> {
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub fn historical_labels(&self) -> &[LabelState] {
        &self.labels
    }
    pub fn fixed_weight(&self) -> &Rational {
        &self.frozen.weight
    }
    pub fn historical_briers(&self) -> Option<(&Rational, &Rational)> {
        self.brier0.as_ref().zip(self.brierz.as_ref())
    }
    pub fn measurement_binding(&self) -> Digest32 {
        self.result
    }
    fn check(
        &self,
        frozen: &FrozenPredictions<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.frozen.check(view)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        if !std::ptr::eq(self.frozen, frozen)
            || self.report.binding.result != self.result
            || self.report.evidence_payload != self.result
        {
            return Err(Error::BindingMismatch);
        }
        for (s, r) in self.frozen.samples.iter().zip(&self.records) {
            if let Some(id) = r.id {
                let observed = self
                    .store
                    .lookup_temporal(
                        id,
                        &s.query,
                        &s.trace,
                        self.frozen.binding.context,
                        self.frozen.binding.selection.scope,
                    )
                    .map_err(Error::Store)?;
                let expected = match observed.truth() {
                    Truth::True => LabelState::Observed(Truth::True),
                    Truth::False => LabelState::Observed(Truth::False),
                    Truth::Unknown => LabelState::Unknown(LabelGap::UnknownTruth),
                };
                if r.state != expected {
                    return Err(Error::BindingMismatch);
                }
            } else if r.state != LabelState::Unknown(LabelGap::EvaluationBudget) {
                return Err(Error::BindingMismatch);
            }
        }
        self.retained.check().map_err(Error::Resource)?;
        Ok(())
    }
    pub fn report_current(
        &self,
        frozen: &FrozenPredictions<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check(frozen, view)?;
        Ok(&self.report)
    }
    pub fn axis_current(
        &self,
        frozen: &FrozenPredictions<'_>,
        pins: SelectionPins,
        view: &ResourceReservationView<'_>,
    ) -> Result<ReportedComponentAxis<'_>, Error> {
        self.check(frozen, view)?;
        if self.frozen.binding.role != SplitRole::Validation {
            return Err(Error::SplitNotSelectable);
        }
        if pins != self.frozen.binding.selection {
            return Err(Error::BindingMismatch);
        }
        project_component_reported(&self.report).map_err(Error::Selection)
    }
    pub fn check_selection_row_current(
        &self,
        frozen: &FrozenPredictions<'_>,
        policy: &SelectionPolicy<'_>,
        row: &JointCandidate<'_>,
        index: usize,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let axis = self.axis_current(frozen, policy.pins, view)?;
        let b = self.frozen.binding;
        if row.pins != policy.pins
            || row.id != b.candidate
            || row.context != b.context
            || row.measurement_binding != self.result
            || row.axes.len() != policy.axes.len()
            || policy.axes.get(index) != Some(&AxisKind::PredictionGain)
        {
            return Err(Error::BindingMismatch);
        }
        let same = match (row.axes.get(index), axis.value) {
            (Some(AxisValue::Exact(a)), AxisValue::Exact(b))
            | (Some(AxisValue::Bounded(a)), AxisValue::Bounded(b)) => std::ptr::eq(*a, b),
            (
                Some(AxisValue::Unavailable(MissingAxis::Unknown)),
                AxisValue::Unavailable(MissingAxis::Unknown),
            ) => matches!(
                self.report.state,
                ComponentState::Unknown(MeasurementGap::MissingObservation)
            ),
            _ => false,
        };
        if !same {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}
/// A NEW private store is created only after the immutable freeze object exists.
/// The budget is already frozen and hashed; callers cannot retune it after labels.
/// Resource cancellation/deadline errors publish no owner; these are not labels.
pub fn evaluate<'a, 'v>(
    frozen: &'a FrozenPredictions<'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedPredictionGain<'a, 'v>, Error> {
    frozen.check(view)?;
    if frozen.binding.role == SplitRole::FinalResearchHoldout {
        return Err(Error::SplitNotSelectable);
    }
    let scratch = claim(view, SCRATCH_RAM, 0)?;
    let retained = claim(view, 1024 * 1024, 1024 * 1024)?;
    let mut store =
        FiniteResultStore::with_resources(frozen.samples.len(), view).map_err(Error::Store)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(frozen.samples.len())
        .map_err(|_| Error::AllocationFailed)?;
    let mut labels = Vec::new();
    labels
        .try_reserve_exact(frozen.samples.len())
        .map_err(|_| Error::AllocationFailed)?;
    let budget =
        EvaluationBudget::checked(frozen.limits.max_node_steps).map_err(Error::LabelEvaluation)?;
    let mut sum0 = Rational::from_integer(0.into());
    let mut sumz = sum0.clone();
    let mut paired_sum = sum0.clone();
    let mut complete = true;
    for s in &frozen.samples {
        frozen.check(view)?;
        let (state, id) = match store.evaluate_temporal(
            &s.query,
            &s.trace,
            frozen.binding.context,
            frozen.binding.selection.scope,
            budget,
            view,
        ) {
            Ok(id) => {
                let report = store
                    .lookup_temporal(
                        id,
                        &s.query,
                        &s.trace,
                        frozen.binding.context,
                        frozen.binding.selection.scope,
                    )
                    .map_err(Error::Store)?;
                let state = match report.truth() {
                    Truth::True => LabelState::Observed(Truth::True),
                    Truth::False => LabelState::Observed(Truth::False),
                    Truth::Unknown => LabelState::Unknown(LabelGap::UnknownTruth),
                };
                (state, Some(id))
            }
            Err(FiniteResultError::Evaluation(TemporalError::ResourceExhausted)) => {
                (LabelState::Unknown(LabelGap::EvaluationBudget), None)
            }
            Err(FiniteResultError::Evaluation(e)) => return Err(Error::LabelEvaluation(e)),
            Err(e) => return Err(Error::Store(e)),
        };
        if let LabelState::Observed(truth) = state {
            let y = Rational::from_integer(if truth == Truth::True {
                1.into()
            } else {
                0.into()
            });
            let e0 = squared_error(&s.baseline, &y)?;
            let ez = squared_error(&s.candidate, &y)?;
            let paired = numeric::checked_subtract(&e0, &ez).map_err(Error::Numeric)?;
            paired_sum = numeric::checked_add(
                &paired_sum,
                &numeric::checked_multiply(&s.weight, &paired).map_err(Error::Numeric)?,
            )
            .map_err(Error::Numeric)?;
            sum0 = numeric::checked_add(
                &sum0,
                &numeric::checked_multiply(&s.weight, &e0).map_err(Error::Numeric)?,
            )
            .map_err(Error::Numeric)?;
            sumz = numeric::checked_add(
                &sumz,
                &numeric::checked_multiply(&s.weight, &ez).map_err(Error::Numeric)?,
            )
            .map_err(Error::Numeric)?;
        } else {
            complete = false;
        }
        records.push(Label { state, id });
        labels.push(state);
    }
    let (brier0, brierz, state) = if complete {
        let b0 = numeric::checked_divide(&sum0, &frozen.weight).map_err(Error::Numeric)?;
        let bz = numeric::checked_divide(&sumz, &frozen.weight).map_err(Error::Numeric)?;
        let delta = numeric::checked_divide(&paired_sum, &frozen.weight).map_err(Error::Numeric)?;
        let interval = RationalInterval::new(delta.clone(), delta).map_err(Error::Numeric)?;
        (Some(b0), Some(bz), ComponentState::Measured(interval))
    } else {
        (
            None,
            None,
            ComponentState::Unknown(MeasurementGap::MissingObservation),
        )
    };
    let mut h = Sha256::new();
    h.update(PRODUCER);
    h.update(frozen.digest.as_bytes());
    for l in &labels {
        h.update([match l {
            LabelState::Observed(Truth::False) => 0,
            LabelState::Observed(Truth::True) => 1,
            LabelState::Observed(Truth::Unknown) => 2,
            LabelState::Unknown(LabelGap::UnknownTruth) => 3,
            LabelState::Unknown(LabelGap::EvaluationBudget) => 4,
        }]);
    }
    match (&brier0, &brierz, &state) {
        (Some(a), Some(b), ComponentState::Measured(i)) => {
            h.update([1]);
            for r in [a, b, i.lower(), i.upper()] {
                hash_rational(&mut h, r);
            }
        }
        _ => h.update([0]),
    };
    let result = Digest32::from_bytes(h.finalize().into());
    let b = frozen.binding;
    let report = ComponentReport {
        metric: ComponentMetric::Prediction,
        definition: b.definition,
        binding: MeasurementBinding {
            scope: b.selection.scope.comparison(),
            candidate: b.candidate_digest,
            source: b.selection.source.digest(),
            target: b.target,
            context: b.context.interpretation_digest(),
            baseline: b.selection.baseline,
            producer: digest(PRODUCER),
            transform: b.transform,
            result,
        },
        probe_set: b.probe_set,
        oracle: b.oracle,
        dependency_group: b.dependency_group,
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state,
    };
    let owned = OwnedPredictionGain {
        report,
        frozen,
        store,
        records,
        labels,
        brier0,
        brierz,
        result,
        retained,
    };
    scratch.check().map_err(Error::Resource)?;
    owned.check(frozen, view)?;
    Ok(owned)
}
fn squared_error(p: &Rational, y: &Rational) -> Result<Rational, Error> {
    let d = numeric::checked_subtract(p, y).map_err(Error::Numeric)?;
    numeric::checked_multiply(&d, &d).map_err(Error::Numeric)
}
