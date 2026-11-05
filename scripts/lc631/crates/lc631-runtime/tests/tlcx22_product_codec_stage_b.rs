//! Stage B negative-first codec/API contract. No product production body yet.
//! Hand-authored schema bytes are independent of all product encode functions.
//! Reported syntax and Given byte accounting NEVER complete current Product K.
#[path = "tlcx22_product_codec_stage_b_support/wire.rs"] mod wire;
#[path = "tlcx22_product_codec_stage_b_support/models.rs"] mod models;
#[path = "tlcx22_product_codec_stage_b_support/cost_contract.rs"] mod cost_contract;

use lc631_runtime::{
    adversarial_measurement::{Response, SemanticAtom},
    product_correspondence::{CaseDeclaration, DeferredReason, IdentityKind, ProductError, ProductPlan, TlProfile},
    resource::*,
    spatial_preservation::SpatialUnit,
};
use lc631_solvers::{
    causal_spatial::{FrameId, Rectangle},
    temporal::{TemporalNode, TemporalProfile, TemporalQuery, Truth},
    typed_query::{BoolExpr, BoolSymbolId},
};
use models::Fixture;
use std::time::Duration;
use wire::{app, encoded, field_mut, sym, Node};

fn resources(f: &Fixture, ram: u64) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker.begin_run(222, f.context.source().digest(), cap,
        Deadline::after(Duration::from_secs(30)).unwrap(), Cancellation::default()).unwrap();
    let reservation = run.reserve(&[
        ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(ram).unwrap()),
        ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(4 * 1024 * 1024).unwrap()),
    ]).unwrap();
    (broker, run, reservation)
}

#[test]
fn tlcx22_stage_b_independent_k6_literal_vectors() {
    assert_eq!(encoded(&Node::Bool(false)), [1, 0]);
    assert_eq!(encoded(&sym("a")), [3, 1, b'a']);
    assert_eq!(encoded(&Node::Int(0)), [2, 0, 1, 0]);
    assert_eq!(encoded(&app("f", vec![Node::Bool(false)])), [4, 3, 1, b'f', 1, 1, 0]);
}

#[test]
fn tlcx22_stage_b_full_product_hand_golden_and_permanent_cost_gap() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 32 * 1024 * 1024);
    let view = reservation.lease();
    let declarations = f.declarations();
    let plan = ProductPlan::declared(declarations.spec(&f), &view).unwrap();
    // No baseline/candidate Response is an input to declaration. Slots, labels,
    // weights and reference contracts are frozen before response attachment.
    assert_eq!(plan.fixed_slots().len(), 2);
    assert_eq!(plan.fixed_slots()[0].native_key(), b"z-empty");
    assert_eq!(plan.fixed_slots()[1].native_key(), b"a-atom");
    assert_eq!(plan.fixed_slots()[0].weight(), &f.weights[0]);
    assert_eq!(plan.fixed_slots()[1].weight(), &f.weights[1]);
    assert_eq!(plan.fixed_slots()[0].reference_contract(), f.references[0]);
    assert_eq!(plan.fixed_slots()[1].oracle(), models::d(20));
    let responses = f.responses();
    let product = plan.project_reported(f.input(&responses), &view).unwrap();
    let expected = encoded(&models::golden(0));
    assert_eq!(product.semantic_bytes(), expected);
    assert_eq!(product.scalar_status(), DeferredReason::FullCostOwnerMissing);
    assert!(!product.formula_evaluated());
    let roundtrip = plan.decode_reported(&expected, product.identity_envelope_bytes(), &view).unwrap();
    assert_eq!(roundtrip.semantic_bytes(), expected);
    assert_eq!(roundtrip.scalar_status(), DeferredReason::FullCostOwnerMissing);
}

#[test]
fn tlcx22_stage_b_global_external_id_rename_and_native_array_permutation_are_invariant() {
    let a = Fixture::new(0);
    let mut b = Fixture::new(100);
    b.atom_response = BoolExpr::Variable(BoolSymbolId::new(83));
    b.atom[0].symbol = BoolSymbolId::new(83);
    let (_broker, _run, reservation) = resources(&a, 32 * 1024 * 1024);
    let view = reservation.lease();
    let da = a.declarations();
    let mut db = b.declarations();
    db.cases.swap(0, 1);
    db.identities.reverse();
    let pa = ProductPlan::declared(da.spec(&a), &view).unwrap();
    let pb = ProductPlan::declared(db.spec(&b), &view).unwrap();
    let ra = a.responses();
    let mut rb = b.responses(); rb.reverse();
    let aa = pa.project_reported(a.input(&ra), &view).unwrap();
    let bb = pb.project_reported(b.input(&rb), &view).unwrap();
    assert_eq!(aa.semantic_bytes(), encoded(&models::golden(0)));
    assert_eq!(aa.semantic_bytes(), bb.semantic_bytes());
    assert_ne!(aa.identity_envelope_bytes(), bb.identity_envelope_bytes());
    assert!(matches!(pa.decode_reported(bb.semantic_bytes(), bb.identity_envelope_bytes(), &view), Err(ProductError::IdentityMismatch)));
}

#[test]
fn tlcx22_stage_b_shared_plan_roles_keep_distinct_equal_meaning_clock_identity() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 32 * 1024 * 1024);
    let view = reservation.lease();
    let declarations = f.declarations();
    let plan = ProductPlan::declared(declarations.spec(&f), &view).unwrap();
    let responses = f.responses();
    let base = plan.project_reported(f.input(&responses), &view).unwrap();
    let other_trace = models::trace(f.secondary_clock, &[Truth::Unknown, Truth::True]);
    let mut other = f.input(&responses); other.temporal.trace = &other_trace;
    let split = plan.project_reported(other, &view).unwrap();
    assert_eq!(split.semantic_bytes(), encoded(&models::golden(1)));
    assert_ne!(base.semantic_bytes(), split.semantic_bytes());
    // Both artifacts use ONE predeclared role table. Independent per-artifact
    // first occurrence would wrongly renumber the second clock to role zero.
    assert_eq!(split.scalar_status(), DeferredReason::FullCostOwnerMissing);
}

#[test]
fn tlcx22_stage_b_role_alias_merge_split_sparse_and_arbitrary_relabel_are_rejected() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 32 * 1024 * 1024);
    let view = reservation.lease();
    let mut sparse = f.declarations(); sparse.identities[0].local_role = 99;
    assert!(matches!(ProductPlan::declared(sparse.spec(&f), &view), Err(ProductError::IdentityMismatch)));
    let mut permuted = f.declarations();
    permuted.identities[0].local_role = 1; permuted.identities[4].local_role = 0;
    assert!(matches!(ProductPlan::declared(permuted.spec(&f), &view), Err(ProductError::IdentityMismatch)));
    let mut merged = f.declarations(); merged.identities[4].local_role = 0;
    assert!(matches!(ProductPlan::declared(merged.spec(&f), &view), Err(ProductError::IdentityMismatch)));
    let mut split = f.declarations(); split.identities[4].native_id = split.identities[0].native_id;
    assert!(matches!(ProductPlan::declared(split.spec(&f), &view), Err(ProductError::IdentityMismatch)));
}

#[test]
fn tlcx22_stage_b_ambiguous_frozen_case_specs_reject_before_any_responses() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 32 * 1024 * 1024);
    let view = reservation.lease();
    let mut declarations = f.declarations();
    declarations.cases[0].semantic_spec = &f.specs[0];
    declarations.cases[0].native_atoms = &[];
    declarations.cases[0].reference_contract = &f.references[0];
    // Different opaque keys, references, or weights cannot invent semantic
    // distinction for a duplicate frozen spec. No Response parameter exists.
    assert_ne!(declarations.cases[0].native_key, declarations.cases[1].native_key);
    let result = ProductPlan::declared(declarations.spec(&f), &view);
    assert!(
        matches!(&result, Err(ProductError::AmbiguousCaseSpec)),
        "actual declaration result: {:?}",
        result.as_ref().map(|_| "ProductPlan")
    );
}

#[test]
fn tlcx22_stage_b_response_swaps_change_payload_without_reassigning_slots_or_denominator() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 32 * 1024 * 1024);
    let view = reservation.lease();
    let declarations = f.declarations();
    let plan = ProductPlan::declared(declarations.spec(&f), &view).unwrap();
    let yes = BoolExpr::Constant(true); let no = BoolExpr::Constant(false);
    let before = [Response { case: b"a-atom", term: &yes }, Response { case: b"z-empty", term: &no }];
    let after = [Response { case: b"z-empty", term: &yes }, Response { case: b"a-atom", term: &no }];
    let a = plan.project_reported(f.input(&before), &view).unwrap();
    let b = plan.project_reported(f.input(&after), &view).unwrap();
    let mut expected_a = models::golden(0);
    let cases = field_mut(&mut expected_a, "tlcx-finite-product-v1", 0);
    *field_mut(field_mut(cases, "response-cases-v1", 1), "case-slot-v1", 2) = Node::Bool(true);
    assert_eq!(a.semantic_bytes(), encoded(&expected_a));
    let mut expected_b = models::golden(0);
    let cases = field_mut(&mut expected_b, "tlcx-finite-product-v1", 0);
    *field_mut(field_mut(cases, "response-cases-v1", 0), "case-slot-v1", 2) = Node::Bool(true);
    *field_mut(field_mut(cases, "response-cases-v1", 1), "case-slot-v1", 2) = Node::Bool(false);
    assert_eq!(b.semantic_bytes(), encoded(&expected_b));
    assert_ne!(a.semantic_bytes(), b.semantic_bytes());
    for (slot, index) in plan.fixed_slots().iter().zip([0, 1]) {
        assert_eq!(slot.weight(), &f.weights[index]);
        assert_eq!(slot.reference_contract(), f.references[index]);
        assert_eq!(slot.semantic_spec(), f.specs[index]);
    }
    assert_eq!(plan.fixed_slots()[0].weight() + plan.fixed_slots()[1].weight(), lc631_core::measurement_numeric::Rational::from_integer(5.into()));
}

#[test]
fn tlcx22_stage_b_missing_meaning_unknown_ids_and_noncanonical_bytes_stay_errors() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 32 * 1024 * 1024);
    let view = reservation.lease();
    let mut missing = f.declarations(); missing.identities.retain(|d| d.kind != IdentityKind::Frame);
    assert!(matches!(ProductPlan::declared(missing.spec(&f), &view), Err(ProductError::MissingMeaningDeclaration)));
    let declarations = f.declarations();
    let plan = ProductPlan::declared(declarations.spec(&f), &view).unwrap();
    let responses = f.responses();
    let mut unknown = f.input(&responses);
    let frame = FrameId([99; 32]);
    unknown.spatial.frame = frame;
    unknown.spatial.left = Some(Rectangle::new(frame, [0, 0], [1, 1]).unwrap());
    unknown.spatial.right = Some(Rectangle::new(frame, [2, 0], [3, 1]).unwrap());
    assert!(matches!(plan.project_reported(unknown, &view), Err(ProductError::MissingMeaningDeclaration)));
    let valid = plan.project_reported(f.input(&responses), &view).unwrap();
    let mut trailing = encoded(&models::golden(0)); trailing.push(0);
    assert!(matches!(plan.decode_reported(&trailing, valid.identity_envelope_bytes(), &view), Err(ProductError::InvalidCanonicalEncoding)));
}

#[test]
fn tlcx22_stage_b_declared_subset_caps_and_required_tl_axes_are_not_relaxed() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 32 * 1024 * 1024);
    let view = reservation.lease();
    let declarations = f.declarations();
    let plan = ProductPlan::declared(declarations.spec(&f), &view).unwrap();
    let responses = f.responses();
    let long_trace = models::trace(f.clock, &[Truth::True; 9]);
    let mut input = f.input(&responses); input.temporal.trace = &long_trace;
    assert!(matches!(plan.project_reported(input, &view), Err(ProductError::UnsupportedProjection)));
    let mut nodes = vec![TemporalNode::Atom(0)];
    for i in 0..32 { nodes.push(TemporalNode::Not(i)); }
    let query = TemporalQuery::checked(f.context, nodes, 32, TemporalProfile::ClosedFiniteKleeneV1).unwrap();
    let mut input = f.input(&responses); input.temporal.query = &query;
    assert!(matches!(plan.project_reported(input, &view), Err(ProductError::UnsupportedProjection)));
    let mut input = f.input(&responses); input.spatial.unit = SpatialUnit::Millimeter;
    assert!(matches!(plan.project_reported(input, &view), Err(ProductError::UnsupportedProjection)));
    let mut spec = declarations.spec(&f); spec.tl_profile = TlProfile::OtherRequiredAxes;
    assert!(matches!(ProductPlan::declared(spec, &view), Err(ProductError::UnsupportedProjection)));
    for (nodes, root) in [
        (vec![TemporalNode::Atom(0), TemporalNode::Constant(true)], 0),
        (vec![TemporalNode::Atom(0), TemporalNode::And(0, 0)], 1),
    ] {
        let query = TemporalQuery::checked(f.context, nodes, root, TemporalProfile::ClosedFiniteKleeneV1).unwrap();
        let mut input = f.input(&responses); input.temporal.query = &query;
        assert!(matches!(plan.project_reported(input, &view), Err(ProductError::UnsupportedProjection)));
    }
    let mut third_spec = models::case_spec(false);
    *field_mut(&mut third_spec, "stage-b-case-spec-v1", 2) = app("theory-v1", vec![Node::Bool(true)]);
    let third_bytes = encoded(&third_spec);
    let cases = [
        CaseDeclaration { native_key: b"z-empty", native_atoms: &[], semantic_spec: &f.specs[0], reference_contract: &f.references[0], oracle: models::d(21), weight: &f.weights[0] },
        CaseDeclaration { native_key: b"a-atom", native_atoms: &f.atom, semantic_spec: &f.specs[1], reference_contract: &f.references[1], oracle: models::d(20), weight: &f.weights[1] },
        CaseDeclaration { native_key: b"third", native_atoms: &[], semantic_spec: &third_bytes, reference_contract: &f.references[0], oracle: models::d(22), weight: &f.weights[0] },
    ];
    let mut spec = declarations.spec(&f); spec.cases = &cases;
    assert!(matches!(ProductPlan::declared(spec, &view), Err(ProductError::UnsupportedProjection)));
    let meanings: [&[u8]; 5] = [b"p", b"q", b"r", b"s", b"t"];
    let atoms: Vec<_> = meanings.iter().enumerate().map(|(i, meaning)| SemanticAtom {
        symbol: BoolSymbolId::new(17 + i as u32), meaning,
    }).collect();
    let mut five = models::case_spec(false);
    *field_mut(&mut five, "stage-b-case-spec-v1", 1) = app("atoms-v1", vec![sym("p"), sym("q"), sym("r"), sym("s"), sym("t")]);
    let five = encoded(&five);
    let cases = [CaseDeclaration { native_key: b"five", native_atoms: &atoms, semantic_spec: &five,
        reference_contract: &f.references[0], oracle: models::d(21), weight: &f.weights[0] }];
    let mut spec = declarations.spec(&f); spec.cases = &cases;
    assert!(matches!(ProductPlan::declared(spec, &view), Err(ProductError::UnsupportedProjection)));
}

#[test]
fn tlcx22_stage_b_resource_exhaustion_is_not_unsupported_or_valid_empty_product() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 1);
    let view = reservation.lease();
    let declarations = f.declarations();
    assert!(matches!(ProductPlan::declared(declarations.spec(&f), &view), Err(ProductError::Resource(_))));
}

#[test]
fn tlcx22_stage_b_full_provided_code_dictionary_accounting_is_given_and_deferred() {
    let f = Fixture::new(0);
    let (_broker, _run, reservation) = resources(&f, 32 * 1024 * 1024);
    let view = reservation.lease();
    let declarations = f.declarations();
    let plan = ProductPlan::declared(declarations.spec(&f), &view).unwrap();
    cost_contract::exercise(&plan, &view);
}
