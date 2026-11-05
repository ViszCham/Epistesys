//! TEST-FIRST: current public producer, not a future missing-module target.
//! Source body/API/frozen A14/CF stay unchanged. Central actual RED is pending.
#![forbid(unsafe_code)]
#[path = "tlcx22_rebind_input_dependence_support/mod.rs"]
mod support;
use support::*;

#[test]
fn tlcx22_rebind_constant_zero_map_discards_candidate_rejected() {
    expect_missing_use(ZERO_CHANGE, 0);
}

#[test]
fn tlcx22_rebind_masked_parameter_and_zero_rejected() {
    // A parameter occurrence is insufficient: both literal outputs remain false.
    expect_missing_use(
        Case {
            map_back: Term::AndZero,
            ..ZERO_CHANGE
        },
        0,
    );
}

#[test]
fn tlcx22_rebind_other_semantically_constant_maps_rejected() {
    for map_back in [Term::One, Term::OrOne] {
        expect_missing_use(
            Case {
                baseline: Term::One,
                candidate: Term::Zero,
                map_back,
                root: Term::One,
            },
            0,
        );
    }
    expect_missing_use(
        Case {
            map_back: Term::Contradiction,
            ..ZERO_CHANGE
        },
        0,
    );
}

#[test]
fn tlcx22_rebind_constant_role_change_with_dependent_not_map_is_valid() {
    for (baseline, candidate, root) in [
        (Term::Zero, Term::One, Term::Zero),
        (Term::One, Term::Zero, Term::One),
    ] {
        let observed = execute(
            Case {
                baseline,
                candidate,
                map_back: Term::NotInput,
                root,
            },
            4096,
            0,
        )
        .unwrap();
        assert_eq!(observed.report.state, ComponentState::Measured(ratio(1, 1)));
        assert!(!observed.has_gaps);
        assert_eq!(observed.legacy_count, 3);
        assert_eq!(observed.computed.len(), 3);
        assert_eq!(observed.recommendation, None);
        assert_eq!(
            observed.recommendation_state,
            RecommendationState::NoGoalProfile
        );
    }
}

#[test]
fn tlcx22_rebind_equivalent_role_does_not_bypass_map_use_check() {
    for map_back in [Term::Zero, Term::AndZero] {
        expect_missing_use(
            Case {
                baseline: Term::Zero,
                candidate: Term::Zero,
                map_back,
                root: Term::Zero,
            },
            0,
        );
    }
}

#[test]
fn tlcx22_rebind_equivalent_constant_role_with_dependent_map_stays_zero() {
    for role in [Term::Zero, Term::One] {
        let observed = execute(
            Case {
                baseline: role,
                candidate: role,
                map_back: Term::Input,
                root: role,
            },
            4096,
            0,
        )
        .unwrap();
        assert_eq!(observed.report.state, ComponentState::Measured(ratio(0, 1)));
        assert!(!observed.has_gaps);
        assert_eq!(observed.legacy_count, 3);
        assert_eq!(observed.computed.len(), 3);
        assert_eq!(observed.recommendation, None);
    }
}

#[test]
fn tlcx22_rebind_dependence_only_budget_cut_retains_unknown() {
    let observed = execute(
        Case {
            map_back: Term::NotInput,
            ..ZERO_CHANGE
        },
        9,
        0,
    )
    .unwrap();
    // Existing checks cost6,9,9 AST visits and must all be Computed.
    // Proposed m(false) vs m(true) check costs12, so cap9 alone cuts use proof.
    assert_eq!(observed.legacy_count, 3);
    assert_eq!(observed.computed.len(), 3);
    assert_eq!(
        observed.computed.iter().map(|r| r.3).collect::<Vec<_>>(),
        vec![6, 9, 9]
    );
    for (kind, soundness, completeness, _) in &observed.computed {
        if *kind != r::CheckKind::RoleDifference {
            assert_eq!(*soundness, DirectionResult::Preserved);
            assert_eq!(*completeness, DirectionResult::Preserved);
        }
    }
    assert!(observed.has_gaps, "dependence-only exhaustion was omitted");
    assert!(
        matches!(
            observed.report.state,
            ComponentState::ResourceExhausted { .. } | ComponentState::Unknown(_)
        ),
        "unmeasured dependence became a numeric success: {:?}",
        observed.report.state
    );
    assert_eq!(observed.recommendation, None);
}

#[test]
fn tlcx22_rebind_dependence_fix_preserves_mapback_failure_priority() {
    // This existing hard failure must retain its cause even though map is constant.
    let result = execute(
        Case {
            baseline: Term::Input,
            candidate: Term::One,
            map_back: Term::Zero,
            root: Term::Input,
        },
        4096,
        0,
    );
    assert!(matches!(
        result,
        Err(r::Error::PrerequisiteFailed {
            check: r::CheckKind::MapBack,
            ..
        })
    ));
}

#[test]
fn tlcx22_rebind_input_dependence_symbol_bijection() {
    for symbol in [0, 17, 99] {
        expect_missing_use(
            Case {
                map_back: Term::AndZero,
                ..ZERO_CHANGE
            },
            symbol,
        );
        let observed = execute(
            Case {
                map_back: Term::NotInput,
                ..ZERO_CHANGE
            },
            4096,
            symbol,
        )
        .unwrap();
        assert_eq!(observed.report.state, ComponentState::Measured(ratio(1, 1)));
        assert_eq!(observed.legacy_count, 3);
    }
}
