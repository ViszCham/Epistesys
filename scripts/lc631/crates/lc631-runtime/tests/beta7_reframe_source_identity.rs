//! 元sourceのbyte/spanと実ownerを固定する有限root captureテスト。
//! Bounded root-capture tests retain source bytes/spans and actual owner identity.
//! No candidate evaluator, compiler evidence, authority or recommendation is made here.
#![forbid(unsafe_code)]
use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::{
    contextual_distillation::{
        BasisAvailability, ContextualDistillation, DistillationLimits, ImmutableRelationBasis,
        RelationEdit, RelationNode,
    },
    joint_selection::SelectionPins,
    reorganization_measurement::reframe as f,
    resource::*,
    structural_gain_measurement::{ClosedGraph, Edge, Vertex},
};
use lc631_substrate::{
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::{sync::Arc, time::Duration};

const DECLARATION: &str = "frame-v2:direct;name=base";
const SOURCE: &str = "frame-v2:direct;name=base\n元source is Given; no authority is minted.\n";
#[derive(Clone, Copy)]
enum Case {
    Original,
    FreshSource,
    ClonedSource,
    ForeignBase,
}
fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}

fn capture(case: Case) {
    let source = SourceStore::new(Arc::<str>::from(SOURCE), 2048).unwrap();
    let fresh = SourceStore::new(Arc::<str>::from(source.text()), 2048).unwrap();
    let cloned = source.clone();
    assert_eq!(source.id(), fresh.id());
    assert_eq!(source.id(), cloned.id());
    assert_eq!(source.text().as_ptr(), cloned.text().as_ptr());
    assert!(!std::ptr::eq(&source, &cloned));
    let saved_bytes = source.text().as_bytes().to_vec();
    let saved_id = source.id();
    let whole = SourceSpan::checked(&source, 0, source.text().len()).unwrap();
    let scope = ContextScope::declared(d(1), d(2), d(3), d(4));
    let original = OriginalContext::checked(
        SourceRevision::of(source.text()),
        scope,
        AuthorityRevision(7),
        vec![
            ContextPremise::from_source(
                d(10),
                &source,
                SourceSpan::checked(&source, 0, DECLARATION.len()).unwrap(),
                PremiseBasis::Given,
                true,
            )
            .unwrap(),
            ContextPremise::from_source(
                d(11),
                &source,
                SourceSpan::checked(&source, DECLARATION.len() + 1, source.text().len()).unwrap(),
                PremiseBasis::Given,
                false,
            )
            .unwrap(),
        ],
        ContextLimits {
            max_contexts: 2,
            max_premises: 2,
            max_edits: 1,
            max_statement_bytes: 2048,
        },
    )
    .unwrap();
    let context = original.state().revision();
    let edges = [Edge(Vertex(0), Vertex(1))];
    let saved_edges = edges;
    let base = ClosedGraph::checked(&source, context, 2, &edges).unwrap();
    let other_base = ClosedGraph::checked(&source, context, 2, &edges).unwrap();
    assert_eq!(base.digest(), other_base.digest());
    assert!(!std::ptr::eq(&base, &other_base));
    let saved_base = base.digest();
    let relation = [RelationEdit {
        from: RelationNode(0),
        to: RelationNode(1),
    }];
    let basis =
        ImmutableRelationBasis::checked(context, scope, 2, &relation, BasisAvailability::Given)
            .unwrap();
    let state = ContextualDistillation::new(
        basis,
        DistillationLimits {
            max_epochs: 1,
            max_proposals: 1,
            max_relations: 1,
        },
    )
    .unwrap();
    let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let mut run = broker
        .begin_run(
            94,
            source.id().digest(),
            cap,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let profiles = [
            f::FrameProfile::DirectEdgeV1,
            f::FrameProfile::CheckedPathMacroV1,
        ];
        let family = f::FrameFamily::register_exact(
            &base,
            &profiles,
            f::OracleId::ExpandOneOuterStepToBasePathV1,
            &view,
        )
        .unwrap();
        let frame = f::FrameSpec {
            profile: profiles[0],
            declaration: DECLARATION,
            macros: &[],
        };
        let goal = f::RootGoal::Reachability(edges[0]);
        let terms = [f::QueryText {
            query: f::ProbeId(0),
            text: "edge(0,1)",
        }];
        let perspectives = [f::PerspectiveQuery {
            id: f::ProbeId(0),
            edge: edges[0],
            weight: 1,
        }];
        let input = f::ExactRootInput {
            source: match case {
                Case::FreshSource => &fresh,
                Case::ClonedSource => &cloned,
                _ => &source,
            },
            original: &original,
            base: if matches!(case, Case::ForeignBase) {
                &other_base
            } else {
                &base
            },
            family: &family,
            state: &state,
            root_goal: &goal,
            frame_premise: d(10),
            baseline_frame: &frame,
            baseline_terms: &terms,
            perspectives: &perspectives,
            pins: SelectionPins {
                source: context.source(),
                scope,
                baseline: d(5),
                hard_invariants: d(6),
                policy: d(7),
                goal_profile: None,
                evidence_revision: d(9),
            },
            source_basis: f::SourceBasis::Given,
        };
        let result = f::ReframePlan::capture_exact(input, f::ExactLimits::finite_path_v2(), &view);
        match case {
            Case::Original => {
                let plan = result.unwrap();
                assert_eq!(plan.denominator(), 1);
                assert!(plan.work_spent() > 0);
                drop(plan);
            }
            Case::FreshSource | Case::ClonedSource => {
                assert!(matches!(result, Err(f::Error::ForeignSourceOwner)));
            }
            Case::ForeignBase => {
                assert!(matches!(
                    result,
                    Err(f::Error::NoComparable(f::Cause::ForeignPlan))
                ));
            }
        }
        assert_eq!(source.text().as_bytes(), saved_bytes.as_slice());
        assert_eq!(source.slice(whole).unwrap(), SOURCE);
        assert_eq!(source.id(), saved_id);
        assert_eq!(base.digest(), saved_base);
        assert_eq!(edges, saved_edges);
        assert_eq!(original.state().premises()[0].statement(), DECLARATION);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn beta7_reframe_original_source_plan_preserves_raw_source_and_base() {
    capture(Case::Original);
}
#[test]
fn beta7_reframe_same_bytes_distinct_source_owner_is_rejected() {
    capture(Case::FreshSource);
}
#[test]
fn beta7_reframe_shared_bytes_clone_source_owner_is_rejected() {
    capture(Case::ClonedSource);
}
#[test]
fn beta7_reframe_equal_payload_distinct_base_owner_is_rejected() {
    capture(Case::ForeignBase);
}
