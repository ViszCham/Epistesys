use lc631_analysis::{
    analyze, run_bounded_tool, AnalysisRequest, BoundedToolRequest, ClosureState, EvidenceState,
    ToolRunState,
};

#[test]
fn arc631_17_tool_result_binds_executable_cwd_environment_and_network_boundary() {
    let result = run_bounded_tool(BoundedToolRequest {
        program: "rustc",
        args: &["--version"],
        stdin: None,
        timeout_ms: 5_000,
        stdout_limit: 4_096,
        stderr_limit: 4_096,
    });
    assert_eq!(result.state, ToolRunState::Observed);
    assert!(result.resolved_executable.is_some());
    assert!(result
        .executable_sha256
        .as_deref()
        .is_some_and(|digest| digest.len() == 71));
    assert_eq!(result.cwd_digest.len(), 71);
    assert_eq!(result.environment_digest.len(), 71);
    assert!(!result.network_isolation_os_enforced);
    assert!(!result.hermetic_execution_observed);
}

#[test]
fn arc631_18_observed_with_blockers_is_not_closed() {
    let report = analyze(AnalysisRequest {
        prompt: "audit",
        repo: None,
        validation: None,
        release: None,
        source_name: None,
        source: None,
    });
    let stage = report.stage("RPA-00").unwrap();
    assert_eq!(stage.state, EvidenceState::Observed);
    assert!(!stage.blockers.is_empty());
    assert_eq!(stage.closure_state, ClosureState::Candidate);
}
