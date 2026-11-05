//! Actual current-owner contract. New profile only; no strict formula/normal128 closure.
mod atoms;
mod checks;
mod code;
mod declarations;
mod expected;
mod lifecycle;
mod metadata;
mod models;
mod native;
mod resources;
mod wire;
use lc631_runtime::product_cost::{
    capture_baseline_current, declare_cost_profile, measure_candidate_current,
    stage_b_copy_pack_package_v1, CostError, NativeProductInput, ProfileSpec,
};
use lc631_substrate::Digest32;
pub use native::Options;

pub struct Snapshot {
    pub source_bundle: Vec<u8>,
    pub target_bundle: Vec<u8>,
    pub raw_envelope: Vec<u8>,
    pub k0: u64,
    pub k: u64,
    pub baseline_binding: Digest32,
    pub candidate_binding: Digest32,
}
pub fn positive(options: Options) -> Snapshot {
    native::with_fixture(options, |fixture| {
        let declarations = expected::declarations(fixture.cases);
        let dictionary = expected::dictionary(declarations.clone());
        let package = stage_b_copy_pack_package_v1(&dictionary).unwrap();
        assert_eq!(package.dictionary(), dictionary);
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(package.as_inline().runtime),
            fixture.view,
        )
        .unwrap();
        let owners = fixture.input.owners;
        let saved = [
            owners.baseline_description.historical_report().clone(),
            owners.target_description.historical_report().clone(),
            owners.baseline_risk.historical_report().clone(),
            owners.target_risk.historical_report().clone(),
        ];
        let saved_lineage = *owners
            .baseline_description
            .lineage_current(fixture.view)
            .unwrap();
        let baseline =
            capture_baseline_current(&profile, &fixture.input, package.as_inline(), fixture.view)
                .unwrap();
        let base = baseline.historical_report();
        checks::whole_bundle(base, package.as_inline(), &fixture.expected_source);
        checks::reconstructed(base, &fixture.expected_source);
        assert_eq!(base.baseline_bits(), None);
        let candidate =
            measure_candidate_current(&baseline, &fixture.input, package.as_inline(), fixture.view)
                .unwrap();
        candidate
            .with_current(&baseline, &fixture.input, fixture.view, |current| {
                checks::borrowed(&current, &fixture, &declarations);
                checks::whole_bundle(
                    current.cost(),
                    package.as_inline(),
                    &fixture.expected_target,
                );
                checks::reconstructed(current.cost(), &fixture.expected_target);
                assert_eq!(current.cost().baseline_bits(), Some(base.bits()));
            })
            .unwrap();
        // Fresh SECOND actual candidate uses the SAME captured K0 owner. Its
        // target ports differ; baseline identity must not contain target ports.
        let second = measure_candidate_current(
            &baseline,
            &fixture.second,
            package.as_inline(),
            fixture.view,
        )
        .unwrap();
        second
            .with_current(&baseline, &fixture.second, fixture.view, |current| {
                checks::whole_bundle(
                    current.cost(),
                    package.as_inline(),
                    &fixture.expected_source,
                );
                checks::reconstructed(current.cost(), &fixture.expected_source);
                assert_eq!(current.cost().baseline_bits(), Some(base.bits()));
                assert_ne!(
                    current.cost().binding(),
                    candidate.historical_report().binding()
                );
            })
            .unwrap();
        assert!(candidate
            .with_current(&baseline, &fixture.second, fixture.view, |_| ())
            .is_err());
        assert!(second
            .with_current(&baseline, &fixture.input, fixture.view, |_| ())
            .is_err());
        assert_eq!(owners.baseline_description.historical_report(), &saved[0]);
        assert_eq!(owners.target_description.historical_report(), &saved[1]);
        assert_eq!(owners.baseline_risk.historical_report(), &saved[2]);
        assert_eq!(owners.target_risk.historical_report(), &saved[3]);
        assert_eq!(
            owners
                .baseline_description
                .lineage_current(fixture.view)
                .unwrap(),
            &saved_lineage
        );
        // Legacy observations remain legacy; new costs did not retag them.
        assert!(!base.bundle_bytes().is_empty());
        assert_eq!(
            lc631_runtime::product_correspondence::inspect_native_current(
                fixture.input.plan,
                fixture.input.baseline,
                fixture.input.target,
                owners,
                fixture.view
            )
            .unwrap()
            .with_current(fixture.view, |old| old.scalar_status())
            .unwrap(),
            lc631_runtime::product_correspondence::DeferredReason::FullCostOwnerMissing
        );
        Snapshot {
            source_bundle: base.bundle_bytes().to_vec(),
            target_bundle: candidate.historical_report().bundle_bytes().to_vec(),
            raw_envelope: fixture.input.target.identity_envelope_bytes().to_vec(),
            k0: base.bits(),
            k: candidate.historical_report().bits(),
            baseline_binding: base.binding(),
            candidate_binding: candidate.historical_report().binding(),
        }
    })
}
pub fn owner_cuts() {
    use lc631_runtime::product_correspondence::NativeOwnerRefs;
    native::with_fixture(Options::default(), |f| {
        let d = expected::dictionary(expected::declarations(f.cases));
        let package = stage_b_copy_pack_package_v1(&d).unwrap();
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(package.as_inline().runtime),
            f.view,
        )
        .unwrap();
        let baseline =
            capture_baseline_current(&profile, &f.input, package.as_inline(), f.view).unwrap();
        let candidate =
            measure_candidate_current(&baseline, &f.input, package.as_inline(), f.view).unwrap();
        let o = f.input.owners;
        let e = f.equal_owners;
        let foreign = [
            NativeOwnerRefs {
                baseline_description: e.baseline_description,
                ..o
            },
            NativeOwnerRefs {
                target_description: e.target_description,
                ..o
            },
            NativeOwnerRefs {
                baseline_risk: e.baseline_risk,
                ..o
            },
            NativeOwnerRefs {
                baseline_responses: e.baseline_responses,
                ..o
            },
            NativeOwnerRefs {
                target_risk: e.target_risk,
                ..o
            },
            NativeOwnerRefs {
                target_responses: e.target_responses,
                ..o
            },
            NativeOwnerRefs {
                tl_root: e.tl_root,
                ..o
            },
            NativeOwnerRefs {
                tl_candidate: e.tl_candidate,
                ..o
            },
            NativeOwnerRefs {
                tl_reduction: e.tl_reduction,
                ..o
            },
        ];
        for ports in foreign {
            let input = NativeProductInput {
                plan: f.input.plan,
                baseline: f.input.baseline,
                target: f.input.target,
                owners: ports,
            };
            assert!(matches!(
                candidate.with_current(&baseline, &input, f.view, |_| ()),
                Err(CostError::ForeignOwner
                    | CostError::CorrespondenceMismatch
                    | CostError::StaleBaseline)
            ));
        }
        assert!(matches!(
            candidate.with_current(&baseline, &f.equal_plan, f.view, |_| ()),
            Err(CostError::ForeignOwner
                | CostError::CorrespondenceMismatch
                | CostError::StaleBaseline)
        ));
        let equal_baseline =
            capture_baseline_current(&profile, &f.input, package.as_inline(), f.view).unwrap();
        assert_eq!(
            equal_baseline.historical_report().bundle_bytes(),
            baseline.historical_report().bundle_bytes()
        );
        assert_eq!(
            equal_baseline.historical_report().bits(),
            baseline.historical_report().bits()
        );
        assert!(matches!(
            candidate.with_current(&equal_baseline, &f.input, f.view, |_| ()),
            Err(CostError::ForeignOwner | CostError::StaleBaseline)
        ));
        candidate
            .with_current(&baseline, &f.input, f.view, |_| ())
            .unwrap();
    });
}
pub use declarations::dictionary_cuts;
pub use lifecycle::{cancellation, expiry, foreign_and_budget, profile_and_inverse};
pub use metadata::metadata_cuts;
