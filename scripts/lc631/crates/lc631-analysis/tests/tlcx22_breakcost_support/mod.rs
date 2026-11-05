//! Given probes and independent hand oracles, never observation factories.
#![allow(dead_code)]
use lc631_analysis::{breakcost_measurement::*, creativity_measurement::*, *};
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
use lc631_substrate::{resource::*, source::SourceStore, Digest32};
use std::{sync::Arc, time::Duration};

pub const SOURCE: &str = "Please test the package.";
pub const V1: &str = "epistesys-dgcl-consumer-view.v1";
pub const V2: &str = "epistesys-dgcl-consumer-view.v2";
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn q(n: i64, den: i64) -> Rational {
    Rational::new(n.into(), den.into())
}
pub fn source() -> SourceStore {
    SourceStore::new(Arc::from(SOURCE), 4096).unwrap()
}
pub fn binding() -> BreakCostBinding {
    let source = SourceRevision::of(SOURCE);
    BreakCostBinding {
        source,
        context: ContextRevision::original(source),
        scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
        baseline: d(5),
        hard_invariants: d(6),
        policy: d(7),
        goal_profile: Some(d(8)),
        evidence_revision: d(9),
        oracle: d(10),
        dependency_group: d(11),
    }
}
pub fn candidate(id: u64) -> CandidateBinding {
    CandidateBinding {
        id: CandidateId(id),
        context: binding().context,
        candidate_digest: d(20 + id as u8),
        target: d(30),
        transform: d(31),
    }
}
pub fn probe() -> DgclStandaloneCandidate {
    probe_with(b"{\"schema_version\":\"local.v1\"}")
}
pub fn probe_with(payload: &[u8]) -> DgclStandaloneCandidate {
    let p = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let mut raw = build_dgcl_standalone_candidate(SOURCE, &p, payload, "local.v1").unwrap();
    raw.requirement_realizations.clear();
    raw
}
pub fn case<'a>(
    id: u64,
    key: &'a [u8],
    input: &'a DgclStandaloneCandidate,
    weight: &'a Rational,
) -> LegacyCase<'a> {
    LegacyCase {
        id,
        canonical_key: key,
        input,
        weight,
    }
}
pub fn rules() -> Vec<PropertyRule> {
    vec![
        PropertyRule {
            property: LegacyProperty::JsonObjectEnvelope,
            class: PropertyClass::Optional,
            weight: q(1, 1),
        },
        PropertyRule {
            property: LegacyProperty::V1StrictConsumerAcceptance,
            class: PropertyClass::Optional,
            weight: q(3, 1),
        },
    ]
}
pub fn budget() -> BreakCostBudget {
    BreakCostBudget {
        max_cases: 8,
        max_input_bytes_per_case: 4096,
        max_input_bytes_total: 32768,
        max_candidate_text_bytes: 1024,
        max_output_bytes_per_case: 8192,
        max_output_bytes_per_run: 65536,
        max_adapter_calls_per_run: 8,
        max_serializations_per_run: 8,
        max_property_checks_per_run: 64,
        max_soft_checks_per_run: 16,
        max_extra_hard_checks: 8,
        max_weight_bits: 64,
        max_intermediate_bits: 4096,
    }
}
pub fn requests(kind: AdapterKind) -> Vec<AdapterRequest<'static>> {
    vec![AdapterRequest {
        case: 1,
        kind,
        requested_schema: match kind {
            AdapterKind::BoundedProjectionV2 => V2,
            AdapterKind::LegacyProjectionV1 | AdapterKind::DirectLegacyDtoSerializeV1 => V1,
        },
    }]
}
pub fn lease(cancel: Cancellation) -> (ResourceLeaseIssuer, ResourceLease) {
    lease_with(
        cancel,
        32 * 1024 * 1024,
        4 * 1024 * 1024,
        Deadline::after(Duration::from_secs(60)).unwrap(),
    )
}
pub fn lease_with(
    cancel: Cancellation,
    ram: u64,
    output: u64,
    deadline: Deadline,
) -> (ResourceLeaseIssuer, ResourceLease) {
    // Explicit local admission fixture, not a runtime broker or Authority receipt.
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let req = |k, n| ResourceRequest::new(k, ResourceAmount::checked(n).unwrap());
    let admitted = ReportedResourceLease::checked(
        22,
        binding().source.digest(),
        &[
            req(ResourceKind::Ram, ram),
            req(ResourceKind::Output, output),
        ],
        deadline,
        cancel,
    )
    .unwrap();
    let live = issuer.issue_admitted(1, admitted).unwrap();
    (issuer, live)
}
pub fn inputs<'a, 'v>(
    source: &'a SourceStore,
    root: &'a RootDefinition<'a, 'v>,
    cases: &'a [LegacyCase<'a>],
    baseline: &'a OwnedLegacyBaseline<'a, 'v>,
    candidate: &'a AdapterCandidate<'a, 'v>,
) -> ExpectedInputs<'a, 'v> {
    ExpectedInputs {
        source,
        root,
        cases,
        baseline,
        candidate,
    }
}
pub fn assert_cost(report: &ComponentReport, n: i64, den: i64) {
    assert_eq!(report.metric, ComponentMetric::BreakCost);
    assert_eq!(
        report.state,
        ComponentState::Measured(RationalInterval::exact_ratio(n, den).unwrap())
    );
    assert_eq!(report.basis, InputBasis::Given);
    assert_eq!(report.validation, SemanticValidation::Unassessed);
    assert_eq!(report.independence, IndependenceStatus::SharedDependency);
}
pub fn assert_no_scalar(report: &ComponentReport) {
    assert_eq!(
        report.state,
        ComponentState::Undefined(MeasurementError::PrerequisiteFailed)
    );
}
pub fn hand_v1(output: &[u8], expected: &DgclStandaloneCandidate) {
    // Literal six-field/schema/false-flag oracle, NOT the production projector/decoder.
    let value: serde_json::Value = serde_json::from_slice(output).unwrap();
    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 6);
    assert_eq!(value["schema_version"], V1);
    assert_eq!(value["source_revision"], expected.source_revision);
    assert_eq!(value["candidate_digest"], expected.candidate_digest);
    for key in [
        "implementation_complete_candidate",
        "host_send_authorized",
        "output_commit_allowed",
    ] {
        assert_eq!(value[key], false);
    }
}
