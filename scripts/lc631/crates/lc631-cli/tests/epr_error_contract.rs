#[test]
fn a_cli_failure_never_claims_unobserved_absence_of_effects() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lc631"))
        .arg("not-a-command")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_ne!(
        value["claim_boundary"],
        "diagnostic failure; no mutation or fallback success was performed"
    );
    assert!(value["effect_state"]["source_effect"].is_string());
}
