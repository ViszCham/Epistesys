use lc631_core::{
    authority_event_payload_digest, decide_verified_authority,
    decide_verified_authority_in_context, stable_sha256, Action, AuthorityEvent, AuthorityRevision,
    AuthoritySourceKind, CallerOrigin, DeonticPolarity, ExecutionPermit, PermitRejection,
    SourceSpan, VerifiedAuthorityContext, VerifiedAuthorityEvent,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision,
};

const SOURCE: &str = "Please edit this file.";
const PARENT: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";

fn verified_event(span: SourceSpan, parent: Option<&str>) -> VerifiedAuthorityEvent {
    event_with_polarity(span, parent, DeonticPolarity::Grant)
}

fn event_with_polarity(
    span: SourceSpan,
    parent: Option<&str>,
    polarity: DeonticPolarity,
) -> VerifiedAuthorityEvent {
    let key = [0x7b_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("epr-authority-fixture", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("epr-authority-fixture", &key).unwrap();
    let event = AuthorityEvent {
        action: Action::Edit,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity,
        span,
        scope: "repo/file".into(),
    };
    let subject = SubjectRevision::checked(stable_sha256(SOURCE)).unwrap();
    let scope = ReceiptScope::checked("authority/edit/repo/file").unwrap();
    let payload = authority_event_payload_digest(&event);
    let raw = issuer
        .issue(
            ReceiptClass::Authority,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            100,
            Some(200),
            parent.map(str::to_string),
        )
        .unwrap();
    let mut policy = ReceiptPolicy::exact(ReceiptClass::Authority, subject, scope, payload, 100);
    if let Some(parent) = parent {
        policy = policy.with_parent(parent);
    }
    let receipt = verifier
        .verify(raw, &policy, &mut ReplayGuard::default())
        .unwrap();
    VerifiedAuthorityEvent::checked(event, receipt).unwrap()
}

fn full_event() -> VerifiedAuthorityEvent {
    verified_event(
        SourceSpan {
            start: 0,
            end: SOURCE.len(),
        },
        Some(PARENT),
    )
}

#[test]
fn epr_03_different_source_cannot_rebind_a_verified_authority_event() {
    let different = "Please push this file.";
    assert_eq!(SOURCE.len(), different.len());
    let event = full_event();
    assert_eq!(
        ExecutionPermit::from_verified_user_event(
            event,
            different,
            AuthorityRevision(7),
            100,
            200,
            3
        ),
        Err(PermitRejection::SourceRevisionMismatch)
    );
}

#[test]
fn epr_03_source_mutations_are_rejected_while_the_original_remains_usable() {
    let event = full_event();
    assert!(ExecutionPermit::from_verified_user_event(
        event.clone(),
        SOURCE,
        AuthorityRevision(7),
        100,
        200,
        3
    )
    .is_ok());
    for index in 0..SOURCE.len() {
        let mut changed = SOURCE.as_bytes().to_vec();
        changed[index] = if changed[index] == b'x' { b'y' } else { b'x' };
        let changed = String::from_utf8(changed).unwrap();
        assert_eq!(
            ExecutionPermit::from_verified_user_event(
                event.clone(),
                &changed,
                AuthorityRevision(7),
                100,
                200,
                3
            ),
            Err(PermitRejection::SourceRevisionMismatch)
        );
    }
}

#[test]
fn epr_03_verified_event_retains_original_receipt_subject_across_clone() {
    let event = full_event();
    for event in [event.clone(), event] {
        let report = serde_json::to_value(&event).unwrap();
        assert_eq!(report["subject_revision"], stable_sha256(SOURCE));
        assert_eq!(report["parent_seed_receipt_digest"], PARENT);
        assert_eq!(report["receipt_issued_at_epoch"], 100);
        assert_eq!(report["receipt_expires_at_epoch"], 200);
        assert_eq!(report["issuer_principal"], "epr-authority-fixture");
    }
}

#[test]
fn epr_03_an_empty_authorizing_span_cannot_create_a_permit() {
    let event = verified_event(SourceSpan { start: 0, end: 0 }, Some(PARENT));
    assert_eq!(
        ExecutionPermit::from_verified_user_event(event, SOURCE, AuthorityRevision(7), 100, 200, 3),
        Err(PermitRejection::PermitBindingInvalid)
    );
}

#[test]
fn epr_03_context_free_authority_is_explicitly_advisory() {
    let decision = decide_verified_authority(Action::Edit, "repo/file", &[full_event()]);
    assert!(!decision.authorized);
    assert!(decision
        .residuals
        .iter()
        .any(|reason| reason.contains("needs_current_context")));
}

fn current_context(fingerprint: &str) -> VerifiedAuthorityContext<'_> {
    VerifiedAuthorityContext {
        action: Action::Edit,
        scope: "repo/file",
        source_text: SOURCE,
        parent_seed_receipt_digest: PARENT,
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: fingerprint,
        now_epoch: 150,
        revoked_authority_receipt_digests: &[],
    }
}

#[test]
fn epr_03_contextual_authority_accepts_current_exact_receipts_but_not_weaker_context() {
    let event = full_event();
    let context = current_context(event.verifier_key_fingerprint());
    let events = std::slice::from_ref(&event);
    let accepted = decide_verified_authority_in_context(&context, events);
    assert!(accepted.authorized);
    assert_eq!(accepted.polarity, DeonticPolarity::Grant);
    assert!(accepted.residuals.is_empty());

    for altered in [
        VerifiedAuthorityContext {
            source_text: "Please push this file.",
            ..context
        },
        VerifiedAuthorityContext {
            parent_seed_receipt_digest:
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            ..context
        },
        VerifiedAuthorityContext {
            trusted_host_fingerprint:
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            ..context
        },
        VerifiedAuthorityContext {
            caller_origin: CallerOrigin::StandaloneCli,
            ..context
        },
        VerifiedAuthorityContext {
            scope: "repo",
            ..context
        },
        VerifiedAuthorityContext {
            action: Action::Push,
            ..context
        },
        VerifiedAuthorityContext {
            now_epoch: 99,
            ..context
        },
        VerifiedAuthorityContext {
            now_epoch: 200,
            ..context
        },
        VerifiedAuthorityContext {
            now_epoch: 201,
            ..context
        },
    ] {
        assert!(!decide_verified_authority_in_context(&altered, events).authorized);
    }
    let revoked = vec![event.receipt_digest().to_string()];
    let revoked_context = VerifiedAuthorityContext {
        revoked_authority_receipt_digests: &revoked,
        ..context
    };
    let decision = decide_verified_authority_in_context(&revoked_context, events);
    assert!(!decision.authorized);
    assert!(decision
        .residuals
        .iter()
        .any(|reason| reason.ends_with("revoked")));
}

#[test]
fn epr_03_contextual_authority_preserves_deny_conflict_unknown_and_missing_parent() {
    let grant = full_event();
    let context = current_context(grant.verifier_key_fingerprint());
    let deny = event_with_polarity(
        SourceSpan {
            start: 0,
            end: SOURCE.len(),
        },
        Some(PARENT),
        DeonticPolarity::Deny,
    );
    let decision = decide_verified_authority_in_context(&context, &[grant.clone(), deny]);
    assert!(!decision.authorized);
    assert_eq!(decision.polarity, DeonticPolarity::Deny);
    let duplicate = decide_verified_authority_in_context(&context, &[grant.clone(), grant.clone()]);
    assert!(!duplicate.authorized);
    assert_eq!(duplicate.polarity, DeonticPolarity::Conflict);
    let absent = decide_verified_authority_in_context(&context, &[]);
    assert!(!absent.authorized);
    assert_eq!(absent.polarity, DeonticPolarity::Unknown);
    let missing_parent = verified_event(
        SourceSpan {
            start: 0,
            end: SOURCE.len(),
        },
        None,
    );
    let rejected = decide_verified_authority_in_context(&context, &[missing_parent]);
    assert!(!rejected.authorized);
    assert!(rejected
        .residuals
        .iter()
        .any(|reason| reason.ends_with("parent_mismatch")));
}
