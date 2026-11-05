//! Current owner identity/profile/lease cuts with a real positive before revocation.
use super::{
    checks, code, expected,
    native::{self, Options},
    resources,
};
use lc631_runtime::{
    product_cost::{
        capture_baseline_current, declare_cost_profile, measure_candidate_current,
        stage_b_copy_pack_package_v1, CompressionEligibility, CostError, CostGap, ExecutionState,
        InlinePackage, ProfileSpec, ReconstructionState, SemanticGoal,
    },
    resource::*,
};
use std::time::Duration;

pub fn cancellation() {
    native::with_fixture(Options::default(), |f| {
        let d = expected::dictionary(expected::declarations(f.cases));
        let p = stage_b_copy_pack_package_v1(&d).unwrap();
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(p.as_inline().runtime),
            f.view,
        )
        .unwrap();
        let baseline = capture_baseline_current(&profile, &f.input, p.as_inline(), f.view).unwrap();
        let candidate =
            measure_candidate_current(&baseline, &f.input, p.as_inline(), f.view).unwrap();
        candidate
            .with_current(&baseline, &f.input, f.view, |_| ())
            .unwrap();
        let historical = candidate.historical_report().bundle_bytes().to_vec();
        assert!(matches!(
            candidate.with_current(&baseline, &f.input, f.view, |_| f.cancellation.cancel()),
            Err(CostError::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            candidate.with_current(&baseline, &f.input, f.view, |_| ()),
            Err(CostError::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            capture_baseline_current(&profile, &f.input, p.as_inline(), f.view),
            Err(CostError::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(candidate.historical_report().bundle_bytes(), historical);
        assert!(candidate.historical_report().bits() > 0);
    });
}
pub fn expiry() {
    native::with_fixture(
        Options {
            expiry: true,
            ..Options::default()
        },
        |f| {
            let d = expected::dictionary(expected::declarations(f.cases));
            let p = stage_b_copy_pack_package_v1(&d).unwrap();
            let profile = declare_cost_profile(
                ProfileSpec::copy_pack_tree_v1(p.as_inline().runtime),
                f.view,
            )
            .unwrap();
            let baseline =
                capture_baseline_current(&profile, &f.input, p.as_inline(), f.view).unwrap();
            let candidate =
                measure_candidate_current(&baseline, &f.input, p.as_inline(), f.view).unwrap();
            // Demonstrate capture and consumption were live BEFORE deliberate wait.
            candidate
                .with_current(&baseline, &f.input, f.view, |_| ())
                .unwrap();
            let remaining = f.view.remaining().unwrap();
            assert!(remaining <= Duration::from_secs(6));
            std::thread::sleep(remaining + Duration::from_millis(5));
            assert!(matches!(
                candidate.with_current(&baseline, &f.input, f.view, |_| ()),
                Err(CostError::Resource(ResourceError::DeadlineExceeded))
            ));
            assert!(matches!(
                capture_baseline_current(&profile, &f.input, p.as_inline(), f.view),
                Err(CostError::Resource(ResourceError::DeadlineExceeded))
            ));
            assert!(candidate.historical_report().bits() > 0);
        },
    );
}
pub fn foreign_and_budget() {
    native::with_fixture(Options::default(), |f| {
        let d = expected::dictionary(expected::declarations(f.cases));
        let p = stage_b_copy_pack_package_v1(&d).unwrap();
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(p.as_inline().runtime),
            f.view,
        )
        .unwrap();
        let baseline = capture_baseline_current(&profile, &f.input, p.as_inline(), f.view).unwrap();
        let candidate =
            measure_candidate_current(&baseline, &f.input, p.as_inline(), f.view).unwrap();
        let (_broker, _run, reservation) = resources::run(Cancellation::default(), false);
        let foreign = reservation.lease();
        foreign.check().unwrap();
        assert!(matches!(
            candidate.with_current(&baseline, &f.input, &foreign, |_| ()),
            Err(CostError::ForeignOwner | CostError::Resource(ResourceError::ForeignOwner))
        ));
        let mut claims = Vec::new();
        let mut exhausted = false;
        for _ in 0..256 {
            match f.view.claim_resources(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(1024 * 1024).unwrap(),
            )]) {
                Ok(claim) => claims.push(claim),
                Err(ResourceError::Exhausted(ResourceKind::Ram)) => {
                    exhausted = true;
                    break;
                }
                Err(error) => panic!("unexpected admission failure: {error:?}"),
            }
        }
        assert!(exhausted && !claims.is_empty());
        assert!(matches!(
            measure_candidate_current(&baseline, &f.input, p.as_inline(), f.view),
            Err(CostError::ResourceExhausted
                | CostError::Resource(ResourceError::Exhausted(ResourceKind::Ram)))
        ));
        drop(claims);
        candidate
            .with_current(&baseline, &f.input, f.view, |_| ())
            .unwrap();
    });
}
pub fn profile_and_inverse() {
    native::with_fixture(Options::default(), |f| {
        let d = expected::dictionary(expected::declarations(f.cases));
        let p = stage_b_copy_pack_package_v1(&d).unwrap();
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(p.as_inline().runtime),
            f.view,
        )
        .unwrap();
        let baseline = capture_baseline_current(&profile, &f.input, p.as_inline(), f.view).unwrap();
        let runtime = code::extended_runtime(p.as_inline().runtime);
        assert_ne!(runtime, p.as_inline().runtime);
        assert!(matches!(
            measure_candidate_current(
                &baseline,
                &f.input,
                code::with_runtime(p.as_inline(), &runtime),
                f.view
            ),
            Err(CostError::ProfileMismatch)
        ));
        let changed_profile =
            declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&runtime), f.view).unwrap();
        let changed_base = capture_baseline_current(
            &changed_profile,
            &f.input,
            code::with_runtime(p.as_inline(), &runtime),
            f.view,
        )
        .unwrap();
        assert_ne!(changed_profile.digest(), profile.digest());
        let inverse = code::wrong_inverse();
        let wrong_i = InlinePackage {
            inverse: &inverse,
            ..code::with_runtime(p.as_inline(), &runtime)
        };
        let candidate =
            measure_candidate_current(&changed_base, &f.input, wrong_i, f.view).unwrap();
        candidate
            .with_current(&changed_base, &f.input, f.view, |current| {
                checks::whole_bundle(
                    current.cost(),
                    InlinePackage {
                        inverse: &inverse,
                        ..code::with_runtime(p.as_inline(), &runtime)
                    },
                    &f.expected_target,
                );
                assert_eq!(current.cost().inverse_state(), ExecutionState::Completed);
                assert_eq!(
                    current.cost().reconstruction_state(),
                    ReconstructionState::Mismatched
                );
                assert_eq!(
                    current.cost().compression_eligibility(),
                    CompressionEligibility::Ineligible
                );
                assert!(current.cost().bits() > 0);
            })
            .unwrap();
        // Valid charged identity A cannot pass by receiving host-canonical S.
        let identity = code::identity_adapter();
        let identity_a = InlinePackage {
            adapter: &identity,
            ..code::with_runtime(p.as_inline(), &runtime)
        };
        assert!(matches!(
            capture_baseline_current(&changed_profile, &f.input, identity_a, f.view),
            Err(CostError::CorrespondenceMismatch
                | CostError::InvalidProgram
                | CostError::NeedsEvidence(CostGap::NativeCanonicalizationCostClosurePending))
        ));
        let mut exact = ProfileSpec::copy_pack_tree_v1(p.as_inline().runtime);
        exact.goal = SemanticGoal::ExactNativeIdentity;
        match declare_cost_profile(exact, f.view) {
            Err(CostError::NeedsEvidence(CostGap::NeedsProfileDecision)) => (),
            Ok(exact_profile) => assert!(matches!(
                capture_baseline_current(&exact_profile, &f.input, p.as_inline(), f.view),
                Err(CostError::NeedsEvidence(CostGap::NeedsProfileDecision))
            )),
            _ => panic!("exact native IDs cannot silently use the semantic-local-roles profile"),
        }
    });
}
