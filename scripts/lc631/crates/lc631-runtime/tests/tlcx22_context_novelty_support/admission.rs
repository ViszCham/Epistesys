//! Negative root admission and actual extraction boundaries; no production body.
use super::*;
macro_rules! recapture {
    ($source:expr,$theory:expr,$goal:expr,$premises:expr,$original:expr,$atoms:expr,$facts:expr,$constructions:expr,$archive:expr,$limits:expr,$view:expr) => {
        n::RootPlan::capture_current(
            n::RootInput {
                source: $source,
                original: $original,
                source_query: $theory,
                premises: $premises,
                target: d(5),
                goal: GoalPolicy {
                    revision: d(2),
                    query: $goal,
                },
                bridge_limits: bridge_limits(),
            },
            n::Declaration {
                atoms: $atoms,
                facts: $facts,
                constructions: $constructions,
                archive: $archive,
                corpus_revision: d(70),
                oracle_revision: d(71),
                policy_revision: d(72),
                limits: $limits,
            },
            $view,
        )
    };
}
#[test]
fn tlcx22_ctx_n05_outside_fixed_fact_universe_is_not_dropped_or_new_scope() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (
            source,
            theory,
            goal,
            premises,
            atoms,
            facts,
            constructions,
            archive,
            _,
            _,
            _,
            view,
            _,
            _,
        ) = f;
        let mut root = recapture!(
            source,
            theory,
            goal,
            premises,
            original(source),
            atoms,
            &facts[..1],
            constructions,
            archive,
            limits(),
            view
        )
        .unwrap();
        let candidate = root
            .generate_current(
                Operator::Recombine {
                    partner: d(6),
                    connective: Connective::Or,
                },
                view,
            )
            .unwrap();
        let e = root.expected_inputs_current(&candidate, view).unwrap();
        assert!(matches!(
            root.measure_current(e, view),
            Err(n::Error::UniverseMismatch)
        ));
    });
}
#[test]
fn tlcx22_ctx_n06_outside_construction_universe_does_not_become_empty_features() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (
            source,
            theory,
            goal,
            premises,
            atoms,
            facts,
            constructions,
            archive,
            _,
            _,
            _,
            view,
            _,
            _,
        ) = f;
        let mut root = recapture!(
            source,
            theory,
            goal,
            premises,
            original(source),
            atoms,
            facts,
            &constructions[..2],
            archive,
            limits(),
            view
        )
        .unwrap();
        let candidate = root
            .generate_current(
                Operator::Recombine {
                    partner: d(6),
                    connective: Connective::Or,
                },
                view,
            )
            .unwrap();
        let e = root.expected_inputs_current(&candidate, view).unwrap();
        assert!(matches!(
            root.measure_current(e, view),
            Err(n::Error::UniverseMismatch)
        ));
    });
}
#[test]
fn tlcx22_ctx_n07_invalid_weights_duplicate_meanings_and_semantic_duplicate_features_reject() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (
            source,
            theory,
            goal,
            premises,
            atoms,
            facts,
            constructions,
            archive,
            _,
            _,
            _,
            view,
            _,
            _,
        ) = f;
        for weight in [q(0, 1), q(-1, 1)] {
            let invalid = [n::FactFeature {
                formula: facts[0].formula,
                weight: &weight,
            }];
            assert!(matches!(
                recapture!(
                    source,
                    theory,
                    goal,
                    premises,
                    original(source),
                    atoms,
                    &invalid,
                    constructions,
                    archive,
                    limits(),
                    view
                ),
                Err(n::Error::InvalidWeight)
            ));
        }
        let huge = Rational::from_integer(q(2, 1).numer().pow(1025));
        let oversized = [n::FactFeature {
            formula: facts[0].formula,
            weight: &huge,
        }];
        assert!(matches!(
            recapture!(
                source,
                theory,
                goal,
                premises,
                original(source),
                atoms,
                &oversized,
                constructions,
                archive,
                limits(),
                view
            ),
            Err(n::Error::Arithmetic(MeasurementError::NumericBudget))
        ));
        let one = q(1, 1);
        let equivalent = dm(0, 1);
        let duplicate = [
            n::FactFeature {
                formula: facts[0].formula,
                weight: &one,
            },
            n::FactFeature {
                formula: &equivalent,
                weight: &one,
            },
        ];
        assert!(matches!(
            recapture!(
                source,
                theory,
                goal,
                premises,
                original(source),
                atoms,
                &duplicate,
                constructions,
                archive,
                limits(),
                view
            ),
            Err(n::Error::DuplicateFeature)
        ));
        let mut bad = *atoms;
        bad[1].meaning = bad[0].meaning;
        assert!(matches!(
            recapture!(
                source,
                theory,
                goal,
                premises,
                original(source),
                &bad,
                facts,
                constructions,
                archive,
                limits(),
                view
            ),
            Err(n::Error::InvalidMap)
        ));
    });
}
#[test]
fn tlcx22_ctx_n20_commutative_construction_duplicates_cannot_double_feature_weight() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (source, theory, goal, premises, atoms, facts, _, archive, _, _, _, view, _, _) = f;
        let one = q(1, 1);
        let swapped = BoolExpr::And(vec![var(1), var(0)]);
        let duplicate = [
            n::ConstructionFeature {
                formula: &theory.assertions()[0],
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &swapped,
                weight: &one,
            },
        ];
        assert!(matches!(
            recapture!(
                source,
                theory,
                goal,
                premises,
                original(source),
                atoms,
                facts,
                &duplicate,
                archive,
                limits(),
                view
            ),
            Err(n::Error::DuplicateFeature)
        ));
    });
}
#[test]
fn tlcx22_ctx_n08_hard_target_is_checked_before_feature_scoring() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (
            source,
            theory,
            goal,
            premises,
            atoms,
            facts,
            constructions,
            archive,
            _,
            _,
            _,
            view,
            _,
            _,
        ) = f;
        let context = OriginalContext::checked(
            SourceRevision::of(source.text()),
            ContextScope::declared(d(1), d(2), d(3), d(4)),
            AuthorityRevision(7),
            vec![
                ContextPremise::from_source(
                    d(5),
                    source,
                    SourceSpan::checked(source, 0, 7).unwrap(),
                    PremiseBasis::Given,
                    false,
                )
                .unwrap(),
                ContextPremise::from_source(
                    d(6),
                    source,
                    SourceSpan::checked(source, 8, 9).unwrap(),
                    PremiseBasis::Given,
                    false,
                )
                .unwrap(),
            ],
            ContextLimits {
                max_contexts: 17,
                max_premises: 2,
                max_edits: 1,
                max_statement_bytes: 8192,
            },
        )
        .unwrap();
        assert!(matches!(
            recapture!(
                source,
                theory,
                goal,
                premises,
                context,
                atoms,
                facts,
                constructions,
                archive,
                limits(),
                view
            ),
            Err(n::Error::HardInvariant)
        ));
    });
}
#[test]
fn tlcx22_ctx_n09_equal_ast_copy_is_not_the_root_native_premise_allocation() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (
            source,
            theory,
            goal,
            premises,
            atoms,
            facts,
            constructions,
            archive,
            _,
            _,
            _,
            view,
            _,
            _,
        ) = f;
        let copied = theory.clone();
        assert!(matches!(
            recapture!(
                source,
                &copied,
                goal,
                premises,
                original(source),
                atoms,
                facts,
                constructions,
                archive,
                limits(),
                view
            ),
            Err(n::Error::ExpectedInputMismatch)
        ));
    });
}
#[test]
fn tlcx22_ctx_n10_integer_comparison_cannot_be_labeled_same_signature_zero() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (source, _, goal, _, atoms, facts, constructions, archive, _, _, _, view, _, _) = f;
        let integer = query(
            0,
            1,
            vec![
                BoolExpr::Equal(IntExpr::Constant(0), IntExpr::Constant(0)),
                var(0),
            ],
        );
        let premises = [
            GivenPremise {
                id: d(5),
                span: SourceSpan::checked(source, 0, 7).unwrap(),
                formula: &integer.assertions()[0],
            },
            GivenPremise {
                id: d(6),
                span: SourceSpan::checked(source, 8, 9).unwrap(),
                formula: &integer.assertions()[1],
            },
        ];
        assert!(matches!(
            recapture!(
                source,
                &integer,
                goal,
                &premises,
                original(source),
                atoms,
                facts,
                constructions,
                archive,
                limits(),
                view
            ),
            Err(n::Error::UnsupportedSignature)
        ));
    });
}
#[test]
fn tlcx22_ctx_n11_fixed_weight_union_is_not_candidate_reweighted() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (
            source,
            theory,
            goal,
            premises,
            atoms,
            facts,
            constructions,
            archive,
            _,
            _,
            _,
            view,
            _,
            _,
        ) = f;
        let three = q(3, 1);
        let one = q(1, 1);
        let weighted = [
            n::FactFeature {
                formula: facts[0].formula,
                weight: &one,
            },
            n::FactFeature {
                formula: facts[1].formula,
                weight: &three,
            },
        ];
        let mut root = recapture!(
            source,
            theory,
            goal,
            premises,
            original(source),
            atoms,
            &weighted,
            constructions,
            archive,
            limits(),
            view
        )
        .unwrap();
        let candidate = root
            .generate_current(
                Operator::Recombine {
                    partner: d(6),
                    connective: Connective::Or,
                },
                view,
            )
            .unwrap();
        let e = root.expected_inputs_current(&candidate, view).unwrap();
        let owner = root.measure_current(e, view).unwrap();
        owner
            .with_current(e, view, |c| {
                assert_point(c.report(M::ContextDistance).unwrap(), 1, 1);
                assert_eq!(c.dctx_union_weight(), &q(4, 1));
            })
            .unwrap();
    });
}
