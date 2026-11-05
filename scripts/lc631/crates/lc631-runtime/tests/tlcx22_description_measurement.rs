//! 日本語: 実K6 ownerのCR-K。負例を実装より先に作成。全F1接続ではない。
//! English: Actual K6 CR-K; negatives authored before implementation, no full F1 join.
mod tlcx22_description_support;
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentState, IndependenceStatus, InputBasis, MeasurementError,
    MeasurementGap, RationalInterval, SemanticValidation,
};
use lc631_runtime::{
    description_measurement::{
        self as description, ComplexityPreference, Error, ReconstructionInput, RootPlan,
    },
    facet_ledger::TransformId,
    reconstruction_measurement::{self as reconstruction, ProtectedFeatureSet},
};
use tlcx22_description_support::*;

#[test]
fn tlcx22_description_zero_scale_and_non_source_baseline_reject() {
    fixture(|context, view, _| {
        let plan = ProtectedFeatureSet::capture(BASE, &probes(), context, scope(), view).unwrap();
        let baseline = reconstruction::produce(&plan, BASE, view).unwrap();
        let input = ReconstructionInput {
            owner: &baseline,
            plan: &plan,
            target: BASE,
            context,
            transform: TransformId([22; 32]),
        };
        assert!(matches!(
            RootPlan::capture(input, 0, view),
            Err(Error::InvalidScale)
        ));
        // This owner is genuine/current and fits the plan, but cannot become K0.
        let short = reconstruction::produce(&plan, SMALL, view).unwrap();
        let input = ReconstructionInput {
            owner: &short,
            plan: &plan,
            target: SMALL,
            context,
            transform: TransformId([23; 32]),
        };
        assert!(matches!(
            RootPlan::capture(input, 16, view),
            Err(Error::BaselineMismatch)
        ));
    });
}

#[test]
fn tlcx22_description_owner_target_context_plan_swaps_reject() {
    fixture(|context, view, _| {
        let plan = ProtectedFeatureSet::capture(BASE, &probes(), context, scope(), view).unwrap();
        let baseline = reconstruction::produce(&plan, BASE, view).unwrap();
        let root = RootPlan::capture(
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: BASE,
                context,
                transform: TransformId([22; 32]),
            },
            16,
            view,
        )
        .unwrap();
        let same_count_other_source =
            ProtectedFeatureSet::capture(LARGE, &probes(), context, scope(), view).unwrap();
        let foreign_owner = reconstruction::produce(&same_count_other_source, BASE, view).unwrap();
        for input in [
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: SMALL,
                context,
                transform: TransformId([24; 32]),
            },
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: BASE,
                context: context.reconstructed([99; 32]),
                transform: TransformId([24; 32]),
            },
            ReconstructionInput {
                owner: &baseline,
                plan: &same_count_other_source,
                target: BASE,
                context,
                transform: TransformId([24; 32]),
            },
            ReconstructionInput {
                owner: &foreign_owner,
                plan: &same_count_other_source,
                target: BASE,
                context,
                transform: TransformId([24; 32]),
            },
        ] {
            assert!(description::produce(&root, input, view).is_err());
        }
        assert_eq!(
            plan.feature_count(),
            same_count_other_source.feature_count()
        );
    });
}

#[test]
fn tlcx22_description_unavailable_costs_stay_typed_and_preference_needs_evidence() {
    fixture(|context, view, _| {
        let plan = ProtectedFeatureSet::capture(BASE, &probes(), context, scope(), view).unwrap();
        let baseline = reconstruction::produce(&plan, BASE, view).unwrap();
        let root = RootPlan::capture(
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: BASE,
                context,
                transform: TransformId([22; 32]),
            },
            16,
            view,
        )
        .unwrap();
        let unsupported = reconstruction::produce(&plan, &[255], view).unwrap();
        let invalid = reconstruction::produce(&plan, &[1, 2], view).unwrap();
        let left = description::produce(
            &root,
            ReconstructionInput {
                owner: &unsupported,
                plan: &plan,
                target: &[255],
                context,
                transform: TransformId([23; 32]),
            },
            view,
        )
        .unwrap();
        let right = description::produce(
            &root,
            ReconstructionInput {
                owner: &invalid,
                plan: &plan,
                target: &[1, 2],
                context,
                transform: TransformId([24; 32]),
            },
            view,
        )
        .unwrap();
        assert_eq!(
            left.report_current(view).unwrap().state,
            ComponentState::Unknown(MeasurementGap::Unsupported)
        );
        assert_eq!(
            right.report_current(view).unwrap().state,
            ComponentState::InvalidConfig(MeasurementError::InvalidInput)
        );
        assert!(left.complexity_match_current(view).unwrap().is_none());
        assert!(right.complexity_match_current(view).unwrap().is_none());
        assert_eq!(
            description::compare_current(&left, &right, view).unwrap(),
            ComplexityPreference::NeedsEvidence
        );
        assert_eq!(
            description::compare_current(&left, &left, view).unwrap(),
            ComplexityPreference::NeedsEvidence
        );
        // A valid canonical 64KiB Symbol fits target decoding but adding the
        // required decoder/dictionary exceeds that producer's bundle budget.
        let mut large_term = vec![3, 0xfc, 0xff, 3]; // shortest ULEB128(65532)
        large_term.resize(65_536, b'a');
        let no_cost = reconstruction::produce(&plan, &large_term, view).unwrap();
        assert_eq!(
            no_cost.historical_gap(),
            Some(reconstruction::Gap::MissingDecoderCost)
        );
        let missing = description::produce(
            &root,
            ReconstructionInput {
                owner: &no_cost,
                plan: &plan,
                target: &large_term,
                context,
                transform: TransformId([25; 32]),
            },
            view,
        )
        .unwrap();
        assert_eq!(
            missing.report_current(view).unwrap().state,
            ComponentState::Unknown(MeasurementGap::MissingEvaluator)
        );
        assert!(missing.complexity_match_current(view).unwrap().is_none());
    });
}

#[test]
fn tlcx22_description_cancel_and_exact_foreign_reservation_reject_current() {
    fixture(|context, view, cancel| {
        let plan = ProtectedFeatureSet::capture(BASE, &probes(), context, scope(), view).unwrap();
        let baseline = reconstruction::produce(&plan, BASE, view).unwrap();
        let root = RootPlan::capture(
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: BASE,
                context,
                transform: TransformId([22; 32]),
            },
            16,
            view,
        )
        .unwrap();
        let owner = description::produce(
            &root,
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: BASE,
                context,
                transform: TransformId([22; 32]),
            },
            view,
        )
        .unwrap();
        fixture(|same_context, foreign, _| {
            assert_eq!(same_context, context);
            assert!(owner.report_current(foreign).is_err());
            assert!(description::compare_current(&owner, &owner, foreign).is_err());
        });
        cancel.cancel();
        assert!(owner.report_current(view).is_err());
        assert!(owner.complexity_match_current(view).is_err());
        assert!(description::compare_current(&owner, &owner, view).is_err());
        assert!(matches!(
            owner.historical_report().state,
            ComponentState::Measured(_)
        ));
    });
}

#[test]
fn tlcx22_description_actual_decoder_inclusive_k_and_original_lineage_unchanged() {
    fixture(|context, view, _| {
        let plan = ProtectedFeatureSet::capture(BASE, &probes(), context, scope(), view).unwrap();
        let baseline = reconstruction::produce(&plan, BASE, view).unwrap();
        let candidate = reconstruction::produce(&plan, SMALL, view).unwrap();
        let base_id = TransformId([22; 32]);
        let target_id = TransformId([23; 32]);
        let base_binding = baseline.binding(base_id);
        let target_binding = candidate.binding(target_id);
        let root = RootPlan::capture(
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: BASE,
                context,
                transform: base_id,
            },
            16,
            view,
        )
        .unwrap();
        let owner = description::produce(
            &root,
            ReconstructionInput {
                owner: &candidate,
                plan: &plan,
                target: SMALL,
                context,
                transform: target_id,
            },
            view,
        )
        .unwrap();
        let report = owner.report_current(view).unwrap();
        let lineage = owner.lineage_current(view).unwrap();
        assert_eq!(lineage.baseline, base_binding);
        assert_eq!(lineage.candidate, target_binding);
        assert_eq!(
            lineage.transform_references,
            description::TransformReferenceBasis::DeclaredGiven
        );
        assert_eq!(baseline.binding(base_id), base_binding);
        assert_eq!(candidate.binding(target_id), target_binding);
        assert_eq!(lineage.source_term, plan.source_term_digest());
        assert_eq!(lineage.common_runtime, baseline.common_runtime_digest());
        assert_eq!(lineage.common_runtime, candidate.common_runtime_digest());
        assert_eq!(report.metric, ComponentMetric::DescriptionBits);
        assert_eq!(report.binding.source.as_bytes(), &target_binding.source.0);
        assert_eq!(report.binding.target.as_bytes(), &target_binding.target.0);
        assert_eq!(
            report.binding.context.as_bytes(),
            &target_binding.target_context.0
        );
        assert_eq!(
            report.binding.baseline.as_bytes(),
            &target_binding.scope.baseline.0
        );
        assert_ne!(
            report.binding.producer.as_bytes(),
            &target_binding.producer.0
        );
        assert_ne!(report.binding.result.as_bytes(), &target_binding.result.0);
        assert_eq!(report.basis, InputBasis::Given);
        assert_eq!(report.validation, SemanticValidation::Unassessed);
        assert_eq!(report.independence, IndependenceStatus::SharedDependency);
        // Independently count K6 Bundle overhead(13), literal semantic term, decoder
        // Symbol tag+shortest ULEB length+actual text bytes, and empty dictionary(2).
        let decoder = include_bytes!("../src/reconstruction_measurement/decoder.rs").len();
        let mut n = decoder;
        let mut uleb = 1;
        while n >= 128 {
            n >>= 7;
            uleb += 1;
        }
        let k0 = 8 * (13 + BASE.len() + 1 + uleb + decoder + 2) as u64;
        assert_eq!(lineage.baseline_bits, k0);
        assert_eq!(lineage.candidate_bits, Some(k0 - 16));
        assert_eq!(
            report.state,
            ComponentState::Measured(RationalInterval::exact_ratio((k0 - 16) as i64, 1).unwrap())
        );
        assert_eq!(
            owner.complexity_match_current(view).unwrap(),
            Some(&RationalInterval::exact_ratio(1, 2).unwrap())
        );
        // K still exists when a protected symbol is lost: no compression prerequisite.
        assert_eq!(
            candidate.historical_bounds(),
            Some(&RationalInterval::exact_ratio(1, 1).unwrap())
        );
        assert_eq!(
            candidate.semantic_status(),
            reconstruction::SemanticStatus::Unverified
        );
    });
}

#[test]
fn tlcx22_description_actual_k_changes_component_preference_and_symmetric_ties() {
    fixture(|context, view, _| {
        let plan = ProtectedFeatureSet::capture(BASE, &probes(), context, scope(), view).unwrap();
        let baseline = reconstruction::produce(&plan, BASE, view).unwrap();
        let small = reconstruction::produce(&plan, SMALL, view).unwrap();
        let large = reconstruction::produce(&plan, LARGE, view).unwrap();
        let root = RootPlan::capture(
            ReconstructionInput {
                owner: &baseline,
                plan: &plan,
                target: BASE,
                context,
                transform: TransformId([22; 32]),
            },
            16,
            view,
        )
        .unwrap();
        let input = |owner, target, n| ReconstructionInput {
            owner,
            plan: &plan,
            target,
            context,
            transform: TransformId([n; 32]),
        };
        let identity = description::produce(&root, input(&baseline, BASE, 22), view).unwrap();
        let left = description::produce(&root, input(&small, SMALL, 23), view).unwrap();
        let right = description::produce(&root, input(&large, LARGE, 24), view).unwrap();
        assert_eq!(
            identity.complexity_match_current(view).unwrap(),
            Some(&RationalInterval::exact_ratio(1, 1).unwrap())
        );
        assert_eq!(
            description::compare_current(&identity, &left, view).unwrap(),
            ComplexityPreference::Left
        );
        assert_eq!(
            description::compare_current(&left, &identity, view).unwrap(),
            ComplexityPreference::Right
        );
        assert_eq!(
            description::compare_current(&left, &right, view).unwrap(),
            ComplexityPreference::Tie
        );
        assert_eq!(
            left.complexity_match_current(view).unwrap(),
            right.complexity_match_current(view).unwrap()
        );
        // Even two root captures with equal numbers are not the same fixed plan.
        let other = RootPlan::capture(input(&baseline, BASE, 22), 32, view).unwrap();
        let other_value = description::produce(&other, input(&small, SMALL, 23), view).unwrap();
        assert!(matches!(
            description::compare_current(&left, &other_value, view),
            Err(Error::RootMismatch)
        ));
        let same_scale = RootPlan::capture(input(&baseline, BASE, 22), 16, view).unwrap();
        let separately_fixed =
            description::produce(&same_scale, input(&small, SMALL, 23), view).unwrap();
        assert!(matches!(
            description::compare_current(&left, &separately_fixed, view),
            Err(Error::RootMismatch)
        ));
    });
}
