//! Natural machine budget/capability boundary cases, independent of owner joins.
use crate::{
    fixture::{program, Fixture},
    resources,
    wire::{app, arg, child, counted, fields_mut, int, literal, sym, Node},
};
use lc631_runtime::product_cost::{declare_cost_profile, execute_reported, CostError, ProfileSpec};

fn padded_to(total: usize) -> Fixture {
    let mut f = Fixture::identity(false);
    let mut lo = 0usize;
    let mut hi = total;
    // Independent size arithmetic on authored trees, never a production encoder.
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        fields_mut(&mut f.dictionary)[2] = app("helper-data-v1", vec![sym(&"x".repeat(mid))]);
        if counted(&f.bundle(Fixture::semantic(false))) < total {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    fields_mut(&mut f.dictionary)[2] = app("helper-data-v1", vec![sym(&"x".repeat(lo))]);
    assert_eq!(counted(&f.bundle(Fixture::semantic(false))), total);
    f
}

#[test]
fn tlcx22_cf_n15_complete_bundle_exact_cap_and_one_byte_over() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let at = padded_to(262_144);
    let bytes = at.bytes();
    let profile =
        declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&bytes.runtime), &view).unwrap();
    let valid = execute_reported(&profile, bytes.input(), bytes.package(), &view).unwrap();
    assert_eq!(valid.historical_report().bundle_bytes().len(), 262_144);
    assert_eq!(valid.historical_report().bits(), 2_097_152);
    let over = padded_to(262_145);
    let bad = over.bytes();
    assert!(matches!(
        execute_reported(&profile, bad.input(), bad.package(), &view),
        Err(CostError::ResourceExhausted)
    ));
}

#[test]
fn tlcx22_cf_n15_declared_instruction_entry_depth_caps_precede_execution() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    for cap in 0..5 {
        let mut spec = ProfileSpec::copy_pack_tree_v1(&b.runtime);
        match cap {
            0 => spec.limits.instructions = 1,
            1 => spec.limits.local_entries = 1,
            2 => spec.limits.depth = 1,
            3 => spec.limits.total_nodes = 1,
            4 => spec.limits.value_cells = 1,
            _ => unreachable!(),
        }
        match declare_cost_profile(spec, &view) {
            Err(CostError::ResourceExhausted) => (),
            Ok(profile) => assert!(matches!(
                execute_reported(&profile, b.input(), b.package(), &view),
                Err(CostError::ResourceExhausted)
            )),
            Err(other) => panic!("expected admitted resource-cap failure: {other:?}"),
        }
    }
}

#[test]
fn tlcx22_cf_n15_explicit_additional_32mib_is_reserved_before_execution() {
    use lc631_runtime::resource::{ResourceError, ResourceKind};
    let (_broker, _run, reservation) = resources::run_with_ram(31 * 1024 * 1024);
    let view = reservation.lease();
    let f = Fixture::identity(false);
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    assert!(matches!(
        execute_reported(&profile, b.input(), b.package(), &view),
        Err(CostError::Resource(ResourceError::Exhausted(
            ResourceKind::Ram
        )))
    ));
}

#[test]
fn tlcx22_cf_n05_n06_n20_opaque_tokens_cannot_be_ranked_unwrapped_or_forged() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let base = Fixture::identity(false);
    let b = base.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let opaque = app("opaque-v1", vec![sym("Clock"), int(9)]);
    let forbidden = [
        app("less", vec![child(child(arg(0), 0), 1), literal(int(10))]),
        app("canonical-less", vec![arg(0), arg(0)]),
        app("equal", vec![child(arg(0), 0), literal(opaque.clone())]),
        app(
            "make",
            vec![
                literal(sym("opaque-v1")),
                app("sequence", vec![literal(sym("Clock")), literal(int(9))]),
            ],
        ),
    ];
    for body in forbidden {
        let mut f = Fixture::identity(false);
        f.fields = app("reported-fields-v1", vec![opaque.clone()]);
        f.adapter = program("adapter", body);
        let bytes = f.bytes();
        assert!(matches!(
            execute_reported(&profile, bytes.input(), bytes.package(), &view),
            Err(CostError::InvalidProgram)
        ));
    }
    // Two Given input tokens of one kind may be compared for equality only.
    let mut equal = Fixture::identity(false);
    equal.fields = app("reported-fields-v1", vec![opaque.clone(), opaque]);
    equal.adapter = program(
        "adapter",
        app("equal", vec![child(arg(0), 0), child(arg(0), 1)]),
    );
    let e = equal.bytes();
    let observed = execute_reported(&profile, e.input(), e.package(), &view).unwrap();
    assert_eq!(
        observed.historical_report().semantic_bytes(),
        crate::wire::encoded(&Node::Bool(true))
    );
}

#[test]
fn tlcx22_cf_n05_canonical_product_is_not_a_raw_field_stream() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let mut f = Fixture::identity(false);
    f.fields = app("tlcx-finite-product-v1", vec![Node::Bool(false); 3]);
    f.adapter = program("adapter", arg(0));
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    assert!(matches!(
        execute_reported(&profile, b.input(), b.package(), &view),
        Err(CostError::InvalidProgram)
    ));
}

#[test]
fn tlcx22_cf_n15_fold_bound_dynamic_index_and_unsigned_underflow() {
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let base = Fixture::identity(false);
    let b = base.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let mut dynamic = Fixture::identity(false);
    dynamic.adapter = program(
        "adapter",
        app(
            "child",
            vec![arg(0), app("add", vec![literal(int(0)), literal(int(0))])],
        ),
    );
    let d = dynamic.bytes();
    let ok = execute_reported(&profile, d.input(), d.package(), &view).unwrap();
    assert_eq!(
        ok.historical_report().semantic_bytes(),
        crate::wire::encoded(&Node::Bool(false))
    );
    for body in [
        app("subtract", vec![literal(int(0)), literal(int(1))]),
        app(
            "fold",
            vec![
                int(1025),
                literal(app("seq", vec![])),
                literal(Node::Bool(false)),
                arg(0),
            ],
        ),
    ] {
        let mut f = Fixture::identity(false);
        f.adapter = program("adapter", body);
        let x = f.bytes();
        assert!(matches!(
            execute_reported(&profile, x.input(), x.package(), &view),
            Err(CostError::InvalidProgram)
        ));
    }
}

#[test]
fn tlcx22_cf_n15_literal_inverse_hand_fuel_eight_succeeds_seven_fails() {
    use lc631_runtime::product_cost::{
        execute_inverse, ExecutionControls, ExecutionState, FixedBootstrap, TargetBundleView,
    };
    let (_broker, _run, reservation) = resources::run();
    let view = reservation.lease();
    let mut f = Fixture::identity(false);
    f.inverse = program("inverse", literal(Node::Bool(false)));
    let b = f.bytes();
    let profile = declare_cost_profile(ProfileSpec::copy_pack_tree_v1(&b.runtime), &view).unwrap();
    let owned = execute_reported(&profile, b.input(), b.package(), &view).unwrap();
    let bytes = owned.historical_report().bundle_bytes().to_vec();
    // I only: entry1+Expr1+copy(1node+2bytes)+encode(1node+2bytes)=8.
    let exact = execute_inverse(
        TargetBundleView::decode(&bytes, &profile).unwrap(),
        FixedBootstrap::k6_tree_v1(),
        ExecutionControls::from_view(&view, 8).unwrap(),
    )
    .unwrap();
    assert_eq!(exact.state(), ExecutionState::Completed);
    assert_eq!(exact.bytes(), Some([1, 0].as_slice()));
    assert_eq!(exact.stats().inverse_fuel, 8);
    let short = execute_inverse(
        TargetBundleView::decode(&bytes, &profile).unwrap(),
        FixedBootstrap::k6_tree_v1(),
        ExecutionControls::from_view(&view, 7).unwrap(),
    )
    .unwrap();
    assert_eq!(short.state(), ExecutionState::FuelExhausted);
    assert!(short.bytes().is_none());
    assert!(owned.historical_report().bits() > 0); // Valid complete cost survives failed new I execution.
}
