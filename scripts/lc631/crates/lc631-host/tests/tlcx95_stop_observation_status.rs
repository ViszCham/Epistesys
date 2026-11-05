//! Local synthetic reports only: no host provider, keys or historical records.
#![forbid(unsafe_code)]

use lc631_host::{
    bind_codex_stop_hook_attested, persist_stop_hook_observation,
    persist_stop_hook_observation_with_resources, CodexStopHookEvent, HostError,
    StopHookObservation,
};
use lc631_process_supervisor::{filesystem::DirectoryAnchor, test_fixtures::bounded_test_lease};
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier, ReplayGuard};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const RAW: &str = r#"{"session_id":"tlcx95-synthetic","cwd":".","hook_event_name":"Stop","turn_id":"synthetic-turn","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"synthetic output"}"#;

fn local_report() -> StopHookObservation {
    let issuer = ReceiptIssuer::from_key_bytes("tlcx95-local-fixture", &[95; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx95-local-fixture", &[95; 32]).unwrap();
    let event: CodexStopHookEvent = serde_json::from_str(RAW).unwrap();
    bind_codex_stop_hook_attested(
        &event,
        RAW,
        &issuer,
        &verifier,
        &mut ReplayGuard::default(),
        10,
    )
    .unwrap()
    .1
}

struct Fixture {
    parent: PathBuf,
    root: PathBuf,
    observation_name: String,
}

impl Fixture {
    fn new(report: &StopHookObservation) -> Self {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let parent = base.join(format!(
            "epistesys-tlcx95-stop-status-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&parent).unwrap();
        let root = parent.join("owned-report");
        assert!(!root.exists());
        let suffix = |value: &str| value.strip_prefix("sha256:").unwrap()[..16].to_owned();
        let observation_name = format!(
            "{}-{}.json",
            suffix(&report.session_digest),
            suffix(&report.host_turn_digest)
        );
        Self {
            parent,
            root,
            observation_name,
        }
    }

    fn output_path(&self) -> PathBuf {
        self.root.join("observations").join(&self.observation_name)
    }

    fn absent_before_cleanup(&self) -> [bool; 4] {
        [
            !self.root.exists(),
            !self.root.join(".resource-owner-v1.lock").exists(),
            !self.root.join("observations").exists(),
            !self.output_path().exists(),
        ]
    }

    // Exact, newly-created test paths only; never recursively remove anything.
    // Unknown, linked, nonregular or locked entries stop cleanup and are retained.
    fn cleanup(self) {
        assert_eq!(self.parent.canonicalize().unwrap(), self.parent);
        let parent = DirectoryAnchor::open_existing(&self.parent).unwrap();
        let expected_parent_entries = if self.root.exists() {
            vec!["owned-report"]
        } else {
            vec![]
        };
        assert_entries(&self.parent, &expected_parent_entries);
        if self.root.exists() {
            assert_regular_directory(&self.root);
            let anchor = DirectoryAnchor::open_existing(&self.root).unwrap();
            let observation_dir = self.root.join("observations");
            let lock = self.root.join(".resource-owner-v1.lock");
            let expected_root_entries = if lock.exists() {
                vec![".resource-owner-v1.lock", "observations"]
            } else {
                vec!["observations"]
            };
            assert_entries(&self.root, &expected_root_entries);
            assert_regular_directory(&observation_dir);
            assert_entries(&observation_dir, &[&self.observation_name]);
            for relative in [
                Some(Path::new("observations").join(&self.observation_name)),
                lock.exists()
                    .then(|| PathBuf::from(".resource-owner-v1.lock")),
            ]
            .into_iter()
            .flatten()
            {
                let path = self.root.join(&relative);
                assert!(fs::symlink_metadata(&path).unwrap().is_file());
                // The anchored reader rejects symlink/reparse and hardlink aliases.
                let file = anchor.open_read_only(&relative).unwrap();
                file.validate_current().unwrap();
            }
            anchor.validate_current().unwrap();
            drop(anchor);
            fs::remove_file(self.output_path()).unwrap();
            if lock.exists() {
                fs::remove_file(&lock).unwrap();
            }
            fs::remove_dir(&observation_dir).unwrap();
            fs::remove_dir(&self.root).unwrap();
        }
        parent.validate_current().unwrap();
        drop(parent);
        fs::remove_dir(&self.parent).unwrap();
    }
}

fn assert_entries(directory: &Path, expected: &[&str]) {
    let mut names: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|item| item.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let mut expected: Vec<_> = expected.iter().map(|name| (*name).to_owned()).collect();
    expected.sort();
    assert_eq!(names, expected, "unknown test entry must be retained");
}

fn assert_regular_directory(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
}

fn persist(
    fixture: &Fixture,
    report: &StopHookObservation,
    owned_resources: bool,
) -> Result<PathBuf, HostError> {
    if owned_resources {
        let lease = bounded_test_lease(95).unwrap();
        let result = {
            let view = lease.view();
            persist_stop_hook_observation_with_resources(&fixture.root, report, &view)
        };
        lease.close_confirmed().unwrap();
        result
    } else {
        persist_stop_hook_observation(&fixture.root, report)
    }
}

#[test]
fn tlcx95_false_local_stop_status_persists_without_host_promotion_in_both_entries() {
    let report = local_report();
    assert!(!report.automatic_host_callback_observed);
    assert!(!report.persistent_supervision_observed);
    for owned_resources in [false, true] {
        let fixture = Fixture::new(&report);
        let result = persist(&fixture, &report, owned_resources);
        let expected = fixture.output_path();
        let stored: serde_json::Value =
            serde_json::from_slice(&fs::read(&expected).unwrap()).unwrap();
        let flags = (
            stored["automatic_host_callback_observed"].as_bool(),
            stored["persistent_supervision_observed"].as_bool(),
        );
        fixture.cleanup();
        assert_eq!(result.unwrap(), expected);
        assert_eq!(flags, (Some(false), Some(false)));
    }
}

fn reject_modified_host_status(automatic: bool) {
    let mut report = local_report();
    if automatic {
        report.automatic_host_callback_observed = true;
    } else {
        report.persistent_supervision_observed = true;
    }
    let mut outcomes = Vec::new();
    for owned_resources in [false, true] {
        let fixture = Fixture::new(&report);
        let result = persist(&fixture, &report, owned_resources);
        let absent = fixture.absent_before_cleanup();
        eprintln!("synthetic status automatic={automatic} resources={owned_resources}: result={result:?}, absent={absent:?}");
        fixture.cleanup();
        outcomes.push((owned_resources, result, absent));
    }
    for (owned_resources, result, absent) in outcomes {
        assert_eq!(
            result,
            Err(HostError::InvalidStopHookEvent),
            "resources={owned_resources}"
        );
        assert_eq!(
            absent, [true; 4],
            "reject before root/lock/directory/file effects"
        );
    }
}

#[test]
fn tlcx95_public_automatic_callback_true_is_rejected_before_persistence() {
    reject_modified_host_status(true);
}

#[test]
fn tlcx95_public_persistent_supervision_true_is_rejected_before_persistence() {
    reject_modified_host_status(false);
}
