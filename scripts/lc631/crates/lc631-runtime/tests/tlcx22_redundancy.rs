//! 日本語: 実公開runtime APIの負例契約。未登録/未実装APIはcompilePendingでありsemantic REDではない。
//! English: Negative contracts for the real public runtime API. Missing registration/methods are
//! compilePending, never an observed behavioral RED. No test-local producer or report injection.
mod tlcx22_redundancy_support;
use lc631_core::CandidateId;
use lc631_runtime::{
    formal_boolean::{Failure, NoComparable, UnconstrainedPolicy},
    redundancy_measurement::*,
    resource::*,
};
use lc631_solvers::typed_query::{BoolExpr, IntExpr};
use tlcx22_redundancy_support::*;
use SemanticRole::{Conclusion, Premise};

#[test]
fn tlcx22_redundancy_global_meanings_are_not_formula_local_and_gamma_is_fixed() {
    fixture(|source, context, view, _, _| {
        let expressions = [variable(0), variable(1)];
        let us = units(&expressions);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise, Premise]);
        let ms = meanings(source, 2);
        let am = map(&[0, 1]);
        let empty = theory(&[0, 1], vec![]);
        let iff = theory(
            &[0, 1],
            vec![BoolExpr::Or(vec![
                BoolExpr::And(vec![variable(0), variable(1)]),
                BoolExpr::And(vec![not(variable(0)), not(variable(1))]),
            ])],
        );
        // Hand tuples p=(0,1,0,1), q=(0,0,1,1): equal density is not equivalence.
        let em = model(source, context, &empty, &am);
        let im = model(source, context, &iff, &am);
        let ei = root_input(&art, &specs, &ms, &em);
        let ii = root_input(&art, &specs, &ms, &im);
        let er = RootRedundancyPlan::capture_closed(&ei, PROFILE_LIMITS, view).unwrap();
        let ir = RootRedundancyPlan::capture_closed(&ii, PROFILE_LIMITS, view).unwrap();
        let eb = BoundCandidateUnits::checked(&er, CandidateId(1), Some(&art), &[], view).unwrap();
        let ib = BoundCandidateUnits::checked(&ir, CandidateId(1), Some(&art), &[], view).unwrap();
        let ep = PairPlan::prepare(&er, &eb, view).unwrap();
        let ip = PairPlan::prepare(&ir, &ib, view).unwrap();
        let eo = OwnedRedundancy::produce(&ep, view).unwrap();
        let io = OwnedRedundancy::produce(&ip, view).unwrap();
        assert_measured(eo.state_current(view).unwrap(), 0, 1);
        assert_measured(io.state_current(view).unwrap(), 1, 2);
        // Identical caller scope/source/map/baseline labels cannot hide different actual Γ.
        assert!(matches!(
            compare_current(&eb, &eo, &ib, &io, view),
            Err(Error::ScopeMismatch) | Err(Error::BindingMismatch)
        ));
    });
}

#[test]
fn tlcx22_redundancy_counts_excess_occurrences_and_keeps_semantic_roles_separate() {
    fixture(|source, context, view, _, _| {
        let gamma = theory(&[0, 1], vec![]);
        let am = map(&[0, 1]);
        let ms = meanings(source, 2);
        let m = model(source, context, &gamma, &am);
        let cases = [
            (
                vec![variable(0), not(not(variable(0)))],
                vec![Premise; 2],
                1,
                2,
            ),
            (vec![variable(0); 3], vec![Premise; 3], 2, 3),
            (
                vec![variable(0), not(not(variable(0))), variable(1)],
                vec![Premise; 3],
                1,
                3,
            ),
            (
                vec![variable(0), variable(0), variable(0), variable(1)],
                vec![Premise, Premise, Premise, Conclusion],
                1,
                2,
            ),
            (
                vec![variable(0), variable(0)],
                vec![Premise, Conclusion],
                0,
                1,
            ),
        ];
        for (expressions, roles, n, d) in cases {
            let us = units(&expressions);
            let art = artifact(source, context, &us);
            let specs = slots(&roles);
            let input = root_input(&art, &specs, &ms, &m);
            assert_measured(
                &actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view).unwrap(),
                n,
                d,
            );
        }
    });
}

#[test]
fn tlcx22_redundancy_all_two_atom_functions_match_independent_four_row_bits() {
    fixture(|source, context, view, _, _| {
        let gamma = theory(&[0, 1], vec![]);
        let am = map(&[0, 1]);
        let ms = meanings(source, 2);
        let m = model(source, context, &gamma, &am);
        let specs = slots(&[Premise; 2]);
        let mut checked = 0;
        for left in 0u8..16 {
            for right in 0u8..16 {
                let expressions = [function(left, [0, 1]), function(right, [0, 1])];
                let us = units(&expressions);
                let art = artifact(source, context, &us);
                let input = root_input(&art, &specs, &ms, &m);
                assert_measured(
                    &actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view).unwrap(),
                    i64::from(left == right),
                    2,
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 256);
    });
}

#[test]
fn tlcx22_redundancy_valid_empty_missing_artifact_and_zero_atom_constants_are_distinct() {
    fixture(|source, context, view, _, _| {
        let gamma = theory(&[], vec![]);
        let m = model(source, context, &gamma, &[]);
        let empty = artifact(source, context, &[]);
        let input = root_input(&empty, &[], &[], &m);
        assert_measured(
            &actual_state(&input, PROFILE_LIMITS, Some(&empty), &[], view).unwrap(),
            0,
            1,
        );
        assert_eq!(
            actual_state(&input, PROFILE_LIMITS, None, &[], view).unwrap(),
            RedundancyState::Unknown(Gap::UnextractedArtifact)
        );
        let expressions = [BoolExpr::Constant(true), BoolExpr::Constant(true)];
        let us = units(&expressions);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise; 2]);
        let input = root_input(&art, &specs, &[], &m);
        assert_measured(
            &actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view).unwrap(),
            1,
            2,
        );
    });
}

#[test]
fn tlcx22_redundancy_false_formula_is_observed_while_unextracted_unit_is_unknown() {
    fixture(|source, context, view, _, _| {
        let gamma = theory(&[], vec![]);
        let m = model(source, context, &gamma, &[]);
        let expressions = [BoolExpr::Constant(false), BoolExpr::Constant(false)];
        let us = units(&expressions);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise; 2]);
        let input = root_input(&art, &specs, &[], &m);
        assert_measured(
            &actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view).unwrap(),
            1,
            2,
        );
        let missing = [
            us[0],
            UnitInput {
                slot: SlotId(1),
                formula: None,
            },
        ];
        let unknown = artifact(source, context, &missing);
        assert_eq!(
            actual_state(&input, PROFILE_LIMITS, Some(&unknown), &[], view).unwrap(),
            RedundancyState::Unknown(Gap::UnextractedUnit(SlotId(1)))
        );
    });
}

#[test]
fn tlcx22_redundancy_candidate_cannot_invent_duplicate_or_drop_root_slots() {
    fixture(|source, context, view, _, _| {
        let gamma = theory(&[0], vec![]);
        let am = map(&[0]);
        let ms = meanings(source, 1);
        let m = model(source, context, &gamma, &am);
        let expressions = [variable(0), variable(0)];
        let us = units(&expressions);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise; 2]);
        let input = root_input(&art, &specs, &ms, &m);
        let root = RootRedundancyPlan::capture_closed(&input, PROFILE_LIMITS, view).unwrap();
        let duplicate = [us[0], us[0]];
        let extra = [
            us[0],
            us[1],
            UnitInput {
                slot: SlotId(99),
                formula: Some(&expressions[0]),
            },
        ];
        for (bad, expected) in [
            (&us[..1], Error::MissingSlot),
            (&[][..], Error::MissingSlot),
            (&duplicate[..], Error::DuplicateSlot),
            (&extra[..], Error::ForeignSlot),
        ] {
            let candidate = artifact(source, context, bad);
            assert!(
                matches!(BoundCandidateUnits::checked(&root, CandidateId(2), Some(&candidate),
                &[], view), Err(error) if error == expected)
            );
        }
        // UnitInput has no role or denominator field: only root assigns roles.
    });
}

#[test]
fn tlcx22_redundancy_protected_payload_rewrite_and_missing_or_forged_delta_are_rejected() {
    fixture(|source, context, view, _, _| {
        let gamma = theory(&[0, 1], vec![]);
        let am = map(&[0, 1]);
        let ms = meanings(source, 2);
        let m = model(source, context, &gamma, &am);
        let p = variable(0);
        let q = variable(1);
        let nn = not(not(variable(0)));
        let us = [
            UnitInput {
                slot: SlotId(0),
                formula: Some(&p),
            },
            UnitInput {
                slot: SlotId(1),
                formula: Some(&p),
            },
        ];
        let baseline = artifact(source, context, &us);
        let mut specs = slots(&[Premise; 2]);
        specs[0].protection = Protection::HardUnchanged;
        let input = root_input(&baseline, &specs, &ms, &m);
        let root = RootRedundancyPlan::capture_closed(&input, PROFILE_LIMITS, view).unwrap();
        let rewritten = [
            UnitInput {
                slot: SlotId(0),
                formula: Some(&nn),
            },
            us[1],
        ];
        let bad = artifact(source, context, &rewritten);
        assert!(matches!(
            BoundCandidateUnits::checked(&root, CandidateId(2), Some(&bad), &[], view),
            Err(Error::UnsupportedProtectedRewrite)
        ));
        let changed = [
            us[0],
            UnitInput {
                slot: SlotId(1),
                formula: Some(&q),
            },
        ];
        let candidate = artifact(source, context, &changed);
        assert!(matches!(
            BoundCandidateUnits::checked(&root, CandidateId(2), Some(&candidate), &[], view),
            Err(Error::MissingDelta)
        ));
        let forged = [MutableChangeInput {
            slot: SlotId(1),
            before: &q,
            after: &q,
        }];
        assert!(matches!(
            BoundCandidateUnits::checked(&root, CandidateId(2), Some(&candidate), &forged, view),
            Err(Error::InvalidDelta)
        ));
    });
}

#[test]
fn tlcx22_redundancy_atom_namespace_requires_complete_global_bijection() {
    fixture(|source, context, view, _, _| {
        let gamma = theory(&[0, 1], vec![]);
        let ms = meanings(source, 2);
        let expressions = [variable(0), variable(1)];
        let us = units(&expressions);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise; 2]);
        let mut duplicate_meaning = map(&[0, 1]);
        duplicate_meaning[1].meaning = RootAtom(0);
        for am in [
            vec![map(&[0])[0]],
            vec![map(&[0, 1])[0]; 2],
            duplicate_meaning,
        ] {
            let m = model(source, context, &gamma, &am);
            let input = root_input(&art, &specs, &ms, &m);
            assert!(matches!(
                RootRedundancyPlan::capture_closed(&input, PROFILE_LIMITS, view),
                Err(Error::InvalidAtomMap)
            ));
        }
    });
}

#[test]
fn tlcx22_redundancy_consistent_global_rename_slot_relabel_and_input_order_preserve_value() {
    fixture(|source, context, view, _, _| {
        let ms = meanings(source, 2);
        for ids in [[0, 1], [90, 7]] {
            let gamma = theory(&ids, vec![]);
            let mut am = map(&ids);
            am.reverse();
            let m = model(source, context, &gamma, &am);
            let expressions = [
                variable(ids[0]),
                not(not(variable(ids[0]))),
                variable(ids[1]),
            ];
            let labels = [SlotId(41), SlotId(8), SlotId(99)];
            let mut us: Vec<_> = expressions
                .iter()
                .zip(labels)
                .map(|(formula, slot)| UnitInput {
                    slot,
                    formula: Some(formula),
                })
                .collect();
            let mut specs: Vec<_> = labels
                .iter()
                .map(|&slot| SlotSpec {
                    slot,
                    role: Premise,
                    protection: Protection::Mutable,
                })
                .collect();
            us.reverse();
            specs.rotate_left(1);
            let art = artifact(source, context, &us);
            let input = root_input(&art, &specs, &ms, &m);
            assert_measured(
                &actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view).unwrap(),
                1,
                3,
            );
        }
    });
}

#[test]
fn tlcx22_redundancy_inconsistent_missing_scope_and_unsupported_are_not_zero() {
    fixture(|source, context, view, _, _| {
        let am = map(&[0]);
        let ms = meanings(source, 1);
        let expressions = [variable(0), variable(0)];
        let us = units(&expressions);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise; 2]);
        let inconsistent = theory(&[0], vec![BoolExpr::Constant(false)]);
        let m = model(source, context, &inconsistent, &am);
        let input = root_input(&art, &specs, &ms, &m);
        assert!(matches!(
            actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view).unwrap(),
            RedundancyState::NoComparable(NoComparable::SourceContextInconsistent)
        ));
        let empty = theory(&[0], vec![]);
        let mut m = model(source, context, &empty, &am);
        m.unconstrained = UnconstrainedPolicy::MissingScope;
        let input = root_input(&art, &specs, &ms, &m);
        assert!(matches!(
            actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view).unwrap(),
            RedundancyState::NoComparable(NoComparable::MissingTheoryScope)
        ));
        let integer = [BoolExpr::LessThan(
            IntExpr::Constant(0),
            IntExpr::Constant(1),
        )];
        let us = units(&integer);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise]);
        let m = model(source, context, &empty, &am);
        let input = root_input(&art, &specs, &ms, &m);
        assert!(matches!(
            actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view),
            Err(Error::Formal(Failure::UnsupportedInteger))
                | Ok(RedundancyState::Unsupported(Failure::UnsupportedInteger))
        ));
    });
}

#[test]
fn tlcx22_redundancy_all_pairs_complete_before_exact_count_and_metadata_stays_reported() {
    exercise_all_pairs_and_metadata();
}

#[test]
fn tlcx22_redundancy_per_call_aggregate_and_invalid_profile_caps_are_distinct() {
    fixture(|source, context, view, _, _| {
        let gamma = theory(&[0, 1], vec![]);
        let am = map(&[0, 1]);
        let ms = meanings(source, 2);
        let m = model(source, context, &gamma, &am);
        let expressions = [variable(0), variable(1)];
        let us = units(&expressions);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise; 2]);
        let input = root_input(&art, &specs, &ms, &m);
        let call = Limits {
            max_call_work: 1,
            ..PROFILE_LIMITS
        };
        assert!(matches!(
            actual_state(&input, call, Some(&art), &[], view),
            Err(Error::PerCallBudget)
        ));
        let total = Limits {
            max_total_work: 1,
            ..PROFILE_LIMITS
        };
        assert!(matches!(
            actual_state(&input, total, Some(&art), &[], view),
            Err(Error::TotalWorkBudget)
        ));
        // Each 64-node predicate pair needs 128*(1+256)=32896 AST visits.
        // Per-call map work still fits 1m; 120 pairs' AST work alone exceeds 3.9m.
        let gamma8 = theory(&[0, 1, 2, 3, 4, 5, 6, 7], vec![]);
        let am8 = map(&[0, 1, 2, 3, 4, 5, 6, 7]);
        let ms8 = meanings(source, 8);
        let m8 = model(source, context, &gamma8, &am8);
        let many = vec![BoolExpr::And(vec![variable(0); 63]); 16];
        let mu = units(&many);
        let ma = artifact(source, context, &mu);
        let specs = slots(&[Premise; 16]);
        let mi = root_input(&ma, &specs, &ms8, &m8);
        let aggregate = Limits {
            max_total_work: 1_000_000,
            ..PROFILE_LIMITS
        };
        assert!(matches!(
            actual_state(&mi, aggregate, Some(&ma), &[], view),
            Err(Error::TotalWorkBudget)
        ));
        let invalid = Limits {
            max_total_work: PROFILE_LIMITS.max_total_work + 1,
            ..PROFILE_LIMITS
        };
        assert!(matches!(
            actual_state(&input, invalid, Some(&art), &[], view),
            Err(Error::InvalidLimits)
        ));
    });
    fixture_ram(1, |source, context, view, _, _| {
        let gamma = theory(&[], vec![]);
        let m = model(source, context, &gamma, &[]);
        let art = artifact(source, context, &[]);
        let input = root_input(&art, &[], &[], &m);
        assert!(matches!(
            actual_state(&input, PROFILE_LIMITS, Some(&art), &[], view),
            Err(Error::Resource(ResourceError::Exhausted(ResourceKind::Ram)))
        ));
    });
}

#[test]
fn tlcx22_redundancy_same_role_mutable_delta_changes_actual_cost_consumer_not_whole_meaning() {
    exercise_mutable_cost_consumer();
}

#[test]
fn tlcx22_redundancy_equal_scalar_does_not_authorize_foreign_native_binding_or_cancelled_reads() {
    exercise_current_binding_and_cancel();
}

#[test]
fn tlcx22_redundancy_fresh_pair_plans_cannot_refresh_root_run_work_budget() {
    exercise_root_budget_reuse();
}
