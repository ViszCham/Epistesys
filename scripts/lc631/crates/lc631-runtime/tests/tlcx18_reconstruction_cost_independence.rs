//! 日本語: 許可されたcost/feature独立性の負例。既存profileの過去成功を書換えない。
//! English: Test-first cost/feature independence refinement; retain historical profile claims.
//! Caller-declared finite inputs, target-only decoding and fixed denominators.
//! A budget-limited Symbol extraction is Unknown, never a confirmed mismatch.
use lc631_analysis::creativity_measurement::{
    ComponentState, MeasurementGap, Rational, RationalInterval,
};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::description_measurement::{ReconstructionInput, RootPlan};
use lc631_runtime::joint_selection::AxisValue;
use lc631_runtime::reconstruction_measurement::{
    self as reconstruction, FeatureKind, Gap, ProbeSpec, ProtectedFeatureSet,
};
use lc631_runtime::resource::*;
use lc631_runtime::{description_measurement as description, facet_ledger as ledger};
use std::time::Duration;

const SOURCE_SYMBOL: &[u8] = &[3, 1, b'a'];
const SOURCE_BOOL: &[u8] = &[1, 1];
const SCALE_BITS: u64 = 16;

fn scope() -> ledger::ComparisonScope {
    ledger::ComparisonScope {
        id: ledger::ScopeId([18; 32]),
        baseline: ledger::Revision([2; 32]),
        root_goal: ledger::Revision([3; 32]),
        probes: ledger::Revision([4; 32]),
        policy: ledger::Revision([5; 32]),
    }
}
fn kind_probe() -> [ProbeSpec; 1] {
    [ProbeSpec {
        path: vec![],
        kind: FeatureKind::NodeKind,
        weight: Rational::from_integer(1.into()),
    }]
}
fn mixed_probes() -> [ProbeSpec; 2] {
    [
        ProbeSpec {
            path: vec![],
            kind: FeatureKind::NodeKind,
            weight: Rational::from_integer(1.into()),
        },
        ProbeSpec {
            path: vec![],
            kind: FeatureKind::Symbol,
            weight: Rational::from_integer(3.into()),
        },
    ]
}
fn fixture(body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>, &Cancellation)) {
    let source = SourceRevision::of("Given finite cost/feature independence refinement");
    let cap = ResourceLimits::uniform(96 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            18,
            source.digest(),
            cap,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(80 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(24 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        body(ContextRevision::original(source), &view, &cancel);
    }
    reservation.release_confirmed().unwrap();
}

// Independent literal canonical Symbol encodings at the feature and cost caps.
// No production encoder creates these test vectors or their expected bit count.
fn symbol(length: usize) -> Vec<u8> {
    let mut bytes = match length {
        4096 => vec![3, 0x80, 0x20],
        4097 => vec![3, 0x81, 0x20],
        5000 => vec![3, 0x88, 0x27],
        65532 => vec![3, 0xfc, 0xff, 3],
        _ => panic!("not an independently specified fixture length"),
    };
    bytes.resize(bytes.len() + length, b'a');
    bytes
}
fn uleb_bytes(mut value: usize) -> usize {
    let mut count = 1;
    while value >= 128 {
        value >>= 7;
        count += 1;
    }
    count
}
fn bundle_bits(semantic_wire_bytes: usize) -> u64 {
    let decoder = include_bytes!("../src/reconstruction_measurement/decoder.rs").len();
    // Apply(Symbol("bundle-v1"), [semantic, Symbol(decoder text), Bool(false)]):
    // fixed prefix13B, actual semantic wire, decoder tag/ULEB/text, dictionary2B.
    (8 * (13 + semantic_wire_bytes + 1 + uleb_bytes(decoder) + decoder + 2)) as u64
}

#[test]
fn tlcx18_independence_feature_budget_retains_measured_cost_and_bounded_k1() {
    fixture(|context, view, _| {
        let plan =
            ProtectedFeatureSet::capture(SOURCE_SYMBOL, &mixed_probes(), context, scope(), view)
                .unwrap();
        let baseline = reconstruction::produce(&plan, SOURCE_SYMBOL, view).unwrap();
        let root = RootPlan::capture(
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: SOURCE_SYMBOL,
                context,
                transform: ledger::TransformId([1; 32]),
            },
            SCALE_BITS,
            view,
        )
        .unwrap();
        for length in [4097, 5000] {
            let target = symbol(length);
            let expected_bits = bundle_bits(target.len());
            assert!(expected_bits / 8 < 65_536);
            // Current code returns Error::Budget here and drops the measured K.
            // Refinement: retain independent outcomes; do not enlarge the4096B
            // feature-extraction cap or infer a mismatch from failed extraction.
            let owner = reconstruction::produce(&plan, &target, view)
                .expect("valid K6 cost must survive the independent feature extraction limit");
            assert_eq!(owner.bundle_bits(), Some(expected_bits));
            assert_eq!(owner.historical_gap(), None); // This legacy gap is decode/cost, not feature evidence.
            let expected_loss = RationalInterval::from_ratio(0, 1, 3, 4).unwrap();
            assert_eq!(owner.historical_bounds(), Some(&expected_loss));
            let axis = owner.axis_current(&plan, &target, context, view).unwrap();
            assert!(matches!(axis, AxisValue::Bounded(value) if value == &expected_loss));
            assert_eq!(plan.feature_count(), 2); // NodeKind1 observed pass, Symbol3 unobserved, W=4.
            let id = ledger::TransformId([2; 32]);
            let description = description::produce(
                &root,
                ReconstructionInput {
                    owner: &owner,
                    plan: &plan,
                    target: &target,
                    context,
                    transform: id,
                },
                view,
            )
            .unwrap();
            assert_eq!(
                description.report_current(view).unwrap().state,
                ComponentState::Measured(
                    RationalInterval::exact_ratio(expected_bits as i64, 1).unwrap()
                )
            );
            let difference = expected_bits - bundle_bits(SOURCE_SYMBOL.len());
            assert_eq!(
                description.complexity_match_current(view).unwrap(),
                Some(
                    &RationalInterval::exact_ratio(
                        SCALE_BITS as i64,
                        (SCALE_BITS + difference) as i64
                    )
                    .unwrap()
                )
            );
            assert_eq!(
                description.lineage_current(view).unwrap().candidate,
                owner.binding(id)
            );
            // Unknown feature observation cannot become a confirmed D entry.
            let _ledger_claim = view
                .claim_resources(&[ResourceRequest::new(
                    ResourceKind::Ram,
                    ResourceAmount::checked(16_384).unwrap(),
                )])
                .unwrap();
            let mut facets = ledger::FacetLedger::new(1, 4).unwrap();
            facets.admit_transform(owner.binding(id)).unwrap();
            owner
                .retain_current(&plan, &target, context, view, &mut facets, id, None)
                .unwrap();
            assert!(matches!(
                owner.retain_current(&plan, &target, context, view, &mut facets, id, Some(1)),
                Err(reconstruction::Error::Binding)
            ));
            assert!(facets
                .entries()
                .iter()
                .all(|entry| matches!(entry.facet, ledger::Facet::Uncertainty(_))));
        }
    });
}

#[test]
fn tlcx18_independence_cost_budget_does_not_hide_observed_node_kind() {
    for (source, expected_loss) in [(SOURCE_SYMBOL, 0), (SOURCE_BOOL, 1)] {
        fixture(|context, view, _| {
            let plan = ProtectedFeatureSet::capture(source, &kind_probe(), context, scope(), view)
                .unwrap();
            let baseline = reconstruction::produce(&plan, source, view).unwrap();
            let root = RootPlan::capture(
                ReconstructionInput {
                    owner: &baseline,
                    plan: &plan,
                    target: source,
                    context,
                    transform: ledger::TransformId([3; 32]),
                },
                SCALE_BITS,
                view,
            )
            .unwrap();
            let target = symbol(65532);
            assert_eq!(target.len(), 65_536);
            assert!(bundle_bits(target.len()) / 8 > 65_536);
            let owner = reconstruction::produce(&plan, &target, view).unwrap();
            assert_eq!(owner.bundle_bits(), None);
            assert_eq!(owner.historical_gap(), Some(Gap::MissingDecoderCost));
            // The actual target-only decoder has a valid Symbol. NodeKind is
            // separately within budget: Symbol->Symbol0; Bool->Symbol1.
            let loss = RationalInterval::exact_ratio(expected_loss, 1).unwrap();
            assert_eq!(owner.historical_bounds(), Some(&loss));
            assert!(
                matches!(owner.axis_current(&plan, &target, context, view).unwrap(),
                AxisValue::Exact(value) if value == &loss)
            );
            let measured = description::produce(
                &root,
                ReconstructionInput {
                    owner: &owner,
                    plan: &plan,
                    target: &target,
                    context,
                    transform: ledger::TransformId([4; 32]),
                },
                view,
            )
            .unwrap();
            assert_eq!(
                measured.report_current(view).unwrap().state,
                ComponentState::Unknown(MeasurementGap::MissingEvaluator)
            );
            assert!(measured.complexity_match_current(view).unwrap().is_none());
        });
    }
}

#[test]
fn tlcx18_independence_feature_cap_boundary_still_observes_real_mismatch() {
    fixture(|context, view, _| {
        let plan =
            ProtectedFeatureSet::capture(SOURCE_SYMBOL, &mixed_probes(), context, scope(), view)
                .unwrap();
        let target = symbol(4096);
        let owner = reconstruction::produce(&plan, &target, view).unwrap();
        let loss = RationalInterval::exact_ratio(3, 4).unwrap();
        assert_eq!(owner.historical_bounds(), Some(&loss));
        assert!(
            matches!(owner.axis_current(&plan, &target, context, view).unwrap(),
            AxisValue::Exact(value) if value == &loss)
        );
        assert_eq!(owner.bundle_bits(), Some(bundle_bits(target.len())));
        assert_eq!(owner.historical_gap(), None);
    });
}

#[test]
fn tlcx18_independence_both_caps_preserve_existing_large_symbol_unknown() {
    fixture(|context, view, _| {
        let plan =
            ProtectedFeatureSet::capture(SOURCE_SYMBOL, &mixed_probes(), context, scope(), view)
                .unwrap();
        let baseline = reconstruction::produce(&plan, SOURCE_SYMBOL, view).unwrap();
        let root = RootPlan::capture(
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: SOURCE_SYMBOL,
                context,
                transform: ledger::TransformId([5; 32]),
            },
            SCALE_BITS,
            view,
        )
        .unwrap();
        let target = symbol(65532);
        let owner = reconstruction::produce(&plan, &target, view).unwrap();
        assert_eq!(owner.bundle_bits(), None);
        assert_eq!(owner.historical_gap(), Some(Gap::MissingDecoderCost));
        let measured = description::produce(
            &root,
            ReconstructionInput {
                owner: &owner,
                plan: &plan,
                target: &target,
                context,
                transform: ledger::TransformId([6; 32]),
            },
            view,
        )
        .unwrap();
        assert_eq!(
            measured.report_current(view).unwrap().state,
            ComponentState::Unknown(MeasurementGap::MissingEvaluator)
        );
        assert!(measured.complexity_match_current(view).unwrap().is_none());
        // Fixed-domain feature evidence is still absent; it cannot certify0.
        if let Some(loss) = owner.historical_bounds() {
            assert_eq!(loss, &RationalInterval::from_ratio(0, 1, 3, 4).unwrap());
        }
    });
}

#[test]
fn tlcx18_independence_invalid_unsupported_and_cancelled_do_not_gain_values() {
    fixture(|context, view, cancel| {
        let plan =
            ProtectedFeatureSet::capture(SOURCE_SYMBOL, &kind_probe(), context, scope(), view)
                .unwrap();
        for (target, gap) in [
            (&[255][..], Gap::Unsupported),
            (&[1, 2][..], Gap::InvalidTarget),
        ] {
            let owner = reconstruction::produce(&plan, target, view).unwrap();
            assert_eq!(owner.historical_gap(), Some(gap));
            assert!(owner.historical_bounds().is_none() && owner.bundle_bits().is_none());
            assert!(matches!(
                owner.axis_current(&plan, target, context, view).unwrap(),
                AxisValue::Unavailable(_)
            ));
        }
        let baseline = reconstruction::produce(&plan, SOURCE_SYMBOL, view).unwrap();
        let root = RootPlan::capture(
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: SOURCE_SYMBOL,
                context,
                transform: ledger::TransformId([7; 32]),
            },
            SCALE_BITS,
            view,
        )
        .unwrap();
        let measured = description::produce(
            &root,
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: SOURCE_SYMBOL,
                context,
                transform: ledger::TransformId([8; 32]),
            },
            view,
        )
        .unwrap();
        cancel.cancel();
        assert!(baseline
            .axis_current(&plan, SOURCE_SYMBOL, context, view)
            .is_err());
        assert!(measured.report_current(view).is_err());
        assert!(baseline.historical_bounds().is_some());
    });
}
