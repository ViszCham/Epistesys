use lc631_receipt_kernel::{
    digest_text, PrincipalId, ReceiptClass, ReceiptError, ReceiptIssuer, ReceiptPolicy,
    ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};

#[derive(Debug)]
struct NestedReceiptContext {
    issuer: ReceiptIssuer,
    verifier: ReceiptVerifier,
}

#[test]
fn epr_02_current_receipt_lease_is_half_open_and_zero_duration_is_invalid() {
    let issuer = ReceiptIssuer::from_key_bytes("epr-lease-fixture", &[91; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("epr-lease-fixture", &[91; 32]).unwrap();
    let subject = SubjectRevision::checked("fixture-revision").unwrap();
    let scope = ReceiptScope::checked("fixture/current-use").unwrap();
    let payload = digest_text("payload");
    let receipt = issuer
        .issue(
            ReceiptClass::Authority,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            10,
            Some(20),
            None,
        )
        .unwrap();
    for now in [10, 19] {
        assert!(verifier
            .verify(
                receipt.clone(),
                &ReceiptPolicy::exact(
                    ReceiptClass::Authority,
                    subject.clone(),
                    scope.clone(),
                    payload.clone(),
                    now
                ),
                &mut ReplayGuard::default()
            )
            .is_ok());
    }
    for now in [20, 21] {
        assert_eq!(
            verifier.verify(
                receipt.clone(),
                &ReceiptPolicy::exact(
                    ReceiptClass::Authority,
                    subject.clone(),
                    scope.clone(),
                    payload.clone(),
                    now
                ),
                &mut ReplayGuard::default()
            ),
            Err(ReceiptError::Expired)
        );
    }
    assert_eq!(
        issuer.issue(
            ReceiptClass::Authority,
            subject,
            scope,
            payload,
            10,
            Some(10),
            None
        ),
        Err(ReceiptError::InvalidExpiry)
    );
}

#[test]
fn epr_02_secret_bytes_never_appear_in_nested_debug_output() {
    // This is a deliberately public fixture key, not application key material.
    let key = [0xa5_u8; 32];
    let context = NestedReceiptContext {
        issuer: ReceiptIssuer::from_key_bytes("epr-debug-fixture", &key).unwrap(),
        verifier: ReceiptVerifier::from_key_bytes("epr-debug-fixture", &key).unwrap(),
    };
    let key_decimal = format!("{:?}", key.as_slice());
    let key_hex = "a5".repeat(32);
    let key_base64 = "paWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaU=";
    for output in [
        format!("{:?}", context.issuer),
        format!("{:#?}", context.issuer),
        format!("{:?}", context.verifier),
        format!("{:#?}", context.verifier),
        format!("{context:?}"),
        format!("{context:#?}"),
    ] {
        assert!(!output.contains(&key_decimal), "decimal key bytes leaked");
        assert!(!output.contains(&key_hex), "hexadecimal key bytes leaked");
        assert!(!output.contains(key_base64), "base64 key bytes leaked");
        assert!(!output.contains("165,"), "pretty Debug key bytes leaked");
        assert!(output.contains("epr-debug-fixture"));
    }
}

#[test]
fn epr_02_checked_text_deserialization_cannot_bypass_constructors() {
    for invalid in ["", " ", "bad\0field", "bad\nfield"] {
        let wire = serde_json::to_string(invalid).unwrap();
        assert!(serde_json::from_str::<PrincipalId>(&wire).is_err());
        assert!(serde_json::from_str::<SubjectRevision>(&wire).is_err());
        assert!(serde_json::from_str::<ReceiptScope>(&wire).is_err());
    }
    assert!(
        serde_json::from_str::<PrincipalId>(&serde_json::to_string(&"p".repeat(129)).unwrap())
            .is_err()
    );
    assert!(serde_json::from_str::<SubjectRevision>(
        &serde_json::to_string(&"r".repeat(257)).unwrap()
    )
    .is_err());
    assert!(serde_json::from_str::<ReceiptScope>(
        &serde_json::to_string(&"s".repeat(513)).unwrap()
    )
    .is_err());
    let valid = "fixture/source";
    let wire = serde_json::to_string(valid).unwrap();
    assert_eq!(
        serde_json::from_str::<PrincipalId>(&wire).unwrap().as_str(),
        valid
    );
    assert_eq!(
        serde_json::from_str::<SubjectRevision>(&wire)
            .unwrap()
            .as_str(),
        valid
    );
    assert_eq!(
        serde_json::from_str::<ReceiptScope>(&wire)
            .unwrap()
            .as_str(),
        valid
    );
}
