//! Public CPU contract for the selection batch that may later be submitted to a backend.
//! An error from finish_selection is too late: these negatives inspect preparation itself.
#[path = "tlcx23_formal_support/mod.rs"]
mod support;

use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    measurement_numeric::RationalInterval,
    CandidateId,
};
use lc631_runtime::{
    joint_selection::{
        select_joint_reported, AxisKind, AxisValue, JointCandidate, ReportedGate, SearchCoverage,
        SelectionError, SelectionPins, SelectionPolicy,
    },
    semantic_accelerator::selection::{
        finish_selection, prepare_numeric_batch, prepare_numeric_batch_with_profile, ConsumerError,
        EpochStamp, NumericPayloadProfile, ParityBasis,
    },
};
use lc631_substrate::resource::{Cancellation, ResourceError, ResourceKind};
use support::{broker, d, reserve, run};

const NORMAL_RAM: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
enum Change {
    None,
    ZeroCandidateLimit,
    ExceededCandidateLimit,
    ZeroKeyBudget,
    ExceededKeyBudget,
    DuplicateAxis,
    UndeclaredGoalSolveOverlap,
    EmptyKey,
    NonpointExact,
    OutOfRangeAxis,
    OutOfRangeTotal,
    SignedGain,
}

#[derive(Clone, Copy)]
enum Expected {
    Rejected(SelectionError),
    RamDenied,
    CpuFrontier,
}

fn check(order: &[usize], change: Change, ram: u64, expected: Expected) {
    let source = SourceRevision::of("beta7 accelerator preflight");
    let context = ContextRevision::original(source);
    let pins = SelectionPins {
        source,
        scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
        baseline: d(5),
        hard_invariants: d(6),
        policy: d(7),
        goal_profile: Some(d(8)),
        evidence_revision: d(9),
    };
    let goal = AxisKind::Goal(d(10));
    let axes = match change {
        Change::DuplicateAxis => [goal, goal],
        Change::UndeclaredGoalSolveOverlap => [goal, AxisKind::SolveGain],
        Change::SignedGain => [goal, AxisKind::CoverageGain],
        _ => [goal, AxisKind::SemanticLoss],
    };
    let policy = SelectionPolicy {
        pins,
        axes: &axes,
        representative_order: Some(order),
        declared_goal_solve_overlap: false,
        max_candidates: match change {
            Change::ZeroCandidateLimit => 0,
            Change::ExceededCandidateLimit => 1,
            _ => 2,
        },
        max_canonical_bytes: match change {
            Change::ZeroKeyBudget => 0,
            Change::ExceededKeyBudget => 1,
            _ => 16,
        },
    };
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    let one = RationalInterval::exact_ratio(1, 1).unwrap();
    let two = RationalInterval::exact_ratio(2, 1).unwrap();
    let negative_one = RationalInterval::exact_ratio(-1, 1).unwrap();
    let uncertain = RationalInterval::from_ratio(0, 1, 1, 1).unwrap();
    let first = [
        AxisValue::Exact(&one),
        AxisValue::Exact(match change {
            Change::NonpointExact => &uncertain,
            Change::OutOfRangeAxis => &two,
            _ => &zero,
        }),
    ];
    let second = [
        AxisValue::Exact(&zero),
        AxisValue::Exact(if matches!(change, Change::SignedGain) {
            &negative_one
        } else {
            &one
        }),
    ];
    let rows = [
        JointCandidate {
            id: CandidateId(1),
            context,
            pins,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &first,
            total_loss: AxisValue::Exact(if matches!(change, Change::OutOfRangeTotal) {
                &two
            } else {
                &zero
            }),
            canonical_key: if matches!(change, Change::EmptyKey) {
                b""
            } else {
                b"a"
            },
            measurement_binding: d(11),
        },
        JointCandidate {
            id: CandidateId(2),
            context,
            pins,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &second,
            total_loss: AxisValue::Exact(&one),
            canonical_key: b"b",
            measurement_binding: d(12),
        },
    ];
    let coverage = SearchCoverage::EvaluatedSet;
    if let Expected::Rejected(error) = expected {
        // The independent hand-selected error is also the existing selector contract.
        assert_eq!(
            select_joint_reported(&policy, &rows, coverage).err(),
            Some(error)
        );
    }
    let broker = broker();
    let mut owner = run(&broker, source.digest(), Cancellation::default());
    let mut reservation = reserve(&owner, ram, 1024 * 1024);
    {
        let view = reservation.lease();
        let stamp = EpochStamp {
            epoch: 1,
            generation: 2,
            fence: 3,
        };
        for profile in [
            NumericPayloadProfile::LegacyV1,
            NumericPayloadProfile::TiledWorld2048V1,
        ] {
            let admission = match profile {
                NumericPayloadProfile::LegacyV1 => {
                    prepare_numeric_batch(&policy, &rows, coverage, stamp, &view, 8)
                }
                NumericPayloadProfile::TiledWorld2048V1 => {
                    prepare_numeric_batch_with_profile(
                        &policy, &rows, coverage, stamp, &view, 8, profile,
                    )
                }
            };
            match expected {
                Expected::Rejected(error) => assert_eq!(
                    admission.err(),
                    Some(ConsumerError::Selection(error)),
                    "{profile:?}: {change:?} order_len={} must fail at preparation; None means a batch was published",
                    order.len()
                ),
                Expected::RamDenied => assert_eq!(
                    admission.err(),
                    Some(ConsumerError::Resource(ResourceError::Exhausted(
                        ResourceKind::Ram
                    )))
                ),
                Expected::CpuFrontier => {
                    let batch = admission.unwrap();
                    let selection = finish_selection(&batch, None, &view).unwrap();
                    assert_eq!(selection.parity_basis(), ParityBasis::FreshCpuOnly);
                    let report = selection.project_current(&view).unwrap();
                    assert_eq!(report.frontier, [0]);
                    assert_eq!(report.recommendation, Some(0));
                    assert_eq!(report.alternatives, [1]);
                }
            }
        }
    }
    reservation.release_confirmed().unwrap();
    owner.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    assert_eq!(broker.used(ResourceKind::Output).unwrap(), 0);
}

fn invalid_order(order: &[usize], ram: u64) {
    check(
        order,
        Change::None,
        ram,
        Expected::Rejected(SelectionError::InvalidRepresentativeOrder),
    );
}

#[test]
fn oversized_order_is_rejected_at_preparation() {
    invalid_order(&vec![0; 65_537], NORMAL_RAM);
}

#[test]
fn duplicate_order_is_rejected_at_preparation() {
    invalid_order(&[1, 1], NORMAL_RAM);
}

#[test]
fn out_of_range_order_is_rejected_at_preparation() {
    invalid_order(&[0, 2], NORMAL_RAM);
}

#[test]
fn too_short_order_is_rejected_at_preparation() {
    invalid_order(&[0], NORMAL_RAM);
}

#[test]
fn malformed_order_is_rejected_before_scratch_admission() {
    // A valid policy under the identical one-byte lease cannot enter token construction.
    check(&[1, 0], Change::None, 1, Expected::RamDenied);
    invalid_order(&vec![0; 65_537], 1);
}

#[test]
fn invalid_policy_or_key_is_rejected_at_preparation() {
    for (change, error) in [
        (Change::ZeroCandidateLimit, SelectionError::InvalidLimits),
        (Change::ExceededCandidateLimit, SelectionError::ResourceExhausted),
        (Change::ZeroKeyBudget, SelectionError::InvalidLimits),
        (Change::ExceededKeyBudget, SelectionError::ResourceExhausted),
        (Change::DuplicateAxis, SelectionError::InvalidAxes),
        (Change::UndeclaredGoalSolveOverlap, SelectionError::InvalidAxes),
        (Change::EmptyKey, SelectionError::MissingCanonicalKey),
    ] {
        check(&[1, 0], change, NORMAL_RAM, Expected::Rejected(error));
    }
}

#[test]
fn invalid_value_is_rejected_at_preparation() {
    for (change, error) in [
        (Change::NonpointExact, SelectionError::InexactExactLabel),
        (Change::OutOfRangeAxis, SelectionError::OutOfRange),
        (Change::OutOfRangeTotal, SelectionError::OutOfRange),
    ] {
        check(&[1, 0], change, NORMAL_RAM, Expected::Rejected(error));
    }
}

#[test]
fn valid_permutation_preserves_the_exact_cpu_selection() {
    check(&[1, 0], Change::None, NORMAL_RAM, Expected::CpuFrontier);
    check(&[1, 0], Change::SignedGain, NORMAL_RAM, Expected::CpuFrontier);
}
