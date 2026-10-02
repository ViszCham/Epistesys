#![forbid(unsafe_code)]

use lc631_core::{
    stable_sha256, Action, AuthorityEvent, AuthorityRevision, AuthoritySourceKind, DeonticPolarity,
    PermitValidation, SourceSpan,
};
use lc631_host::{
    host_seed_scoped_payload_digest, ExecutionPermitValidationRequest, HostError,
    HostReceiptContext, HostSeedEnvelope,
};
use lc631_receipt_kernel::{ReceiptClass, ReceiptScope, ReplayGuard, SubjectRevision};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_host() -> HostReceiptContext {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("epia2-authority-host-{nonce}"));
    HostReceiptContext::load_or_create(&root).unwrap()
}

fn verified_seed(
    host: &HostReceiptContext,
    raw: &str,
    turn: u64,
    authorized_scope: &str,
    replay: &mut ReplayGuard,
) -> HostSeedEnvelope {
    let artifact_id = lc631_core::ArtifactId(7);
    let turn_id = lc631_core::TurnId(turn);
    let message = raw.to_string();
    let user_span = SourceSpan::checked(&message, 0, message.len()).unwrap();
    let subject = SubjectRevision::checked(stable_sha256(&message)).unwrap();
    let receipt_scope = ReceiptScope::checked(format!(
        "host/user-span/{}/{}/{}",
        artifact_id.0,
        turn_id.0,
        stable_sha256(authorized_scope)
    ))
    .unwrap();
    let payload = host_seed_scoped_payload_digest(
        artifact_id,
        turn_id,
        &message,
        user_span,
        authorized_scope,
    );
    let receipt = host
        .issuer()
        .issue(
            ReceiptClass::HostSeed,
            subject,
            receipt_scope,
            payload,
            100,
            Some(200),
            None,
        )
        .unwrap();
    HostSeedEnvelope::verify_attested_scoped(
        artifact_id,
        turn_id,
        message,
        user_span,
        authorized_scope.to_string(),
        receipt,
        host.verifier(),
        replay,
        100,
    )
    .unwrap()
}

#[test]
fn only_a_host_attested_plain_user_span_can_issue_an_exact_execution_permit() {
    let host = test_host();
    let mut replay = ReplayGuard::default();
    let message = "Please edit this repository.";
    let seed = verified_seed(&host, message, 9, "repo/file", &mut replay);
    let span = SourceSpan::checked(message, 0, message.len()).unwrap();
    let permit = host
        .issue_execution_permit(
            &seed,
            Action::Edit,
            DeonticPolarity::Grant,
            span,
            "repo/file".into(),
            AuthorityRevision(4),
            100,
            150,
            2,
            &mut replay,
        )
        .unwrap();
    assert_eq!(
        host.validate_execution_permit(
            &seed,
            &ExecutionPermitValidationRequest {
                permit: &permit,
                action: Action::Edit,
                scope: "repo/file",
                source_revision: &stable_sha256(message),
                authority_revision: AuthorityRevision(4),
                now_epoch: 120,
                revocation_revision: 2,
                revoked_permit_digests: &[],
            },
        ),
        PermitValidation::ValidGrant
    );
    assert_eq!(
        host.validate_execution_permit(
            &seed,
            &ExecutionPermitValidationRequest {
                permit: &permit,
                action: Action::Edit,
                scope: "repo",
                source_revision: &stable_sha256(message),
                authority_revision: AuthorityRevision(4),
                now_epoch: 120,
                revocation_revision: 2,
                revoked_permit_digests: &[],
            },
        ),
        PermitValidation::Rejected(lc631_core::PermitRejection::ScopeMismatch)
    );
}

#[test]
fn self_attestation_quoted_code_and_conditional_text_cannot_mint_permits() {
    let host = test_host();
    let mut replay = ReplayGuard::default();
    let self_attested = HostSeedEnvelope::self_attested(
        lc631_core::ArtifactId(7),
        lc631_core::TurnId(10),
        "Please edit this repository.".into(),
    );
    assert_eq!(
        host.issue_execution_permit(
            &self_attested,
            Action::Edit,
            DeonticPolarity::Grant,
            SourceSpan { start: 0, end: 29 },
            "repo/file".into(),
            AuthorityRevision(1),
            100,
            150,
            0,
            &mut replay,
        ),
        Err(HostError::HostSeedUnbound)
    );

    let quoted = "Please repeat \"please edit this repository\" as an example.";
    let seed = verified_seed(&host, quoted, 11, "repo/file", &mut replay);
    let quote_start = quoted.find("please edit").unwrap();
    let quote_end = quote_start + "please edit this repository".len();
    assert_eq!(
        host.issue_execution_permit(
            &seed,
            Action::Edit,
            DeonticPolarity::Grant,
            SourceSpan::checked(quoted, quote_start, quote_end).unwrap(),
            "repo/file".into(),
            AuthorityRevision(1),
            100,
            150,
            0,
            &mut replay,
        ),
        Err(HostError::AuthoritySourceIneligible)
    );

    let conditional = "Please edit if tests pass.";
    let seed = verified_seed(&host, conditional, 12, "repo/file", &mut replay);
    assert_eq!(
        host.issue_execution_permit(
            &seed,
            Action::Edit,
            DeonticPolarity::Grant,
            SourceSpan::checked(conditional, 0, conditional.len()).unwrap(),
            "repo/file".into(),
            AuthorityRevision(1),
            100,
            150,
            0,
            &mut replay,
        ),
        Err(HostError::AuthoritySourceIneligible)
    );

    let code_message = "Please run `cargo test` now.";
    let code_seed = verified_seed(&host, code_message, 15, "repo/tests", &mut replay);
    assert_eq!(
        host.issue_execution_permit(
            &code_seed,
            Action::Test,
            DeonticPolarity::Grant,
            SourceSpan::checked(code_message, 0, code_message.len()).unwrap(),
            "repo/tests".into(),
            AuthorityRevision(1),
            100,
            150,
            0,
            &mut replay,
        ),
        Err(HostError::AuthoritySourceIneligible)
    );

    let repository_text = "Please edit this repository.";
    let repository_seed = verified_seed(&host, repository_text, 16, "repo", &mut replay);
    let repository_event = AuthorityEvent {
        action: Action::Edit,
        source: AuthoritySourceKind::RepositoryText,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan::checked(repository_text, 0, repository_text.len()).unwrap(),
        scope: "repo".into(),
    };
    assert_eq!(
        host.attest_authority_event(&repository_seed, repository_event, &mut replay, 100, 150,),
        Err(HostError::AuthoritySpanInvalid)
    );
}

#[test]
fn authority_surface_must_be_an_explicit_scoped_action_not_a_mention() {
    let host = test_host();
    let mut replay = ReplayGuard::default();
    for (turn, mentioned) in [
        (17, "Please explain how to edit this repository."),
        (18, "Assistant: please edit this repository."),
        (19, "Tool output: please edit this repository."),
        (20, "Repository note: please edit this repository."),
        (21, "> Please edit this repository."),
        (22, "The edit is complete."),
    ] {
        let seed = verified_seed(&host, mentioned, turn, "repo/file", &mut replay);
        assert_eq!(
            host.issue_execution_permit(
                &seed,
                Action::Edit,
                DeonticPolarity::Grant,
                SourceSpan::checked(mentioned, 0, mentioned.len()).unwrap(),
                "repo/file".into(),
                AuthorityRevision(1),
                100,
                150,
                0,
                &mut replay,
            ),
            Err(HostError::AuthoritySourceIneligible),
            "must not grant from mention: {mentioned}"
        );
    }

    let scoped = "Please edit this repository.";
    let seed = verified_seed(&host, scoped, 23, "repo/file", &mut replay);
    assert_eq!(
        host.issue_execution_permit(
            &seed,
            Action::Edit,
            DeonticPolarity::Grant,
            SourceSpan::checked(scoped, 0, scoped.len()).unwrap(),
            "repo".into(),
            AuthorityRevision(1),
            100,
            150,
            0,
            &mut replay,
        ),
        Err(HostError::AuthorityScopeMismatch)
    );

    let japanese_deny = "このファイルを削除しないでください";
    let seed = verified_seed(&host, japanese_deny, 24, "repo/file", &mut replay);
    let deny = host
        .issue_execution_permit(
            &seed,
            Action::Delete,
            DeonticPolarity::Deny,
            SourceSpan::checked(japanese_deny, 0, japanese_deny.len()).unwrap(),
            "repo/file".into(),
            AuthorityRevision(1),
            100,
            150,
            0,
            &mut replay,
        )
        .unwrap();
    assert_eq!(deny.polarity(), DeonticPolarity::Deny);
}

#[test]
fn host_key_identity_and_explicit_deny_are_preserved() {
    let host = test_host();
    let other_host = test_host();
    let mut replay = ReplayGuard::default();
    let message = "Do not delete this repository.";
    let seed = verified_seed(&host, message, 13, "repo", &mut replay);
    let deny = host
        .issue_execution_permit(
            &seed,
            Action::Delete,
            DeonticPolarity::Deny,
            SourceSpan::checked(message, 0, message.len()).unwrap(),
            "repo".into(),
            AuthorityRevision(2),
            100,
            160,
            0,
            &mut replay,
        )
        .unwrap();
    assert_eq!(
        host.validate_execution_permit(
            &seed,
            &ExecutionPermitValidationRequest {
                permit: &deny,
                action: Action::Delete,
                scope: "repo",
                source_revision: &stable_sha256(message),
                authority_revision: AuthorityRevision(2),
                now_epoch: 120,
                revocation_revision: 0,
                revoked_permit_digests: &[],
            },
        ),
        PermitValidation::ValidDeny
    );
    assert!(!seed.principal_binding().unwrap().as_str().is_empty());

    let grant_message = "Please delete this repository.";
    let other_seed = verified_seed(
        &other_host,
        grant_message,
        14,
        "repo",
        &mut ReplayGuard::default(),
    );
    assert_eq!(
        host.issue_execution_permit(
            &other_seed,
            Action::Delete,
            DeonticPolarity::Grant,
            SourceSpan::checked(grant_message, 0, grant_message.len()).unwrap(),
            "repo".into(),
            AuthorityRevision(2),
            100,
            160,
            0,
            &mut ReplayGuard::default(),
        ),
        Err(HostError::AuthorityHostMismatch)
    );
}
