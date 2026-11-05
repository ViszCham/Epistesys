//! Independent whole-bundle accounting and actual borrowed provenance checks.
use super::{expected, native::Fixture, wire};
use lc631_runtime::product_cost::{
    CompressionEligibility, CostObservation, CurrentCostObservation, EvidenceGap, ExecutionState,
    IndependenceStatus, InlinePackage, InputBasis, ReconstructionState,
};
use lc631_substrate::semantic_term::Term;

pub fn whole_bundle(
    observation: &CostObservation,
    package: InlinePackage<'_>,
    expected_semantic: &[u8],
) {
    assert!(observation.bits() > 0);
    assert_eq!(
        observation.bits(),
        expected::independent_count(observation.bundle_bytes())
    );
    let decoder = wire::app(
        "decoder-v1",
        vec![
            expected::decode(package.adapter),
            expected::decode(package.projection),
            expected::decode(package.inverse),
            expected::decode(package.helpers),
            expected::decode(package.runtime),
        ],
    );
    let complete = wire::app(
        "bundle-v1",
        vec![
            expected::decode(observation.representation_bytes()),
            decoder,
            expected::decode(package.dictionary),
        ],
    );
    // Every A/P/I/H/R and provided D entry, including unused bodies, is present.
    assert_eq!(wire::encode(&complete), observation.bundle_bytes());
    assert_eq!(observation.dictionary_bytes(), package.dictionary);
    assert_eq!(observation.semantic_bytes(), expected_semantic);
    assert_eq!(
        observation.input_basis(),
        InputBasis::ActualOwnerObservedWithGivenMeanings
    );
    assert_eq!(
        observation.independence(),
        IndependenceStatus::SharedDependency
    );
    assert!(!observation.formula_evaluated());
    assert_eq!(observation.native_cost_gap(), None);
    assert!(matches!(
        observation.compression_eligibility(),
        CompressionEligibility::Pending | CompressionEligibility::Ineligible
    ));
    for gap in [
        EvidenceGap::NativeGoalPinMissing,
        EvidenceGap::HardInvariantPinMissing,
        EvidenceGap::EvidencePinMissing,
    ] {
        assert!(observation.evidence_gaps().contains(&gap));
    }
    let stats = observation.stats();
    assert!(stats.adapter_steps > 0 && stats.projection_steps > 0 && stats.runtime_steps > 0);
    assert!(stats.native_field_reads > 0 && stats.dictionary_reads > 0 && stats.copied_bytes > 0);
    assert!(stats.codec_fuel > 0 && stats.adapter_fuel > 0 && stats.projection_fuel > 0);
    let mut total = 0u64;
    for fuel in [
        stats.codec_fuel,
        stats.adapter_fuel,
        stats.projection_fuel,
        stats.inverse_fuel,
    ] {
        assert!(fuel <= 1_048_576);
        total = total.checked_add(fuel).unwrap();
    }
    assert_eq!(total, stats.fuel_used);
    assert_eq!(stats.external_lookups, 0);
}
pub fn reconstructed(observation: &CostObservation, expected_semantic: &[u8]) {
    assert_eq!(observation.inverse_state(), ExecutionState::Completed);
    assert_eq!(
        observation.reconstruction_state(),
        ReconstructionState::Matched
    );
    assert_eq!(observation.reconstruction_bytes(), Some(expected_semantic));
    assert!(observation.stats().inverse_steps > 0);
}
pub fn borrowed(
    current: &CurrentCostObservation<'_>,
    fixture: &Fixture<'_, '_>,
    declarations: &Term,
) {
    let o = fixture.input.owners;
    assert!(std::ptr::eq(
        current.baseline_description_report(),
        o.baseline_description.report_current(fixture.view).unwrap()
    ));
    assert!(std::ptr::eq(
        current.target_description_report(),
        o.target_description.report_current(fixture.view).unwrap()
    ));
    assert!(std::ptr::eq(
        current.baseline_risk_report(),
        o.baseline_risk
            .report_current(o.baseline_responses, fixture.view)
            .unwrap()
    ));
    assert!(std::ptr::eq(
        current.target_risk_report(),
        o.target_risk
            .report_current(o.target_responses, fixture.view)
            .unwrap()
    ));
    assert!(std::ptr::eq(
        current.translation_report(),
        o.tl_reduction
            .current(o.tl_root, o.tl_candidate, fixture.view)
            .unwrap()
    ));
    assert_eq!(
        current.native_identity_admission().raw_envelope_bytes(),
        fixture.input.target.identity_envelope_bytes()
    );
    assert!(current.native_identity_admission().work() > 0);
    let _actual_adapter_revision = current.native_identity_admission().adapter_revision();
    let entries = match declarations {
        Term::Apply(name, entries) if name == "declarations-v1" => entries,
        _ => panic!("independent fixture declaration table"),
    };
    assert_eq!(
        current.cost().declarations_bytes(),
        wire::encode(declarations)
    );
    let rows = current.meaning_correspondence();
    assert_eq!(rows.len(), entries.len());
    // Full kind/path/meaning equality, not diagnostic count agreement.
    for (row, entry) in rows.iter().zip(entries) {
        let fields = match entry {
            Term::Apply(name, fields) if name == "declaration-v1" => fields,
            _ => panic!("independent declaration row"),
        };
        assert_eq!(row.kind(), text(&fields[0]));
        assert_eq!(row.role_path(), text(&fields[1]));
        let full = wire::encode(&fields[2]);
        assert_eq!(row.forward_meaning_bytes(), full);
        assert_eq!(row.dictionary_meaning_bytes(), full);
    }
}
fn text(term: &Term) -> &str {
    match term {
        Term::Symbol(s) => s.as_str(),
        _ => panic!("declaration text"),
    }
}
