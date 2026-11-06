//! Public synthetic Stop/replay persistence fixtures. No real callback or sink.
#![forbid(unsafe_code)]

use lc631_host::{
    bind_codex_stop_hook_attested, persist_stop_hook_observation,
    persist_stop_hook_observation_with_resources, verify_durable_replay,
    verify_durable_replay_with_resources, CodexStopHookEvent, HostError, StopHookObservation,
};
use lc631_process_supervisor::test_fixtures::bounded_test_lease;
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier, ReplayGuard};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const RAW: &str = r#"{"session_id":"beta7-fixture","cwd":".","hook_event_name":"Stop","turn_id":"turn","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"合成\r\nfixture"}"#;
const LOCK: &str = ".resource-owner-v1.lock";

#[derive(Clone, Copy, Debug)]
enum Entry {
    Legacy,
    Owned,
}

const ENTRIES: [Entry; 2] = [Entry::Owned, Entry::Legacy];

impl Entry {
    fn persist(self, root: &Path, report: &StopHookObservation) -> Result<PathBuf, HostError> {
        match self {
            Self::Legacy => persist_stop_hook_observation(root, report),
            Self::Owned => {
                let lease = bounded_test_lease(71).unwrap();
                let result = persist_stop_hook_observation_with_resources(root, report, &lease.view());
                lease.close_confirmed().unwrap();
                result
            }
        }
    }

    fn replay(self, ledger: &Path) -> Result<usize, HostError> {
        match self {
            Self::Legacy => verify_durable_replay(ledger),
            Self::Owned => {
                let lease = bounded_test_lease(72).unwrap();
                let result = verify_durable_replay_with_resources(ledger, &lease.view());
                lease.close_confirmed().unwrap();
                result
            }
        }
    }
}

fn outer() -> PathBuf {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "beta7-host-stop-{}-{time}-{}", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&p).unwrap();
    p
}

fn observation() -> StopHookObservation {
    let actor = "beta7-stop-synthetic";
    let issuer = ReceiptIssuer::from_key_bytes(actor, &[17; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes(actor, &[17; 32]).unwrap();
    let event: CodexStopHookEvent = serde_json::from_str(RAW).unwrap();
    bind_codex_stop_hook_attested(&event, RAW, &issuer, &verifier, &mut ReplayGuard::default(), 11)
        .unwrap().1
}

fn entries(path: &Path) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = fs::read_dir(path).unwrap().map(|e| e.unwrap().file_name()).collect();
    names.sort();
    names
}

#[test]
fn beta7_host_regular_stop_is_idempotent_without_host_flags_in_both_entries() {
    let report = observation();
    for mode in ENTRIES {
        let root = outer().join("store");
        let path = mode.persist(&root, &report).unwrap();
        assert_eq!(path.parent().unwrap(), root.join("observations"));
        let bytes = fs::read(&path).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["output_digest"].as_str(), Some(report.output_digest.as_str()));
        assert_eq!(value["automatic_host_callback_observed"].as_bool(), Some(false));
        assert_eq!(value["persistent_supervision_observed"].as_bool(), Some(false));
        assert_eq!(mode.persist(&root, &report).unwrap(), path);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn beta7_host_all_four_stop_digests_require_canonical_bytes_before_root_creation() {
    let report = observation();
    let cases = [
        ("session", format!("sha256:{}", "a".repeat(16))),
        ("turn", format!("sha256:{}G", "b".repeat(63))),
        ("output", format!("sha256:{}", "C".repeat(64))),
        ("callback", format!("sha256:{}", "d".repeat(65))),
    ];
    let mut observations = Vec::new();
    for (field, invalid) in cases {
        let mut changed = report.clone();
        match field {
            "session" => changed.session_digest = invalid,
            "turn" => changed.host_turn_digest = invalid,
            "output" => changed.output_digest = invalid,
            "callback" => changed.callback_payload_digest = invalid,
            _ => unreachable!(),
        }
        for mode in ENTRIES {
            let parent = outer();
            let root = parent.join("uncreated");
            let result = mode.persist(&root, &changed);
            observations.push((mode, field, result, entries(&parent)));
        }
    }
    for (mode, field, result, visible) in observations {
        assert_eq!(result, Err(HostError::InvalidStopHookEvent), "{mode:?}, {field}");
        assert!(visible.is_empty(), "{mode:?}, {field} created a root, lock or file");
    }
}

#[test]
fn beta7_host_forged_stop_origin_or_supervision_cannot_create_a_store() {
    let report = observation();
    for mode in ENTRIES {
        for (automatic, supervised) in [(true, false), (false, true), (true, true)] {
            let mut changed = report.clone();
            changed.automatic_host_callback_observed = automatic;
            changed.persistent_supervision_observed = supervised;
            let p = outer();
            let result = mode.persist(&p.join("uncreated"), &changed);
            assert_eq!(result, Err(HostError::InvalidStopHookEvent), "{mode:?}");
            assert!(entries(&p).is_empty());
        }
    }
}

#[test]
fn beta7_host_identical_stop_bytes_cannot_make_a_hardlink_idempotent() {
    let report = observation();
    for mode in ENTRIES {
        let p = outer();
        let root = p.join("store");
        let file = mode.persist(&root, &report).unwrap();
        let alias = p.join("fixture-alias.json");
        fs::hard_link(&file, &alias).unwrap();
        let before = fs::read(&file).unwrap();
        let members = entries(&root);
        let result = mode.persist(&root, &report);
        assert_eq!(fs::read(&alias).unwrap(), before);
        assert_eq!(fs::read(&file).unwrap(), before);
        assert_eq!(entries(&root), members);
        assert!(result.is_err(), "{mode:?} accepted an observation with a second name");
    }
}

#[cfg(unix)]
fn alias_observations(p: &Path) {
    std::os::unix::fs::symlink(p.join("sibling"), p.join("store/observations")).unwrap();
}

#[cfg(windows)]
fn alias_observations(p: &Path) {
    use std::os::windows::process::CommandExt;
    use std::path::{Component, Prefix};

    let root = p.canonicalize().unwrap();
    let link = root.join("store/observations");
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
fn beta7_host_stop_cannot_write_through_an_observations_directory_alias() {
    let report = observation();
    for mode in ENTRIES {
        let p = outer();
        let store = p.join("store");
        let sibling = p.join("sibling");
        fs::create_dir(&store).unwrap();
        fs::create_dir(&sibling).unwrap();
        alias_observations(&p);
        assert_eq!(fs::canonicalize(store.join("observations")).unwrap(), sibling.canonicalize().unwrap());
        let before = entries(&store);
        let result = mode.persist(&store, &report);
        assert!(entries(&sibling).is_empty(), "{mode:?} wrote through the alias");
        assert!(!store.join(LOCK).exists(), "{mode:?} created a lock before rejection");
        assert_eq!(entries(&store), before);
        assert!(result.is_err(), "{mode:?} accepted an aliased observations directory");
    }
}

#[test]
fn beta7_host_replay_rejects_hardlinked_ledger_even_when_regular_bytes_are_valid() {
    for mode in ENTRIES {
        let p = outer();
        let ledger = p.join("ledger.jsonl");
        fs::write(&ledger, b"\n").unwrap();
        assert_eq!(mode.replay(&ledger), Ok(0));
        let alias = p.join("alias.jsonl");
        fs::hard_link(&ledger, &alias).unwrap();
        let result = mode.replay(&ledger);
        assert_eq!(fs::read(&alias).unwrap(), b"\n");
        assert!(result.is_err(), "{mode:?} validated an aliased ledger");
    }
}
