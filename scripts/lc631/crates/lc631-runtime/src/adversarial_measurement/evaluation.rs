//! 日本語: 実有限searchのrowを別SMT evaluatorで再検査。内部model入力はstdoutではない。
//! English: Recheck actual finite-search rows with the separate SMT evaluator.
//! Internal model syntax is constructed witness input, never solver stdout/receipt.
use super::*;
use lc631_solvers::{
    protocol::sexpr::{parse, SExprLimits},
    smt::validate_model,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrialGap {
    NotDispatched,
    SearchBudget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrialState {
    Pass,
    Failure,
    Unknown(TrialGap),
    NoComparable(formal::NoComparable),
}
struct Witness {
    row: u16,
    query: SmtStatusQuery,
    model: Vec<u8>,
}
struct Record {
    state: TrialState,
    witness: Option<Witness>,
    assignments: u32,
    visits: u64,
}
pub struct OwnedCounterRisk<'a, 'v> {
    report: ComponentReport,
    frozen: &'a FrozenResponses<'a, 'v>,
    records: Vec<Record>,
    states: Vec<TrialState>,
    result: Digest32,
    retained: ResourceClaim<'v>,
}
impl OwnedCounterRisk<'_, '_> {
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub fn historical_trials(&self) -> &[TrialState] {
        &self.states
    }
    pub fn fixed_weight(&self) -> &Rational {
        &self.frozen.catalog.weight
    }
    pub fn measurement_binding(&self) -> Digest32 {
        self.result
    }
    pub fn counterrow(&self, i: usize) -> Option<u16> {
        self.records.get(i)?.witness.as_ref().map(|w| w.row)
    }
    pub fn witness_query(&self, i: usize) -> Option<&SmtStatusQuery> {
        self.records.get(i)?.witness.as_ref().map(|w| &w.query)
    }
    pub fn witness_model(&self, i: usize) -> Option<&[u8]> {
        self.records
            .get(i)?
            .witness
            .as_ref()
            .map(|w| w.model.as_slice())
    }
    fn check(
        &self,
        frozen: &FrozenResponses<'_, '_>,
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
        let scratch = claim(view, 4 * 1024 * 1024, 0)?;
        for r in &self.records {
            if let Some(w) = &r.witness {
                check_witness(w)?;
            } else if r.state == TrialState::Failure {
                return Err(Error::Witness);
            }
        }
        scratch.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        Ok(())
    }
    pub fn report_current(
        &self,
        frozen: &FrozenResponses<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check(frozen, view)?;
        Ok(&self.report)
    }
    pub fn axis_current(
        &self,
        frozen: &FrozenResponses<'_, '_>,
        pins: SelectionPins,
        view: &ResourceReservationView<'_>,
    ) -> Result<ReportedComponentAxis<'_>, Error> {
        self.check(frozen, view)?;
        if pins != self.frozen.catalog.binding.selection {
            return Err(Error::BindingMismatch);
        }
        project_component_reported(&self.report).map_err(Error::Selection)
    }
    pub fn check_selection_row_current(
        &self,
        frozen: &FrozenResponses<'_, '_>,
        policy: &SelectionPolicy<'_>,
        row: &JointCandidate<'_>,
        index: usize,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let axis = self.axis_current(frozen, policy.pins, view)?;
        let b = self.frozen.binding;
        if row.pins != policy.pins
            || row.id != b.id
            || row.context != b.context
            || row.measurement_binding != self.result
            || row.axes.len() != policy.axes.len()
            || policy.axes.get(index) != Some(&AxisKind::CounterexampleCost)
        {
            return Err(Error::BindingMismatch);
        }
        let same = match (row.axes.get(index), axis.value) {
            (Some(AxisValue::Exact(a)), AxisValue::Exact(b))
            | (Some(AxisValue::Bounded(a)), AxisValue::Bounded(b)) => std::ptr::eq(*a, b),
            (Some(AxisValue::Unavailable(a)), AxisValue::Unavailable(b)) => *a == b,
            _ => false,
        };
        if !same {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}
pub fn execute<'a, 'v>(
    frozen: &'a FrozenResponses<'a, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedCounterRisk<'a, 'v>, Error> {
    frozen.check(view)?;
    let scratch = claim(view, SCRATCH, 0)?;
    let retained = claim(view, 8 * 1024 * 1024, 1024 * 1024)?;
    let catalog = frozen.catalog;
    let mut records = Vec::new();
    records
        .try_reserve_exact(catalog.cases.len())
        .map_err(|_| Error::AllocationFailed)?;
    let mut states = Vec::new();
    states
        .try_reserve_exact(catalog.cases.len())
        .map_err(|_| Error::AllocationFailed)?;
    let zero = Rational::from_integer(0.into());
    let mut failed = zero.clone();
    let mut unknown = zero;
    let mut observed = 0usize;
    let mut no_comparable = false;
    for (i, (c, response)) in catalog.cases.iter().zip(&frozen.terms).enumerate() {
        frozen.check(view)?;
        let record = if i >= catalog.limits.max_trials {
            Record {
                state: TrialState::Unknown(TrialGap::NotDispatched),
                witness: None,
                assignments: 0,
                visits: 0,
            }
        } else if catalog.limits.max_ast_visits == 0 {
            Record {
                state: TrialState::Unknown(TrialGap::SearchBudget),
                witness: None,
                assignments: 0,
                visits: 0,
            }
        } else {
            let property = ast::agreement(response, &c.reference);
            let yes = BoolExpr::Constant(true);
            let mut request = ast::request(catalog.binding, c, &property, &yes);
            request.binding.probe_map =
                formal::probe_map_digest(&request, 1_000_000).map_err(Error::Formal)?;
            match formal::evaluate(&request, catalog.limits.max_ast_visits)
                .map_err(Error::Formal)?
            {
                formal::Evaluation::Computed(report) => {
                    let assignments = report.assignments();
                    let visits = report.ast_visits();
                    match report.soundness() {
                        formal::DirectionResult::Preserved => Record {
                            state: TrialState::Pass,
                            witness: None,
                            assignments,
                            visits,
                        },
                        formal::DirectionResult::Countermodel(row) => {
                            let witness = Witness {
                                row,
                                query: ast::failure(c, &property)?,
                                model: model(c.atoms.len(), row)?,
                            };
                            check_witness(&witness)?;
                            Record {
                                state: TrialState::Failure,
                                witness: Some(witness),
                                assignments,
                                visits,
                            }
                        }
                    }
                }
                formal::Evaluation::Unknown { ast_visits } => Record {
                    state: TrialState::Unknown(TrialGap::SearchBudget),
                    witness: None,
                    assignments: 0,
                    visits: ast_visits,
                },
                formal::Evaluation::NoComparable { reason, ast_visits } => Record {
                    state: TrialState::NoComparable(reason),
                    witness: None,
                    assignments: 0,
                    visits: ast_visits,
                },
            }
        };
        match record.state {
            TrialState::Pass => observed += 1,
            TrialState::Failure => {
                observed += 1;
                failed = numeric::checked_add(&failed, &c.weight).map_err(Error::Numeric)?;
            }
            TrialState::Unknown(_) => {
                unknown = numeric::checked_add(&unknown, &c.weight).map_err(Error::Numeric)?
            }
            TrialState::NoComparable(_) => no_comparable = true,
        }
        states.push(record.state);
        records.push(record);
    }
    let state = if no_comparable {
        ComponentState::Incomparable {
            scope: catalog.binding.attack_scope,
        }
    } else if observed == 0 {
        ComponentState::Unknown(MeasurementGap::MissingObservation)
    } else {
        let lower = numeric::checked_divide(&failed, &catalog.weight).map_err(Error::Numeric)?;
        let upper = numeric::checked_divide(
            &numeric::checked_add(&failed, &unknown).map_err(Error::Numeric)?,
            &catalog.weight,
        )
        .map_err(Error::Numeric)?;
        ComponentState::Measured(RationalInterval::new(lower, upper).map_err(Error::Numeric)?)
    };
    let mut h = Sha256::new();
    h.update(PRODUCER);
    h.update(frozen.digest.as_bytes());
    for r in &records {
        h.update([match r.state {
            TrialState::Pass => 0,
            TrialState::Failure => 1,
            TrialState::Unknown(TrialGap::NotDispatched) => 2,
            TrialState::Unknown(TrialGap::SearchBudget) => 3,
            TrialState::NoComparable(formal::NoComparable::MissingTheoryScope) => 4,
            TrialState::NoComparable(formal::NoComparable::SourceContextInconsistent) => 5,
            TrialState::NoComparable(formal::NoComparable::TargetContextInconsistent) => 6,
        }]);
        h.update(r.assignments.to_le_bytes());
        h.update(r.visits.to_le_bytes());
        if let Some(w) = &r.witness {
            h.update(w.row.to_le_bytes());
            field(&mut h, &w.model);
            h.update((w.query.assertions().len() as u64).to_le_bytes());
            for e in w.query.assertions() {
                ast::hash_expr(&mut h, e);
            }
        }
    }
    match &state {
        ComponentState::Measured(i) => {
            h.update([1]);
            hash_rational(&mut h, i.lower());
            hash_rational(&mut h, i.upper());
        }
        ComponentState::Incomparable { .. } => h.update([2]),
        _ => h.update([3]),
    };
    let result = Digest32::from_bytes(h.finalize().into());
    let b = catalog.binding;
    let cb = frozen.binding;
    let report = ComponentReport {
        metric: ComponentMetric::CounterexampleRisk,
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
    let owned = OwnedCounterRisk {
        report,
        frozen,
        records,
        states,
        result,
        retained,
    };
    scratch.check().map_err(Error::Resource)?;
    owned.check(frozen, view)?;
    Ok(owned)
}
fn model(atoms: usize, row: u16) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(4096)
        .map_err(|_| Error::AllocationFailed)?;
    bytes.extend_from_slice(b"(model");
    for i in 0..atoms {
        bytes.extend_from_slice(b" (define-fun b");
        bytes.push(b'0' + i as u8);
        bytes.extend_from_slice(b" () Bool ");
        bytes.extend_from_slice(if row & (1 << i) != 0 {
            b"true"
        } else {
            b"false"
        });
        bytes.push(b')');
    }
    bytes.push(b')');
    Ok(bytes)
}
fn check_witness(w: &Witness) -> Result<(), Error> {
    let document = parse(
        &w.model,
        SExprLimits::checked(4096, 128, 8).map_err(Error::Parse)?,
    )
    .map_err(Error::Parse)?;
    let checked = validate_model(&w.query, &document).map_err(Error::Model)?;
    for (i, symbol) in w.query.boolean_symbols().iter().enumerate() {
        if checked.boolean(*symbol) != Some(w.row & (1 << i) != 0) {
            return Err(Error::Witness);
        }
    }
    Ok(())
}
