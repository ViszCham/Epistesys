use super::*;

const DRIVER: &str = "#[test]\nfn production_api_connection() {\n    let observed = example::entry(3, 7);\n    ::std::assert_eq!(observed, Some(10));\n}\n";

fn spec() -> DgclConnectionSpec {
    DgclConnectionSpec {
        schema_version: "epistesys-dgcl-connection-spec.v1".into(),
        integration_target: "connection".into(),
        test_name: "production_api_connection".into(),
        entrypoint: "example::entry".into(),
        implementation_path: "src/lib.rs".into(),
        driver_path: "tests/connection.rs".into(),
        driver_digest: stable_sha256(DRIVER),
        observer_id: DIRECT_API_OBSERVER.into(),
        oracle_id: "independent-test-oracle.v1".into(),
        oracle_revision: stable_sha256("::std::assert_eq!(observed, Some(10))"),
        coverage: DIRECT_API_COVERAGE.into(),
        registration: None,
    }
}

#[test]
fn epr_08_direct_driver_is_source_anchored_and_macro_hidden_calls_are_rejected() {
    assert!(validate_driver(DRIVER, &spec()).is_ok());
    for modified in [
        DRIVER.replace(
            "let observed = example::entry(3, 7);",
            "let observed = Some(10);",
        ),
        DRIVER.replace(
            "let observed = example::entry(3, 7);",
            "let observed = || example::entry(3, 7);",
        ),
        DRIVER.replace(
            "let observed = example::entry(3, 7);",
            "let observed = example::entry(panic!(), 7);",
        ),
        DRIVER.replace(
            "let observed = example::entry(3, 7);",
            "return; let observed = example::entry(3, 7);",
        ),
        DRIVER.replace("#[test]", "#[test]\n#[ignore]"),
        DRIVER.replace("Some(10)", "Some(11)"),
        format!("mod example {{}}\n{DRIVER}"),
    ] {
        assert!(
            validate_driver(&modified, &spec()).is_err(),
            "must reject {modified}"
        );
    }
}

#[test]
fn epr_08_exact_named_test_is_necessary_but_not_a_standalone_authority() {
    let success = b"running 1 test\ntest production_api_connection ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";
    assert!(connection_test_observed(success, &spec()));
    assert!(!connection_test_observed(
        b"test unrelated ... ok\ntest result: ok. 1 passed; 0 failed;",
        &spec()
    ));
    assert!(!connection_test_observed(
        b"test result: ok. 1 passed; 0 failed;",
        &spec()
    ));
    assert!(!connection_test_observed(
        b"test production_api_connection ... ignored\ntest result: ok. 0 passed; 0 failed;",
        &spec()
    ));
    assert!(!check_cargo_test_result(
        CodingEvidenceKind::ProductionConnection,
        success
    ));
}

#[test]
fn epr_08_connection_registration_rejects_missing_tampered_and_expired_metadata() {
    use lc631_receipt_kernel::ReceiptIssuer;
    let key = [42; 32];
    let issuer = ReceiptIssuer::from_key_bytes("registered-driver-test-only", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("registered-driver-test-only", &key).unwrap();
    let mut driver = spec();
    assert!(verify_connection_registration(&driver, &verifier, 10).is_err());
    driver.registration = Some(
        issuer
            .issue(
                ReceiptClass::VerifierCapability,
                SubjectRevision::checked(&driver.driver_digest).unwrap(),
                ReceiptScope::checked(dgcl_connection_registration_scope(&driver)).unwrap(),
                dgcl_connection_registration_payload(&driver),
                10,
                Some(20),
                None,
            )
            .unwrap(),
    );
    assert_eq!(
        verify_connection_registration(&driver, &verifier, 10),
        Ok(20)
    );
    let mut changed = driver.clone();
    changed.entrypoint = "example::unrelated".into();
    assert!(verify_connection_registration(&changed, &verifier, 10).is_err());
    assert!(verify_connection_registration(&driver, &verifier, 21).is_err());
}

#[test]
fn epr_17_full_metadata_rejects_std_names_aliases_and_unresolved_graphs() {
    let metadata = serde_json::json!({
        "packages": [{
            "targets": [{"name": "example"}],
            "dependencies": [{"name": "safe-dependency", "rename": null}]
        }],
        "resolve": {"nodes": [{"deps": [{"name": "safe_dependency"}]}]}
    });
    assert!(unshadowed_standard_library(&metadata));
    for changed in [
        {
            let mut value = metadata.clone();
            value["resolve"] = serde_json::Value::Null;
            value
        },
        {
            let mut value = metadata.clone();
            value["packages"][0]["targets"][0]["name"] = "std".into();
            value
        },
        {
            let mut value = metadata.clone();
            value["packages"][0]["dependencies"][0]["name"] = "std".into();
            value
        },
        {
            let mut value = metadata.clone();
            value["packages"][0]["dependencies"][0]["rename"] = "std".into();
            value
        },
        {
            let mut value = metadata.clone();
            value["resolve"]["nodes"][0]["deps"][0]["name"] = "std".into();
            value
        },
    ] {
        assert!(!unshadowed_standard_library(&changed));
    }
}
