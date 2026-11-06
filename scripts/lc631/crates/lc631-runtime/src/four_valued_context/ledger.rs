//! 日本語: 前提変更Delta/Uと実評価Uを別transformで同じ台帳へ原子的に追記する。
//! English: Atomically append separate context Delta/U and native-evaluation U transforms.
use super::*;
use crate::facet_ledger::{
    ComparisonScope, Delta, Direction, EditKind, Entry, EvidenceRef, Facet, FacetLedger, Gap,
    GapReason, PropertyId, Revision, ScopeId, TransformBinding, TransformId,
};
use lc631_core::context::PremiseEdit;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordedBranch {
    context: Option<TransformId>,
    result: TransformId,
}
impl RecordedBranch {
    pub const fn context_transform(self) -> Option<TransformId> {
        self.context
    }
    pub const fn result_transform(self) -> TransformId {
        self.result
    }
}

impl BranchObservation<'_, '_, '_, '_> {
    /// Content identities and declared edit provenance only. No receipt issuer,
    /// permission, measured loss/gain, source truth or completion is produced.
    /// `admit_batch` preflights every event before modifying the SAME ledger.
    /// Current checks observe the lease, not an atomic revocation fence.
    pub fn record(
        &self,
        ledger: &mut FacetLedger,
        view: &ResourceReservationView<'_>,
    ) -> Result<RecordedBranch, BranchError> {
        let plan = self.input.plan;
        self.report_current(plan.graph, plan.context(), plan.query, view)?;
        let actual = self
            .store
            .binding(self.id, self.input.request())
            .map_err(BranchError::Observation)?;
        let scope = comparison(plan);
        let producer = hash(
            b"epistesys.context-four-valued.native-producer.v1\0",
            &[
                PROFILE.as_bytes(),
                lc631_solvers::four_valued::PROFILE_REVISION.as_bytes(),
            ],
        );
        let result_id = TransformId(hash(
            b"epistesys.context-four-valued.result-transform.v1\0",
            &[
                &actual.source.0,
                &actual.context.0,
                &actual.query.0,
                &actual.result.0,
                &producer,
                &scope.id.0,
                &scope.baseline.0,
                &scope.root_goal.0,
                &scope.probes.0,
                &scope.policy.0,
            ],
        ));
        let native = TransformBinding {
            id: result_id,
            source: actual.source,
            target: actual.result,
            source_context: actual.context,
            target_context: actual.context,
            direction: Direction::Forward,
            scope,
            producer: Revision(producer),
            query: actual.query,
            result: actual.result,
        };
        let edits = plan.state.declared_delta().map_or(0, |d| d.edits().len());
        let max_entries = edits
            .checked_add(2)
            .ok_or(BranchError::ArithmeticOverflow)?;
        let bytes = max_entries
            .checked_mul(std::mem::size_of::<Entry>())
            .ok_or(BranchError::ArithmeticOverflow)?;
        let held = claim(view, bytes)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(max_entries)
            .map_err(|_| BranchError::AllocationFailed)?;
        if entries.capacity() > max_entries {
            return Err(BranchError::AllocationFailed);
        }
        let mut transforms = [native; 2];
        let context = if let Some(delta) = plan.state.declared_delta() {
            let parent = plan
                .graph
                .get(delta.parent())
                .ok_or(BranchError::UnknownParent)?;
            let transition = context_transform(plan, parent, scope);
            context_entries(plan, transition.id, &mut entries)?;
            transforms[0] = transition;
            Some(transition.id)
        } else {
            None
        };
        entries.push(Entry {
            transform: result_id,
            facet: Facet::Uncertainty(Gap {
                property: PropertyId(hash(
                    b"epistesys.context-four-valued.source-unverified.v1\0",
                    &[],
                )),
                reason: GapReason::Unverified,
                required_evidence: Revision(hash(
                    b"epistesys.context-four-valued.source-evidence-needed.v1\0",
                    &[],
                )),
                next_action: actual.query,
            }),
        });
        held.check().map_err(BranchError::Resource)?;
        self.report_current(plan.graph, plan.context(), plan.query, view)?;
        let count = if context.is_some() { 2 } else { 1 };
        ledger
            .admit_batch(&transforms[..count], &entries)
            .map_err(BranchError::Ledger)?;
        Ok(RecordedBranch {
            context,
            result: result_id,
        })
    }
}

fn comparison(plan: &BranchPlan<'_, '_>) -> ComparisonScope {
    let scope = plan.state.scope();
    let original = plan.graph.original().state().revision();
    ComparisonScope {
        id: ScopeId(*scope.comparison().as_bytes()),
        baseline: Revision(hash(
            b"epistesys.context-four-valued.original-baseline.v1\0",
            &[
                original.source().digest().as_bytes(),
                original.interpretation_digest().as_bytes(),
                PROFILE.as_bytes(),
            ],
        )),
        root_goal: Revision(*scope.goal().as_bytes()),
        probes: Revision(*scope.probes().as_bytes()),
        policy: Revision(*scope.semantics().as_bytes()),
    }
}
fn context_transform(
    plan: &BranchPlan<'_, '_>,
    parent: &ContextState,
    scope: ComparisonScope,
) -> TransformBinding {
    let before = *parent.revision().interpretation_digest().as_bytes();
    let after = *plan.state.revision().interpretation_digest().as_bytes();
    let producer = hash(
        b"epistesys.context-four-valued.declared-context-producer.v1\0",
        &[PROFILE.as_bytes()],
    );
    let id = TransformId(hash(
        b"epistesys.context-four-valued.context-transform.v1\0",
        &[
            parent.revision().source().digest().as_bytes(),
            &before,
            &after,
            &plan.query_digest,
            &producer,
            &scope.id.0,
            &scope.baseline.0,
            &scope.root_goal.0,
            &scope.probes.0,
            &scope.policy.0,
        ],
    ));
    // Immutable source bytes remain identical; context interpretations change.
    // This shape composes with the native source→result edge via check_path.
    TransformBinding {
        id,
        source: Revision(*parent.revision().source().digest().as_bytes()),
        target: Revision(*plan.state.revision().source().digest().as_bytes()),
        source_context: Revision(before),
        target_context: Revision(after),
        direction: Direction::Forward,
        scope,
        producer: Revision(producer),
        query: Revision(plan.query_digest),
        result: Revision(after),
    }
}
fn context_entries(
    plan: &BranchPlan<'_, '_>,
    transform: TransformId,
    entries: &mut Vec<Entry>,
) -> Result<(), BranchError> {
    let delta = plan
        .state
        .declared_delta()
        .ok_or(BranchError::UnknownParent)?;
    let parent = plan
        .graph
        .get(delta.parent())
        .ok_or(BranchError::UnknownParent)?;
    let before = Revision(*parent.revision().interpretation_digest().as_bytes());
    let after = Revision(*plan.state.revision().interpretation_digest().as_bytes());
    // These are explicitly DECLARED/MISSING evidence references, never receipts.
    let declared = EvidenceRef(hash(
        b"epistesys.context-four-valued.declared-edit.v1\0",
        &[&before.0, &after.0],
    ));
    let permission = EvidenceRef(hash(
        b"epistesys.context-four-valued.declared-authority-pin.v1\0",
        &[&plan.graph.original().authority().0.to_le_bytes()],
    ));
    let missing_map = EvidenceRef(hash(
        b"epistesys.context-four-valued.map-back-unverified.v1\0",
        &[],
    ));
    for edit in delta.edits() {
        let (property, kind) = match edit {
            PremiseEdit::Add(p) => (p.id(), EditKind::AddPremise),
            PremiseEdit::Replace { id, .. } => (*id, EditKind::Substitution),
            PremiseEdit::Retract { id } => (*id, EditKind::RetractPremise),
        };
        entries.push(Entry {
            transform,
            facet: Facet::Delta(Delta {
                property: PropertyId(*property.as_bytes()),
                before,
                after,
                kind,
                reason: declared,
                permission_basis: permission,
                map_back: missing_map,
            }),
        });
    }
    entries.push(Entry {
        transform,
        facet: Facet::Uncertainty(Gap {
            property: PropertyId(hash(
                b"epistesys.context-four-valued.context-evidence-unverified.v1\0",
                &[],
            )),
            reason: GapReason::Unverified,
            required_evidence: Revision(hash(
                b"epistesys.context-four-valued.edit-mapback-evidence-needed.v1\0",
                &[],
            )),
            next_action: Revision(plan.query_digest),
        }),
    });
    Ok(())
}
fn hash(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}
