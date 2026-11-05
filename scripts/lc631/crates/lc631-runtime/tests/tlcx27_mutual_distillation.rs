use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::contextual_distillation::*;
use lc631_substrate::Digest32;

fn scope() -> ContextScope {
    ContextScope::declared(digest(1), digest(2), digest(3), digest(4))
}

fn digest(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}

fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of("fixed relation basis"))
}

fn edge(a: u8, b: u8) -> RelationEdit {
    RelationEdit {
        from: RelationNode(a),
        to: RelationNode(b),
    }
}

fn proposal(edit: RelationEdit) -> RelationProposal {
    RelationProposal {
        context: context(),
        scope: scope(),
        edit,
    }
}

fn engine(status: BasisAvailability) -> ContextualDistillation {
    let basis =
        ImmutableRelationBasis::checked(context(), scope(), 4, &[edge(0, 1), edge(1, 2)], status)
            .unwrap();
    ContextualDistillation::new(
        basis,
        DistillationLimits {
            max_epochs: 4,
            max_proposals: 8,
            max_relations: 16,
        },
    )
    .unwrap()
}

#[test]
fn tlcx27_negative_geometry_candidate_cannot_rewrite_oracle() {
    let mut run = engine(BasisAvailability::Given);
    let before = run.state_revision();
    let report = run.epoch(&[proposal(edge(2, 0))]).unwrap();
    assert_eq!(report.outcomes[0].status, RelationValidation::Refuted);
    assert_eq!(run.state_revision(), before);
    assert!(!run.relations().contains(&edge(2, 0)));
    assert_eq!(report.stop, EpochStop::NoProgress);
}

#[test]
fn tlcx27_negative_digest_only_and_membership_are_not_progress() {
    let mut run = engine(BasisAvailability::Given);
    let before = run.state_revision();
    let report = run.epoch(&[proposal(edge(0, 1))]).unwrap();
    assert_eq!(
        report.outcomes[0].status,
        RelationValidation::ExistingMembership
    );
    assert_eq!(report.added, 0);
    assert_eq!(report.before, report.after);
    assert_eq!(before, report.after);
    assert_eq!(report.stop, EpochStop::NoProgress);
}

#[test]
fn tlcx27_negative_unknown_is_retained_and_never_zero_or_success() {
    let mut run = engine(BasisAvailability::Unavailable);
    let report = run.epoch(&[proposal(edge(0, 2))]).unwrap();
    assert_eq!(report.outcomes[0].status, RelationValidation::NeedsEvidence);
    assert_eq!(report.residuals().count(), 1);
    assert_eq!(report.added, 0);
    assert_eq!(report.before, report.after);
}

#[test]
fn tlcx27_negative_scope_and_context_mismatch_stay_incomparable() {
    let mut run = engine(BasisAvailability::Given);
    let mut foreign = proposal(edge(0, 2));
    foreign.context = context().reconstructed([7; 32]);
    let mut changed_scope = proposal(edge(0, 2));
    changed_scope.scope = ContextScope::declared(digest(9), digest(2), digest(3), digest(4));
    let report = run.epoch(&[foreign, changed_scope]).unwrap();
    assert!(report
        .outcomes
        .iter()
        .all(|o| o.status == RelationValidation::Incomparable));
    assert_eq!(report.residuals().count(), 2);
    assert_eq!(report.before, report.after);
}

#[test]
fn tlcx27_same_relation_in_distinct_bindings_keeps_each_outcome() {
    let mut run = engine(BasisAvailability::Given);
    let valid = proposal(edge(0, 2));
    let mut foreign_context = valid;
    foreign_context.context = context().reconstructed([7; 32]);
    let mut foreign_scope = valid;
    foreign_scope.scope = ContextScope::declared(digest(9), digest(2), digest(3), digest(4));
    let report = run.epoch(&[foreign_context, foreign_scope, valid]).unwrap();
    assert_eq!(report.outcomes.len(), 3);
    assert_eq!(report.outcomes[0].status, RelationValidation::Incomparable);
    assert_eq!(report.outcomes[1].status, RelationValidation::Incomparable);
    assert_eq!(report.outcomes[2].status, RelationValidation::FiniteDerived);
    assert_eq!(report.residuals().count(), 2);
    assert_eq!(report.added, 1);
    assert_ne!(report.before, report.after);
    assert_eq!(report.next_query.unwrap().context, context());
}

#[test]
fn tlcx27_negative_invalid_duplicate_and_empty_batches_do_not_mutate() {
    let mut run = engine(BasisAvailability::Given);
    let before = run.state_revision();
    assert_eq!(run.epoch(&[]), Err(DistillationError::EmptyBatch));
    assert_eq!(
        run.epoch(&[proposal(edge(0, 2)); 2]),
        Err(DistillationError::DuplicateProposal)
    );
    assert_eq!(
        run.epoch(&[proposal(edge(0, 2)), proposal(edge(0, 9))]),
        Err(DistillationError::InvalidRelation)
    );
    assert_eq!(
        run.epoch(&[proposal(edge(0, 0))]),
        Err(DistillationError::InvalidRelation)
    );
    assert_eq!(run.state_revision(), before);
    assert_eq!(run.epochs_used(), 0);
}

#[test]
fn tlcx27_negative_cycles_and_overlarge_domains_are_rejected() {
    assert_eq!(
        ImmutableRelationBasis::checked(context(), scope(), 65, &[], BasisAvailability::Given)
            .unwrap_err(),
        DistillationError::InvalidLimits
    );
    assert_eq!(
        ImmutableRelationBasis::checked(
            context(),
            scope(),
            3,
            &[edge(0, 1), edge(1, 0)],
            BasisAvailability::Given
        )
        .unwrap_err(),
        DistillationError::CyclicBasis
    );
}

#[test]
fn tlcx27_negative_budget_failure_is_atomic() {
    let basis = ImmutableRelationBasis::checked(
        context(),
        scope(),
        4,
        &[edge(0, 1), edge(1, 2)],
        BasisAvailability::Given,
    )
    .unwrap();
    let mut run = ContextualDistillation::new(
        basis,
        DistillationLimits {
            max_epochs: 1,
            max_proposals: 1,
            max_relations: 2,
        },
    )
    .unwrap();
    let before = run.state_revision();
    assert_eq!(
        run.epoch(&[proposal(edge(0, 2))]),
        Err(DistillationError::RelationBudget)
    );
    assert_eq!(run.state_revision(), before);
    assert_eq!(run.epochs_used(), 0);
}

#[test]
fn tlcx27_new_relation_changes_content_and_next_query() {
    let mut run = engine(BasisAvailability::Given);
    let report = run
        .epoch(&[proposal(edge(0, 2)), proposal(edge(2, 3))])
        .unwrap();
    assert_eq!(report.outcomes[0].status, RelationValidation::FiniteDerived);
    assert_eq!(report.outcomes[1].status, RelationValidation::Refuted);
    assert_eq!(report.added, 1);
    assert_ne!(report.before, report.after);
    assert_eq!(report.residuals().count(), 1);
    let next = report.next_query.unwrap();
    assert_eq!(next.state_revision, report.after);
    assert_eq!(next.relation, edge(0, 2));
    assert_eq!(next.basis, ValidationBasis::GivenFiniteReachability);
    assert!(run.relations().contains(&edge(0, 2)));
    let repeated = run.epoch(&[proposal(edge(0, 2))]).unwrap();
    assert_eq!(repeated.stop, EpochStop::NoProgress);
    assert_eq!(repeated.before, repeated.after);
    assert!(repeated.next_query.is_none());
}

#[test]
fn tlcx27_validation_cut_prevents_relation_and_query() {
    let mut connected = engine(BasisAvailability::Given);
    let mut cut = engine(BasisAvailability::Unsupported);
    let candidate = [proposal(edge(0, 2))];
    assert!(connected.epoch(&candidate).unwrap().next_query.is_some());
    let report = cut.epoch(&candidate).unwrap();
    assert_eq!(report.outcomes[0].status, RelationValidation::Unsupported);
    assert!(report.next_query.is_none());
    assert_eq!(report.before, report.after);
}

#[test]
fn tlcx27_epoch_budget_is_explicit() {
    let basis = ImmutableRelationBasis::checked(
        context(),
        scope(),
        3,
        &[edge(0, 1), edge(1, 2)],
        BasisAvailability::Given,
    )
    .unwrap();
    let mut run = ContextualDistillation::new(
        basis,
        DistillationLimits {
            max_epochs: 1,
            max_proposals: 1,
            max_relations: 3,
        },
    )
    .unwrap();
    assert_eq!(
        run.epoch(&[proposal(edge(0, 2))]).unwrap().stop,
        EpochStop::EpochBudget
    );
    let revision = run.state_revision();
    assert_eq!(
        run.epoch(&[proposal(edge(0, 2))]),
        Err(DistillationError::EpochBudget)
    );
    assert_eq!(run.state_revision(), revision);
}

#[test]
fn tlcx27_independent_small_dag_oracle() {
    // A separate walk enumerates all forward DAGs over four vertices.
    let possible = [
        edge(0, 1),
        edge(0, 2),
        edge(0, 3),
        edge(1, 2),
        edge(1, 3),
        edge(2, 3),
    ];
    for mask in 0u8..64 {
        let edges: Vec<_> = possible
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, e)| *e)
            .collect();
        let basis = ImmutableRelationBasis::checked(
            context(),
            scope(),
            4,
            &edges,
            BasisAvailability::Given,
        )
        .unwrap();
        for a in 0..4 {
            for b in 0..4 {
                if a == b {
                    continue;
                }
                let mut stack = vec![a];
                let mut seen = [false; 4];
                while let Some(n) = stack.pop() {
                    for e in &edges {
                        if e.from.0 == n && !seen[usize::from(e.to.0)] {
                            seen[usize::from(e.to.0)] = true;
                            stack.push(e.to.0);
                        }
                    }
                }
                let outcome = basis.validate(proposal(edge(a, b)));
                assert_eq!(
                    matches!(
                        outcome,
                        RelationValidation::ExistingMembership | RelationValidation::FiniteDerived
                    ),
                    seen[usize::from(b)]
                );
            }
        }
    }
}
