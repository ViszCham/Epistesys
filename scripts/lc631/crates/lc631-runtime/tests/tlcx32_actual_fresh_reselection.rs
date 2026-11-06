//! 実SDK fixtureのprocess境界。製品CLI／外部issuer／G08完成ではない。
//! Actual SDK fixture process boundary; not product CLI/external issuer/full G08.
#[path = "tlcx31_coding_support/mod.rs"]
mod coding_support;
#[path = "tlcx32_verified_support/mod.rs"]
mod file_fixture;
#[path = "tlcx32_fresh_support/mod.rs"]
mod fresh_support;

use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const ENTRY: &str = "tlcx32_sdk_fixture_entry";
const ROOT_ENV: &str = "EPISTESYS_TLCX32_SDK_FIXTURE_ROOT";
const MODE_ENV: &str = "EPISTESYS_TLCX32_SDK_FIXTURE_MODE";
const MARKER: &[u8] = b"Epistesys/TLCX32/owned-sdk-fixture/v1\n";

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn run_phase(root: &Path, mode: &str) {
    let stdout_path = root.with_extension(format!("{mode}.stdout.log"));
    let stderr_path = root.with_extension(format!("{mode}.stderr.log"));
    let stdout = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stdout_path)
        .unwrap();
    let stderr = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stderr_path)
        .unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", ENTRY, "--ignored", "--test-threads=1"])
        .env(ROOT_ENV, root)
        .env(MODE_ENV, mode)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .unwrap();
    let mut guard = ChildGuard(child);
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        for path in [&stdout_path, &stderr_path] {
            assert!(
                std::fs::metadata(path).unwrap().len() <= 1_048_576,
                "owned {mode} diagnostic output exceeded cap; retain {}",
                root.display()
            );
        }
        if let Some(status) = guard.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "owned {mode} child failed: {status}; retain {}; stdout={}; stderr={}",
                root.display(),
                stdout_path.display(),
                stderr_path.display()
            );
            return;
        }
        assert!(
            Instant::now() < deadline,
            "owned {mode} child timed out; retain {}; root-child kill/reap is best effort",
            root.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "selected only by the owned SDK process-boundary parent"]
fn tlcx32_sdk_fixture_entry() {
    let root = std::path::PathBuf::from(std::env::var_os(ROOT_ENV).unwrap());
    assert!(root.is_absolute() && root.is_dir());
    assert_eq!(
        std::fs::read(root.join("tlcx32-owned.marker")).unwrap(),
        MARKER
    );
    match std::env::var(MODE_ENV).unwrap().as_str() {
        "create" => fresh_support::create_checkpoint(&root),
        "resume" => fresh_support::resume_checkpoint(&root, false),
        "cut" => fresh_support::resume_checkpoint(&root, true),
        "physical-cut" => fresh_support::resume_checkpoint_with_physical_cut(&root),
        "cargo-cut" => fresh_support::resume_checkpoint_with_build_cut(&root, true),
        "source-cut" => fresh_support::resume_checkpoint_with_build_cut(&root, false),
        "late-cut" => fresh_support::resume_checkpoint_with_late_authority_cut(&root),
        other => panic!("unknown fixture mode {other}"),
    }
}

#[test]
fn tlcx32_actual_repair_exit_fresh_native_pair_reselects_and_revocation_cut_has_no_cached_winner() {
    let root = file_fixture::root();
    let mut marker = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("tlcx32-owned.marker"))
        .unwrap();
    use std::io::Write;
    marker.write_all(MARKER).unwrap();
    marker.sync_all().unwrap();
    drop(marker);
    // Reuse this existing test executable and two small compiled native artifacts;
    // do not copy a large test binary or a shared build target into this root.
    run_phase(&root, "create");
    let native_bytes = ["baseline", "current"]
        .into_iter()
        .map(|name| {
            std::fs::metadata(root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)))
                .unwrap()
                .len()
        })
        .sum::<u64>();
    assert!(native_bytes > 0 && native_bytes <= 64 * 1024 * 1024);
    let checkpoint = fresh_support::read_owned(&root.join("transition.bin"), 4096);
    let history = fresh_support::read_owned(&root.join("history.json"), 65_536);
    assert!(!checkpoint.is_empty() && !history.is_empty());
    run_phase(&root, "resume");
    let result: serde_json::Value = serde_json::from_slice(&fresh_support::read_owned(
        &root.join("fresh-result.json"),
        4096,
    ))
    .unwrap();
    assert_eq!(result["baseline_correct"], 1);
    assert_eq!(result["current_correct"], 4);
    assert_eq!(result["gain_quarters"], 3);
    assert_eq!(result["selected_current"], true);
    assert_eq!(result["before_build_bound"], true);
    assert_eq!(result["fresh_authority_revision"], 2);
    run_phase(&root, "cut");
    assert_eq!(
        fresh_support::read_owned(&root.join("cut-result.txt"), 4096),
        b"revoked_current_no_winner\n"
    );
    for mode in ["cargo-cut", "source-cut"] {
        run_phase(&root, mode);
        assert_eq!(
            fresh_support::read_owned(&root.join(format!("{mode}-result.txt")), 4096),
            b"actual_declared_build_changed_no_winner\n"
        );
    }
    run_phase(&root, "late-cut");
    assert_eq!(
        fresh_support::read_owned(&root.join("late-cut-result.txt"), 4096),
        b"late_authority_failure_retains_actual_capture\n"
    );
    assert_eq!(
        fresh_support::read_owned(&root.join("transition.bin"), 4096),
        checkpoint
    );
    assert_eq!(
        fresh_support::read_owned(&root.join("history.json"), 65_536),
        history
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        coding_support::AFTER
    );
    run_phase(&root, "physical-cut");
    assert_eq!(
        fresh_support::read_owned(&root.join("physical-cut-result.txt"), 4096),
        b"changed_current_no_fresh_winner\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        coding_support::AFTER.replace("checked_add", "checked_sub")
    );
    assert_eq!(
        std::fs::read_dir(root.join(".dgcl-repair-backups"))
            .unwrap()
            .count(),
        1
    );
    // All files are this fixture's unique owned artifacts. Retain evidence on
    // failure; central owns the existing bounded scratch-retention/cleanup policy.
    // Fixture signing keys are synthetic constants, never external host providers.
}
