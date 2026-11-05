//! Explicit ONE native strict-improvement step. No optimizer completion/proof.
use super::{types::*, ExecutionBudget};
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_solvers::{
    runner::SolverExecutionReport,
    smt::native_probe::bounded::BoundsError,
    smt::{native_model::NativeModelError, native_probe::NativeProbeError, ObjectiveDirection},
};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    Digest32,
};
use std::time::Instant;

mod observations;
mod produce;
mod snapshot;
use snapshot::ModelSnapshot;

pub const OBJECTIVE_RECEIPT_RAM: u64 = 2 * 1024 * 1024;
pub const OBJECTIVE_RECEIPT_OUTPUT: u64 = 512 * 1024;
pub const OBJECTIVE_STREAM_CAP: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OneStepBudget {
    attempts: u8,
    wall_ms: u64,
}
impl OneStepBudget {
    pub fn checked(
        max_native_attempts: u8,
        max_total_wall_ms: u64,
    ) -> Result<Self, AdmissionError> {
        if max_native_attempts != 3 || max_total_wall_ms == 0 || max_total_wall_ms > 60_000 {
            return Err(AdmissionError::InvalidBudget);
        }
        Ok(Self {
            attempts: max_native_attempts,
            wall_ms: max_total_wall_ms,
        })
    }
    pub const fn max_native_attempts(self) -> u8 {
        self.attempts
    }
    pub const fn max_total_wall_ms(self) -> u64 {
        self.wall_ms
    }
    /// All three native calls share this elapsed canonical-solve allowance.
    /// The outer broker's existing absolute deadline also remains mandatory.
    /// Preparation/cleanup and final rendering are not an atomic wall cutoff;
    /// this clamps dispatch allowances/timeouts, not a proven total duration.
    pub(super) fn execution(
        self,
        base: ExecutionBudget,
        started: Instant,
    ) -> Result<Option<ExecutionBudget>, AdmissionError> {
        let elapsed = u64::try_from(started.elapsed().as_millis())
            .map_err(|_| AdmissionError::InvalidBudget)?;
        let Some(remaining) = self.wall_ms.checked_sub(elapsed).filter(|n| *n > 0) else {
            return Ok(None);
        };
        if base.max_stdout_bytes() > OBJECTIVE_STREAM_CAP
            || base.max_stderr_bytes() > OBJECTIVE_STREAM_CAP
        {
            return Err(AdmissionError::InvalidBudget);
        }
        ExecutionBudget::checked(
            base.timeout_ms().min(remaining),
            base.max_stdin_bytes(),
            base.max_stdout_bytes(),
            base.max_stderr_bytes(),
            base.max_argv_count(),
            base.max_argv_bytes(),
            base.ram_bytes(),
            base.disk_bytes(),
            base.cleanup_bytes(),
        )
        .map(Some)
        .map_err(|_| AdmissionError::InvalidBudget)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveStepState {
    MissingNativeModel,
    UnavailableStatusCapability,
    BudgetLimited,
    ReportedNoStrictImprovement,
    ImprovedModelRechecked,
    ProbeUnresolved,
    ImprovedModelRejected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveNormalization {
    PositiveWidth,
    UndefinedZeroWidth,
    MissingSelectedValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptimizationError {
    Resource(ResourceError),
    AllocationFailed,
    SourceBinding,
    Probe(NativeProbeError),
    Bounds(BoundsError),
    Query(lc631_solvers::typed_query::QueryEmitError),
    Store(lc631_solvers::supervisor_adapter::OwnedPayloadStoreInitError),
    UnexpectedProbeStatus,
    InvalidBudget,
    Numeric(lc631_substrate::measurement_numeric::NumericError),
}

/// Private construction requires actual native execution and checked snapshots.
/// Selected report may be the INITIAL actual model (e.g. status UNSAT/unknown);
/// it never implies a third execution. Raw probe stdout/stderr are UNOWNED:
/// only the real report, digests/counts/status and exact query bytes are kept.
pub struct OwnedObjectiveStep<'view> {
    pub(super) source: SourceRevision,
    pub(super) context: ContextRevision,
    pub(super) goal_revision: Digest32,
    pub(super) objective: lc631_solvers::typed_query::IntSymbolId,
    pub(super) direction: ObjectiveDirection,
    pub(super) lower: i64,
    pub(super) upper: i64,
    pub(super) initial_value: Option<i64>,
    pub(super) selected_value: Option<i64>,
    pub(super) quality: Option<RationalInterval>,
    pub(super) gap: Option<u128>,
    pub(super) normalization: ObjectiveNormalization,
    pub(super) state: ObjectiveStepState,
    pub(super) attempts: u8,
    initial: ModelSnapshot,
    improvement: ModelSnapshot,
    pub(super) probe_query: Vec<u8>,
    pub(super) probe_report: Option<SolverExecutionReport>,
    pub(super) native_error: Option<NativeModelError<'view>>,
    pub(super) model_store: Option<lc631_solvers::supervisor_adapter::OwnedPayloadStore<'view>>,
    pub(super) binding_at_render: Option<Digest32>,
    pub(super) failure: Option<OptimizationError>,
    pub(super) claim: ResourceClaim<'view>,
}
impl OwnedObjectiveStep<'_> {
    pub fn state(&self) -> ObjectiveStepState {
        self.state
    }
    pub fn normalization(&self) -> ObjectiveNormalization {
        self.normalization
    }
    pub fn direction(&self) -> ObjectiveDirection {
        self.direction
    }
    pub fn lower(&self) -> i64 {
        self.lower
    }
    pub fn upper(&self) -> i64 {
        self.upper
    }
    pub fn initial_value(&self) -> Option<i64> {
        self.initial_value
    }
    pub fn selected_value(&self) -> Option<i64> {
        self.selected_value
    }
    pub fn goal_quality(&self) -> Option<&RationalInterval> {
        self.quality.as_ref()
    }
    pub fn gap_upper_bound(&self) -> Option<u128> {
        self.gap
    }
    pub fn attempts_requested(&self) -> u8 {
        self.attempts
    }
    pub fn probe_report(&self) -> Option<SolverExecutionReport> {
        self.probe_report
    }
    pub fn selected_report(&self) -> Option<SolverExecutionReport> {
        self.selected().report
    }
    pub fn selected_stdout(&self) -> &[u8] {
        &self.selected().stdout
    }
    pub fn selected_query_bytes(&self) -> &[u8] {
        &self.selected().query
    }
    pub fn probe_query_bytes(&self) -> &[u8] {
        &self.probe_query
    }
    pub fn native_error(&self) -> Option<&NativeModelError<'_>> {
        self.native_error.as_ref()
    }
    pub fn failure(&self) -> Option<&OptimizationError> {
        self.failure.as_ref()
    }
    pub fn raw_probe_streams_owned(&self) -> bool {
        false
    }
    pub fn wall_cutoff_is_atomic(&self) -> bool {
        false
    }
    pub fn third_model_report(&self) -> Option<SolverExecutionReport> {
        observations::report(self)
    }
    pub fn third_model_stdout(&self) -> Option<&[u8]> {
        observations::stdout(self)
    }
    pub fn third_observation_binding(&self) -> Option<Digest32> {
        observations::binding(self)
    }
    /// Exact observation identity supplied to candidate binding at rendering.
    /// This snapshot does not renew eligibility or prove delivery.
    pub fn third_binding_at_render(&self) -> Option<Digest32> {
        self.binding_at_render
    }
    pub fn improved_stored_results(&self) -> usize {
        self.model_store.as_ref().map_or(0, |store| store.len())
    }
    pub fn selected_integer(&self, id: lc631_solvers::typed_query::IntSymbolId) -> Option<i128> {
        self.selected()
            .integers
            .iter()
            .find(|(actual, _)| *actual == id)
            .map(|(_, value)| *value)
    }
    pub fn selected_boolean(&self, id: lc631_solvers::typed_query::BoolSymbolId) -> Option<bool> {
        self.selected()
            .booleans
            .iter()
            .find(|(actual, _)| *actual == id)
            .map(|(_, value)| *value)
    }
    pub fn selected_assertions_checked(&self) -> usize {
        self.selected().assertions_checked
    }
    fn selected(&self) -> &ModelSnapshot {
        if self.state == ObjectiveStepState::ImprovedModelRechecked {
            &self.improvement
        } else {
            &self.initial
        }
    }
    pub(super) fn check_current(
        &self,
        evidence: &NativeEvidence<'_, '_>,
        current: &ResourceReservationView<'_>,
    ) -> Result<(), AdmissionError> {
        self.claim.check().map_err(AdmissionError::Resource)?;
        if !self.claim.same_reservation(current) {
            return Err(AdmissionError::Resource(ResourceError::ForeignOwner));
        }
        if self.source != evidence.request.source_revision()
            || self.context != evidence.request.context.revision()
            || self.goal_revision != evidence.request.context.scope().goal()
        {
            return Err(AdmissionError::ContextMismatch);
        }
        let FormalInput::Problem(problem) = evidence.request.input else {
            return Err(AdmissionError::ContextMismatch);
        };
        let goal = problem.goal().ok_or(AdmissionError::GoalConflict)?;
        if goal.symbol() != self.objective
            || goal.direction() != self.direction
            || goal.lower() != self.lower
            || goal.upper() != self.upper
        {
            return Err(AdmissionError::ContextMismatch);
        }
        if let Some(native) = evidence.native_model() {
            if self.initial.report != Some(native.observation().report())
                || self.initial.stdout != native.observation().stdout()
            {
                return Err(AdmissionError::ContextMismatch);
            }
        } else if self.state != ObjectiveStepState::MissingNativeModel {
            return Err(AdmissionError::ContextMismatch);
        }
        Ok(())
    }
}

pub(super) use produce::{admit, build};
