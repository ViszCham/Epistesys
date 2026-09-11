use lc631_core::{
    authority_event_payload_digest, closure_witness_payload_digest, decide_authority,
    decide_verified_authority, Action, AuthorityEvent, AuthoritySourceKind, ClosureLedger,
    ClosureLevel, ClosureWitness, DeonticPolarity, ReceiptOwner, SourceSpan,
    VerifiedAuthorityEvent, VerifiedClosureWitness,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision,
};

fn verified(
    class: ReceiptClass,
    payload: String,
    scope: &str,
) -> lc631_receipt_kernel::VerifiedReceipt {
    let key = [31_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("lc631-test-root", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("lc631-test-root", &key).unwrap();
    let subject = SubjectRevision::checked("sha256:subject-r1").unwrap();
    let scope = ReceiptScope::checked(scope).unwrap();
    let wire = issuer
        .issue(
            class,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            1,
            None,
            None,
        )
        .unwrap();
    verifier
        .verify(
            wire,
            &ReceiptPolicy::exact(class, subject, scope, payload, 1),
            &mut ReplayGuard::default(),
        )
        .unwrap()
}

#[test]
fn arc631_03_public_verified_host_enum_cannot_grant() {
    let event = AuthorityEvent {
        action: Action::Edit,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan { start: 0, end: 4 },
        scope: "repo".into(),
    };
    assert!(!decide_authority(Action::Edit, "repo", &[event]).authorized);
}

#[test]
fn arc631_03_only_attested_event_can_grant_exact_scope() {
    let event = AuthorityEvent {
        action: Action::Edit,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan { start: 0, end: 4 },
        scope: "repo".into(),
    };
    let receipt = verified(
        ReceiptClass::Authority,
        authority_event_payload_digest(&event),
        "authority/edit/repo",
    );
    let verified = VerifiedAuthorityEvent::checked(event, receipt).unwrap();
    assert!(decide_verified_authority(Action::Edit, "repo", &[verified]).authorized);
}

#[test]
fn arc631_04_public_host_bound_witness_does_not_close_ledger() {
    let witness = ClosureWitness {
        component: "host-output".into(),
        level: ClosureLevel::HostBound,
        owner: ReceiptOwner::HostBridge,
        revision_digest: "sha256:revision".into(),
        blockers: vec![],
    };
    let mut ledger = ClosureLedger::default();
    assert!(ledger.insert(witness.clone()).is_err());
    let receipt = verified(
        ReceiptClass::Closure,
        closure_witness_payload_digest(&witness),
        "closure/host-output",
    );
    ledger
        .insert_verified(VerifiedClosureWitness::checked(witness, receipt).unwrap())
        .unwrap();
    assert!(ledger.host_bound());
}
