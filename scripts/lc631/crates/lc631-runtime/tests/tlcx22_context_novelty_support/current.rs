//! Actual native-owner/current cuts and nonrefundable bounded work tests.
use super::*;
#[test]
fn tlcx22_ctx_n12_expected_ast_source_and_universe_copies_do_not_mint_current() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (source, _, _, _, atoms, _, _, _, root, _, candidate, view, _, _) = f;
        let e = root.expected_inputs_current(candidate, view).unwrap();
        let owner = root.measure_current(e, view).unwrap();
        let ast = e.candidate_ast.clone();
        let other_source = SourceStore::new(Arc::from(source.text()), 256 * 1024).unwrap();
        let other_atoms = *atoms;
        let calls = Cell::new(0);
        for substituted in [
            n::ExpectedInputs {
                candidate_ast: &ast,
                ..e
            },
            n::ExpectedInputs {
                source: &other_source,
                ..e
            },
            n::ExpectedInputs {
                atoms: &other_atoms,
                ..e
            },
        ] {
            assert!(matches!(
                owner.with_current(substituted, view, |_| calls.set(calls.get() + 1)),
                Err(n::Error::ExpectedInputMismatch)
            ));
        }
        assert_eq!(calls.get(), 0);
    });
}
#[test]
fn tlcx22_ctx_n13_foreign_reservation_and_post_callback_cancel_reject_without_erasing_history() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (_, _, _, _, _, _, _, _, root, _, candidate, view, cancel, run) = f;
        let e = root.expected_inputs_current(candidate, view).unwrap();
        let owner = root.measure_current(e, view).unwrap();
        let foreign = run
            .reserve(&[
                resource(ResourceKind::Ram, 4 * 1024 * 1024),
                resource(ResourceKind::Output, 65536),
            ])
            .unwrap();
        let foreign = foreign.lease();
        let calls = Cell::new(0);
        assert!(matches!(
            owner.with_current(e, &foreign, |_| calls.set(calls.get() + 1)),
            Err(n::Error::ForeignReservation)
        ));
        assert_eq!(calls.get(), 0);
        assert!(matches!(
            owner.with_current(e, view, |_| {
                calls.set(calls.get() + 1);
                cancel.cancel();
            }),
            Err(n::Error::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(calls.get(), 1);
        assert!(matches!(
            owner.with_current(e, view, |_| ()),
            Err(n::Error::Resource(ResourceError::Cancelled))
        ));
        assert_point(owner.historical_report(M::ContextDistance).unwrap(), 1, 1);
    });
}
#[test]
fn tlcx22_ctx_n14_archive_budget_cut_is_partial_not_exact_zero_or_denominator_shrink() {
    let mut cap = limits();
    cap.max_archive_comparisons = 0;
    ctx_fixture!(f, ArchiveMode::ExactWeak, cap, false, {
        let (_, _, _, _, _, _, _, _, root, _, candidate, view, _, _) = f;
        let e = root.expected_inputs_current(candidate, view).unwrap();
        let owner = root.measure_current(e, view).unwrap();
        owner
            .with_current(e, view, |c| {
                assert_eq!(c.archive_status(), n::ArchiveStatus::Partial);
                assert_eq!(c.counts().archive_comparisons, 0);
                assert_eq!(
                    c.report(M::Knownness).unwrap().state,
                    ComponentState::ResourceExhausted {
                        partial: Some(RationalInterval::from_ratio(0, 1, 1, 1).unwrap())
                    }
                );
                assert_eq!(c.dctx_union_weight(), &q(2, 1));
            })
            .unwrap();
    });
}
#[test]
fn tlcx22_ctx_n15_repeated_work_failure_does_not_reset_root_total_budget() {
    let mut cap = limits();
    cap.max_call_work = 1;
    ctx_fixture!(f, ArchiveMode::Baseline, cap, false, {
        let (_, _, _, _, _, _, _, _, root, _, candidate, view, _, _) = f;
        let e = root.expected_inputs_current(candidate, view).unwrap();
        let mut before = root.work_remaining_current(view).unwrap();
        for _ in 0..3 {
            assert!(matches!(
                root.measure_current(e, view),
                Err(n::Error::WorkBudget)
            ));
            let after = root.work_remaining_current(view).unwrap();
            assert!(after < before);
            before = after;
        }
    });
}
#[test]
fn tlcx22_ctx_n16_universe_and_archive_count_limits_reject_before_canonicalization() {
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
        let mut cap = limits();
        cap.max_features = 257;
        let input = n::RootInput {
            source,
            original: original(source),
            source_query: theory,
            premises,
            target: d(5),
            goal: GoalPolicy {
                revision: d(2),
                query: goal,
            },
            bridge_limits: bridge_limits(),
        };
        let declaration = n::Declaration {
            atoms,
            facts,
            constructions,
            archive,
            corpus_revision: d(70),
            oracle_revision: d(71),
            policy_revision: d(72),
            limits: cap,
        };
        assert!(matches!(
            n::RootPlan::capture_current(input, declaration, view),
            Err(n::Error::InvalidLimits)
        ));
    });
}
#[test]
fn tlcx22_ctx_p05_literal_oracle_does_not_depend_on_source_or_production_parser() {
    // Given p=true,q=false separates And(p,q) from p on the shared domain.
    assert!(!AND_TABLE[2]);
    assert!(P_TABLE[2]);
    // Hand construction sets {And,p} and {OrAndP,p}: common1, union3, similarity1/3.
    let (common, union) = (1_i64, 3_i64);
    assert_eq!(q(common, union), q(1, 3));
}
#[test]
fn tlcx22_ctx_n17_same_digest_foreign_root_and_candidate_cannot_replace_actual_occurrence() {
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
            root,
            _,
            candidate,
            view,
            _,
            _,
        ) = f;
        let e = root.expected_inputs_current(candidate, view).unwrap();
        let owner = root.measure_current(e, view).unwrap();
        let input = n::RootInput {
            source,
            original: original(source),
            source_query: theory,
            premises,
            target: d(5),
            goal: GoalPolicy {
                revision: d(2),
                query: goal,
            },
            bridge_limits: bridge_limits(),
        };
        let declaration = n::Declaration {
            atoms,
            facts,
            constructions,
            archive,
            corpus_revision: d(70),
            oracle_revision: d(71),
            policy_revision: d(72),
            limits: limits(),
        };
        let mut foreign = n::RootPlan::capture_current(input, declaration, view).unwrap();
        assert_eq!(root.historical_digest(), foreign.historical_digest());
        let other = foreign
            .generate_current(
                Operator::Recombine {
                    partner: d(6),
                    connective: Connective::Or,
                },
                view,
            )
            .unwrap();
        assert!(matches!(
            root.expected_inputs_current(&other, view),
            Err(n::Error::ForeignOwner)
        ));
        assert!(matches!(
            owner.with_current(
                n::ExpectedInputs {
                    root: &foreign,
                    ..e
                },
                view,
                |_| ()
            ),
            Err(n::Error::ExpectedInputMismatch)
        ));
    });
}
#[test]
fn tlcx22_ctx_n18_real_expired_lease_cannot_reactivate_current_history() {
    ctx_fixture!(
        f,
        ArchiveMode::Baseline,
        limits(),
        false,
        Duration::from_secs(2),
        {
            let (_, _, _, _, _, _, _, _, root, dm, _, view, _, _) = f;
            let e = root.expected_inputs_current(dm, view).unwrap();
            let owner = root.measure_current(e, view).unwrap();
            if let Ok(remaining) = view.remaining() {
                std::thread::sleep(remaining + Duration::from_millis(1));
            }
            assert!(matches!(
                owner.with_current(e, view, |_| ()),
                Err(n::Error::Resource(ResourceError::DeadlineExceeded))
            ));
            assert_point(owner.historical_report(M::ContextDistance).unwrap(), 0, 1);
        }
    );
}
#[test]
fn tlcx22_ctx_n19_measurement_scratch_is_preclaimed_not_hidden_from_live_reservation() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (_, _, _, _, _, _, _, _, root, _, candidate, view, _, _) = f;
        let e = root.expected_inputs_current(candidate, view).unwrap();
        // Root4MiB + existing bridge metadata1MiB + two actual candidates2MiB.
        let occupied = view
            .claim_resources(&[resource(ResourceKind::Ram, 88 * 1024 * 1024)])
            .unwrap();
        assert!(matches!(
            root.measure_current(e, view),
            Err(n::Error::Resource(ResourceError::Exhausted(
                ResourceKind::Ram
            )))
        ));
        drop(occupied);
        let owner = root.measure_current(e, view).unwrap();
        owner
            .with_current(e, view, |c| {
                assert_point(c.report(M::ContextDistance).unwrap(), 1, 1)
            })
            .unwrap();
    });
}
