//! Actual six-stage case execution and private measured ownership, not report injection.
use super::*;

/// Actual frames/models and arithmetic drop before their retained RAM/Output claim.
/// Logical accounting excludes allocator headers/native stacks/RSS; global OOM is not proven fallible.
///
/// ```compile_fail
/// use lc631_runtime::compatibility_measurement::OwnedCompatibility;
/// use lc631_analysis::creativity_measurement::ComponentReport;
/// fn inject(report: ComponentReport) { OwnedCompatibility::from_report(report); }
/// ```
/// ```compile_fail
/// use lc631_runtime::compatibility_measurement::OwnedCompatibility;
/// let _: OwnedCompatibility<'_, '_, '_, '_> = serde_json::from_str("{}").unwrap();
/// ```
/// ```compile_fail
/// use lc631_runtime::compatibility_measurement::OwnedCompatibility;
/// fn inject() { OwnedCompatibility::from_success(true); }
/// ```
/// ```compile_fail
/// use lc631_runtime::compatibility_measurement::OwnedCompatibility;
/// fn inject() { OwnedCompatibility::from_scalar(1); }
/// ```
pub struct OwnedCompatibility<'o, 'p, 'i, 'v> {
    pub(super) trials: [Trial<'i>; 4],
    report: ComponentReport,
    lineage: Lineage,
    pub(super) plan: &'p Plan<'i, 'v>,
    pub(super) program: &'o Program<'p, 'i, 'v>,
    retained: ResourceClaim<'v>,
}
impl OwnedCompatibility<'_, '_, '_, '_> {
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub fn report_current(
        &self,
        plan: &Plan<'_, '_>,
        program: &Program<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, Error> {
        self.check(plan, program, view)?;
        Ok(&self.report)
    }
    pub fn trials_current(
        &self,
        plan: &Plan<'_, '_>,
        program: &Program<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&[Trial<'_>], Error> {
        self.check(plan, program, view)?;
        Ok(&self.trials)
    }
    pub fn lineage_current(
        &self,
        plan: &Plan<'_, '_>,
        program: &Program<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&Lineage, Error> {
        self.check(plan, program, view)?;
        Ok(&self.lineage)
    }
    pub fn hard_current(
        &self,
        plan: &Plan<'_, '_>,
        program: &Program<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<HardState, Error> {
        self.check(plan, program, view)?;
        Ok(hard_state(&self.assessments()))
    }
    pub(super) fn check(
        &self,
        plan: &Plan<'_, '_>,
        program: &Program<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.plan.check(view)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        if !same_owner(self.plan, plan) || !same_owner(self.program, program) {
            return Err(Error::BindingMismatch);
        }
        self.program.check(self.plan, view)?;
        for trial in &self.trials {
            trial.check_backing()?;
        }
        self.retained.check().map_err(Error::Resource)
    }
    pub(super) fn assessments(&self) -> [Assessment; 4] {
        std::array::from_fn(|i| self.trials[i].assessment)
    }
}
pub fn measure<'o, 'p, 'i, 'v>(
    plan: &'p Plan<'i, 'v>,
    program: &'o Program<'p, 'i, 'v>,
    schedule: &[Dispatch],
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedCompatibility<'o, 'p, 'i, 'v>, Error> {
    program.check(plan, view)?;
    // Validate the WHOLE schedule before any actual stage or work debit.
    admission::schedule(schedule)?;
    let retained = claim(view, 1024 * 1024, 64 * 1024)?;
    let scratch = claim(view, 256 * 1024, 0)?;
    let mut trials = std::array::from_fn(|_| Trial::missing());
    for &case in &plan.order {
        if let Some(dispatch) = schedule.iter().find(|d| d.case == case) {
            trials[case] = run_case(plan, program, case, dispatch.through, view)?;
        }
    }
    let values = std::array::from_fn(|i| trials[i].assessment);
    let (report, lineage) = identity::build(plan, program, &values)?;
    scratch.check().map_err(Error::Resource)?;
    program.check(plan, view)?;
    retained.check().map_err(Error::Resource)?;
    Ok(OwnedCompatibility {
        trials,
        report,
        lineage,
        plan,
        program,
        retained,
    })
}
impl<'i> Trial<'i> {
    fn missing() -> Self {
        Self {
            assessment: Assessment::missing(),
            input: None,
            output: None,
            inverse: None,
            source_model: None,
            target_model: None,
        }
    }
    fn check_backing(&self) -> Result<(), Error> {
        if self.output.as_ref().map(identity::frame) != self.assessment.output
            || self.inverse.as_ref().map(identity::frame) != self.assessment.inverse
            || self.source_model.as_ref().map(|m| identity::wire(&m.bytes))
                != self.assessment.source_wire
            || self.target_model.as_ref().map(|m| identity::wire(&m.bytes))
                != self.assessment.target_wire
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}
fn step(
    plan: &Plan<'_, '_>,
    trial: &mut Trial<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<bool, Error> {
    match plan.debit(1, view) {
        Ok(()) => Ok(true),
        Err(Error::WorkBudget) => {
            trial.assessment.gap = Some(Gap::WorkBudget);
            Ok(false)
        }
        Err(error) => Err(error),
    }
}
fn run_case<'i>(
    plan: &Plan<'i, '_>,
    program: &Program<'_, 'i, '_>,
    case: usize,
    through: Stage,
    view: &ResourceReservationView<'_>,
) -> Result<Trial<'i>, Error> {
    let mut trial = Trial::missing();
    if !transforms(plan, program, case, &mut trial, view)? {
        return Ok(trial);
    }
    if !guards(plan, &mut trial, view)? {
        return Ok(trial);
    }
    if through == Stage::Use {
        uses(plan, &mut trial, view)?;
    }
    plan.check(view)?;
    Ok(trial)
}
fn transforms(
    plan: &Plan<'_, '_>,
    program: &Program<'_, '_, '_>,
    case: usize,
    trial: &mut Trial<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<bool, Error> {
    if !step(plan, trial, view)? {
        return Ok(false);
    }
    let input = plan.case_frame(case)?;
    let output = program.forward_frame(&input);
    trial.input = Some(input);
    trial.output = Some(output);
    trial.assessment.output = Some(identity::frame(&output));
    if !step(plan, trial, view)? {
        return Ok(false);
    }
    let inverse = program.inverse_frame(&output);
    trial.inverse = Some(inverse);
    trial.assessment.inverse = Some(identity::frame(&inverse));
    trial.assessment.roundtrip = if input == inverse {
        Check::Pass
    } else {
        Check::Fail
    };
    Ok(true)
}
fn guards<'i>(
    plan: &Plan<'i, '_>,
    trial: &mut Trial<'i>,
    view: &ResourceReservationView<'_>,
) -> Result<bool, Error> {
    if !step(plan, trial, view)? {
        return Ok(false);
    }
    let input = trial.input.as_ref().ok_or(Error::BindingMismatch)?;
    trial.source_model = Some(frame::guard(
        input,
        plan.input.source_theory,
        plan.limits.max_frame_bytes,
    )?);
    trial.assessment.source_wire = trial
        .source_model
        .as_ref()
        .map(|m| identity::wire(&m.bytes));
    if !step(plan, trial, view)? {
        return Ok(false);
    }
    let output = trial.output.as_ref().ok_or(Error::BindingMismatch)?;
    trial.target_model = Some(frame::guard(
        output,
        plan.input.target_theory,
        plan.limits.max_frame_bytes,
    )?);
    trial.assessment.target_wire = trial
        .target_model
        .as_ref()
        .map(|m| identity::wire(&m.bytes));
    let a = trial
        .source_model
        .as_ref()
        .ok_or(Error::BindingMismatch)?
        .hard;
    let b = trial
        .target_model
        .as_ref()
        .ok_or(Error::BindingMismatch)?
        .hard;
    trial.assessment.typed = Check::Pass;
    trial.assessment.hard = if a == Check::Fail || b == Check::Fail {
        HardState::Fail
    } else {
        HardState::Pass
    };
    Ok(true)
}
fn uses<'i>(
    plan: &Plan<'i, '_>,
    trial: &mut Trial<'i>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    if !step(plan, trial, view)? {
        return Ok(());
    }
    let input = trial.input.as_ref().ok_or(Error::BindingMismatch)?;
    let source = frame::use_stage(
        input,
        plan.input.source_theory,
        plan.input.source_use,
        plan.source_use,
        plan.limits.max_frame_bytes,
    )?;
    trial.assessment.source = source.value;
    trial.assessment.source_wire = Some(identity::wire(&source.bytes));
    trial.source_model = Some(source);
    if !step(plan, trial, view)? {
        return Ok(());
    }
    let output = trial.output.as_ref().ok_or(Error::BindingMismatch)?;
    let target = frame::use_stage(
        output,
        plan.input.target_theory,
        plan.input.target_use,
        plan.target_use,
        plan.limits.max_frame_bytes,
    )?;
    trial.assessment.target = target.value;
    trial.assessment.target_wire = Some(identity::wire(&target.bytes));
    trial.target_model = Some(target);
    trial.assessment.used = compare(trial.assessment.source, trial.assessment.target);
    trial.assessment.gap = None;
    Ok(())
}
pub(super) fn compare(a: Option<bool>, b: Option<bool>) -> Check {
    match (a, b) {
        (Some(a), Some(b)) if a == b => Check::Pass,
        (Some(_), Some(_)) => Check::Fail,
        (None, _) | (_, None) => Check::Unknown,
    }
}
