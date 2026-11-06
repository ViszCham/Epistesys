//! Current capability observations affect one existing native dispatch ticket.
//! Owner checks are not an atomic revocation fence or a semantic/proof oracle.
#![forbid(unsafe_code)]

use super::{
    DispatchError, DispatchStep, NativeResult, NativeScheduler, NativeWork, NativeWorkSpec,
};
use crate::capability_measurement::{OwnedPriority, Plan, Registry};
use crate::resource::RunResources;
use lc631_solvers::registry::SolverRegistry;
use lc631_substrate::resource::ResourceReservationView;
use lc631_world::exploration_schedule::MAX_GROUPS;
use std::path::Path;

/// The priority retains its actual assessment/plan/registry owners. Detached
/// equal numbers cannot construct this join; the work/query stays immutable.
pub struct CapabilityNativeWork<'q, 'owner, 'input, 'plan, 'reg, 'view> {
    native: NativeWork<'q>,
    priority: &'owner OwnedPriority<'input, 'plan, 'reg, 'view>,
    plan: &'plan Plan<'reg, 'view>,
    registry: &'reg Registry<'view>,
}
impl<'q, 'owner, 'input, 'plan, 'reg, 'view>
    CapabilityNativeWork<'q, 'owner, 'input, 'plan, 'reg, 'view>
{
    pub fn checked(
        spec: NativeWorkSpec<'q>,
        priority: &'owner OwnedPriority<'input, 'plan, 'reg, 'view>,
        plan: &'plan Plan<'reg, 'view>,
        registry: &'reg Registry<'view>,
        view: &ResourceReservationView<'_>,
    ) -> Result<Self, DispatchError> {
        let work = Self {
            native: NativeWork::checked(spec)?,
            priority,
            plan,
            registry,
        };
        work.check(view)?;
        Ok(work)
    }

    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), DispatchError> {
        let actual = self
            .priority
            .current(self.plan, self.registry, view)
            .map_err(DispatchError::Capability)?;
        let context = self
            .registry
            .context_current(view)
            .map_err(DispatchError::Capability)?;
        if !std::ptr::eq(actual, self.native.spec.priority)
            || context != self.native.spec.context.revision()
            || self.registry.scope() != self.native.spec.binding.comparison_scope
        {
            return Err(DispatchError::CapabilityBinding);
        }
        Ok(())
    }
}

// These private traits are implemented only by this owner-bound module. Public
// callers cannot supply a trusted callback that skips current checks.
pub(super) trait CurrentProfile {
    fn plan(&self) -> &Plan<'_, '_>;
    fn registry(&self) -> &Registry<'_>;
}
impl CurrentProfile for CapabilityNativeWork<'_, '_, '_, '_, '_, '_> {
    fn plan(&self) -> &Plan<'_, '_> {
        self.plan
    }
    fn registry(&self) -> &Registry<'_> {
        self.registry
    }
}
pub(super) trait CurrentBatch {
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), DispatchError>;
}
struct Batch<'works, 'q, 'owner, 'input, 'plan, 'reg, 'view> {
    works: &'works [CapabilityNativeWork<'q, 'owner, 'input, 'plan, 'reg, 'view>],
}
impl CurrentBatch for Batch<'_, '_, '_, '_, '_, '_, '_> {
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), DispatchError> {
        for (index, work) in self.works.iter().enumerate() {
            work.check(view)?;
            for previous in &self.works[..index] {
                if work.native.canonical_key() == previous.native.canonical_key()
                    && (!work.native.same_payload(&previous.native)
                        || !work
                            .priority
                            .same_evidence_current(
                                work.plan,
                                work.registry,
                                previous.priority,
                                previous.plan,
                                previous.registry,
                                view,
                            )
                            .map_err(DispatchError::Capability)?)
                {
                    return Err(DispatchError::ConflictingWork);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityFreshness {
    CurrentAtReturn,
    ObservedStale,
}

/// Historical access never renews a reservation, owner or scheduling ticket.
/// A stale result retains the actual native report/value and both settlements.
pub struct CapabilityDispatch<T> {
    step: DispatchStep<T>,
    freshness: CapabilityFreshness,
}
impl<T> CapabilityDispatch<T> {
    pub fn historical(&self) -> &DispatchStep<T> {
        &self.step
    }
    pub fn freshness_at_return(&self) -> CapabilityFreshness {
        self.freshness
    }
}

impl<'view, 'lease> NativeScheduler<'view, 'lease> {
    /// Uses the existing scheduler exactly once. This limited capability-only
    /// uncertainty profile does not claim to combine every D/G/U measurement.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_capability_next<'q, 'owner, 'input, 'plan, 'reg, 'held, T, F>(
        &mut self,
        run: &RunResources,
        works: &'view [CapabilityNativeWork<'q, 'owner, 'input, 'plan, 'reg, 'held>],
        frontier: [u8; 32],
        now: u64,
        cancelled: bool,
        registry: &SolverRegistry,
        working_directory: &Path,
        consume: F,
    ) -> Result<CapabilityDispatch<T>, DispatchError>
    where
        'q: 'view,
        'owner: 'view,
        'input: 'view,
        'plan: 'view,
        'reg: 'view,
        'held: 'view,
        F: for<'call> FnOnce(NativeResult<'q, 'call>) -> T,
    {
        self.validate_dispatch_run(run)?;
        if works.len() > MAX_GROUPS {
            return Err(DispatchError::GroupLimit);
        }
        let batch = Batch { works };
        batch.check(self.metadata)?;
        if let Some(first) = works.first() {
            let basis = self.capability_basis.unwrap_or(first);
            for work in works {
                if !basis
                    .plan()
                    .same_profile_current(basis.registry(), work.plan, work.registry, self.metadata)
                    .map_err(DispatchError::Capability)?
                {
                    return Err(DispatchError::CapabilityBinding);
                }
            }
            // Borrow the immutable first accepted profile for this series.
            // Later batches cannot change it through equal caller scope labels.
            self.capability_basis = Some(basis);
        }
        let mut native = Vec::new();
        native
            .try_reserve_exact(works.len())
            .map_err(|_| DispatchError::AllocationFailed)?;
        native.extend(works.iter().map(|work| work.native.borrowed()));
        let step = self.dispatch_inner(
            run,
            &native,
            frontier,
            now,
            cancelled,
            registry,
            working_directory,
            Some(&batch),
            consume,
        )?;
        let freshness = if batch.check(self.metadata).is_ok() {
            CapabilityFreshness::CurrentAtReturn
        } else {
            CapabilityFreshness::ObservedStale
        };
        Ok(CapabilityDispatch { step, freshness })
    }
}
