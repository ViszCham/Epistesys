#![forbid(unsafe_code)]

use lc631_analysis::{
    analyze_connection_graph_sources, ConnectionGraphEdgeKind, StaticConnectionState,
};
use lc631_core::stable_sha256;

const IMPLEMENTATION: &str = "pub fn analyze_dg1() {}\n";
const ROUTER: &str = r#"
use lc631_tldg::analyze_dg1;
pub fn dispatch(command: &str) {
    match command.as_str() {
        "lc631-dg1-doctor" => { analyze_dg1(); }
        _ => {}
    }
}
"#;

fn graph(router: &str) -> lc631_analysis::StaticConnectionGraphReport {
    let source = "Please parse this source.";
    let plan = lc631_analysis::CodingConnectionPlan {
        schema_version: "epistesys-coding-connection-plan.v1".into(),
        source: source.into(),
        source_revision: stable_sha256(source),
        requirement_id: 1,
        coding_requirement_id: None,
        coding_task_id: None,
        coding_target_ref: None,
        coding_target_digest: None,
        expected_polarity: "positive".into(),
        expected_condition_kind: None,
        expected_scope: None,
        implementation_path: "crates/lc631-tldg/src/dg1.rs".into(),
        implementation_digest: stable_sha256(IMPLEMENTATION),
        implementation_symbol: "analyze_dg1".into(),
        router_path: "crates/lc631-cli/src/lib.rs".into(),
        router_digest: stable_sha256(router),
        executable_path: "target/debug/lc631.exe".into(),
        executable_digest: stable_sha256("binary"),
        command: "lc631-dg1-doctor".into(),
        arguments: vec!["--prompt".into(), source.into()],
        expected_json_pointer: "/payload/schema_version".into(),
        expected_json_value: serde_json::Value::String("epistesys-dg1-parse.v1".into()),
    };
    analyze_connection_graph_sources(&plan, IMPLEMENTATION, router).unwrap()
}

#[test]
fn direct_import_definition_and_route_arm_form_a_source_anchored_path() {
    let graph = graph(ROUTER);
    assert_eq!(graph.state, StaticConnectionState::Connected);
    assert_eq!(
        graph.requirement_target_binding_state,
        StaticConnectionState::Unresolved
    );
    assert_eq!(graph.definition_count, 1);
    assert!(graph.import_bound);
    assert_eq!(graph.route_arm_count, 1);
    assert_eq!(graph.matching_call_count, 1);
    assert_eq!(graph.route_subjects, vec!["command.as_str()"]);
    assert!(graph.route_input_bound);
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == ConnectionGraphEdgeKind::ImportBindsDefinition
            && edge.state == StaticConnectionState::Connected
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == ConnectionGraphEdgeKind::RouteDispatchesToCall
            && edge.state == StaticConnectionState::Connected
    }));
    assert!(!graph.authority_created);
    assert!(!graph.coding_closure_allowed);
}

#[test]
fn same_named_local_shadows_the_import_inside_the_exact_route_arm() {
    let router = r#"
use lc631_tldg::analyze_dg1;
pub fn dispatch(command: &str) {
    match command {
        "lc631-dg1-doctor" => { let analyze_dg1 = || {}; analyze_dg1(); }
        _ => {}
    }
}
"#;
    let graph = graph(router);
    assert_eq!(graph.state, StaticConnectionState::Unresolved);
    assert!(graph
        .residuals
        .iter()
        .any(|residual| residual.contains("local_shadow")));
}

#[test]
fn same_named_module_function_does_not_resolve_the_imported_implementation() {
    let router = r#"
use lc631_tldg::analyze_dg1;
fn analyze_dg1() {}
pub fn dispatch(command: &str) {
    match command {
        "lc631-dg1-doctor" => { analyze_dg1(); }
        _ => {}
    }
}
"#;
    let graph = graph(router);
    assert_eq!(graph.state, StaticConnectionState::Unresolved);
    assert!(graph.module_symbol_shadowed);
    assert!(graph
        .residuals
        .iter()
        .any(|residual| residual.contains("same_named_module_definition")));
}

#[test]
fn dead_route_strings_and_wrong_callees_do_not_create_connected_edges() {
    let dead_string = r#"
use lc631_tldg::analyze_dg1;
const DEAD: &str = "lc631-dg1-doctor";
pub fn dispatch() { analyze_dg1(); }
"#;
    let dead = graph(dead_string);
    assert_eq!(dead.state, StaticConnectionState::Disconnected);
    assert_eq!(dead.route_arm_count, 0);

    let wrong_callee = r#"
use lc631_tldg::analyze_dg1;
pub fn dispatch(command: &str) {
    match command {
        "lc631-dg1-doctor" => { analyze_other(); }
        _ => {}
    }
}
"#;
    let wrong = graph(wrong_callee);
    assert_eq!(wrong.state, StaticConnectionState::Disconnected);
    assert_eq!(wrong.route_arm_count, 1);
    assert_eq!(wrong.matching_call_count, 0);

    let wrong_subject = r#"
use lc631_tldg::analyze_dg1;
pub fn dispatch(command: &str, mode: &str) {
    match mode {
        "lc631-dg1-doctor" => { analyze_dg1(); }
        _ => {}
    }
}
"#;
    let mismatched_subject = graph(wrong_subject);
    assert_eq!(
        mismatched_subject.state,
        StaticConnectionState::Disconnected
    );
    assert!(!mismatched_subject.route_input_bound);
    assert_eq!(mismatched_subject.route_subjects, vec!["mode"]);
}

#[test]
fn opaque_macro_and_feature_gated_routes_remain_unresolved() {
    let macro_route = r#"
use lc631_tldg::analyze_dg1;
pub fn dispatch(command: &str) {
    match command {
        "lc631-dg1-doctor" => { route_macro!(analyze_dg1); }
        _ => {}
    }
}

"#;
    let opaque = graph(macro_route);
    assert_eq!(opaque.state, StaticConnectionState::Unresolved);
    assert!(opaque
        .residuals
        .iter()
        .any(|residual| residual.contains("opaque_macro")));

    let feature_route = r#"
use lc631_tldg::analyze_dg1;
#[cfg(feature = "diagnostics")]
pub fn dispatch(command: &str) {
    match command {
        "lc631-dg1-doctor" => { analyze_dg1(); }
        _ => {}
    }
}
"#;
    let conditional = graph(feature_route);
    assert_eq!(conditional.state, StaticConnectionState::Unresolved);
    assert!(conditional
        .residuals
        .iter()
        .any(|residual| residual.contains("configuration_unobserved")));

    let guarded_route = r#"
use lc631_tldg::analyze_dg1;
pub fn dispatch(command: &str, enabled: bool) {
    match command {
        "lc631-dg1-doctor" if enabled => { analyze_dg1(); }
        _ => {}
    }
}
"#;
    let guarded = graph(guarded_route);
    assert_eq!(guarded.state, StaticConnectionState::Unresolved);
    assert!(guarded.configuration_unobserved);
    assert!(guarded
        .residuals
        .iter()
        .any(|residual| residual.contains("route_match_guard_unobserved")));
}

#[test]
fn legacy_connection_plan_without_additive_action_binding_fields_still_deserializes() {
    let plan: lc631_analysis::CodingConnectionPlan = serde_json::from_str(include_str!(
        "../../../../../fixtures/dgcl-legacy-connection-plan-v1.json"
    ))
    .unwrap();
    assert_eq!(plan.schema_version, "epistesys-coding-connection-plan.v1");
    assert_eq!(plan.coding_requirement_id, None);
    assert_eq!(plan.coding_task_id, None);
    assert_eq!(plan.coding_target_ref, None);
    assert_eq!(plan.coding_target_digest, None);
}

#[test]
fn a10_real_call_edge_cut_and_repair_restore_the_same_source_requirement() {
    let source = "Please parse this source.";
    let pipeline = lc631_analysis::build_dgcl_pipeline(
        source,
        lc631_analysis::DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let requirement = pipeline.completion_plan.requirements[0]
        .requirement_id
        .clone();
    assert_eq!(graph(ROUTER).state, StaticConnectionState::Connected);
    let cut = ROUTER.replace("{ analyze_dg1(); }", "{ /* actual call removed */ }");
    assert_ne!(cut, ROUTER);
    let broken = graph(&cut);
    assert_eq!(broken.state, StaticConnectionState::Disconnected);
    assert_eq!(broken.matching_call_count, 0);
    let restored = graph(ROUTER);
    assert_eq!(restored.state, StaticConnectionState::Connected);
    assert!(restored.edges.iter().any(|edge| edge.kind
        == ConnectionGraphEdgeKind::RouteDispatchesToCall
        && edge.state == StaticConnectionState::Connected));
    assert_eq!(
        requirement,
        pipeline.completion_plan.requirements[0].requirement_id
    );
    assert!(
        !restored.coding_closure_allowed,
        "a static edge is not all five evidence kinds"
    );
}
