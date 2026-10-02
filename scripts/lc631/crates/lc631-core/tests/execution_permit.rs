use lc631_core::stable_sha256;
use lc631_core::{
    authority_event_payload_digest, decide_execution_permits, Action, AuthorityEvent,
    AuthorityRevision, AuthoritySourceKind, CallerOrigin, DeonticPolarity, ExecutionPermit,
    ExecutionPermitContext, PermitRejection, PermitValidation, PrincipalBinding, SourceSpan,
    VerifiedAuthorityEvent,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision,
};

const SOURCE: &str = "Please edit this file.";
const SEED_RECEIPT: &str =
    "sha256:2222222222222222222222222222222222222222222222222222222222222222";

fn verified_event(polarity: DeonticPolarity) -> (VerifiedAuthorityEvent, String) {
    let key = [51_u8; 32];
    let issuer = ReceiptIssuer::from_key_bytes("host-test-root", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("host-test-root", &key).unwrap();
    let event = AuthorityEvent {
        action: Action::Edit,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity,
        span: SourceSpan {
            start: 0,
            end: SOURCE.len(),
        },
        scope: "repo/file".into(),
    };
    let subject = SubjectRevision::checked(stable_sha256(SOURCE)).unwrap();
    let scope = ReceiptScope::checked("authority/edit/repo/file").unwrap();
    let payload = authority_event_payload_digest(&event);
    let wire = issuer
        .issue(
            ReceiptClass::Authority,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            100,
            Some(200),
            Some(SEED_RECEIPT.into()),
        )
        .unwrap();
    let receipt = verifier
        .verify(
            wire,
            &ReceiptPolicy::exact(ReceiptClass::Authority, subject, scope, payload, 100)
                .with_parent(SEED_RECEIPT),
            &mut ReplayGuard::default(),
        )
        .unwrap();
    let fingerprint = verifier.key_fingerprint().to_string();
    (
        VerifiedAuthorityEvent::checked(event, receipt).unwrap(),
        fingerprint,
    )
}

fn permit(polarity: DeonticPolarity) -> (ExecutionPermit, String, PrincipalBinding) {
    let (event, fingerprint) = verified_event(polarity);
    let principal = PrincipalBinding::from_attested_seed(SEED_RECEIPT, &fingerprint).unwrap();
    let permit =
        ExecutionPermit::from_verified_user_event(event, SOURCE, AuthorityRevision(7), 100, 200, 3)
            .unwrap();
    (permit, fingerprint, principal)
}

fn context<'a>(
    principal: &'a PrincipalBinding,
    fingerprint: &'a str,
    source_revision: &'a str,
) -> ExecutionPermitContext<'a> {
    ExecutionPermitContext {
        principal,
        action: Action::Edit,
        scope: "repo/file",
        source_revision,
        authority_revision: AuthorityRevision(7),
        now_epoch: 150,
        revocation_revision: 3,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: fingerprint,
    }
}

#[test]
fn permit_is_exactly_bound_to_principal_action_scope_source_revision_and_epoch() {
    let (permit, fingerprint, principal) = permit(DeonticPolarity::Grant);
    let source_revision = stable_sha256(SOURCE);
    let context = context(&principal, &fingerprint, &source_revision);
    assert_eq!(permit.validate(&context), PermitValidation::ValidGrant);

    let wrong_scope = ExecutionPermitContext {
        scope: "repo",
        ..context
    };
    assert_eq!(
        permit.validate(&wrong_scope),
        PermitValidation::Rejected(PermitRejection::ScopeMismatch)
    );
    let wrong_source = ExecutionPermitContext {
        source_revision: "sha256:other",
        ..context
    };
    assert_eq!(
        permit.validate(&wrong_source),
        PermitValidation::Rejected(PermitRejection::SourceRevisionMismatch)
    );
    let wrong_authority = ExecutionPermitContext {
        authority_revision: AuthorityRevision(8),
        ..context
    };
    assert_eq!(
        permit.validate(&wrong_authority),
        PermitValidation::Rejected(PermitRejection::AuthorityRevisionMismatch)
    );
    let wrong_principal = PrincipalBinding::checked(
        "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    )
    .unwrap();
    let wrong_principal_context = ExecutionPermitContext {
        principal: &wrong_principal,
        ..context
    };
    assert_eq!(
        permit.validate(&wrong_principal_context),
        PermitValidation::Rejected(PermitRejection::PrincipalMismatch)
    );
    let standalone = ExecutionPermitContext {
        caller_origin: CallerOrigin::StandaloneCli,
        ..context
    };
    assert_eq!(
        permit.validate(&standalone),
        PermitValidation::Rejected(PermitRejection::UntrustedCallerOrigin)
    );
}

#[test]
fn deny_expiry_revocation_staleness_and_issuer_mismatch_fail_closed() {
    let (grant, fingerprint, principal) = permit(DeonticPolarity::Grant);
    let (deny, _, _) = permit(DeonticPolarity::Deny);
    let source_revision = stable_sha256(SOURCE);
    let base = context(&principal, &fingerprint, &source_revision);
    let decision = decide_execution_permits(&[grant.clone(), deny], &base);
    assert!(!decision.authorized);
    assert_eq!(decision.polarity, DeonticPolarity::Deny);

    let duplicate_grants = decide_execution_permits(&[grant.clone(), grant.clone()], &base);
    assert!(!duplicate_grants.authorized);
    assert_eq!(duplicate_grants.polarity, DeonticPolarity::Conflict);

    let expired = ExecutionPermitContext {
        now_epoch: 200,
        ..base
    };
    assert_eq!(
        grant.validate(&expired),
        PermitValidation::Rejected(PermitRejection::Expired)
    );
    let revoked = ExecutionPermitContext {
        revocation_revision: 4,
        ..base
    };
    assert_eq!(
        grant.validate(&revoked),
        PermitValidation::Rejected(PermitRejection::RevocationRevisionChanged)
    );
    let revoked_digest = vec![grant.digest().to_string()];
    let explicitly_revoked = ExecutionPermitContext {
        revoked_permit_digests: &revoked_digest,
        ..base
    };
    assert_eq!(
        grant.validate(&explicitly_revoked),
        PermitValidation::Rejected(PermitRejection::Revoked)
    );
    let wrong_issuer = ExecutionPermitContext {
        trusted_host_fingerprint: "sha256:wrong",
        ..base
    };
    assert_eq!(
        grant.validate(&wrong_issuer),
        PermitValidation::Rejected(PermitRejection::TrustedIssuerMismatch)
    );
    let stale_authority = ExecutionPermitContext {
        source_revision: "sha256:stale",
        ..base
    };
    assert_eq!(
        grant.validate(&stale_authority),
        PermitValidation::Rejected(PermitRejection::SourceRevisionMismatch)
    );
}

#[test]
fn standalone_cannot_mint_permits_from_mention_or_non_user_sources() {
    let (event, _) = verified_event(DeonticPolarity::Mention);
    assert!(ExecutionPermit::from_verified_user_event(
        event,
        SOURCE,
        AuthorityRevision(7),
        100,
        200,
        3,
    )
    .is_err());
}
