//! 日本語: 課金済み定数の合法性と、実native join ownerの成立を区別する。
//! English: Keep paid constants legal while requiring an actual native adapter join.
//! 日本語: 構築・正例・固有REDは中央leaseで別々に観測する。
//! English: Construction, positive controls and distinguishing REDs require the central lease.
#![forbid(unsafe_code)]

#[path = "tlcx22_product_cost_current_support/atoms.rs"]
mod atoms;
#[path = "tlcx22_product_cost_current_support/expected.rs"]
mod expected;
#[path = "tlcx22_product_cost_current_support/models.rs"]
mod models;
#[path = "tlcx22_product_cost_current_support/native.rs"]
mod native;
#[path = "tlcx22_product_cost_current_support/resources.rs"]
mod resources;
#[path = "tlcx22_product_cost_current_support/wire.rs"]
mod wire;

use lc631_runtime::{
    product_cost::{
        capture_baseline_current, declare_cost_profile, execute_inverse, execute_reported,
        stage_b_copy_pack_package_v1, CompressionEligibility, CostError, CostGap, CostLimits,
        CostObservation, DeclaredCostProfile, ExecutionControls, ExecutionState, FixedBootstrap,
        IndependenceStatus, InlinePackage, InputBasis, OwnedSemanticProductBaseline, ProfileSpec,
        ReconstructionState, ReportedFieldInput,
    },
    resource::ResourceReservationView,
};
use lc631_substrate::semantic_term::Term;
use wire::{app, encode, integer, symbol};

fn adapter(body: Term) -> Vec<u8> {
    encode(&app(
        "program-v1",
        vec![symbol("adapter"), integer(2), body],
    ))
}

fn paid_copy_report(
    report: &CostObservation,
    package: InlinePackage<'_>,
    semantic: &[u8],
    declarations: &[u8],
    basis: InputBasis,
    gap: Option<CostGap>,
) {
    let decoder = app(
        "decoder-v1",
        vec![
            expected::decode(package.adapter),
            expected::decode(package.projection),
            expected::decode(package.inverse),
            expected::decode(package.helpers),
            expected::decode(package.runtime),
        ],
    );
    // 日本語: 未使用のH/R/Dも、独立encoderで完全bundleに含める。
    // English: The independent encoder includes every H/R/D entry, even when unused.
    let whole = encode(&app(
        "bundle-v1",
        vec![
            expected::decode(semantic),
            decoder,
            expected::decode(package.dictionary),
        ],
    ));
    assert_eq!(report.bundle_bytes(), whole.as_slice());
    assert_eq!(
        report.bits(),
        u64::try_from(whole.len()).unwrap().checked_mul(8).unwrap()
    );
    assert!(report.bits() > 0);
    assert_eq!(report.semantic_bytes(), semantic);
    assert_eq!(report.representation_bytes(), semantic);
    assert_eq!(report.reconstruction_bytes(), Some(semantic));
    assert_eq!(report.inverse_state(), ExecutionState::Completed);
    assert_eq!(report.reconstruction_state(), ReconstructionState::Matched);
    assert_eq!(report.compression_eligibility(), CompressionEligibility::Pending);
    assert_eq!(report.dictionary_bytes(), package.dictionary);
    assert_eq!(report.declarations_bytes(), declarations);
    assert_eq!(report.input_basis(), basis);
    assert_eq!(report.independence(), IndependenceStatus::SharedDependency);
    assert_eq!(report.native_cost_gap(), gap);
    assert_eq!(report.baseline_bits(), None);
    assert!(!report.formula_evaluated());
}

fn reported_and_isolated_control<'v>(
    profile: &DeclaredCostProfile<'v>,
    package: InlinePackage<'_>,
    semantic: &[u8],
    declarations: &[u8],
    view: &'v ResourceReservationView<'_>,
) {
    let fields = encode(&app("reported-fields-v1", vec![Term::Bool(false)]));
    let reported = execute_reported(
        profile,
        ReportedFieldInput {
            fields: &fields,
            declarations,
        },
        package,
        view,
    )
    .expect("a fully paid constant adapter remains valid on the Given machine path");
    paid_copy_report(
        reported.historical_report(),
        package,
        semantic,
        declarations,
        InputBasis::Given,
        Some(CostGap::NativeCanonicalizationCostClosurePending),
    );
    let isolated = execute_inverse(
        reported.target_bundle(),
        FixedBootstrap::k6_tree_v1(),
        ExecutionControls::from_view(view, CostLimits::default().fuel_per_direction).unwrap(),
    )
    .expect("the paid inverse consumes only the target bundle and resource controls");
    assert_eq!(isolated.state(), ExecutionState::Completed);
    assert_eq!(isolated.bytes(), Some(semantic));
}

fn require_native_join_gap(
    result: Result<OwnedSemanticProductBaseline<'_, '_>, CostError>,
    case: &str,
) {
    match result {
        Err(CostError::NeedsEvidence(CostGap::NativeCanonicalizationCostClosurePending)) => (),
        Err(other) => panic!("{case}: unrelated factory failure, not the required join gap: {other:?}"),
        Ok(owner) => panic!(
            "{case}: native factory accepted a constant adapter without its paid joins; K0={} gap={:?}",
            owner.historical_report().bits(),
            owner.historical_report().native_cost_gap()
        ),
    }
}

#[test]
fn actual_native_literal_adapter_requires_paid_join_closure() {
    native::with_fixture(native::Options::default(), |fixture| {
        let declarations = expected::declarations(fixture.cases);
        let declaration_bytes = encode(&declarations);
        let dictionary = expected::dictionary(declarations);
        let package = stage_b_copy_pack_package_v1(&dictionary).unwrap();
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(package.as_inline().runtime),
            fixture.view,
        )
        .unwrap();
        {
            let normal = capture_baseline_current(
                &profile, &fixture.input, package.as_inline(), fixture.view,
            )
            .expect("funded genuine native baseline with the normal paid adapter");
            paid_copy_report(
                normal.historical_report(), package.as_inline(), &fixture.expected_source,
                &declaration_bytes, InputBasis::ActualOwnerObservedWithGivenMeanings, None,
            );
        }
        let literal = adapter(app("literal", vec![expected::decode(&fixture.expected_source)]));
        let constant = InlinePackage { adapter: &literal, ..package.as_inline() };
        reported_and_isolated_control(
            &profile, constant, &fixture.expected_source, &declaration_bytes, fixture.view,
        );
        // 日本語: cost K0は入力にしない。元のgenuine native portsをそのまま渡す。
        // English: No cost K0 is an input. Keep the original genuine native ports unchanged.
        require_native_join_gap(
            capture_baseline_current(&profile, &fixture.input, constant, fixture.view),
            "literal[S0]",
        );
    });
}

#[test]
fn actual_native_dictionary_constant_adapter_requires_paid_join_closure() {
    native::with_fixture(native::Options::default(), |fixture| {
        let declarations = expected::declarations(fixture.cases);
        let declaration_bytes = encode(&declarations);
        let original_dictionary = expected::dictionary(declarations.clone());
        let dictionary = encode(&app(
            "dictionary-v1",
            vec![
                declarations,
                app("constants-v1", vec![expected::decode(&fixture.expected_source)]),
                app("helper-data-v1", vec![]),
                app("residuals-v1", vec![]),
            ],
        ));
        assert_ne!(dictionary, original_dictionary);
        let package = stage_b_copy_pack_package_v1(&dictionary).unwrap();
        let profile = declare_cost_profile(
            ProfileSpec::copy_pack_tree_v1(package.as_inline().runtime),
            fixture.view,
        )
        .unwrap();
        {
            let normal = capture_baseline_current(
                &profile, &fixture.input, package.as_inline(), fixture.view,
            )
            .expect("funded native baseline with unchanged declarations and a paid unused S0");
            paid_copy_report(
                normal.historical_report(), package.as_inline(), &fixture.expected_source,
                &declaration_bytes, InputBasis::ActualOwnerObservedWithGivenMeanings, None,
            );
        }
        let from_dictionary = adapter(app(
            "dictionary", vec![integer(1), app("literal", vec![integer(0)])],
        ));
        let constant = InlinePackage { adapter: &from_dictionary, ..package.as_inline() };
        reported_and_isolated_control(
            &profile, constant, &fixture.expected_source, &declaration_bytes, fixture.view,
        );
        require_native_join_gap(
            capture_baseline_current(&profile, &fixture.input, constant, fixture.view),
            "D.constants[S0]",
        );
    });
}
