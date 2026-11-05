//! 実file driverを所有し、実予約ticketと実戻り値をprivateに保持する。
//! Owns the actual file driver; pending tickets and actual return identities are private.
use super::run_binding::{digest_bytes, parse_digest, BudgetRequestSpec, CheckedBudgetRun};
use super::{
    BudgetError, BudgetPhase, BudgetSession, BudgetSummary, PersistedBudgetReservation,
    ReservationProposal,
};
use lc631_analysis::{
    dgcl_repair_application_payload_digest, dgcl_repository_root_digest, DgclFileEffectOwner,
    DgclFileRepairConfig, DgclFileRepairDriver, DgclRepairApplication, DgclRepairDriver,
    DgclRepairDriverError, DgclRepairRequest, DgclRepairValidation, DgclVerifiedRepairApplication,
    ToolProcessRunner,
};
use lc631_analysis::declared_build_capture::{
    CurrentSnapshotAuthority, SnapshotCaptureAttempt, SnapshotCaptureError,
};
use lc631_core::ExecutionPermit;
use lc631_substrate::Digest32;
use std::time::Duration;

pub(super) struct PendingFileApplication {
    ticket: PersistedBudgetReservation,
    spec: BudgetRequestSpec,
    identity: Digest32,
    preimage: Digest32,
}
impl PendingFileApplication {
    pub(super) fn into_parts(
        self,
    ) -> (
        PersistedBudgetReservation,
        BudgetRequestSpec,
        Digest32,
        Digest32,
    ) {
        (self.ticket, self.spec, self.identity, self.preimage)
    }
}

pub(super) fn actual_identity(a: &DgclRepairApplication) -> Result<Digest32, BudgetError> {
    let receipt = a
        .attestation
        .as_ref()
        .ok_or(BudgetError::BindingMismatch)?
        .receipt_digest();
    let bytes = format!(
        "{}\0{}\0{}\0{}",
        dgcl_repair_application_payload_digest(a),
        a.schema_version,
        a.applied,
        receipt
    );
    Ok(digest_bytes(
        b"Epistesys/TLCX/RepairBudget/ActualApplication/v2\0",
        bytes.as_bytes(),
    ))
}

/// No generic-driver or externally supplied pending-state constructor exists.
/// ```compile_fail
/// use lc631_runtime::contextual_resume::budget::BudgetedFileRepairDriver;
/// let _ = BudgetedFileRepairDriver::from_raw_report(true);
/// ```
pub struct BudgetedFileRepairDriver<'driver, 'budget, 'trust> {
    inner: DgclFileRepairDriver<'driver>,
    budget: BudgetSession<'budget>,
    run: CheckedBudgetRun<'trust>,
    pending: Option<PendingFileApplication>,
    current_snapshot: Digest32,
    last_epoch: u64,
    diagnostic: Option<BudgetError>,
    application_key: Digest32,
}
impl<'driver, 'budget, 'trust> BudgetedFileRepairDriver<'driver, 'budget, 'trust> {
    /// A concrete owner, never a generic driver/report-injection seam. The actual
    /// bounded runner and file-effect owner are required; no compatibility fallback.
    pub fn new(
        config: DgclFileRepairConfig<'driver>,
        runner: &'driver mut dyn ToolProcessRunner,
        effects: &'driver mut dyn DgclFileEffectOwner,
        budget: BudgetSession<'budget>,
        run: CheckedBudgetRun<'trust>,
    ) -> Result<Self, BudgetError> {
        let summary = budget.summary()?;
        if summary.binding != run.binding()
            || summary.caps != run.caps()
            || summary.phase != BudgetPhase::ReadyForReservation
            || parse_digest(&config.initial_snapshot.snapshot_digest)?
                != summary.binding.initial_snapshot
            || parse_digest(&config.pipeline.source_revision)? != summary.binding.source
            || parse_digest(&config.pipeline.completion_plan.plan_digest)? != run.plan()
            || parse_digest(
                &dgcl_repository_root_digest(config.root)
                    .map_err(|_| BudgetError::BindingMismatch)?,
            )? != summary.binding.root_target
        {
            return Err(BudgetError::BindingMismatch);
        }
        let initial = summary.binding.initial_snapshot;
        let application_key = parse_digest(config.verifier.key_fingerprint())?;
        let inner = DgclFileRepairDriver::new_with_runner(config, runner, Some(effects))
            .map_err(|_| BudgetError::BindingMismatch)?;
        Ok(Self {
            inner,
            budget,
            run,
            pending: None,
            current_snapshot: initial,
            last_epoch: 0,
            diagnostic: None,
            application_key,
        })
    }
    pub fn budget_summary(&self) -> Result<BudgetSummary, BudgetError> {
        self.budget.summary()
    }
    pub fn budget_diagnostic(&self) -> Option<&BudgetError> {
        self.diagnostic.as_ref()
    }
    /// Immutable concrete-driver identity only; does not renew a signed run.
    pub fn declared_program_digest(&self) -> Result<Digest32, SnapshotCaptureError> {
        self.inner.declared_program_digest()
    }
    /// Narrow read-only forwarding. Fresh Test authority belongs to the inner
    /// capture service. No apply, journal append, quota refund or validation runs.
    pub fn recapture_current_build<'res>(
        &mut self,
        expected_root: Digest32,
        expected_program: Digest32,
        expected_plan: Digest32,
        authority: &mut dyn CurrentSnapshotAuthority,
        resources: &'res lc631_substrate::resource::ResourceReservationView<'_>,
    ) -> SnapshotCaptureAttempt<'res> {
        if expected_root != self.run.binding().root_target {
            return SnapshotCaptureAttempt::failure(SnapshotCaptureError::RootMismatch);
        }
        self.inner.recapture_current_build(
            expected_root, expected_program, expected_plan, authority, resources,
        )
    }
    fn fail(&mut self, error: BudgetError) -> DgclRepairDriverError {
        self.diagnostic = Some(error);
        DgclRepairDriverError::Failed
    }
}

impl DgclRepairDriver for BudgetedFileRepairDriver<'_, '_, '_> {
    fn observation_epoch(&self) -> Option<u64> {
        self.inner.observation_epoch()
    }
    fn apply(
        &mut self,
        r: &DgclRepairRequest,
        permit: &ExecutionPermit,
        time: Duration,
        delta: u64,
    ) -> Result<DgclRepairApplication, DgclRepairDriverError> {
        if self.pending.is_some() {
            return Err(self.fail(BudgetError::ReconciliationRequired));
        }
        let preparation = (|| {
            let now = self
                .inner
                .observation_epoch()
                .ok_or(BudgetError::BindingMismatch)?;
            if now < self.last_epoch {
                return Err(BudgetError::BindingMismatch);
            }
            self.run.check_at(now)?;
            let spec = self.run.request(r)?;
            let summary = self.budget.summary()?;
            if r.attempt as u64
                != summary
                    .charged_attempts
                    .checked_add(1)
                    .ok_or(BudgetError::BudgetExhausted)?
                || spec.before_snapshot != self.current_snapshot
                || spec.reserved_delta_bytes > delta
                || time.is_zero()
            {
                return Err(BudgetError::BindingMismatch);
            }
            let p = ReservationProposal {
                reservation: spec.request_digest,
                candidate: parse_digest(&r.candidate_digest)?,
                before_snapshot: spec.before_snapshot,
                intended_after_snapshot: spec.intended_snapshot,
                authority_binding: parse_digest(permit.digest())?,
                intent_binding: self.run.digest(),
                delta_bytes: spec.reserved_delta_bytes,
            };
            let ticket = self.budget.reserve_before_apply(p)?;
            Ok((now, spec, ticket))
        })();
        let (now, spec, ticket) = match preparation {
            Ok(x) => x,
            Err(e) => return Err(self.fail(e)),
        };
        self.last_epoch = now;
        // Reservation is physically persisted BEFORE this concrete file driver call.
        // On any delegate failure the durable Pending state remains charged.
        let result = self.inner.apply(r, permit, time, spec.reserved_delta_bytes);
        let application = match result {
            Ok(a) => a,
            Err(e) => {
                self.diagnostic = Some(BudgetError::ReconciliationRequired);
                return Err(e);
            }
        };
        let capture = (|| {
            if parse_digest(&application.after_snapshot.snapshot_digest)? != spec.intended_snapshot
                || application.delta_bytes > spec.reserved_delta_bytes
            {
                return Err(BudgetError::BindingMismatch);
            }
            Ok(PendingFileApplication {
                ticket,
                spec,
                identity: actual_identity(&application)?,
                preimage: parse_digest(&r.target_digest_before)?,
            })
        })();
        self.pending = match capture {
            Ok(p) => Some(p),
            Err(e) => return Err(self.fail(e)),
        };
        Ok(application)
    }
    fn on_verified_application(
        &mut self,
        token: DgclVerifiedRepairApplication<'_>,
    ) -> Result<(), DgclRepairDriverError> {
        let pending = self
            .pending
            .take()
            .ok_or_else(|| self.fail(BudgetError::ReconciliationRequired))?;
        let now = self
            .inner
            .observation_epoch()
            .ok_or_else(|| self.fail(BudgetError::BindingMismatch))?;
        if now < self.last_epoch
            || now < token.checked_epoch()
            || parse_digest(token.validated_receipt().verifier_key_fingerprint())
                .map_err(|e| self.fail(e))?
                != self.application_key
        {
            return Err(self.fail(BudgetError::BindingMismatch));
        }
        let after = parse_digest(&token.application().after_snapshot.snapshot_digest)
            .map_err(|e| self.fail(e))?;
        self.budget
            .finish_concrete_application(pending, token, &self.run, now)
            .map_err(|e| self.fail(e))?;
        self.current_snapshot = after;
        self.last_epoch = now;
        Ok(())
    }
    fn revalidate(
        &mut self,
        r: &DgclRepairRequest,
        a: &DgclRepairApplication,
        time: Duration,
    ) -> Result<DgclRepairValidation, DgclRepairDriverError> {
        self.inner.revalidate(r, a, time)
    }
}
