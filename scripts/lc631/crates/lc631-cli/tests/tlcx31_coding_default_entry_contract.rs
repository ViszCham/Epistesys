//! 日本語: 実binaryのprovider未供給負例。configured SDK正例/実hostとは別scope。
//! English: Actual-binary missing-provider negatives; not a configured SDK or host positive.
//! Planned runtime RED until the new explicit profiles exist; central executes.
#![forbid(unsafe_code)]
use lc631_analysis::{run_bounded_tool_with_resources, BoundedToolRequest, ToolRunState};
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::*;
use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SOURCE: &str = "Please edit the source and test the package.";
const COMMANDS: [&str; 2] = ["lc631-dgcl-coding-repair", "lc631-dgcl-coding-resume"];
type FileState = (String, bool, Vec<u8>, SystemTime);

fn fixture() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("t31cli-{}-{nonce:x}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("receipts")).unwrap();
    fs::create_dir_all(root.join("author")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        b"[package]\nname=\"cli_hold\"\nversion=\"0.0.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), b"pub fn value()->u8{1}\n").unwrap();
    // Deliberately UNTRUSTED input. Provider availability must be decided before
    // expensive packet/history admission. Flags cannot establish a trust domain.
    fs::write(
        root.join("packet.bin"),
        b"EPITLCX1 untrusted locator input\n",
    )
    .unwrap();
    fs::write(
        root.join("receipts/host.key"),
        b"untrusted local key material is not a host issuer\n",
    )
    .unwrap();
    fs::write(
        root.join("author/host.key"),
        b"untrusted author material is not an independent intent\n",
    )
    .unwrap();
    eprintln!("retained owned default-CLI fixture: {}", root.display());
    root
}

fn inventory(root: &Path) -> Vec<FileState> {
    let mut pending = vec![root.to_path_buf()];
    let mut result = Vec::new();
    let mut visited = 0_usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            visited += 1;
            assert!(visited <= 64, "unexpected fixture-tree growth");
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "unexpected fixture symlink"
            );
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                assert_eq!(
                    metadata.file_attributes() & 0x400,
                    0,
                    "unexpected reparse point"
                );
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            if metadata.is_dir() {
                result.push((relative, true, Vec::new(), metadata.modified().unwrap()));
                pending.push(path);
            } else {
                assert!(metadata.is_file() && metadata.len() <= 4096);
                let mut bytes = Vec::new();
                fs::File::open(&path)
                    .unwrap()
                    .take(4097)
                    .read_to_end(&mut bytes)
                    .unwrap();
                assert!(bytes.len() <= 4096);
                result.push((relative, false, bytes, metadata.modified().unwrap()));
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}

fn scenario(command: &str, supplied_untrusted_root_flags: bool) {
    let root = fixture();
    let before = inventory(&root);
    let limits = ResourceLimits::checked([16 * 1024 * 1024, 0, 256 * 1024, 0, 1, 1]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            SourceRevision::of(SOURCE).digest(),
            limits,
            Deadline::after(Duration::from_secs(15)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 4 * 1024 * 1024),
            request(ResourceKind::Output, 128 * 1024),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let mut arguments = vec![
            command.to_owned(),
            "--repo".into(),
            root.to_str().unwrap().into(),
            "--prompt".into(),
            SOURCE.into(),
            "--execute".into(),
            "--evidence-file".into(),
            root.join("packet.bin").to_str().unwrap().into(),
        ];
        if supplied_untrusted_root_flags {
            arguments.extend([
                "--receipt-root".into(),
                root.join("receipts").to_str().unwrap().into(),
                "--author-receipt-root".into(),
                root.join("author").to_str().unwrap().into(),
            ]);
        }
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        // This is the actual Cargo-built production binary, never a fake runner,
        // source-string registry assertion or environment-selected test provider.
        let capture = run_bounded_tool_with_resources(
            BoundedToolRequest {
                program: env!("CARGO_BIN_EXE_lc631"),
                args: &borrowed,
                stdin: None,
                timeout_ms: 8000,
                stdout_limit: 65536,
                stderr_limit: 16384,
            },
            &view,
        )
        .unwrap();
        let process = capture.reported();
        assert_eq!(
            process.state,
            ToolRunState::Failed,
            "exit={:?}; timeout={}; stdout={}; stderr={}",
            process.exit_code,
            process.timed_out,
            String::from_utf8_lossy(capture.stdout())
                .chars()
                .take(512)
                .collect::<String>(),
            String::from_utf8_lossy(capture.stderr())
                .chars()
                .take(512)
                .collect::<String>()
        );
        assert_eq!(
            process.exit_code,
            Some(1),
            "new default profile must return a non-success Hold"
        );
        assert!(!process.timed_out && !process.stdout_truncated && !process.stderr_truncated);
        assert!(process.process_tree_kill_verified);
        assert_eq!(
            inventory(&root),
            before,
            "default admission changed owned fixture files"
        );
        // Empty/unknown-command output is RED. Merely rejecting the command is
        // not consumption by the new normal profile's missing-provider path.
        let envelope: Value =
            serde_json::from_slice(capture.stdout()).expect("typed Hold summary required");
        assert_eq!(envelope["schema_version"], "lc631-wire.v1");
        assert_eq!(envelope["command"], command);
        let summary = &envelope["payload"];
        assert_eq!(
            summary["schema_version"],
            "epistesys-coding-command-summary.v1"
        );
        assert_eq!(summary["profile"], command);
        assert_eq!(summary["status"], "hold");
        assert_eq!(summary["reason"], "missing_external_host_provider");
        assert_eq!(summary["authority_created"], false);
        assert!(summary["current_recommendation"].is_null());
        for effect in ["source", "native", "persistence"] {
            assert_eq!(summary["effects"][effect], "not_attempted");
        }
        // These are bounded public REPORT fields, not an independent OS-wide
        // proof that no other child/syscall occurred. SDK marker tests are separate.
        assert_eq!(summary["source_writes"], 0);
        assert_eq!(summary["native_invocations"], 0);
        for flag in [
            "repair_reapplied",
            "head_renewed",
            "quota_reset",
            "resend_attempted",
        ] {
            assert_eq!(summary[flag], false);
        }
        assert!(capture.stdout().len() <= 65536);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx31_actual_binary_default_repair_provider_hold_precedes_effects() {
    scenario(COMMANDS[0], false);
}
#[test]
fn tlcx32_actual_binary_default_resume_provider_hold_precedes_effects() {
    scenario(COMMANDS[1], false);
}
#[test]
fn tlcx31_actual_binary_seed_execute_and_root_flags_cannot_supply_host_authority() {
    scenario(COMMANDS[0], true);
}
#[test]
fn tlcx32_actual_binary_resume_root_flags_cannot_restore_completed_or_grant() {
    scenario(COMMANDS[1], true);
}
