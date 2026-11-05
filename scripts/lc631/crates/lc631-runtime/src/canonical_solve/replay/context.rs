//! Fixed replay-context membership only. No parse, solver or Authority receipt.
use super::super::{SourceSpan, SourceStore};
use super::types::{ReplayError, ReplayProfile};
use lc631_core::{
    context::{ContextLimits, ContextPremise, OriginalContext, PremiseBasis, SourceRevision},
    AuthorityRevision,
};

/// Caller holds source/context metadata accounting before allocation. Reuses
/// the admitted SourceStore; no second source Arc, parse or native query.
pub(super) fn original_context(
    store: &SourceStore,
    profile: ReplayProfile,
) -> Result<OriginalContext, ReplayError> {
    let source = store.text();
    let span = SourceSpan::checked(store, 0, source.len()).map_err(|_| ReplayError::Binding)?;
    let premise = ContextPremise::from_source(
        SourceRevision::of("epistesys.cli.formal.v1\0whole-given-source").digest(),
        store,
        span,
        PremiseBasis::Given,
        false,
    )
    .map_err(|_| ReplayError::Binding)?;
    let mut premises = Vec::new();
    premises
        .try_reserve_exact(1)
        .map_err(|_| ReplayError::Allocation)?;
    premises.push(premise);
    OriginalContext::checked(
        SourceRevision::of(source),
        profile.scope(source)?,
        AuthorityRevision(0),
        premises,
        ContextLimits {
            max_contexts: 1,
            max_premises: 1,
            max_edits: 1,
            max_statement_bytes: 16384,
        },
    )
    .map_err(|_| ReplayError::Binding)
}
