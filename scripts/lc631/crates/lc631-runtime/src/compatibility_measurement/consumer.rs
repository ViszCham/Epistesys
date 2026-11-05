//! Current interface consumption: two fresh bounded UseStages, never cached Ready as execution.
use super::*;
use lc631_solvers::smt::CheckedModel;

/// A new private owner retains actual fresh checked models and an immutable child report.
/// No Clone/Deref/Deserialize/status or detached-report constructor exists.
///
/// ```compile_fail
/// use lc631_runtime::compatibility_measurement::OwnedUse;
/// fn inject() { OwnedUse::from_success(true); }
/// ```
/// ```compile_fail
/// use lc631_runtime::compatibility_measurement::OwnedUse;
/// let _: OwnedUse<'_, '_, '_, '_, '_> = serde_json::from_str("{}").unwrap();
/// ```
pub struct OwnedUse<'r, 'o, 'p, 'i, 'v> {
    source: Option<frame::ModelFrame<'i>>,
    target: Option<frame::ModelFrame<'i>>,
    report: ComponentReport,
    lineage: Lineage,
    readiness: NextUse,
    outcome: UseOutcome,
    parent: &'r OwnedCompatibility<'o, 'p, 'i, 'v>,
    retained: ResourceClaim<'v>,
}
impl OwnedUse<'_, '_, '_, '_, '_> {
    pub fn outcome_current(&self, view: &ResourceReservationView<'_>) -> Result<UseOutcome, Error> {
        self.check(view)?;
        Ok(self.outcome)
    }
    pub fn models_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(&CheckedModel<'_>, &CheckedModel<'_>), Error> {
        self.check(view)?;
        let source = self
            .source
            .as_ref()
            .and_then(|m| m.model.as_ref())
            .ok_or(Error::NoExecutedUse)?;
        let target = self
            .target
            .as_ref()
            .and_then(|m| m.model.as_ref())
            .ok_or(Error::NoExecutedUse)?;
        Ok((source, target))
    }
    pub fn actual_use_count(&self) -> usize {
        // Historical occurrence count only; this accessor does not renew eligibility.
        match self.outcome {
            UseOutcome::Executed { .. } => 2,
            UseOutcome::NeedsProbe { .. } | UseOutcome::RejectedCompatibility => 0,
        }
    }
    pub fn report_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check(view)?;
        Ok(&self.report)
    }
    pub fn lineage_current(&self, view: &ResourceReservationView<'_>) -> Result<&Lineage, Error> {
        self.check(view)?;
        Ok(&self.lineage)
    }
    pub fn readiness_current(&self, view: &ResourceReservationView<'_>) -> Result<NextUse, Error> {
        self.check(view)?;
        Ok(self.readiness)
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.parent
            .check(self.parent.plan, self.parent.program, view)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check().map_err(Error::Resource)
    }
}
pub fn next_use_current(
    plan: &Plan<'_, '_>,
    program: &Program<'_, '_, '_>,
    owner: &OwnedCompatibility<'_, '_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<NextUse, Error> {
    owner.check(plan, program, view)?;
    let values = owner.assessments();
    if hard_state(&values) == HardState::Fail {
        return Err(Error::HardInvariantFailed);
    }
    let next = readiness(&plan.order, &values);
    owner.check(plan, program, view)?;
    Ok(next)
}
/// The checked pair budget is not refunded after execution. Current checks are
/// point-in-time admission, not atomic revocation, a permission issuer or rollback.
pub fn use_current<'r, 'o, 'p, 'i, 'v>(
    plan: &'p Plan<'i, 'v>,
    program: &'o Program<'p, 'i, 'v>,
    owner: &'r OwnedCompatibility<'o, 'p, 'i, 'v>,
    case: usize,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedUse<'r, 'o, 'p, 'i, 'v>, Error> {
    owner.check(plan, program, view)?;
    let trial = owner.trials.get(case).ok_or(Error::InvalidInput)?;
    let values = owner.assessments();
    if hard_state(&values) == HardState::Fail {
        return Err(Error::HardInvariantFailed);
    }
    if readiness(&plan.order, &values) == NextUse::RejectedCompatibility {
        return unexecuted(
            plan,
            program,
            owner,
            values,
            UseOutcome::RejectedCompatibility,
            view,
        );
    }
    plan.preflight(2, view)?;
    if trial.assessment.typed != Check::Pass
        || trial.assessment.roundtrip != Check::Pass
        || trial.input.is_none()
        || trial.output.is_none()
    {
        return unexecuted(
            plan,
            program,
            owner,
            values,
            UseOutcome::NeedsProbe { case },
            view,
        );
    }
    execute(plan, program, owner, case, view)
}
fn unexecuted<'r, 'o, 'p, 'i, 'v>(
    plan: &'p Plan<'i, 'v>,
    program: &'o Program<'p, 'i, 'v>,
    owner: &'r OwnedCompatibility<'o, 'p, 'i, 'v>,
    values: [Assessment; 4],
    outcome: UseOutcome,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedUse<'r, 'o, 'p, 'i, 'v>, Error> {
    let retained = claim(view, 128 * 1024, 64 * 1024)?;
    let (report, lineage) = identity::build(plan, program, &values)?;
    owner.check(plan, program, view)?;
    retained.check().map_err(Error::Resource)?;
    Ok(OwnedUse {
        source: None,
        target: None,
        report,
        lineage,
        readiness: readiness(&plan.order, &values),
        outcome,
        parent: owner,
        retained,
    })
}
fn execute<'r, 'o, 'p, 'i, 'v>(
    plan: &'p Plan<'i, 'v>,
    program: &'o Program<'p, 'i, 'v>,
    owner: &'r OwnedCompatibility<'o, 'p, 'i, 'v>,
    case: usize,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedUse<'r, 'o, 'p, 'i, 'v>, Error> {
    let retained = claim(view, 512 * 1024, 64 * 1024)?;
    let scratch = claim(view, 256 * 1024, 0)?;
    let trial = &owner.trials[case];
    let input = trial.input.as_ref().ok_or(Error::BindingMismatch)?;
    let output = trial.output.as_ref().ok_or(Error::BindingMismatch)?;
    // Reserve/debit both logical UseStages together before either actual invocation.
    plan.debit(2, view)?;
    let source = frame::use_stage(
        input,
        plan.input.source_theory,
        plan.input.source_use,
        plan.source_use,
        plan.limits.max_frame_bytes,
    )?;
    let target = frame::use_stage(
        output,
        plan.input.target_theory,
        plan.input.target_use,
        plan.target_use,
        plan.limits.max_frame_bytes,
    )?;
    let source_value = source.value.ok_or(Error::HardInvariantFailed)?;
    let target_value = target.value.ok_or(Error::HardInvariantFailed)?;
    let mut values = owner.assessments();
    values[case].source = Some(source_value);
    values[case].target = Some(target_value);
    values[case].source_wire = Some(identity::wire(&source.bytes));
    values[case].target_wire = Some(identity::wire(&target.bytes));
    values[case].used = evaluation::compare(Some(source_value), Some(target_value));
    values[case].gap = None;
    let (report, lineage) = identity::build(plan, program, &values)?;
    scratch.check().map_err(Error::Resource)?;
    owner.check(plan, program, view)?;
    retained.check().map_err(Error::Resource)?;
    Ok(OwnedUse {
        source: Some(source),
        target: Some(target),
        report,
        lineage,
        readiness: readiness(&plan.order, &values),
        outcome: UseOutcome::Executed {
            case,
            source_value,
            target_value,
        },
        parent: owner,
        retained,
    })
}
