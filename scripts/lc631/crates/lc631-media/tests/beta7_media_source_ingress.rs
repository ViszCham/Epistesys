//! Synthetic bytes and a marker-only local backend exercise the public v2/v3 entries.
#![forbid(unsafe_code)]

use lc631_media::{
    authorize_media_source, authorize_media_source_attested, execute_local_media_backend_attested,
    execute_local_media_backend_attested_v3, media_source_payload_digest, media_source_scope,
    model_cache_manifest_sha256, BackendDescriptor, BackendFamily, MediaBackendConfig, MediaError,
    MediaResourceBudget, MediaSourceKind, MediaSourceRequest,
};
use lc631_process_supervisor::{run_bounded, RunRequest};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SOURCE: &[u8] = b"synthetic beta7 source\n";
const BACKEND: &str = "beta7-source-ingress-fixture";
#[cfg(target_os = "linux")]
const WORKER_ROOT: &str = "EPISTESYS_BETA7_INGRESS_FIFO_WORKER_ROOT";

#[derive(Clone, Copy, Debug)]
enum Change {
    Stable,
    Before,
    During,
    #[cfg(target_os = "linux")]
    Fifo,
}

fn owner_root() -> PathBuf {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for attempt in 0..32 {
        let dir = base.join(format!(
            "epistesys-beta7-media-ingress-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot create isolated test root: {e}"),
        }
    }
    panic!("cannot reserve isolated test root")
}

fn python(root: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "python.exe"
    } else {
        "python3"
    };
    let interpreter = std::env::var_os("EPISTESYS_TEST_MEDIA_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|dir| dir.join(name))
                .find(|file| file.is_file())
        })
        .expect("this actual-process fixture requires local Python");
    let result = run_bounded(RunRequest {
        executable: interpreter.as_os_str(),
        arguments: &[
            "-B".into(),
            "-c".into(),
            "import sys;print(sys.executable)".into(),
        ],
        current_dir: root,
        environment: &std::env::vars_os().collect::<Vec<_>>(),
        stdin: None,
        timeout: Duration::from_secs(10),
        stdout_limit: 4096,
        stderr_limit: 4096,
    })
    .unwrap();
    assert_eq!(
        result.exit_code,
        Some(0),
        "Python discovery: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!result.timed_out && !result.stdout_truncated && result.io_cleanup_complete);
    PathBuf::from(String::from_utf8(result.stdout).unwrap().trim())
        .canonicalize()
        .unwrap()
}

struct ResultFacts {
    result: Result<String, MediaError>,
    replay: usize,
    snapshot_read: bool,
    mutation_observed: bool,
    snapshot_closed: bool,
}

fn execute(root: &Path, v3: bool, change: Change) -> ResultFacts {
    let source = root.join("source.bin");
    fs::write(&source, SOURCE).unwrap();
    let models = root.join("models");
    fs::create_dir(&models).unwrap();
    fs::write(
        models.join("fixture.bin"),
        b"synthetic model descriptor only",
    )
    .unwrap();
    let model = model_cache_manifest_sha256(&models).unwrap();
    let ffmpeg = root.join("ffmpeg.fixture");
    fs::write(&ffmpeg, b"never executed").unwrap();
    let script = root.join("backend.py");
    let payload = serde_json::json!({
        "schema_version":"lc631-media-backend.v1", "source_sha256":lc631_core::stable_sha256("synthetic beta7 source\n"),
        "backend_id":BACKEND, "backend_version":"fixture-v1", "model_revision":model,
        "model_cache_manifest_sha256":model, "model_card_license_hint":"synthetic only",
        "executed_families":["decode"], "executed_methods":{"decode":"ffmpeg-cli"},
        "observations":[], "diagnostics":[]
    });
    let mutation = match change {
        Change::Stable | Change::Before => "",
        Change::During => "original.rename(root / 'source.before'); (root / 'replacement.bin').rename(original)\n(root / 'swap.done').touch(exist_ok=False)\n",
        #[cfg(target_os = "linux")]
        Change::Fifo => "original.rename(root / 'source.before'); os.mkfifo(original)\n(root / 'swap.done').touch(exist_ok=False)\n",
    };
    fs::write(
        &script,
        format!(
            concat!(
        "import os, sys\nfrom pathlib import Path\nroot = Path(__file__).resolve().parent\n",
        "original = root / 'source.bin'\nsnap = Path(sys.argv[sys.argv.index('--media') + 1])\n",
        "assert snap != original and snap.read_bytes() == b'synthetic beta7 source\\n'\n",
        "(root / 'snapshot.seen').write_text(str(snap), encoding='utf-8')\n",
        "{}print({:?})\n"
    ),
            mutation,
            payload.to_string()
        ),
    )
    .unwrap();
    if matches!(change, Change::Before | Change::During) {
        fs::write(root.join("replacement.bin"), SOURCE).unwrap();
    }

    let issuer = ReceiptIssuer::from_key_bytes("beta7-ingress", &[75; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("beta7-ingress", &[75; 32]).unwrap();
    let request = MediaSourceRequest {
        kind: MediaSourceKind::LocalUserProvided,
        locator: source.to_str().unwrap().into(),
        user_asserts_rights: true,
        media_bytes_requested: true,
        network_requested: false,
    };
    let pending = authorize_media_source(&request);
    let wire = issuer
        .issue(
            ReceiptClass::MediaSource,
            SubjectRevision::checked(pending.source_snapshot_sha256.as_ref().unwrap()).unwrap(),
            ReceiptScope::checked(media_source_scope(request.kind)).unwrap(),
            media_source_payload_digest(&request, &pending).unwrap(),
            1,
            None,
            None,
        )
        .unwrap();
    let authority =
        authorize_media_source_attested(&request, wire, &verifier, &mut ReplayGuard::default(), 1)
            .unwrap();
    if matches!(change, Change::Before) {
        fs::rename(&source, root.join("source.before")).unwrap();
        fs::rename(root.join("replacement.bin"), &source).unwrap();
    }
    let config = MediaBackendConfig {
        python_executable: python(root),
        backend_script: script.clone(),
        ffmpeg_executable: ffmpeg,
        model_cache: models,
        timeout_seconds: 10,
    };
    let descriptor = BackendDescriptor::configured(
        BackendFamily::Decode,
        BACKEND,
        Some(script),
        Some(model),
        Some("synthetic only".into()),
    );
    let run = if v3 {
        execute_local_media_backend_attested_v3
    } else {
        execute_local_media_backend_attested
    };
    let mut replay = ReplayGuard::default();
    let result = run(
        &authority,
        &descriptor,
        &config,
        &MediaResourceBudget::default(),
        &issuer,
        &verifier,
        &mut replay,
        1,
    )
    .map(|receipt| {
        receipt
            .authenticity_attestation
            .unwrap()
            .claims()
            .scope()
            .as_str()
            .to_owned()
    });
    let snapshot = fs::read_to_string(root.join("snapshot.seen"))
        .ok()
        .map(PathBuf::from);
    ResultFacts {
        result,
        replay: replay.observed_count(),
        snapshot_read: snapshot.is_some(),
        mutation_observed: root.join("swap.done").is_file(),
        snapshot_closed: snapshot.as_ref().is_none_or(|path| !path.exists()),
    }
}

#[test]
fn beta7_stable_regular_source_preserves_both_receipt_domains() {
    for v3 in [false, true] {
        let facts = execute(&owner_root(), v3, Change::Stable);
        let version = if v3 { "v3" } else { "v2" };
        assert_eq!(
            facts.result,
            Ok(format!("media/backend/{version}/{BACKEND}"))
        );
        assert_eq!(facts.replay, 1);
        assert!(facts.snapshot_read && facts.snapshot_closed);
    }
}

#[test]
fn beta7_equal_byte_replacement_before_execution_starts_no_backend() {
    let cases = [false, true].map(|v3| (v3, execute(&owner_root(), v3, Change::Before)));
    for (v3, facts) in cases {
        assert_eq!(
            facts.result,
            Err(MediaError::SourceSnapshotChanged),
            "profile v3={v3} accepted another regular source occurrence at the same path"
        );
        assert!(
            !facts.snapshot_read,
            "a backend started for the pre-execution replacement"
        );
        assert_eq!(facts.replay, 0);
    }
}

#[test]
fn beta7_equal_byte_replacement_during_snapshot_use_is_not_signed() {
    let cases = [false, true].map(|v3| (v3, execute(&owner_root(), v3, Change::During)));
    for (v3, facts) in cases {
        assert!(
            facts.snapshot_read && facts.mutation_observed,
            "the actual backend did not exercise the cut"
        );
        assert!(facts.snapshot_closed, "the snapshot was not reclaimed");
        assert_eq!(
            facts.result,
            Err(MediaError::SourceSnapshotChanged),
            "profile v3={v3} issued a result after an equal-byte physical replacement"
        );
        assert_eq!(facts.replay, 0);
    }
}

// The watchdog lives outside the worker that may block reopening the writerless FIFO.
// The marker demonstrates that capture, actual backend start and the deliberate swap all occurred.
#[cfg(target_os = "linux")]
fn fifo(v3: bool, test: &str) {
    if std::env::var_os(WORKER_ROOT).as_deref() == Some(std::ffi::OsStr::new("1")) {
        let root = std::env::current_dir().unwrap().canonicalize().unwrap();
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(root.parent(), Some(temporary.as_path()));
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("epistesys-beta7-media-ingress-"));
        let case = root.join("worker-case");
        fs::create_dir(&case).expect("worker may only use a newly created fixture");
        let facts = execute(&case, v3, Change::Fifo);
        assert!(facts.snapshot_read && facts.mutation_observed && facts.snapshot_closed);
        assert!(
            matches!(
                facts.result,
                Err(MediaError::SourceSnapshotChanged | MediaError::SourceSnapshotUnavailable)
            ),
            "unexpected result for a writerless FIFO: {:?}",
            facts.result
        );
        assert_eq!(facts.replay, 0);
        return;
    }
    let root = owner_root();
    let exe = std::env::current_exe().unwrap();
    let mut env = std::env::vars_os()
        .filter(|(k, _)| k != WORKER_ROOT)
        .collect::<Vec<_>>();
    env.push((WORKER_ROOT.into(), "1".into()));
    let worker = run_bounded(RunRequest {
        executable: exe.as_os_str(),
        arguments: &["--exact".into(), test.into(), "--nocapture".into()],
        current_dir: &root,
        environment: &env,
        stdin: None,
        timeout: Duration::from_secs(15),
        stdout_limit: 8192,
        stderr_limit: 8192,
    })
    .unwrap();
    assert!(
        root.join("worker-case/swap.done").is_file(),
        "fixture never reached the FIFO swap: {}",
        String::from_utf8_lossy(&worker.stderr)
    );
    assert!(worker.io_cleanup_complete && worker.process_tree_kill_verified);
    assert!(
        !worker.timed_out,
        "execution's post-child original-source hash blocked opening the writerless FIFO"
    );
    assert_eq!(
        worker.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&worker.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn beta7_v2_writerless_fifo_after_backend_is_bounded() {
    fifo(false, "beta7_v2_writerless_fifo_after_backend_is_bounded");
}

#[cfg(target_os = "linux")]
#[test]
fn beta7_v3_writerless_fifo_after_backend_is_bounded() {
    fifo(true, "beta7_v3_writerless_fifo_after_backend_is_bounded");
}
