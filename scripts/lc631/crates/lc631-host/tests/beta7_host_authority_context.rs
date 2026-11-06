//! Scoped local issuance, external evidence, and fixture-only key paths are separate.
#![forbid(unsafe_code)]

use lc631_core::{
    authority_event_payload_digest, stable_sha256, Action, ArtifactId, AuthorityEvent,
    AuthorityRevision, AuthoritySourceKind, DeonticPolarity, ExecutionPermit, PermitValidation,
    SourceSpan, TurnId, VerifiedAuthorityEvent,
};
use lc631_host::{
    host_seed_scoped_payload_digest, ExecutionPermitValidationRequest, HostError,
    HostExecutionPermitAdmission, HostReceiptContext, HostSeedEnvelope,
};
use lc631_process_supervisor::test_fixtures::bounded_test_lease;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReplayGuard, SubjectRevision, UntrustedReceipt,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const ARTIFACT: ArtifactId = ArtifactId(31);
const TURN: TurnId = TurnId(8);
const SCOPE: &str = "fixture/tests";
const KEY_NAME: &str = "receipt-root-v1.key";

fn outer() -> PathBuf {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "beta7-host-context-{}-{stamp}-{}", std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    root
}

fn synthetic_key(path: &Path) {
    // Public fixture bytes; never read an installed key or print key contents.
    fs::write(path, "c7".repeat(32)).unwrap();
}

fn local_host() -> HostReceiptContext {
    let root = outer();
    synthetic_key(&root.join(KEY_NAME));
    HostReceiptContext::load_existing(&root).unwrap()
}

fn whole(raw: &str) -> SourceSpan {
    SourceSpan::checked(raw, 0, raw.len()).unwrap()
}

fn part(raw: &str, excerpt: &str) -> SourceSpan {
    let start = raw.find(excerpt).unwrap();
    SourceSpan::checked(raw, start, start + excerpt.len()).unwrap()
}

fn seed(host: &HostReceiptContext, raw: &str, attested: SourceSpan) -> HostSeedEnvelope {
    let wire = host.issuer().issue(
        ReceiptClass::HostSeed,
        SubjectRevision::checked(stable_sha256(raw)).unwrap(),
        ReceiptScope::checked(format!("host/user-span/31/8/{}", stable_sha256(SCOPE))).unwrap(),
        host_seed_scoped_payload_digest(ARTIFACT, TURN, raw, attested, SCOPE),
        100, Some(200), None,
    ).unwrap();
    HostSeedEnvelope::verify_attested_scoped(
        ARTIFACT, TURN, raw.into(), attested, SCOPE.into(), wire, host.verifier(),
        &mut ReplayGuard::default(), 100,
    ).unwrap()
}

fn issue(
    host: &HostReceiptContext, seed: &HostSeedEnvelope, requested: SourceSpan,
    scope: &str, now: u64, expires: u64,
) -> Result<ExecutionPermit, HostError> {
    host.issue_execution_permit(
        seed, Action::Test, DeonticPolarity::Grant, requested, scope.into(), AuthorityRevision(2),
        now, expires, 0, &mut ReplayGuard::default(),
    )
}

fn assert_valid(host: &HostReceiptContext, seed: &HostSeedEnvelope, permit: &ExecutionPermit) {
    let source = stable_sha256(&seed.raw_user_message);
    assert_eq!(host.validate_execution_permit(seed, &ExecutionPermitValidationRequest {
        permit, action: Action::Test, scope: SCOPE, source_revision: &source,
        authority_revision: AuthorityRevision(2), now_epoch: 130,
        revocation_revision: 0, revoked_permit_digests: &[],
    }), PermitValidation::ValidGrant);
}

fn event(selected: SourceSpan) -> AuthorityEvent {
    AuthorityEvent {
        action: Action::Test, source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant, span: selected, scope: SCOPE.into(),
    }
}

fn signed_authority(
    host: &HostReceiptContext, seed: &HostSeedEnvelope, event: &AuthorityEvent,
    parent: Option<String>, expiry: u64,
) -> UntrustedReceipt {
    host.issuer().issue(
        ReceiptClass::Authority, SubjectRevision::checked(stable_sha256(&seed.raw_user_message)).unwrap(),
        ReceiptScope::checked("authority/test/fixture/tests").unwrap(),
        authority_event_payload_digest(event), 110, Some(expiry), parent,
    ).unwrap()
}

fn admit(
    host: &HostReceiptContext, seed: &HostSeedEnvelope, selected: SourceSpan,
    signed: UntrustedReceipt, replay: &mut ReplayGuard,
) -> Result<ExecutionPermit, HostError> {
    host.admit_execution_permit(seed, HostExecutionPermitAdmission {
        action: Action::Test, scope: SCOPE.into(), span: selected, authority_revision: AuthorityRevision(2),
        revocation_revision: 0, now_epoch: 120, attestation: signed,
    }, replay)
}

const EXCLUSIONS: [(&str, &str); 5] = [
    ("Do not test the project.", "test the project"),
    ("If approved, please test the project.", "please test the project"),
    ("Please test the project only if approved.", "Please test the project"),
    ("The note reads ‘please test the project’.", "please test the project"),
    ("例として「テストしてください」と入力します。", "テストしてください"),
];

#[test]
fn beta7_host_local_issuance_checks_full_raw_clause_even_with_clipped_attested_span() {
    let host = local_host();
    let mut outcomes = Vec::new();
    for (raw, excerpt) in EXCLUSIONS {
        let selected = part(raw, excerpt);
        for attested in [whole(raw), selected] {
            let bound = seed(&host, raw, attested);
            outcomes.push((raw, attested, issue(&host, &bound, selected, SCOPE, 120, 150)));
        }
    }
    for (raw, attested, result) in outcomes {
        assert_eq!(result, Err(HostError::AuthoritySourceIneligible), "{raw}, attested={attested:?}");
    }
}

#[test]
fn beta7_host_ja_only_imperative_issues_and_validates_an_exact_scoped_grant() {
    let host = local_host();
    let raw = "テストしてください";
    let bound = seed(&host, raw, whole(raw));
    let permit = issue(&host, &bound, whole(raw), SCOPE, 120, 150).unwrap();
    assert_eq!(permit.source_span(), whole(raw));
    assert_eq!(permit.expires_at_epoch(), 150);
    assert_valid(&host, &bound, &permit);
}

#[test]
fn beta7_host_prior_unrelated_denial_does_not_mask_an_independent_instruction() {
    let host = local_host();
    let raw = "Do not edit the project. Please test the project.";
    let selected = part(raw, "Please test the project.");
    for attested in [whole(raw), selected] {
        let bound = seed(&host, raw, attested);
        let permit = issue(&host, &bound, selected, SCOPE, 120, 150).unwrap();
        assert_eq!(permit.source_span(), selected);
        assert_valid(&host, &bound, &permit);
    }
}

#[test]
fn beta7_host_external_signed_event_cannot_strip_the_same_negation_condition_or_quote() {
    let host = local_host();
    let mut outcomes = Vec::new();
    for (raw, excerpt) in EXCLUSIONS {
        let selected = part(raw, excerpt);
        for attested in [whole(raw), selected] {
            let bound = seed(&host, raw, attested);
            let wire = signed_authority(&host, &bound, &event(selected), bound.host_receipt_digest.clone(), 150);
            let mut replay = ReplayGuard::default();
            let result = admit(&host, &bound, selected, wire, &mut replay);
            outcomes.push((raw, attested, result, replay.observed_count()));
        }
    }
    for (raw, attested, result, consumed) in outcomes {
        assert_eq!(result, Err(HostError::AuthoritySourceIneligible), "{raw}, attested={attested:?}");
        assert_eq!(consumed, 0, "ineligible source must reject before receipt consumption");
    }
}

#[test]
fn beta7_host_neutral_prose_requires_external_authority_and_accepts_its_exact_binding() {
    let host = local_host();
    let raw = "The verification stage is ready.";
    let bound = seed(&host, raw, whole(raw));
    assert_eq!(issue(&host, &bound, whole(raw), SCOPE, 120, 150), Err(HostError::AuthoritySourceIneligible));

    let declared = event(whole(raw));
    let wire = signed_authority(&host, &bound, &declared, bound.host_receipt_digest.clone(), 150);
    let mut replay = ReplayGuard::default();
    let permit = admit(&host, &bound, whole(raw), wire.clone(), &mut replay).unwrap();
    assert_valid(&host, &bound, &permit);
    assert_eq!(permit.source_span(), whole(raw));
    assert_eq!(admit(&host, &bound, whole(raw), wire, &mut replay), Err(HostError::ReceiptReplay));
    assert_eq!(replay.observed_count(), 1);

    let wrong = signed_authority(&host, &bound, &declared, Some(stable_sha256("another parent")), 150);
    assert!(admit(&host, &bound, whole(raw), wrong, &mut ReplayGuard::default()).is_err());
    let expired = signed_authority(&host, &bound, &declared, bound.host_receipt_digest.clone(), 120);
    assert!(admit(&host, &bound, whole(raw), expired, &mut ReplayGuard::default()).is_err());
}

#[test]
fn beta7_host_modified_scope_expiry_and_self_attestation_keep_existing_rejections() {
    let host = local_host();
    let raw = "Please test the project.";
    let selected = whole(raw);
    let bound = seed(&host, raw, selected);
    assert_eq!(issue(&host, &bound, selected, "fixture", 120, 150), Err(HostError::AuthorityScopeMismatch));
    assert_eq!(issue(&host, &bound, selected, SCOPE, 200, 205), Err(HostError::HostSeedUnbound));
    assert_eq!(issue(&host, &bound, selected, SCOPE, 120, 201), Err(HostError::AuthorityPermitRejected));
    let self_report = HostSeedEnvelope::self_attested(ARTIFACT, TURN, raw.into());
    assert_eq!(issue(&host, &self_report, selected, SCOPE, 120, 150), Err(HostError::HostSeedUnbound));
}

#[test]
fn beta7_host_validator_rechecks_contradicted_context_on_a_preexisting_signed_permit() {
    let host = local_host();
    let raw = "Do not test the project.";
    let selected = part(raw, "test the project");
    let bound = seed(&host, raw, selected);
    let declared = event(selected);
    let parent = bound.host_receipt_digest.as_deref().unwrap();
    let receipt = signed_authority(&host, &bound, &declared, Some(parent.into()), 150);
    let verified = host.verifier().verify(receipt, &ReceiptPolicy::exact(
        ReceiptClass::Authority, SubjectRevision::checked(stable_sha256(raw)).unwrap(),
        ReceiptScope::checked("authority/test/fixture/tests").unwrap(),
        authority_event_payload_digest(&declared), 120,
    ).with_parent(parent), &mut ReplayGuard::default()).unwrap();
    let already_issued = ExecutionPermit::from_verified_user_event(
        VerifiedAuthorityEvent::checked(declared, verified).unwrap(), raw, AuthorityRevision(2),
        120, 150, 0,
    ).unwrap();
    let revision = stable_sha256(raw);
    let validation = host.validate_execution_permit(&bound, &ExecutionPermitValidationRequest {
        permit: &already_issued, action: Action::Test, scope: SCOPE, source_revision: &revision,
        authority_revision: AuthorityRevision(2), now_epoch: 130, revocation_revision: 0,
        revoked_permit_digests: &[],
    });
    assert!(matches!(validation, PermitValidation::Rejected(_)), "{validation:?}");
}

#[derive(Clone, Copy, Debug)]
enum KeyOpen {
    Existing,
    Create,
    ExistingOwned,
    CreateOwned,
}

fn key_open(mode: KeyOpen, root: &Path) -> Result<HostReceiptContext, HostError> {
    match mode {
        KeyOpen::Existing => HostReceiptContext::load_existing(root),
        KeyOpen::Create => HostReceiptContext::load_or_create(root),
        KeyOpen::ExistingOwned | KeyOpen::CreateOwned => {
            let lease = bounded_test_lease(81).unwrap();
            let result = match mode {
                KeyOpen::ExistingOwned => HostReceiptContext::load_existing_with_resources(root, &lease.view()),
                KeyOpen::CreateOwned => HostReceiptContext::load_or_create_with_resources(root, &lease.view()),
                _ => unreachable!(),
            };
            lease.close_confirmed().unwrap();
            result
        }
    }
}
const KEY_ENTRIES: [KeyOpen; 4] = [
    KeyOpen::ExistingOwned, KeyOpen::CreateOwned, KeyOpen::Existing, KeyOpen::Create,
];

#[test]
fn beta7_host_existing_and_creating_key_entries_reject_a_second_hardlink_name() {
    let mut failures = Vec::new();
    for mode in KEY_ENTRIES {
        let p = outer();
        let root = p.join("store");
        fs::create_dir(&root).unwrap();
        let key = root.join(KEY_NAME);
        synthetic_key(&key);
        assert!(key_open(mode, &root).is_ok(), "{mode:?} ordinary synthetic control failed");
        fs::hard_link(&key, p.join("fixture-alias")).unwrap();
        if key_open(mode, &root).is_ok() {
            failures.push(mode);
        }
    }
    assert!(failures.is_empty(), "entries accepted a hardlinked key: {failures:?}");
}

#[cfg(unix)]
fn alias_parent(p: &Path) {
    std::os::unix::fs::symlink(p.join("sibling"), p.join("alias")).unwrap();
}

#[cfg(windows)]
fn alias_parent(p: &Path) {
    use std::os::windows::process::CommandExt;
    use std::path::{Component, Prefix};

    let root = p.canonicalize().unwrap();
    let link = root.join("alias");
    let sibling = root.join("sibling");
    assert_eq!(link.parent().unwrap().canonicalize().unwrap(), link.parent().unwrap());
    assert_eq!(sibling.canonicalize().unwrap(), sibling);
    assert!(matches!(fs::symlink_metadata(&link), Err(e) if e.kind() == std::io::ErrorKind::NotFound));
    let operand = |path: &Path| {
        assert!(path.is_absolute() && path.starts_with(&root));
        assert!(matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))));
        let absolute = path.to_str().expect("fixture path must be Unicode");
        let dos = absolute.strip_prefix(r"\\?\").unwrap_or(absolute);
        assert!(!dos.contains(['[', ']', '*', '?', '\x60']), "fixture path contains a wildcard");
        dos.to_owned()
    };
    let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    // The command is fixed. Both absolute operands are data, independent of the
    // interpreter's current directory, and never parsed as PowerShell source.
    let status = std::process::Command::new(powershell)
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg("$ErrorActionPreference='Stop'; $null = Microsoft.PowerShell.Management\\New-Item -ItemType Junction -Path ([Environment]::GetEnvironmentVariable('BETA7_JUNCTION_LINK')) -Target ([Environment]::GetEnvironmentVariable('BETA7_JUNCTION_SIBLING'))")
        .env("BETA7_JUNCTION_LINK", operand(&link))
        .env("BETA7_JUNCTION_SIBLING", operand(&sibling))
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status().unwrap();
    assert!(status.success(), "test fixture could not create its directory junction");
}

#[cfg(any(unix, windows))]
#[test]
fn beta7_host_key_parent_alias_cannot_load_or_create_in_the_sibling() {
    let mut failures = Vec::new();
    for mode in KEY_ENTRIES {
        let p = outer();
        let sibling = p.join("sibling");
        fs::create_dir(&sibling).unwrap();
        alias_parent(&p);
        assert_eq!(p.join("alias").canonicalize().unwrap(), sibling.canonicalize().unwrap());
        let store = sibling.join("keys");
        let existing = matches!(mode, KeyOpen::Existing | KeyOpen::ExistingOwned);
        if existing {
            fs::create_dir(&store).unwrap();
            synthetic_key(&store.join(KEY_NAME));
            assert!(key_open(mode, &store).is_ok(), "{mode:?} ordinary control failed");
        }
        let accepted = key_open(mode, &p.join("alias/keys")).is_ok();
        let named_entries = if store.exists() {
            fs::read_dir(&store).unwrap().map(|e| e.unwrap().file_name()).collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let changed = if existing {
            named_entries.len() != 1 || !store.join(KEY_NAME).is_file()
        } else {
            store.exists()
        };
        if accepted || changed {
            failures.push((mode, accepted, changed));
        }
    }
    assert!(failures.is_empty(), "alias was accepted or mutated its sibling: {failures:?}");
}

#[test]
fn beta7_host_linked_governing_text_cannot_disappear_before_context_veto() {
    let host = local_host();
    let mut outcomes = Vec::new();
    for (raw, excerpt) in [
        ("[Do not](https://example.invalid) test the project.", "test the project"),
        (
            "Please test the project [only if approved](https://example.invalid).",
            "Please test the project",
        ),
    ] {
        let selected = part(raw, excerpt);
        for attested in [whole(raw), selected] {
            let bound = seed(&host, raw, attested);
            let mut local_replay = ReplayGuard::default();
            let local = host.issue_execution_permit(
                &bound, Action::Test, DeonticPolarity::Grant, selected, SCOPE.into(),
                AuthorityRevision(2), 120, 150, 0, &mut local_replay,
            );
            let wire = signed_authority(
                &host, &bound, &event(selected), bound.host_receipt_digest.clone(), 150,
            );
            let mut external_replay = ReplayGuard::default();
            let external = admit(&host, &bound, selected, wire, &mut external_replay);
            outcomes.push((
                raw, attested, local, local_replay.observed_count(),
                external, external_replay.observed_count(),
            ));
        }
    }
    for (raw, attested, local, local_consumed, external, external_consumed) in outcomes {
        assert_eq!(local, Err(HostError::AuthoritySourceIneligible), "local: {raw}, attested={attested:?}");
        assert_eq!(local_consumed, 0, "local context rejection consumed a receipt: {raw}");
        assert_eq!(external, Err(HostError::AuthoritySourceIneligible), "external: {raw}, attested={attested:?}");
        assert_eq!(external_consumed, 0, "external context rejection consumed a receipt: {raw}");
    }
}

#[test]
fn beta7_host_common_abbreviation_cannot_end_a_governing_clause() {
    let host = local_host();
    let mut outcomes = Vec::new();
    for raw in [
        "Do not treat this example as authorization, e.g. please test the project.",
        "If approved, e.g. please test the project.",
    ] {
        let selected = part(raw, "please test the project");
        for attested in [whole(raw), selected] {
            let bound = seed(&host, raw, attested);
            let mut local_replay = ReplayGuard::default();
            let local = host.issue_execution_permit(
                &bound, Action::Test, DeonticPolarity::Grant, selected, SCOPE.into(),
                AuthorityRevision(2), 120, 150, 0, &mut local_replay,
            );
            let wire = signed_authority(
                &host, &bound, &event(selected), bound.host_receipt_digest.clone(), 150,
            );
            let mut external_replay = ReplayGuard::default();
            let external = admit(&host, &bound, selected, wire, &mut external_replay);
            outcomes.push((
                raw, attested, local, local_replay.observed_count(),
                external, external_replay.observed_count(),
            ));
        }
    }
    for (raw, attested, local, local_consumed, external, external_consumed) in outcomes {
        assert_eq!(local, Err(HostError::AuthoritySourceIneligible), "local: {raw}, attested={attested:?}");
        assert_eq!(local_consumed, 0, "local context rejection consumed a receipt: {raw}");
        assert_eq!(external, Err(HostError::AuthoritySourceIneligible), "external: {raw}, attested={attested:?}");
        assert_eq!(external_consumed, 0, "external context rejection consumed a receipt: {raw}");
    }
}

#[test]
fn beta7_host_excluded_prior_sample_and_path_components_preserve_valid_directives() {
    let host = local_host();
    let mut outcomes = Vec::new();
    for (raw, excerpt) in [
        (
            "```text\nDo not test this sample\n```\nPlease test the project.",
            "Please test the project.",
        ),
        ("Please test not_this.rs.", "test not_this.rs"),
        ("Please test not/file.", "test not/file"),
    ] {
        let selected = part(raw, excerpt);
        for attested in [whole(raw), selected] {
            let bound = seed(&host, raw, attested);
            let mut local_replay = ReplayGuard::default();
            let local = host.issue_execution_permit(
                &bound, Action::Test, DeonticPolarity::Grant, selected, SCOPE.into(),
                AuthorityRevision(2), 120, 150, 0, &mut local_replay,
            );
            let wire = signed_authority(
                &host, &bound, &event(selected), bound.host_receipt_digest.clone(), 150,
            );
            let mut external_replay = ReplayGuard::default();
            let external = admit(&host, &bound, selected, wire, &mut external_replay);
            outcomes.push((
                raw, selected, attested, bound, local, local_replay.observed_count(),
                external, external_replay.observed_count(),
            ));
        }
    }
    for (raw, selected, attested, bound, local, local_consumed, external, external_consumed) in outcomes {
        let local = local.unwrap_or_else(|error| panic!("local: {raw}, attested={attested:?}: {error:?}"));
        let external = external.unwrap_or_else(|error| panic!("external: {raw}, attested={attested:?}: {error:?}"));
        assert_eq!(local_consumed, 1, "local valid directive did not consume its receipt: {raw}");
        assert_eq!(external_consumed, 1, "external valid directive did not consume its receipt: {raw}");
        for permit in [&local, &external] {
            assert_eq!(permit.source_span(), selected, "{raw}, attested={attested:?}");
            assert_eq!(permit.expires_at_epoch(), 150);
            assert_valid(&host, &bound, permit);
        }
    }
}
