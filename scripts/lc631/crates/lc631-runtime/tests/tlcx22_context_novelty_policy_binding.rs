//! TEST-FIRST only: historical policy binding, preflight, aggregate-cap interpretation.
//! No body fix or observed RED is claimed by adding these assertions.
use lc631_analysis::creativity_measurement::{ComponentMetric as M, *};
use lc631_core::{
    context::{
        ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis, SourceRevision,
    },
    AuthorityRevision,
};
use lc631_runtime::{
    context_novelty_measurement as n,
    creative_candidate_bridge::*,
    resource::{ResourceBroker, ResourceLimits},
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    resource::*,
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::{cell::Cell, sync::Arc, time::Duration};
#[path = "tlcx22_context_novelty_support/mod.rs"]
mod frozen_support;
use frozen_support::{assert_point, bridge_limits, d, limits, q, query, resource, var};
mod tlcx22_context_novelty_policy_support;
use tlcx22_context_novelty_policy_support::*;

#[test]
fn tlcx22_ctx_policy_n01_actual_role_map_changes_distance_and_historical_root_identity() {
    policy_native_fixture!(native, view, BoolExpr::And(vec![var(0), var(1)]), {
        let a = original_three(native.source, false, true);
        let b = original_three(native.source, true, false);
        // The Core text-profile intentionally forgets which identical statement
        // occurrence bears each role. It is not the complete measurement policy.
        assert_eq!(a.state().revision(), b.state().revision());
        let mut root_a = native.capture(a, limits(), &view).unwrap();
        let mut root_b = native.capture(b, limits(), &view).unwrap();
        let operator = Operator::Recombine {
            partner: d(6),
            connective: Connective::Or,
        };
        let candidate_a = root_a.generate_current(operator, &view).unwrap();
        let candidate_b = root_b.generate_current(operator, &view).unwrap();
        let expected_a = root_a.expected_inputs_current(&candidate_a, &view).unwrap();
        let expected_b = root_b.expected_inputs_current(&candidate_b, &view).unwrap();
        let owner_a = root_a.measure_current(expected_a, &view).unwrap();
        let owner_b = root_b.measure_current(expected_b, &view).unwrap();
        // Independent literal truth rows (p,q)=00,01,10,11; no production gold.
        assert_eq!(AND_ROWS, [false, false, false, true]);
        assert_eq!(P_ROWS, [false, false, true, true]);
        assert_eq!(Q_ROWS, [false, true, false, true]);
        assert_ne!(AND_ROWS, P_ROWS);
        assert_ne!(P_ROWS, Q_ROWS);
        // a mutable sets {And,q}->{p,q}: symmetric difference2, union3.
        // b mutable sets {And,p}->{p}: symmetric difference1, union2.
        owner_a
            .with_current(expected_a, &view, |c| {
                assert_point(c.report(M::ContextDistance).unwrap(), 2, 3);
                assert_eq!(c.dctx_union_weight(), &q(3, 1));
            })
            .unwrap();
        owner_b
            .with_current(expected_b, &view, |c| {
                assert_point(c.report(M::ContextDistance).unwrap(), 1, 2);
                assert_eq!(c.dctx_union_weight(), &q(2, 1));
            })
            .unwrap();
        // Preserve the separate current-occurrence protection, even if the
        // historical identity assertion below exposes a policy-binding alias.
        assert!(matches!(
            root_a.expected_inputs_current(&candidate_b, &view),
            Err(n::Error::ForeignOwner)
        ));
        let calls = Cell::new(0);
        assert!(matches!(
            owner_a.with_current(
                n::ExpectedInputs {
                    root: &root_b,
                    ..expected_a
                },
                &view,
                |_| calls.set(calls.get() + 1)
            ),
            Err(n::Error::ExpectedInputMismatch)
        ));
        assert_eq!(calls.get(), 0);
        assert_ne!(root_a.historical_digest(), root_b.historical_digest(),
            "root policy must bind the actual premise ID-to-basis/changeable/statement/span correspondence");
    });
}

#[test]
fn tlcx22_ctx_policy_n02_known_truth_payload_requirement_precedes_evaluation_work_cut() {
    policy_native_fixture!(native, view, BoolExpr::And(vec![var(0), var(1)]), {
        // Three fact prototypes * four root-global rows =12 known bytes. Even
        // one vector is4bytes, exceeding this declared1byte cap before reserve.
        assert_eq!(3usize.checked_mul(1usize.checked_shl(2).unwrap()), Some(12));
        for work_cap in [1, 600, 8_000_000] {
            let mut cap = limits();
            cap.max_payload_bytes = 1;
            cap.max_total_work = work_cap;
            assert!(matches!(native.capture(original_three(native.source, false, true), cap, &view),
                Err(n::Error::PayloadBudget)),
                "a statically known truth-payload violation must be preflighted before canonicalization/evaluation, work cap={work_cap}");
        }
    });
}

#[test]
fn tlcx22_ctx_policy_b01_source_plus_goal_aggregate_512_513_is_a_separate_bridge_limit() {
    // Contract interpretation/boundary observation only, not a severity claim:
    // the existing bridge counter aggregates all source and goal AST nodes.
    for total in [512usize, 513] {
        let target_nodes = total - 3; // two other source vars + one goal var.
        let mut children = vec![var(0), var(1)];
        children.extend((0..target_nodes - 3).map(|_| BoolExpr::Constant(true)));
        assert_eq!(1 + children.len() + 2 + 1, total);
        policy_native_fixture!(native, view, BoolExpr::And(children), {
            let admitted =
                native.capture(original_three(native.source, false, true), limits(), &view);
            if total == 512 {
                assert!(admitted.is_ok(), "exact aggregate cap512 should fit");
            } else {
                assert!(matches!(admitted, Err(n::Error::Creative(CreativeError::Budget))),
                    "source query512 fits its native shape cap but source+goal513 exceeds the existing bridge aggregate cap");
            }
        });
    }
}
