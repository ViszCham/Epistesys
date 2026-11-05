//! 日本語: actual C parentとfresh Useの所有。partialでも成功値へ変換しない。
//! English: Own actual C parents/fresh uses; partial execution never becomes success.
use super::*;

pub struct BridgeBranch<'o, 'p, 'i, 'v> {
    native: c::OwnedCompatibility<'o, 'p, 'i, 'v>,
    pub(super) program: &'o BridgeProgram<'p, 'i, 'v>,
    retained: ResourceClaim<'v>,
}
pub(super) struct UseRecord<'b, 'o, 'p, 'i, 'v> {
    pub(super) case: usize,
    pub(super) owner: c::OwnedUse<'b, 'o, 'p, 'i, 'v>,
    pub(super) work_end: u64,
}
pub struct BridgeUseBatch<'b, 'o, 'p, 'i, 'v> {
    uses: Vec<UseRecord<'b, 'o, 'p, 'i, 'v>>,
    pub(super) branch: &'b BridgeBranch<'o, 'p, 'i, 'v>,
    retained: ResourceClaim<'v>,
}
pub struct BridgeUseFailure<'b, 'o, 'p, 'i, 'v> {
    uses: Vec<UseRecord<'b, 'o, 'p, 'i, 'v>>,
    branch: &'b BridgeBranch<'o, 'p, 'i, 'v>,
    cause: BridgeError,
    retained: Option<ResourceClaim<'v>>,
}
impl<'i, 'v> BridgeRoot<'i, 'v> {
    pub fn prepare<'p, 'o>(
        &'p self,
        program: &'o BridgeProgram<'p, 'i, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<BridgeBranch<'o, 'p, 'i, 'v>, BridgeError> {
        program.check(self, view)?;
        let retained = claim(view, 16 * 1024)?;
        let schedule = self.order.map(|case| c::Dispatch {
            case,
            through: c::Stage::Roundtrip,
        });
        let native = c::measure(&self.native, &program.native, &schedule, view).map_err(from_c)?;
        let result = BridgeBranch {
            native,
            program,
            retained,
        };
        result.check(view)?;
        Ok(result)
    }
}
impl<'o, 'p, 'i, 'v> BridgeBranch<'o, 'p, 'i, 'v> {
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), BridgeError> {
        self.program.check(self.program.root, view)?;
        check_claim(&self.retained, view)?;
        self.native
            .report_current(&self.program.root.native, &self.program.native, view)
            .map_err(from_c)?;
        check_claim(&self.retained, view)
    }
    pub fn parent_report_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, BridgeError> {
        self.check(view)?;
        self.native
            .report_current(&self.program.root.native, &self.program.native, view)
            .map_err(from_c)
    }
    // C's public getter shortens the model/query borrow to the current view;
    // it does not expose an independently owned or lifetime-extended Trial.
    pub fn trials_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<&[c::Trial<'_>], BridgeError> {
        self.check(view)?;
        self.native
            .trials_current(&self.program.root.native, &self.program.native, view)
            .map_err(from_c)
    }
    pub fn hard_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<c::HardState, BridgeError> {
        self.check(view)?;
        self.native
            .hard_current(&self.program.root.native, &self.program.native, view)
            .map_err(from_c)
    }
    fn ready(&self, view: &ResourceReservationView<'_>) -> Result<(), BridgeError> {
        match self.hard_current(view)? {
            c::HardState::Fail => return Err(BridgeError::HardInvariantFailed),
            c::HardState::NeedsEvidence => return Err(BridgeError::MissingPreparation),
            c::HardState::Pass => {}
        }
        let trials = self.trials_current(view)?;
        // Roundtrip-only parents intentionally retain the not-yet-used gap.
        // Hard Pass requires all four actual guards; do not demand cached Use.
        if trials.len() != 4
            || trials
                .iter()
                .any(|t| t.type_check() != c::Check::Pass || t.roundtrip_check() != c::Check::Pass)
        {
            return Err(BridgeError::MissingPreparation);
        }
        Ok(())
    }
    pub fn execute_uses<'b>(
        &'b self,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<BridgeUseBatch<'b, 'o, 'p, 'i, 'v>, BridgeUseFailure<'b, 'o, 'p, 'i, 'v>> {
        let mut failure = BridgeUseFailure {
            uses: Vec::new(),
            branch: self,
            cause: BridgeError::MissingPreparation,
            retained: None,
        };
        let execute = (|| -> Result<(), BridgeError> {
            self.ready(view)?;
            failure.retained = Some(claim(view, 16 * 1024)?);
            failure
                .uses
                .try_reserve_exact(4)
                .map_err(|_| BridgeError::AllocationFailed)?;
            for case in self.program.root.order {
                self.check(view)?;
                let owner = c::use_current(
                    &self.program.root.native,
                    &self.program.native,
                    &self.native,
                    case,
                    view,
                )
                .map_err(from_c)?;
                // Retain every returned native owner before another fallible read.
                let work_end = match self.program.root.work_used_current(view) {
                    Ok(value) => value,
                    Err(error) => {
                        failure.uses.push(UseRecord {
                            case,
                            owner,
                            work_end: 0,
                        });
                        return Err(error);
                    }
                };
                failure.uses.push(UseRecord {
                    case,
                    owner,
                    work_end,
                });
                checked_use(self, &failure.uses[failure.uses.len() - 1], view)?;
            }
            self.check(view)
        })();
        if let Err(error) = execute {
            failure.cause = error;
            return Err(failure);
        }
        let Some(retained) = failure.retained.take() else {
            failure.cause = BridgeError::MissingPreparation;
            return Err(failure);
        };
        Ok(BridgeUseBatch {
            uses: failure.uses,
            branch: self,
            retained,
        })
    }
}
impl<'b, 'o, 'p, 'i, 'v> BridgeUseBatch<'b, 'o, 'p, 'i, 'v> {
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), BridgeError> {
        self.branch.check(view)?;
        check_claim(&self.retained, view)?;
        if self.uses.len() != 4 {
            return Err(BridgeError::MissingPreparation);
        }
        for case in self.branch.program.root.order {
            self.record(case, view)?;
        }
        check_claim(&self.retained, view)
    }
    pub(super) fn record(
        &self,
        case: usize,
        view: &ResourceReservationView<'_>,
    ) -> Result<&UseRecord<'b, 'o, 'p, 'i, 'v>, BridgeError> {
        self.branch.check(view)?;
        check_claim(&self.retained, view)?;
        let record = self
            .uses
            .iter()
            .find(|r| r.case == case)
            .ok_or(BridgeError::MissingExecution { case })?;
        checked_use(self.branch, record, view)?;
        Ok(record)
    }
    pub fn use_owner_current(
        &self,
        case: usize,
        view: &ResourceReservationView<'_>,
    ) -> Result<&c::OwnedUse<'b, 'o, 'p, 'i, 'v>, BridgeError> {
        self.check(view)?;
        Ok(&self.record(case, view)?.owner)
    }
}
impl<'b, 'o, 'p, 'i, 'v> BridgeUseFailure<'b, 'o, 'p, 'i, 'v> {
    pub fn cause(&self) -> &BridgeError {
        &self.cause
    }
    pub fn completed_use_count(&self) -> usize {
        self.uses
            .iter()
            .filter(|r| r.owner.actual_use_count() == 2)
            .count()
    }
    pub fn scalar(&self) -> Option<&RationalInterval> {
        None
    }
    pub fn use_owner_current(
        &self,
        case: usize,
        view: &ResourceReservationView<'_>,
    ) -> Result<&c::OwnedUse<'b, 'o, 'p, 'i, 'v>, BridgeError> {
        self.branch.check(view)?;
        if let Some(retained) = &self.retained {
            check_claim(retained, view)?;
        }
        let record = self
            .uses
            .iter()
            .find(|r| r.case == case)
            .ok_or(BridgeError::MissingExecution { case })?;
        checked_use(self.branch, record, view)?;
        Ok(&record.owner)
    }
}
pub(super) fn checked_use(
    branch: &BridgeBranch<'_, '_, '_, '_>,
    record: &UseRecord<'_, '_, '_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<bool, BridgeError> {
    branch.check(view)?;
    let root = branch.program.root;
    let input = root.input;
    let (case, source_value, target_value) =
        match record.owner.outcome_current(view).map_err(from_c)? {
            c::UseOutcome::Executed {
                case,
                source_value,
                target_value,
            } => (case, source_value, target_value),
            c::UseOutcome::NeedsProbe { case } => {
                return Err(BridgeError::MissingExecution { case })
            }
            c::UseOutcome::RejectedCompatibility => return Err(BridgeError::MissingPreparation),
        };
    if case != record.case || case >= 4 || record.owner.actual_use_count() != 2 {
        return Err(BridgeError::BindingMismatch);
    }
    let parent = branch
        .native
        .lineage_current(&root.native, &branch.program.native, view)
        .map_err(from_c)?;
    let child = record.owner.lineage_current(view).map_err(from_c)?;
    if !same_lineage(parent, child) {
        return Err(BridgeError::BindingMismatch);
    }
    let (source, target) = record.owner.models_current(view).map_err(from_c)?;
    if !std::ptr::eq(source.query(), input.source_theory)
        || !std::ptr::eq(target.query(), input.target_theory)
    {
        return Err(BridgeError::BindingMismatch);
    }
    if source.boolean(input.source_hard) != Some(true)
        || target.boolean(input.target_hard) != Some(true)
    {
        return Err(BridgeError::HardInvariantFailed);
    }
    for a in input.cases[case].input {
        if source.boolean(a.symbol) != Some(a.value) {
            return Err(BridgeError::BindingMismatch);
        }
    }
    let target_symbol = input
        .expected_mapping
        .iter()
        .find(|m| m.source == root.use_symbol)
        .ok_or(BridgeError::BindingMismatch)?
        .target;
    if source.boolean(root.use_symbol) != Some(source_value)
        || target.boolean(target_symbol) != Some(target_value)
    {
        return Err(BridgeError::BindingMismatch);
    }
    let mut semantic_equal = true;
    for map in input.expected_mapping {
        let a = source
            .boolean(map.source)
            .ok_or(BridgeError::BindingMismatch)?;
        let b = target
            .boolean(map.target)
            .ok_or(BridgeError::BindingMismatch)?;
        semantic_equal &= a == b;
    }
    record.owner.report_current(view).map_err(from_c)?;
    branch.check(view)?;
    Ok(semantic_equal && source_value == target_value)
}
fn same_lineage(a: &c::Lineage, b: &c::Lineage) -> bool {
    a.source_context == b.source_context
        && a.target_context == b.target_context
        && a.root_goal_revision == b.root_goal_revision
        && a.oracle == b.oracle
        && a.baseline == b.baseline
        && a.forward_program == b.forward_program
        && a.inverse_program == b.inverse_program
        && a.source_store == b.source_store
        && a.target_store == b.target_store
        && a.queries == b.queries
        && a.declared_inputs == b.declared_inputs
        && a.output_frames == b.output_frames
        && a.inverse_frames == b.inverse_frames
        && a.expected_mapping == b.expected_mapping
        && a.plan == b.plan
        && a.definition == b.definition
        && a.policy == b.policy
        && a.budget_series == b.budget_series
}
