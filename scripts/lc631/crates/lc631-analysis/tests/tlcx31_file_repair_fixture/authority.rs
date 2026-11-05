//! Fixture issuers do not observe the real host or establish human independence.
use lc631_analysis::*;
use lc631_core::*;
use lc631_receipt_kernel::*;
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn file_hash(path: &Path) -> String {
    let digest = Sha256::digest(std::fs::read(path).unwrap());
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

pub fn permit(
    source: &str,
    scope: &str,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    action: Action,
) -> ExecutionPermit {
    let parent = stable_sha256("TLCX31 local test ingress, not actual host observation");
    let event = AuthorityEvent {
        action,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan {
            start: 0,
            end: source.len(),
        },
        scope: scope.into(),
    };
    let subject = SubjectRevision::checked(stable_sha256(source)).unwrap();
    let name = if action == Action::Edit {
        "edit"
    } else {
        "test"
    };
    let receipt_scope = ReceiptScope::checked(format!("authority/{name}/{scope}")).unwrap();
    let payload = authority_event_payload_digest(&event);
    let raw = issuer
        .issue(
            ReceiptClass::Authority,
            subject.clone(),
            receipt_scope.clone(),
            payload.clone(),
            100,
            Some(400),
            Some(parent.clone()),
        )
        .unwrap();
    let checked = verifier
        .verify(
            raw,
            &ReceiptPolicy::exact(
                ReceiptClass::Authority,
                subject,
                receipt_scope,
                payload,
                100,
            )
            .with_parent(&parent),
            &mut ReplayGuard::default(),
        )
        .unwrap();
    ExecutionPermit::from_verified_user_event(
        VerifiedAuthorityEvent::checked(event, checked).unwrap(),
        source,
        AuthorityRevision(1),
        100,
        400,
        0,
    )
    .unwrap()
}

pub fn registration(issuer: &ReceiptIssuer, cargo: &Path) -> UntrustedReceipt {
    let digest = file_hash(cargo);
    issuer
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(&digest).unwrap(),
            ReceiptScope::checked(format!("dgcl/registered-tool/cargo/{digest}")).unwrap(),
            dgcl_cargo_registration_payload(&cargo.to_string_lossy(), &digest),
            100,
            Some(400),
            None,
        )
        .unwrap()
}

pub fn connection_spec(root: &Path, issuer: &ReceiptIssuer) -> DgclConnectionSpec {
    let mut spec = DgclConnectionSpec {
        schema_version: "epistesys-dgcl-connection-spec.v1".into(),
        integration_target: "connection".into(),
        test_name: "production_api_connection".into(),
        entrypoint: "dgcl_producer_fixture::checked_add".into(),
        implementation_path: "src/lib.rs".into(),
        driver_path: "tests/connection.rs".into(),
        driver_digest: file_hash(&root.join("tests/connection.rs")),
        observer_id: DIRECT_API_OBSERVER.into(),
        oracle_id: "checked-add-example.v1".into(),
        oracle_revision: stable_sha256("::std::assert_eq!(observed, Some(10))"),
        coverage: DIRECT_API_COVERAGE.into(),
        registration: None,
    };
    spec.registration = Some(
        issuer
            .issue(
                ReceiptClass::VerifierCapability,
                SubjectRevision::checked(&spec.driver_digest).unwrap(),
                ReceiptScope::checked(dgcl_connection_registration_scope(&spec)).unwrap(),
                dgcl_connection_registration_payload(&spec),
                100,
                Some(400),
                None,
            )
            .unwrap(),
    );
    spec
}
