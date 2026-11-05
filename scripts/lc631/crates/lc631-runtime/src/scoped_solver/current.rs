//! Current-use admission is separate from retaining a historical observation.
//! A successful read is still a solver report, never a Grant or proof. Checks
//! are point-in-time admission, not an atomic cancellation/revocation fence.

use super::{ScopedLookupError, ScopedSolverStore};
use crate::solver_lowering::PreparedAbstractSatisfiabilityQuery;
use lc631_core::context::{ContextScope, ContextState};
use lc631_solvers::{
    result_store::StoredSolverResult,
    runner::{InvocationDigest, QuerySourceIdentity},
};
use lc631_substrate::resource::{ResourceError, ResourceReservationView};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScopedCurrentLookupError {
    Resource(ResourceError),
    UnaccountedStore,
    Lookup(ScopedLookupError),
}

impl ScopedSolverStore<'_, '_> {
    /// Check the exact owner-backed lease before looking up the same prepared
    /// object. Cancellation makes this operation unavailable without erasing
    /// the historical report exposed by `lookup`.
    pub fn lookup_current(
        &self,
        prepared: &PreparedAbstractSatisfiabilityQuery<'_>,
        context: &ContextState,
        goal_scope: Option<ContextScope>,
        invocation: Option<InvocationDigest>,
        reservation: &ResourceReservationView<'_>,
    ) -> Result<&StoredSolverResult, ScopedCurrentLookupError> {
        self.check_current_owner(prepared, reservation)?;
        self.lookup(prepared, context, goal_scope, invocation)
            .map_err(ScopedCurrentLookupError::Lookup)
    }

    /// Select a local matching occurrence without treating its append ordinal
    /// as cross-owner, durable, or globally unique execution identity.
    #[allow(clippy::too_many_arguments)]
    pub fn lookup_current_occurrence(
        &self,
        prepared: &PreparedAbstractSatisfiabilityQuery<'_>,
        context: &ContextState,
        goal_scope: Option<ContextScope>,
        invocation: Option<InvocationDigest>,
        occurrence: usize,
        reservation: &ResourceReservationView<'_>,
    ) -> Result<&StoredSolverResult, ScopedCurrentLookupError> {
        self.check_current_owner(prepared, reservation)?;
        self.lookup_occurrence(prepared, context, goal_scope, invocation, occurrence)
            .map_err(ScopedCurrentLookupError::Lookup)
    }

    fn check_current_owner(
        &self,
        prepared: &PreparedAbstractSatisfiabilityQuery<'_>,
        reservation: &ResourceReservationView<'_>,
    ) -> Result<(), ScopedCurrentLookupError> {
        let claim = self
            .metadata_claim
            .as_ref()
            .ok_or(ScopedCurrentLookupError::UnaccountedStore)?;
        if !claim.same_reservation(reservation) {
            return Err(ScopedCurrentLookupError::Resource(
                ResourceError::ForeignOwner,
            ));
        }
        claim.check().map_err(ScopedCurrentLookupError::Resource)?;
        reservation
            .check()
            .map_err(ScopedCurrentLookupError::Resource)?;
        let source = QuerySourceIdentity::from_bytes(*reservation.source_revision().as_bytes());
        if self.resource_source != Some(source) || source != prepared.binding().source_identity() {
            return Err(ScopedCurrentLookupError::Resource(
                ResourceError::SourceMismatch,
            ));
        }
        Ok(())
    }
}
