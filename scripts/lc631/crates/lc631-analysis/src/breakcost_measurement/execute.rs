//! Bounded real dispatch, all hard guards before any optional checks, absolute K1.
use super::*;
pub(super) struct Actual {
    pub report: ComponentReport,
    pub binding: Digest32,
    pub outputs: Vec<Vec<u8>>,
    pub outcomes: Vec<Vec<PropertyOutcome>>,
    pub hard: LocalHardStatus,
    pub counters: ExecutionCounters,
}
pub(super) struct Observation {
    pub outputs: Vec<Vec<u8>>,
    pub outcomes: Vec<Vec<PropertyOutcome>>,
    pub kinds: Vec<AdapterKind>,
    pub hard: LocalHardStatus,
    pub counters: ExecutionCounters,
}
struct Run<'r, 'a, 'v> {
    root: &'r RootDefinition<'a, 'v>,
    outputs: Vec<Vec<u8>>,
    values: Vec<serde_json::Value>,
    outcomes: Vec<Vec<PropertyOutcome>>,
    kinds: Vec<AdapterKind>,
    hard: LocalHardStatus,
    counters: ExecutionCounters,
    extra_hard: usize,
}
pub(super) fn run(
    root: &RootDefinition<'_, '_>,
    modes: &[AdapterKind],
    candidate: Option<CandidateBinding>,
    baseline_result: Option<Digest32>,
    prerequisite: LocalHardStatus,
    view: &ResourceReservationView<'_>,
) -> Result<Actual, Error> {
    root.check(view)?;
    let scratch = resources::claim(view, 0)?;
    let mut run = Run {
        root,
        outputs: resources::vector(root.cases.len())?,
        values: resources::vector(root.cases.len())?,
        outcomes: resources::vector(root.cases.len())?,
        kinds: resources::vector(root.cases.len())?,
        hard: prerequisite,
        counters: ExecutionCounters {
            adapter_calls: 0,
            serializations: 0,
            property_checks: 0,
            soft_checks: 0,
        },
        extra_hard: 0,
    };
    run.produce(modes, view)?;
    run.hard_checks(view)?;
    if run.hard == LocalHardStatus::NotRejected {
        run.optional_checks(view)?;
    }
    let state = aggregate::state(root, run.hard, &run.outcomes)?;
    let observation = Observation {
        outputs: run.outputs,
        outcomes: run.outcomes,
        kinds: run.kinds,
        hard: run.hard,
        counters: run.counters,
    };
    let (report, binding) = identity::report(root, candidate, baseline_result, &observation, state);
    root.check(view)?;
    resources::current(&scratch, view)?;
    Ok(Actual {
        report,
        binding,
        outputs: observation.outputs,
        outcomes: observation.outcomes,
        hard: observation.hard,
        counters: observation.counters,
    })
}
impl Run<'_, '_, '_> {
    fn produce(
        &mut self,
        modes: &[AdapterKind],
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let b = self.root.budget;
        let mut total = 0usize;
        for &index in &self.root.order {
            self.root.check(view)?;
            if self.counters.adapter_calls >= b.max_adapter_calls_per_run
                || self.counters.serializations >= b.max_serializations_per_run
            {
                return Err(Error::DispatchBudget);
            }
            let cap = b.max_output_bytes_per_case.min(
                b.max_output_bytes_per_run
                    .checked_sub(total)
                    .ok_or(Error::OutputBudget)?,
            );
            if cap == 0 {
                return Err(Error::OutputBudget);
            }
            let kind = if modes.is_empty() {
                self.root.baseline
            } else {
                *modes.get(index).ok_or(Error::InvalidSuite)?
            };
            self.counters.adapter_calls = self
                .counters
                .adapter_calls
                .checked_add(1)
                .ok_or(Error::DispatchBudget)?;
            self.counters.serializations = self
                .counters
                .serializations
                .checked_add(1)
                .ok_or(Error::DispatchBudget)?;
            let bytes = output::produce(self.root.cases[index].input, kind, cap)?;
            total = total.checked_add(bytes.len()).ok_or(Error::OutputBudget)?;
            let value = serde_json::from_slice(&bytes).map_err(|_| Error::ProducerFailure)?;
            let mut outcomes = resources::vector(self.root.rules.len())?;
            outcomes.resize(
                self.root.rules.len(),
                PropertyOutcome::NotObserved(NotObservedReason::SoftCheckBudget),
            );
            self.outputs.push(bytes);
            self.values.push(value);
            self.outcomes.push(outcomes);
            self.kinds.push(kind);
        }
        self.root.check(view)
    }
    fn hard_checks(&mut self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        use LegacyProperty::*;
        let registry = [
            SourceIdentity,
            CandidateIdentity,
            NoHostSend,
            NoOutputCommit,
            NoLegacyCompletion,
            NoAuthorityCreation,
            NegotiatedSchema,
            JsonObjectEnvelope,
            V1StrictConsumerAcceptance,
        ];
        for case in 0..self.outputs.len() {
            for property in registry {
                let elected = self.root.rules.iter().position(|r| r.property == property);
                if admission::optional(property)
                    && !elected
                        .map(|i| self.root.rules[i].class == PropertyClass::Hard)
                        .unwrap_or(false)
                {
                    continue;
                }
                if admission::optional(property) {
                    if self.extra_hard >= self.root.budget.max_extra_hard_checks {
                        return Err(Error::PropertyBudget);
                    }
                    self.extra_hard = self
                        .extra_hard
                        .checked_add(1)
                        .ok_or(Error::PropertyBudget)?;
                }
                self.check_slot(view)?;
                let result = self.property(case, property);
                self.hard = combine(self.hard, result);
                if let Some(i) = elected {
                    self.outcomes[case][i] = result;
                }
            }
        }
        self.root.check(view)
    }
    fn check_slot(&mut self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.root.check(view)?;
        if self.counters.property_checks >= self.root.budget.max_property_checks_per_run {
            return Err(Error::PropertyBudget);
        }
        self.counters.property_checks = self
            .counters
            .property_checks
            .checked_add(1)
            .ok_or(Error::PropertyBudget)?;
        Ok(())
    }
    fn optional_checks(&mut self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        for case in 0..self.outputs.len() {
            for rule in 0..self.root.rules.len() {
                if self.root.rules[rule].class != PropertyClass::Optional {
                    continue;
                }
                if self.counters.soft_checks >= self.root.budget.max_soft_checks_per_run {
                    continue;
                }
                if self.counters.property_checks >= self.root.budget.max_property_checks_per_run {
                    self.outcomes[case][rule] =
                        PropertyOutcome::NotObserved(NotObservedReason::PropertyBudget);
                    continue;
                }
                self.check_slot(view)?;
                self.counters.soft_checks = self
                    .counters
                    .soft_checks
                    .checked_add(1)
                    .ok_or(Error::PropertyBudget)?;
                self.outcomes[case][rule] = self.property(case, self.root.rules[rule].property);
            }
        }
        self.root.check(view)
    }
    fn property(&self, case: usize, property: LegacyProperty) -> PropertyOutcome {
        output::property(
            property,
            &self.values[case],
            &self.outputs[case],
            self.root.cases[self.root.order[case]].input,
            self.kinds[case],
        )
    }
}
fn combine(status: LocalHardStatus, result: PropertyOutcome) -> LocalHardStatus {
    match (status, result) {
        (LocalHardStatus::Failed, _) | (_, PropertyOutcome::ConfirmedViolation) => {
            LocalHardStatus::Failed
        }
        (LocalHardStatus::Pending, _) | (_, PropertyOutcome::NotObserved(_)) => {
            LocalHardStatus::Pending
        }
        (LocalHardStatus::NotRejected, PropertyOutcome::Preserved) => LocalHardStatus::NotRejected,
    }
}
