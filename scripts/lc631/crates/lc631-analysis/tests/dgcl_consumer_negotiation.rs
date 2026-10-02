use lc631_analysis::*;

#[test]
fn old_consumer_rejects_v2_and_explicit_v1_projection_never_promotes_success() {
    let source = "Please test the package.";
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let mut candidate = build_dgcl_standalone_candidate(
        source,
        &pipeline,
        b"{\"schema_version\":\"local.v1\"}",
        "local.v1",
    )
    .unwrap();
    // Even caller-supplied report flags cannot make the legacy projection an
    // output permit. Negotiation is a format boundary, not evidence issuance.
    candidate.implementation_complete_candidate = true;
    candidate.host_send_authorized = true;
    candidate.output_commit_allowed = true;
    assert!(project_dgcl_consumer_view(
        &candidate,
        DgclConsumerVersion::LegacyV1,
        "epistesys-dgcl-consumer-view.v2"
    )
    .is_err());
    let legacy = project_dgcl_consumer_view(
        &candidate,
        DgclConsumerVersion::LegacyV1,
        "epistesys-dgcl-consumer-view.v1",
    )
    .unwrap();
    let old: LegacyDgclConsumerView = serde_json::from_value(legacy).unwrap();
    assert!(
        !old.implementation_complete_candidate
            && !old.host_send_authorized
            && !old.output_commit_allowed
    );
    let modern = project_dgcl_consumer_view(
        &candidate,
        DgclConsumerVersion::BoundedV2,
        "epistesys-dgcl-consumer-view.v2",
    )
    .unwrap();
    assert_eq!(modern["reported_implementation_complete_candidate"], true);
    assert_eq!(modern["host_send_authorized"], false);
    assert_eq!(modern["output_commit_allowed"], false);
    assert!(serde_json::from_value::<LegacyDgclConsumerView>(modern).is_err());
    let mut unknown = serde_json::to_value(old).unwrap();
    unknown["future_permission"] = serde_json::json!(true);
    assert!(serde_json::from_value::<LegacyDgclConsumerView>(unknown).is_err());
}
