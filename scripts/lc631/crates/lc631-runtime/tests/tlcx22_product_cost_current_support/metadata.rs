//! Operational contract pins remain checked even when complete semantic bytes coincide.
use super::{
    expected, models,
    native::{self, Options},
};
use lc631_analysis::creativity_measurement::Rational;
use lc631_core::context::ContextScope;
use lc631_runtime::{
    adversarial_measurement::Response,
    product_correspondence::{
        CaseDeclaration, IdentityDeclaration, PlanSpec, ProductError, ProductInput, ProductLimits,
        ProductPlan, TlProfile,
    },
    product_cost::{
        capture_baseline_current, declare_cost_profile, stage_b_copy_pack_package_v1, CostError,
        CostGap, NativeProductInput, ProfileSpec,
    },
    temporal_preservation::TemporalModelInput,
};
use lc631_solvers::temporal::{TemporalNode, TemporalProfile, TemporalQuery};

pub fn metadata_cuts() {
    native::with_fixture(Options::default(), |f| {
        let d = expected::dictionary(expected::declarations(f.cases));
        let package = stage_b_copy_pack_package_v1(&d).unwrap();
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(package.as_inline().runtime),
            f.view,
        )
        .unwrap();
        let saved = f.input.owners.baseline_risk.historical_report().clone();
        for mutation in 0..8 {
            let context = if mutation == 0 {
                models::context().reconstructed([98; 32])
            } else {
                models::context()
            };
            let scope = ContextScope::declared(
                if mutation == 1 {
                    models::pin(98)
                } else {
                    models::pin(1)
                },
                if mutation == 2 {
                    models::pin(98)
                } else {
                    models::pin(2)
                },
                if mutation == 3 {
                    models::pin(98)
                } else {
                    models::pin(3)
                },
                if mutation == 4 {
                    models::pin(98)
                } else {
                    models::pin(4)
                },
            );
            let weight = Rational::from_integer(if mutation == 7 { 2.into() } else { 1.into() });
            let cases = f.cases.each_ref().map(|c| CaseDeclaration {
                native_key: c.key,
                native_atoms: &c.atoms,
                semantic_spec: &c.spec,
                reference_contract: &c.contract,
                oracle: models::pin(if mutation == 6 { 99 } else { 11 }),
                weight: &weight,
            });
            let plan = ProductPlan::declared(
                PlanSpec {
                    context,
                    scope,
                    goal_profile: if mutation == 5 {
                        Some(models::pin(98))
                    } else {
                        None
                    },
                    tl_profile: TlProfile::TemporalSpatial1To3,
                    cases: &cases,
                    identities: f.identities,
                    temporal_atom_meanings: f.temporal_meanings,
                    limits: ProductLimits::default(),
                },
                f.view,
            )
            .unwrap();
            let source_query = TemporalQuery::checked(
                context,
                vec![TemporalNode::Atom(0)],
                0,
                TemporalProfile::ClosedFiniteKleeneV1,
            )
            .unwrap();
            let target_query = TemporalQuery::checked(
                context,
                vec![TemporalNode::Atom(0)],
                0,
                TemporalProfile::ClosedFiniteKleeneV1,
            )
            .unwrap();
            let source = TemporalModelInput {
                query: &source_query,
                trace: f.source_temporal.trace,
                scope,
            };
            let target = TemporalModelInput {
                query: &target_query,
                trace: f.target_temporal.trace,
                scope,
            };
            let base_responses = f.cases.each_ref().map(|c| Response {
                case: c.key,
                term: &c.baseline,
            });
            let target_responses = f.cases.each_ref().map(|c| Response {
                case: c.key,
                term: &c.target,
            });
            let baseline = plan
                .project_reported(
                    ProductInput {
                        responses: &base_responses,
                        temporal: source,
                        spatial: lc631_runtime::spatial_preservation::SpatialModelInput {
                            context,
                            scope,
                            ..f.source_spatial
                        },
                    },
                    f.view,
                )
                .unwrap();
            let candidate = plan
                .project_reported(
                    ProductInput {
                        responses: &target_responses,
                        temporal: target,
                        spatial: lc631_runtime::spatial_preservation::SpatialModelInput {
                            context,
                            scope,
                            ..f.target_spatial
                        },
                    },
                    f.view,
                )
                .unwrap();
            assert_eq!(baseline.semantic_bytes(), f.expected_source);
            assert_eq!(candidate.semantic_bytes(), f.expected_target);
            let input = NativeProductInput {
                plan: &plan,
                baseline: &baseline,
                target: &candidate,
                owners: f.input.owners,
            };
            // The expected comparands match this wrong plan completely. Actual
            // source owners retain their original full context/scope/contracts.
            assert!(matches!(
                capture_baseline_current(&profile, &input, package.as_inline(), f.view),
                Err(CostError::CorrespondenceMismatch
                    | CostError::NeedsEvidence(CostGap::ApplicabilityUnknown))
            ));
        }
        assert_eq!(f.input.owners.baseline_risk.historical_report(), &saved);
        // These are explicit UPSTREAM native-identity admission cuts, not an
        // execution claim about the new private A role-map collector. The old
        // fixed-TL profile rejects candidate-only cross-clock splits earlier.
        for merge in [true, false] {
            let mut identities = f.identities.each_ref().map(|i| IdentityDeclaration {
                kind: i.kind,
                native_id: i.native_id,
                local_role: i.local_role,
                meaning: i.meaning,
            });
            if merge {
                identities[1].native_id = identities[0].native_id;
            } else {
                identities[1].local_role = 0;
            }
            let weight = Rational::from_integer(1.into());
            let cases = f.cases.each_ref().map(|c| CaseDeclaration {
                native_key: c.key,
                native_atoms: &c.atoms,
                semantic_spec: &c.spec,
                reference_contract: &c.contract,
                oracle: models::pin(11),
                weight: &weight,
            });
            assert!(matches!(
                ProductPlan::declared(
                    PlanSpec {
                        context: models::context(),
                        scope: models::scope(),
                        goal_profile: None,
                        tl_profile: TlProfile::TemporalSpatial1To3,
                        cases: &cases,
                        identities: &identities,
                        temporal_atom_meanings: f.temporal_meanings,
                        limits: ProductLimits::default()
                    },
                    f.view
                ),
                Err(ProductError::IdentityMismatch)
            ));
        }
        capture_baseline_current(&profile, &f.input, package.as_inline(), f.view).unwrap();
    });
}
