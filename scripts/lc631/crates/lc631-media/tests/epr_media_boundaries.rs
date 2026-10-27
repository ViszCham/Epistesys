use lc631_media::*;
use lc631_receipt_kernel::*;

fn fixture() -> (MediaSourceAuthority, ReceiptVerifier) {
    let root = std::env::temp_dir().join(format!(
        "epr-media-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("source.bin");
    std::fs::write(&path, b"owned fixture").unwrap();
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: path.to_string_lossy().into_owned(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let issuer = ReceiptIssuer::from_key_bytes("fixture", &[73; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("fixture", &[73; 32]).unwrap();
    let candidate = authorize_media_source(&request);
    let wire = issuer
        .issue(
            ReceiptClass::MediaSource,
            SubjectRevision::checked(candidate.source_snapshot_sha256.clone().unwrap()).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(&request, &candidate).unwrap(),
            10,
            Some(20),
            None,
        )
        .unwrap();
    (
        authorize_media_source_attested(&request, wire, &verifier, &mut ReplayGuard::default(), 10)
            .unwrap(),
        verifier,
    )
}

#[test]
fn public_media_dto_changes_cannot_preserve_the_verified_token() {
    let (authority, _) = fixture();
    assert!(authority.attestation_verified());
    let mut changed = authority.clone();
    changed.local_path = Some(std::path::PathBuf::from("different-source"));
    assert!(!changed.attestation_verified());
    let mut changed = authority.clone();
    changed.source_bytes = Some(1);
    assert!(!changed.attestation_verified());
}

#[test]
fn duplicate_media_registration_is_transactional() {
    let a = BackendDescriptor::configured(BackendFamily::Decode, "A", None, None, None);
    let b = BackendDescriptor::configured(BackendFamily::Decode, "B", None, None, None);
    let mut registry = BackendRegistry::default();
    registry.register(a).unwrap();
    let before = serde_json::to_value(&registry).unwrap();
    assert!(registry.register(b).is_err());
    assert_eq!(before, serde_json::to_value(&registry).unwrap());
}

#[test]
fn media_source_use_rechecks_original_receipt_expiry_and_issuer() {
    let (authority, verifier) = fixture();
    assert!(validate_current_media_authority(&authority, &verifier, 15).is_ok());
    assert!(validate_current_media_authority(&authority, &verifier, 21).is_err());
    let other = ReceiptVerifier::from_key_bytes("fixture", &[74; 32]).unwrap();
    assert!(validate_current_media_authority(&authority, &other, 15).is_err());
}

#[test]
fn manifest_v3_matches_the_cross_runtime_golden_vector() {
    let root = std::env::temp_dir().join(format!(
        "epr-manifest-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("a")).unwrap();
    std::fs::write(root.join("a.txt"), b"alpha").unwrap();
    std::fs::write(root.join("a/file.bin"), b"beta").unwrap();
    assert_eq!(
        model_cache_manifest_sha256(&root).unwrap(),
        "sha256:6e5aac85f5366a5b882ce7db231e4dcd5a9d185c80475f18c4c515481cbe8a77"
    );
}

#[test]
fn local_locator_is_fixed_to_absolute_identity_before_attestation() {
    let relative = std::path::PathBuf::from(".cache").join(format!(
        "epr-relative-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&relative).unwrap();
    let file = relative.join("source.bin");
    std::fs::write(&file, b"owned fixture").unwrap();
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: file.to_string_lossy().into_owned(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let candidate = authorize_media_source(&request);
    assert!(candidate.local_path.as_ref().unwrap().is_absolute());
}
