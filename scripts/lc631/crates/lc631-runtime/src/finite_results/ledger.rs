//! Store-produced lineage into the four-facet ledger, without truth promotion.
use super::{FiniteResultError, FiniteResultId, FiniteResultStore};
use crate::facet_ledger::{
    FacetLedger, FiniteObservationBinding, LedgerError, PropertyId, Revision, TransformId,
};
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::temporal::{FiniteTrace, TemporalEvaluation, TemporalQuery, Truth};
use sha2::{Digest, Sha256};

pub struct TemporalLedgerRequest<'a> {
    pub observation: FiniteResultId,
    pub query: &'a TemporalQuery,
    pub trace: &'a FiniteTrace,
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub transform: TransformId,
    pub property: PropertyId,
    pub required_evidence: Revision,
    pub next_action: Revision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FiniteLedgerError {
    Store(FiniteResultError),
    Ledger(LedgerError),
}

impl FiniteResultStore<'_, '_> {
    /// Build references from an actual stored evaluation, not caller-supplied
    /// status flags. The digest is content identity, not evidence authenticity.
    pub fn temporal_binding(
        &self,
        observation: FiniteResultId,
        query: &TemporalQuery,
        trace: &FiniteTrace,
        context: ContextRevision,
        scope: ContextScope,
    ) -> Result<FiniteObservationBinding, FiniteResultError> {
        let report = self.lookup_temporal(observation, query, trace, context, scope)?;
        Ok(FiniteObservationBinding {
            source: Revision(*context.source().digest().as_bytes()),
            context: Revision(*context.interpretation_digest().as_bytes()),
            query: Revision(*report.query_digest().as_bytes()),
            result: Revision(result_digest(report)),
        })
    }

    /// Preserve an actual finite observation as Unverified uncertainty. A
    /// model evaluation alone neither establishes source/target preservation
    /// nor creates a numerical defect, gain, authority or completion state.
    pub fn retain_temporal_observation(
        &self,
        request: TemporalLedgerRequest<'_>,
        ledger: &mut FacetLedger,
    ) -> Result<FiniteObservationBinding, FiniteLedgerError> {
        let binding = self
            .temporal_binding(
                request.observation,
                request.query,
                request.trace,
                request.context,
                request.scope,
            )
            .map_err(FiniteLedgerError::Store)?;
        let transform = ledger
            .transform(request.transform)
            .ok_or(FiniteLedgerError::Ledger(LedgerError::UnknownTransform))?;
        let scope = transform.scope;
        if scope.id.0 != *request.scope.comparison().as_bytes()
            || scope.root_goal.0 != *request.scope.goal().as_bytes()
            || scope.probes.0 != *request.scope.probes().as_bytes()
            || scope.policy.0 != *request.scope.semantics().as_bytes()
        {
            return Err(FiniteLedgerError::Ledger(LedgerError::ScopeMismatch));
        }
        ledger
            .retain_finite_observation(
                request.transform,
                binding,
                request.property,
                request.required_evidence,
                request.next_action,
            )
            .map_err(FiniteLedgerError::Ledger)?;
        Ok(binding)
    }
}

fn result_digest(report: &TemporalEvaluation) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.finite-temporal-observation.v1\0");
    for digest in [
        report.source().digest(),
        report.context().interpretation_digest(),
        report.query_digest(),
        report.trace_digest(),
        report.profile_digest(),
    ] {
        hash.update(digest.as_bytes());
    }
    hash.update([match report.truth() {
        Truth::False => 0,
        Truth::Unknown => 1,
        Truth::True => 2,
    }]);
    // The admitted profile caps this value at 1024*256 on every platform.
    hash.update((report.node_steps() as u64).to_le_bytes());
    hash.finalize().into()
}
