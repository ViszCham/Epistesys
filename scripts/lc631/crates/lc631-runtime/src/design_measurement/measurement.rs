//! 日本語: 私有の実baseline/candidate観測から既存K2と実DesignGainへ。
//! English: Existing K2 over private actual baseline/candidate observations.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingReason {
    EventBudget,
    ReplayBudget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateState {
    Observed,
    Pending(PendingReason),
}
/// Historical output is inspectable; no supplied scalar/trace/report constructor
/// can mint this private current result or replace its underlying observations.
pub struct OwnedDesignGain<'a, 'v> {
    report: ComponentReport,
    frozen: &'a FrozenDesign<'a, 'v>,
    candidates: Vec<Option<Outcome>>,
    states: Vec<CandidateState>,
    result: Digest32,
    retained: ResourceClaim<'v>,
}
impl OwnedDesignGain<'_, '_> {
    pub fn candidate_states(&self) -> &[CandidateState] {
        &self.states
    }
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub fn candidate_schedule(&self, i: usize) -> Option<&ReportedSchedule> {
        self.candidates.get(i)?.as_ref().map(|o| &o.schedule)
    }
    pub fn candidate_observation(&self, i: usize) -> Option<&ScoreObservation> {
        self.candidates.get(i)?.as_ref().map(|o| &o.observation)
    }
    /// Historical node identity metadata only.
    pub fn candidate_score_node(&self, i: usize) -> Option<Digest32> {
        self.candidates.get(i)?.as_ref().map(|o| o.node)
    }
    pub fn measurement_binding(&self) -> Digest32 {
        self.result
    }
    fn check(
        &self,
        frozen: &FrozenDesign<'_, '_>,
        v: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.frozen.check(v)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(v) {
            return Err(Error::ForeignReservation);
        }
        if !std::ptr::eq(self.frozen, frozen)
            || self.report.binding.result != self.result
            || self.report.evidence_payload != self.result
        {
            return Err(Error::BindingMismatch);
        }
        let scratch = claim(v, 4 * 1024 * 1024, 0)?;
        let suite = self.frozen.suite;
        for (i, w) in suite.workloads.iter().enumerate() {
            if !oracle::tick_replay(w, &w.baseline, &suite.baselines[i].schedule, suite.limits)? {
                return Err(Error::PrerequisitePending);
            }
            if let Some(o) = &self.candidates[i] {
                if !oracle::tick_replay(w, &self.frozen.programs[i], &o.schedule, suite.limits)? {
                    return Err(Error::PrerequisitePending);
                }
            }
        }
        scratch.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        Ok(())
    }
    pub fn report_current(
        &self,
        frozen: &FrozenDesign<'_, '_>,
        v: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check(frozen, v)?;
        Ok(&self.report)
    }
    pub fn axis_current(
        &self,
        frozen: &FrozenDesign<'_, '_>,
        pins: SelectionPins,
        v: &ResourceReservationView<'_>,
    ) -> Result<ReportedComponentAxis<'_>, Error> {
        self.check(frozen, v)?;
        if pins != self.frozen.suite.binding.selection {
            return Err(Error::BindingMismatch);
        }
        project_component_reported(&self.report).map_err(Error::Selection)
    }
    pub fn check_selection_row_current(
        &self,
        frozen: &FrozenDesign<'_, '_>,
        policy: &SelectionPolicy<'_>,
        row: &JointCandidate<'_>,
        index: usize,
        v: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let axis = self.axis_current(frozen, policy.pins, v)?;
        let b = self.frozen.binding;
        if row.pins != policy.pins
            || row.id != b.id
            || row.context != b.context
            || row.measurement_binding != self.result
            || row.axes.len() != policy.axes.len()
            || policy.axes.get(index) != Some(&AxisKind::DesignGain)
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
pub fn execute<'a, 'v>(
    frozen: &'a FrozenDesign<'a, 'v>,
    v: &'v ResourceReservationView<'_>,
) -> Result<OwnedDesignGain<'a, 'v>, Error> {
    frozen.check(v)?;
    // Full candidate traces are reserved BEFORE native producer and tick replay.
    let scratch = claim(v, SCRATCH, 0)?;
    let retained = claim(v, RESULT_RAM, 2 * 1024 * 1024)?;
    let suite = frozen.suite;
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(suite.workloads.len())
        .map_err(|_| Error::AllocationFailed)?;
    let mut states = Vec::new();
    states
        .try_reserve_exact(suite.workloads.len())
        .map_err(|_| Error::AllocationFailed)?;
    for (w, program) in suite.workloads.iter().zip(&frozen.programs) {
        frozen.check(v)?;
        let (outcome, status) = match oracle::event_run(w, program, suite.limits)? {
            None => (None, CandidateState::Pending(PendingReason::EventBudget)),
            Some(schedule) => {
                if !oracle::tick_replay(w, program, &schedule, suite.limits)? {
                    (None, CandidateState::Pending(PendingReason::ReplayBudget))
                } else {
                    let observation = scoring::normalize(w, schedule.makespan)?;
                    let node = scoring::node(
                        w,
                        program,
                        &schedule,
                        &observation,
                        scoring::ScorePhase::Candidate(suite.input_profile, &frozen.binding),
                    );
                    (
                        Some(Outcome {
                            schedule,
                            observation,
                            node,
                        }),
                        CandidateState::Observed,
                    )
                }
            }
        };
        candidates.push(outcome);
        states.push(status);
    }
    let state = if candidates.iter().all(Option::is_some) {
        let mut pairs = Vec::new();
        pairs
            .try_reserve_exact(candidates.len())
            .map_err(|_| Error::AllocationFailed)?;
        for (i, candidate) in candidates.iter().enumerate() {
            let actual = candidate.as_ref().ok_or(Error::PrerequisitePending)?;
            let baseline = &suite.baselines[i];
            pairs.push(ProbePair {
                canonical_payload: &suite.workloads[i].key,
                weight: &suite.workloads[i].weight,
                baseline: &baseline.observation.score,
                candidate: &actual.observation.score,
                baseline_node: baseline.node,
                candidate_node: actual.node,
            });
        }
        ComponentState::Measured(paired_gain(&pairs).map_err(Error::Measurement)?)
    } else {
        ComponentState::Unknown(MeasurementGap::MissingObservation)
    };
    let mut h = Sha256::new();
    h.update(PRODUCER);
    h.update(frozen.digest.as_bytes());
    for status in &states {
        h.update([match status {
            CandidateState::Observed => 0,
            CandidateState::Pending(PendingReason::EventBudget) => 1,
            CandidateState::Pending(PendingReason::ReplayBudget) => 2,
        }]);
    }
    for o in &candidates {
        match o {
            Some(o) => {
                h.update([1]);
                h.update(o.node.as_bytes());
            }
            None => h.update([0]),
        };
    }
    match &state {
        ComponentState::Measured(i) => {
            h.update([1]);
            hash_rational(&mut h, i.lower());
            hash_rational(&mut h, i.upper());
        }
        _ => h.update([0]),
    };
    let result = Digest32::from_bytes(h.finalize().into());
    let b = suite.binding;
    let cb = frozen.binding;
    let report = ComponentReport {
        metric: ComponentMetric::Design,
        definition: b.definition,
        binding: MeasurementBinding {
            scope: b.selection.scope.comparison(),
            candidate: cb.candidate_digest,
            source: b.selection.source.digest(),
            target: cb.target,
            context: cb.context.interpretation_digest(),
            baseline: b.selection.baseline,
            producer: digest(PRODUCER),
            transform: cb.transform,
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
    let out = OwnedDesignGain {
        report,
        frozen,
        candidates,
        states,
        result,
        retained,
    };
    scratch.check().map_err(Error::Resource)?;
    out.check(frozen, v)?;
    Ok(out)
}
