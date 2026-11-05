use lc631_analysis::{
    CheckpointBody, CheckpointEventKind, CheckpointIntegrityState, CheckpointReadIdentity,
    DeliveryState,
};
use lc631_cli::binary_checkpoint_resume::{
    inspect_existing_binary_checkpoint_root, inspect_persisted_binary_checkpoint_resume,
    parse_binary_checkpoint_resume_inspect_args, report_binary_checkpoint_resume_inspection,
    BinaryCheckpointResumeDecision, BinaryCheckpointResumeHold,
    BinaryCheckpointResumeInspectArgsError,
};
use lc631_core::stable_sha256;
use lc631_host::persistent_owner::{
    HostPersistentStoreOwner, HostPersistentStoreReader, PersistentStoreLimits,
};
use lc631_runtime::journal::{
    decode_segment, CheckpointChainResumeHold, JournalChainLimits, JournalRecordKind,
    JournalSegmentBuilder,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
    ResourceLimits, ResourceRequest, ResourceReservationView, RunResources,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TEST_ROOT_PREFIX: &str = "epistesys-tlcx07-cli-resume-";
const SEGMENT_LIMIT: u64 = 4 * 1024 * 1024;

struct ScratchRoot {
    path: PathBuf,
    parent: PathBuf,
}

impl ScratchRoot {
    fn create() -> Self {
        let parent = std::env::temp_dir()
            .canonicalize()
            .expect("system temporary directory is accessible");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows the Unix epoch")
            .as_nanos();
        for attempt in 0..32_u32 {
            let path = parent.join(format!(
                "{TEST_ROOT_PREFIX}{}-{}",
                std::process::id(),
                nonce.saturating_add(u128::from(attempt))
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path, parent },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test-owned root create failed: {error}"),
            }
        }
        panic!("could not allocate a unique test-owned root")
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchRoot {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if !metadata.is_dir() || is_reparse(&metadata) {
            return;
        }
        let Ok(canonical) = self.path.canonicalize() else {
            return;
        };
        if canonical.parent() != Some(self.parent.as_path())
            || !canonical
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(TEST_ROOT_PREFIX))
        {
            return;
        }
        let Ok(entries) = fs::read_dir(&canonical) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name != ".resource-owner-v1.lock"
                && name != ".writer.lock"
                && !name.starts_with("segment-")
            {
                return;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                return;
            };
            if !metadata.is_file() || is_reparse(&metadata) || fs::remove_file(&path).is_err() {
                return;
            }
        }
        let _ = fs::remove_dir(canonical);
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn reservation() -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(96 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([0x71; 32]),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            request(ResourceKind::Disk, 32 * 1024 * 1024),
            request(ResourceKind::Ram, 32 * 1024 * 1024),
            request(ResourceKind::Output, 32 * 1024 * 1024),
        ])
        .unwrap();
    (broker, run, reservation)
}

fn request(kind: ResourceKind, bytes: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(bytes).unwrap())
}

fn write_segment<'a>(
    root: &Path,
    sequence: u64,
    bytes: &[u8],
    view: &'a ResourceReservationView<'a>,
) {
    assert!(!bytes.is_empty());
    assert!(bytes.len() as u64 <= SEGMENT_LIMIT);
    let target = root.join(format!("segment-{sequence:020}.bin"));
    let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
        root,
        &target,
        bytes.len() as u64,
        PersistentStoreLimits::default(),
        view,
    )
    .expect("test-owned segment write is admitted");
    owner
        .write_binary_checkpoint_segment(bytes)
        .expect("test-owned segment handoff is observed");
}

fn identity<'a>(source: &'a str, plan: &'a str, authority: &'a str) -> CheckpointReadIdentity<'a> {
    CheckpointReadIdentity {
        source_revision: source,
        plan_digest: plan,
        authority_revision: authority,
    }
}

fn checkpoint_record(
    sequence: u64,
    source: &str,
    plan: &str,
    authority: &str,
    parent_digest: Option<String>,
) -> (Vec<u8>, String) {
    let body = CheckpointBody {
        schema_version: "epistesys-dgcl-checkpoint.v1".to_owned(),
        sequence,
        source_revision: source.to_owned(),
        plan_digest: plan.to_owned(),
        authority_revision: authority.to_owned(),
        action_key: format!("action-{sequence}"),
        event_kind: CheckpointEventKind::LegacyUnclassified,
        delivery: DeliveryState::ConfirmedSuccess,
        result_digest: Some(stable_sha256("observed-result")),
        parent_digest,
    };
    let canonical_body = serde_json::to_string(&body).unwrap();
    let digest = stable_sha256(&canonical_body);
    let stored = serde_json::json!({ "body": body, "digest": digest });
    (serde_json::to_vec(&stored).unwrap(), digest)
}

fn checkpoint_segments(source: &str, plan: &str, authority: &str) -> (Vec<u8>, Vec<u8>) {
    let (first_payload, first_digest) = checkpoint_record(0, source, plan, authority, None);
    let (second_payload, _) = checkpoint_record(1, source, plan, authority, Some(first_digest));

    let mut first = JournalSegmentBuilder::genesis();
    first
        .append_at(1, JournalRecordKind::Checkpoint, digest(1), &first_payload)
        .unwrap();
    let first_segment = first.finish().unwrap();
    let first_cursor = decode_segment(&first_segment).unwrap().cursor();

    let mut second = JournalSegmentBuilder::after(first_cursor);
    second
        .append_at(2, JournalRecordKind::Checkpoint, digest(2), &second_payload)
        .unwrap();
    (first_segment, second.finish().unwrap())
}

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn reader<'a>(root: &Path, view: &'a ResourceReservationView<'a>) -> HostPersistentStoreReader<'a> {
    HostPersistentStoreReader::open_binary_checkpoint_root(
        root,
        PersistentStoreLimits::default(),
        view,
    )
    .expect("existing test-owned checkpoint root is readable")
}

fn arg_values(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn tlcx07_cli_inspect_requires_existing_root_argument() {
    let args = arg_values(&[
        "--source-revision",
        "sha256:source",
        "--plan-digest",
        "sha256:plan",
        "--authority-revision",
        "authority-r1",
    ]);
    assert_eq!(
        parse_binary_checkpoint_resume_inspect_args(&args),
        Err(BinaryCheckpointResumeInspectArgsError::MissingLedgerRoot)
    );
}

#[test]
fn tlcx07_cli_inspect_requires_each_identity_component() {
    let cases = [
        (
            &[][..],
            BinaryCheckpointResumeInspectArgsError::MissingLedgerRoot,
        ),
        (
            &["--ledger-root", "existing-root"][..],
            BinaryCheckpointResumeInspectArgsError::MissingSourceRevision,
        ),
        (
            &[
                "--ledger-root",
                "existing-root",
                "--source-revision",
                "source-r1",
            ][..],
            BinaryCheckpointResumeInspectArgsError::MissingPlanDigest,
        ),
        (
            &[
                "--ledger-root",
                "existing-root",
                "--source-revision",
                "source-r1",
                "--plan-digest",
                "plan-r1",
            ][..],
            BinaryCheckpointResumeInspectArgsError::MissingAuthorityRevision,
        ),
    ];
    for (raw, expected) in cases {
        assert_eq!(
            parse_binary_checkpoint_resume_inspect_args(&arg_values(raw)),
            Err(expected)
        );
    }
}

#[test]
fn tlcx07_cli_inspect_rejects_execution_or_unknown_flags() {
    let args = arg_values(&[
        "--ledger-root",
        "existing-root",
        "--source-revision",
        "source-r1",
        "--plan-digest",
        "plan-r1",
        "--authority-revision",
        "authority-r1",
        "--execute",
    ]);
    assert_eq!(
        parse_binary_checkpoint_resume_inspect_args(&args),
        Err(BinaryCheckpointResumeInspectArgsError::UnknownOption)
    );
}

#[test]
fn tlcx07_cli_inspect_accepts_only_explicit_root_and_identity() {
    let args = arg_values(&[
        "--ledger-root",
        "existing-root",
        "--source-revision",
        "source-r1",
        "--plan-digest",
        "plan-r1",
        "--authority-revision",
        "authority-r1",
    ]);
    let parsed = parse_binary_checkpoint_resume_inspect_args(&args).unwrap();
    assert_eq!(parsed.ledger_root, PathBuf::from("existing-root"));
    assert_eq!(parsed.source_revision, "source-r1");
    assert_eq!(parsed.plan_digest, "plan-r1");
    assert_eq!(parsed.authority_revision, "authority-r1");
}

#[test]
fn tlcx07_cli_persisted_reader_revalidates_chain_but_restores_no_state() {
    let root = ScratchRoot::create();
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        let source = stable_sha256("source");
        let plan = stable_sha256("plan");
        let authority = "authority-current";
        let (first, second) = checkpoint_segments(&source, &plan, authority);
        write_segment(root.path(), 1, &first, &view);
        write_segment(root.path(), 2, &second, &view);

        let reader = reader(root.path(), &view);
        let decision = inspect_persisted_binary_checkpoint_resume(
            &reader,
            None,
            JournalChainLimits::default(),
            identity(&source, &plan, authority),
        );
        match decision {
            BinaryCheckpointResumeDecision::Candidate { chain, observation } => {
                assert_eq!(chain.segment_count(), 2);
                assert_eq!(
                    observation.integrity_state,
                    CheckpointIntegrityState::Unanchored
                );
                assert!(observation.fresh_revalidation_required);
                assert!(!observation.authority_created);
                assert!(!observation.completion_restored);
            }
            other => panic!("valid persisted chain should be inspectable: {other:?}"),
        }
        drop(reader);

        let decision = inspect_existing_binary_checkpoint_root(
            root.path(),
            &view,
            JournalChainLimits::default(),
            identity(&source, &plan, authority),
        );
        let report = report_binary_checkpoint_resume_inspection(&decision);
        assert_eq!(report.status, "candidate_unanchored");
        assert_eq!(report.head_state, Some("unanchored"));
        assert_eq!(report.authenticated_head_provider_state, "not_wired");
        assert!(!report.authority_created);
        assert!(!report.completion_restored);
        assert!(!report.external_anchor_observed);
        assert!(!report.rollback_resistance_claimed);
        assert!(!report.output_commit_allowed);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_cli_inspect_missing_existing_root_holds_without_creating_it() {
    let scratch = ScratchRoot::create();
    let missing = scratch.path().join("not-created-by-reader");
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        assert!(!missing.exists());
        let decision = inspect_existing_binary_checkpoint_root(
            &missing,
            &view,
            JournalChainLimits::default(),
            identity("source-r1", "plan-r1", "authority-r1"),
        );
        let report = report_binary_checkpoint_resume_inspection(&decision);
        assert_eq!(report.status, "hold");
        assert_eq!(report.hold_code, Some("storage_read_hold"));
        assert_eq!(report.authenticated_head_provider_state, "not_wired");
        assert!(!report.external_anchor_observed);
        assert!(!report.rollback_resistance_claimed);
        assert!(!missing.exists());
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_cli_persisted_reader_holds_a_descriptor_sequence_gap() {
    let root = ScratchRoot::create();
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        write_segment(root.path(), 1, b"untrusted first bytes", &view);
        write_segment(root.path(), 3, b"untrusted later bytes", &view);

        let reader = reader(root.path(), &view);
        let decision = inspect_persisted_binary_checkpoint_resume(
            &reader,
            None,
            JournalChainLimits::default(),
            identity("source", "plan", "authority"),
        );
        assert!(matches!(
            decision,
            BinaryCheckpointResumeDecision::Hold(BinaryCheckpointResumeHold::DescriptorSequence {
                expected: 2,
                actual: 3
            })
        ));
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_cli_persisted_reader_enforces_the_callers_lower_byte_cap() {
    let root = ScratchRoot::create();
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        let source = stable_sha256("source");
        let plan = stable_sha256("plan");
        let authority = "authority-current";
        let (first, second) = checkpoint_segments(&source, &plan, authority);
        let total = first.len() + second.len();
        write_segment(root.path(), 1, &first, &view);
        write_segment(root.path(), 2, &second, &view);

        let reader = reader(root.path(), &view);
        let limits = JournalChainLimits::checked(8, total - 1).unwrap();
        let decision = inspect_persisted_binary_checkpoint_resume(
            &reader,
            None,
            limits,
            identity(&source, &plan, authority),
        );
        assert!(matches!(
            decision,
            BinaryCheckpointResumeDecision::Hold(
                BinaryCheckpointResumeHold::AggregateBytesExceeded { .. }
            )
        ));
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_cli_runtime_holds_remain_explicit_through_the_reader_adapter() {
    let root = ScratchRoot::create();
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        let source = stable_sha256("source");
        let plan = stable_sha256("plan");
        let authority = "authority-current";
        let (first, second) = checkpoint_segments(&source, &plan, authority);
        write_segment(root.path(), 1, &first, &view);
        write_segment(root.path(), 2, &second, &view);

        let reader = reader(root.path(), &view);
        let decision = inspect_persisted_binary_checkpoint_resume(
            &reader,
            None,
            JournalChainLimits::default(),
            identity("stale-source", &plan, authority),
        );
        assert!(matches!(
            decision,
            BinaryCheckpointResumeDecision::Hold(BinaryCheckpointResumeHold::Runtime(
                CheckpointChainResumeHold::Analysis(lc631_analysis::ResumeError::StaleSource)
            ))
        ));
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_cli_inspection_metadata_must_fit_beside_retained_raw_segments() {
    let root = ScratchRoot::create();
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        let source = stable_sha256("metadata-source");
        let plan = stable_sha256("metadata-plan");
        let authority = "authority-current";
        let (first, second) = checkpoint_segments(&source, &plan, authority);
        write_segment(root.path(), 1, &first, &view);
        write_segment(root.path(), 2, &second, &view);
        let reader = reader(root.path(), &view);
        // The reader holds 1 MiB for its inventory. Leave exactly the two
        // bounded raw reads (including each overrun sentinel), not Vec metadata.
        let raw_bytes = u64::try_from(first.len() + second.len() + 2).unwrap();
        let pressure_bytes = 32 * 1024 * 1024 - 1024 * 1024 - raw_bytes;
        let pressure = view
            .claim_resources(&[
                request(ResourceKind::Ram, pressure_bytes),
                request(ResourceKind::Output, pressure_bytes),
            ])
            .unwrap();
        let inspect = || {
            inspect_persisted_binary_checkpoint_resume(
                &reader,
                None,
                JournalChainLimits::default(),
                identity(&source, &plan, authority),
            )
        };
        assert!(matches!(
            inspect(),
            BinaryCheckpointResumeDecision::Hold(BinaryCheckpointResumeHold::Storage(_))
        ));
        drop(pressure);
        assert!(matches!(
            inspect(),
            BinaryCheckpointResumeDecision::Candidate { .. }
        ));
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_reader_metadata_claim_holds_both_quotas_until_drop() {
    let root = ScratchRoot::create();
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        let reader = reader(root.path(), &view);
        let metadata = reader.claim_inspection_metadata(128).unwrap();
        for kind in [ResourceKind::Ram, ResourceKind::Output] {
            assert!(
                matches!(view.claim_resources(&[request(kind, 31 * 1024 * 1024)]),
                Err(lc631_process_supervisor::ResourceError::Exhausted(denied)) if denied == kind)
            );
        }
        drop(metadata);
        let available = view
            .claim_resources(&[
                request(ResourceKind::Ram, 31 * 1024 * 1024),
                request(ResourceKind::Output, 31 * 1024 * 1024),
            ])
            .unwrap();
        drop(available);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_cli_missing_first_physical_segment_is_not_a_candidate() {
    let root = ScratchRoot::create();
    let (_broker, _run, mut reservation) = reservation();
    {
        let view = reservation.lease();
        let source = stable_sha256("source");
        let plan = stable_sha256("plan");
        let authority = "authority-current";
        let (first, second) = checkpoint_segments(&source, &plan, authority);
        // Internally valid genesis bytes cannot conceal a missing file prefix.
        write_segment(root.path(), 2, &first, &view);
        write_segment(root.path(), 3, &second, &view);
        let reader = reader(root.path(), &view);
        let decision = inspect_persisted_binary_checkpoint_resume(
            &reader,
            None,
            JournalChainLimits::default(),
            identity(&source, &plan, authority),
        );
        assert!(
            matches!(
                &decision,
                BinaryCheckpointResumeDecision::Hold(
                    BinaryCheckpointResumeHold::DescriptorSequence {
                        expected: 1,
                        actual: 2,
                    }
                )
            ),
            "missing-prefix refusal: {decision:?}"
        );
        assert!(!root
            .path()
            .join("segment-00000000000000000001.bin")
            .exists());
        assert_eq!(
            fs::read(root.path().join("segment-00000000000000000002.bin")).unwrap(),
            first
        );
        assert_eq!(
            fs::read(root.path().join("segment-00000000000000000003.bin")).unwrap(),
            second
        );
    }
    reservation.release_confirmed().unwrap();
}
