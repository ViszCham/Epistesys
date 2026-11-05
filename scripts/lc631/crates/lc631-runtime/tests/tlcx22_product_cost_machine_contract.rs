//! CF machine ABI/API negative-first target. Production product_cost is ABSENT.
//! Independent hand K6/code fixtures are Given; no actual native/current claim.
#[path = "tlcx22_product_cost_machine_support/boundaries.rs"]
mod boundaries;
#[path = "tlcx22_product_cost_machine_support/fixture.rs"]
mod fixture;
#[path = "tlcx22_product_cost_machine_support/resources.rs"]
mod resources;
#[path = "tlcx22_product_cost_machine_support/wire.rs"]
mod wire;
use fixture::{program, Fixture};
use lc631_runtime::product_cost::{
    declare_cost_profile, execute_inverse, execute_reported, CompressionEligibility, CostError,
    CostGap, CostLimits, ExecutionControls, ExecutionState, FixedBootstrap, IndependenceStatus,
    InputBasis, ProfileSpec, ReconstructionState, TargetBundleView,
};
use wire::{app, arg, counted, encoded, fields_mut, int, literal, op, sym, Node};

#[test]
fn tlcx22_cf_p04_literal_k6_and_full_bundle_independent_count() {
    assert_eq!(encoded(&Node::Bool(false)), [1, 0]);
    assert_eq!(encoded(&sym("a")), [3, 1, 97]);
    assert_eq!(encoded(&int(0)), [2, 0, 1, 0]);
    let empty = app("bundle-v1", vec![Node::Bool(false); 3]);
    let literal = [
        4, 3, 9, 98, 117, 110, 100, 108, 101, 45, 118, 49, 3, 1, 0, 1, 0, 1, 0,
    ];
    assert_eq!(encoded(&empty), literal);
    assert_eq!(counted(&empty), 19); // 1+11+1+3*2 =19, 152bits; encoding ONLY.
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let observed = execute_reported(&profile, b.input(), b.package(), &view).unwrap();
    let expected = f.bundle(Fixture::semantic(false));
    let report = observed.historical_report();
    assert_eq!(report.bundle_bytes(), encoded(&expected));
    assert_eq!(
        report.bits(),
        8 * u64::try_from(counted(&expected)).unwrap()
    );
    assert_eq!(report.semantic_bytes(), encoded(&Fixture::semantic(false)));
    assert_eq!(
        report.representation_bytes(),
        encoded(&Fixture::semantic(false))
    );
    assert_eq!(report.reconstruction_state(), ReconstructionState::Matched);
    assert_eq!(report.inverse_state(), ExecutionState::Completed);
    assert_eq!(report.input_basis(), InputBasis::Given);
    assert_eq!(
        report.native_cost_gap(),
        Some(CostGap::NativeCanonicalizationCostClosurePending)
    );
    assert!(!report.formula_evaluated());
    let s = report.stats();
    assert!(
        s.adapter_steps > 0 && s.projection_steps > 0 && s.inverse_steps > 0 && s.runtime_steps > 0
    );
    assert!(s.native_field_reads > 0 && s.dictionary_reads > 0 && s.copied_bytes > 0);
    assert_eq!(s.external_lookups, 0);
    assert_eq!(
        s.fuel_used,
        s.codec_fuel + s.adapter_fuel + s.projection_fuel + s.inverse_fuel
    );
    assert!([
        s.codec_fuel,
        s.adapter_fuel,
        s.projection_fuel,
        s.inverse_fuel
    ]
    .iter()
    .all(|n| *n <= 1_048_576));
}

#[test]
fn tlcx22_cf_n04_n05_p05_precursor_runtime_is_executed_not_decorative() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let a = Fixture::identity(false);
    let b = Fixture::identity(true);
    let aa = a.bytes();
    let bb = b.bytes();
    assert_eq!(aa.runtime.len(), bb.runtime.len()); // ONE executable Bool bit differs.
    let pa = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&aa.runtime), &view).unwrap();
    let pb = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&bb.runtime), &view).unwrap();
    let ca = execute_reported(&pa, aa.input(), aa.package(), &view).unwrap();
    let cb = execute_reported(&pb, bb.input(), bb.package(), &view).unwrap();
    assert_eq!(
        ca.historical_report().semantic_bytes(),
        encoded(&Fixture::semantic(false))
    );
    assert_eq!(
        cb.historical_report().semantic_bytes(),
        encoded(&Fixture::semantic(true))
    );
    assert_ne!(pa.digest(), pb.digest());
    assert_eq!(ca.historical_report().bits(), cb.historical_report().bits());
    assert_ne!(
        ca.historical_report().binding(),
        cb.historical_report().binding()
    );
    assert!(matches!(
        execute_reported(&pa, bb.input(), bb.package(), &view),
        Err(CostError::ProfileMismatch)
    ));
    // P05 downstream formula effect is explicitly deferred; no synthetic score here.
    let mut hidden = Fixture::identity(false);
    hidden.adapter = program("adapter", op("ReadCase", vec![arg(0)]));
    let h = hidden.bytes();
    assert!(matches!(
        execute_reported(&pa, h.input(), h.package(), &view),
        Err(CostError::InvalidProgram)
    ));
}

#[test]
fn tlcx22_cf_n01_n02_n03_given_text_missing_code_and_external_payload_cuts() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    execute_reported(&profile, b.input(), b.package(), &view).unwrap(); // Valid precondition.
    for role in 0..6 {
        let mut p = b.package();
        match role {
            0 => p.adapter = &[],
            1 => p.projection = &[],
            2 => p.inverse = &[],
            3 => p.helpers = &[],
            4 => p.runtime = &[],
            5 => p.dictionary = &[],
            _ => unreachable!(),
        }
        assert!(matches!(
            execute_reported(&profile, b.input(), p, &view),
            Err(CostError::MissingDependency)
        ));
    }
    let mut text = b.package();
    text.adapter = b"fn adapter() { true }";
    assert!(matches!(
        execute_reported(&profile, b.input(), text, &view),
        Err(CostError::InvalidCanonicalEncoding)
    ));
    let external = encoded(&app("external-v1", vec![sym("SourceStore/path/digest")]));
    let mut p = b.package();
    p.dictionary = &external;
    assert!(matches!(
        execute_reported(&profile, b.input(), p, &view),
        Err(CostError::ExternalLookupForbidden)
    ));
    let mut missing = Fixture::identity(false);
    missing.projection = program("projection", app("helper", vec![sym("absent"), arg(0)]));
    let m = missing.bytes();
    assert!(matches!(
        execute_reported(&profile, m.input(), m.package(), &view),
        Err(CostError::MissingDependency)
    ));
    // No public machine report/token is accepted by either actual owner factory.
}

#[test]
fn tlcx22_cf_n04_n08_same_count_dictionary_mutation_and_inverse_failure_keep_k() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    execute_reported(&profile, b.input(), b.package(), &view).unwrap();
    let mut mutant = Fixture::identity(false);
    let declarations = &mut fields_mut(&mut mutant.dictionary)[0];
    let first = &mut fields_mut(declarations)[0];
    fields_mut(first)[2] = sym("q"); // ONLY meaning payload; all schema/operator bytes intact.
    assert_eq!(fields_mut(declarations).len(), 1);
    let changed = mutant.bytes();
    assert_eq!(changed.declarations, b.declarations); // Forward inventory still p.
    assert_eq!(changed.dictionary.len(), b.dictionary.len());
    assert!(matches!(
        execute_reported(&profile, changed.input(), changed.package(), &view),
        Err(CostError::CorrespondenceMismatch)
    ));
    let mut dropped = Fixture::identity(false);
    fields_mut(&mut dropped.dictionary)[0] = app("declarations-v1", vec![]);
    let d = dropped.bytes();
    assert!(matches!(
        execute_reported(&profile, d.input(), d.package(), &view),
        Err(CostError::CorrespondenceMismatch)
    ));
    let mut lossy = Fixture::identity(false);
    lossy.inverse = program("inverse", literal(Node::Bool(false)));
    let l = lossy.bytes();
    let actual = execute_reported(&profile, l.input(), l.package(), &view).unwrap();
    let report = actual.historical_report();
    assert!(report.bits() > 0);
    assert_eq!(
        report.bundle_bytes(),
        encoded(&lossy.bundle(Fixture::semantic(false)))
    );
    assert_eq!(
        report.reconstruction_state(),
        ReconstructionState::Mismatched
    );
    assert_eq!(
        report.compression_eligibility(),
        CompressionEligibility::Ineligible
    );
    assert_eq!(
        report.reconstruction_bytes(),
        Some(encoded(&Node::Bool(false)).as_slice())
    );
    assert!(!report.formula_evaluated());
}

#[test]
fn tlcx22_cf_p02_n07_n22_repeated_subtree_pack_is_counted_not_assumed_gain() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::packed();
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let actual = execute_reported(&profile, b.input(), b.package(), &view).unwrap();
    let report = actual.historical_report();
    let expected_source = encoded(&app("pair-v1", vec![Node::Bool(false); 2]));
    let expected_target = app("packed-v1", vec![]);
    assert_eq!(report.semantic_bytes(), expected_source);
    assert_eq!(
        report.reconstruction_bytes(),
        Some(expected_source.as_slice())
    );
    assert_eq!(report.reconstruction_state(), ReconstructionState::Matched);
    assert_eq!(
        report.bundle_bytes(),
        encoded(&f.bundle(expected_target.clone()))
    );
    assert_eq!(
        report.bits(),
        8 * u64::try_from(counted(&f.bundle(expected_target))).unwrap()
    );
    assert!(!report.formula_evaluated()); // Complete source literal/dictionary is fully paid.
}

#[test]
fn tlcx22_cf_n06_n20_inverse_owns_only_target_bundle_and_nonsemantic_controls() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let actual = execute_reported(&profile, b.input(), b.package(), &view).unwrap();
    let bytes = actual.historical_report().bundle_bytes().to_vec();
    let before = actual.historical_report().bits();
    // No field/source/owner/plan/envelope/expected-S argument reaches inverse.
    let isolated = execute_inverse(
        TargetBundleView::decode(&bytes, &profile).unwrap(),
        FixedBootstrap::k6_tree_v1(),
        ExecutionControls::from_view(&view, 1_048_576).unwrap(),
    )
    .unwrap();
    assert_eq!(
        isolated.bytes(),
        Some(encoded(&Fixture::semantic(false)).as_slice())
    );
    assert_eq!(isolated.stats().external_lookups, 0);
    assert_eq!(before, actual.historical_report().bits());
    for name in [
        "READ_CONTROL",
        "source-lookup",
        "owner-lookup",
        "context-lookup",
        "raw-envelope",
    ] {
        let mut cut = Fixture::identity(false);
        cut.inverse = program("inverse", app(name, vec![]));
        let c = cut.bytes();
        assert!(matches!(
            execute_reported(&profile, c.input(), c.package(), &view),
            Err(CostError::InvalidProgram) | Err(CostError::ExternalLookupForbidden)
        ));
    }
}

#[test]
fn tlcx22_cf_n19_unknown_unbound_cyclic_and_unused_bad_body_are_not_admitted() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    for body in [arg(2), app("unknown-opcode", vec![])] {
        let mut cut = Fixture::identity(false);
        cut.projection = program("projection", body);
        let c = cut.bytes();
        assert!(matches!(
            execute_reported(&profile, c.input(), c.package(), &view),
            Err(CostError::InvalidProgram)
        ));
    }
    let mut cut = Fixture::identity(false);
    cut.helpers = app(
        "helpers-v1",
        vec![app(
            "function-v1",
            vec![sym("cycle"), int(0), app("helper", vec![sym("cycle")])],
        )],
    ); // Unused cyclic body still invalid.
    let c = cut.bytes();
    assert!(matches!(
        execute_reported(&profile, c.input(), c.package(), &view),
        Err(CostError::InvalidProgram)
    ));
}

#[test]
fn tlcx22_cf_n27_declared_unused_payload_is_reaccounted_as_new_artifact() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let base = Fixture::identity(false);
    let b = base.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let original = execute_reported(&profile, b.input(), b.package(), &view).unwrap();
    let extra = app(
        "function-v1",
        vec![sym("unused"), int(0), literal(sym("spare"))],
    );
    let mut with_extra = Fixture::identity(false);
    with_extra.helpers = app("helpers-v1", vec![extra.clone()]);
    let e = with_extra.bytes();
    let full = execute_reported(&profile, e.input(), e.package(), &view).unwrap();
    assert_eq!(
        full.historical_report().bits() - original.historical_report().bits(),
        8 * counted(&extra) as u64
    );
    assert_ne!(
        full.historical_report().binding(),
        original.historical_report().binding()
    );
    assert_eq!(
        full.historical_report().semantic_bytes(),
        original.historical_report().semantic_bytes()
    );
    let extra_data = sym("unused-complete-dictionary-data");
    let mut with_data = Fixture::identity(false);
    fields_mut(&mut with_data.dictionary)[2] = app("helper-data-v1", vec![extra_data.clone()]);
    let d = with_data.bytes();
    let paid = execute_reported(&profile, d.input(), d.package(), &view).unwrap();
    assert_eq!(
        paid.historical_report().bits() - original.historical_report().bits(),
        8 * counted(&extra_data) as u64
    );
}

#[test]
fn tlcx22_cf_n18_malformed_canonical_bytes_fail_before_execution() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    for malformed in [
        vec![1, 2],
        vec![3, 0x81, 0, 97],
        vec![2, 1, 1, 0],
        vec![0xff],
        vec![4, 3, 1, 102, 1],
    ] {
        assert!(matches!(
            TargetBundleView::decode(&malformed, &profile),
            Err(CostError::InvalidCanonicalEncoding)
        ));
    }
    let full = encoded(&f.bundle(Fixture::semantic(false)));
    let mut trailing = full.clone();
    trailing.push(0);
    assert!(matches!(
        TargetBundleView::decode(&trailing, &profile),
        Err(CostError::InvalidCanonicalEncoding)
    ));
    assert!(matches!(
        TargetBundleView::decode(&full[..full.len() - 1], &profile),
        Err(CostError::InvalidCanonicalEncoding)
    ));
}

#[test]
fn tlcx22_cf_n15_n25_fuel_overflow_and_limits_are_distinct() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let full = execute_reported(&profile, b.input(), b.package(), &view).unwrap();
    let inverse = execute_inverse(
        full.target_bundle(),
        FixedBootstrap::k6_tree_v1(),
        ExecutionControls::from_view(&view, 0).unwrap(),
    )
    .unwrap();
    assert_eq!(inverse.state(), ExecutionState::FuelExhausted);
    assert!(inverse.bytes().is_none());
    assert!(full.historical_report().bits() > 0);
    let mut overflow = Fixture::identity(false);
    overflow.adapter = program(
        "adapter",
        app("add", vec![literal(int(u64::MAX as i128)), literal(int(1))]),
    );
    let o = overflow.bytes();
    assert!(matches!(
        execute_reported(&profile, o.input(), o.package(), &view),
        Err(CostError::InvalidProgram)
    ));
    let mut spec = ProfileSpec::copy_pack_tree_v1(&b.runtime);
    spec.limits.bundle_bytes = 262_145;
    assert!(matches!(
        declare_cost_profile(spec, &view),
        Err(CostError::UnsupportedProfile)
    ));
    assert_eq!(CostLimits::default().semantic_bytes, 65_536);
    assert_eq!(CostLimits::default().bundle_bytes, 262_144);
    assert_eq!(CostLimits::default().additional_ram_bytes, 32 * 1024 * 1024);
}

#[test]
fn tlcx22_cf_n21_given_independent_oracle_does_not_self_certify() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let full = execute_reported(&profile, b.input(), b.package(), &view).unwrap();
    assert_eq!(
        full.historical_report().independence(),
        IndependenceStatus::SharedDependency
    );
    assert_eq!(full.historical_report().input_basis(), InputBasis::Given);
    assert_eq!(full.historical_report().baseline_bits(), None);
    assert!(!full.historical_report().formula_evaluated());
}
