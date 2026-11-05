//! Actual anchored configuration ingress; test-owned files are retained.
use lc631_cli::tlcx_commands::services::{
    file::{read_registry_file, ConfigFile},
    CONFIG_RAM_BYTES,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn admitted(path: &Path) -> bool {
    let limits = ResourceLimits::uniform(CONFIG_RAM_BYTES).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            34,
            Digest32::from_bytes([34; 32]),
            limits,
            Deadline::after(Duration::from_secs(5)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(CONFIG_RAM_BYTES).unwrap(),
        )])
        .unwrap();
    let accepted = {
        let view = reservation.lease();
        let request = ConfigFile::checked(path).unwrap();
        let accepted = read_registry_file(request, &view).is_ok();
        accepted
    };
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    accepted
}
fn owned_dir() -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "epistesys-tlcx34-config-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
fn config() -> String {
    format!(
        r#"{{"schema_version":"epistesys-solver-registry.v1","entries":[{{"id":"z3","profile":"smt_boolean_qf_lia","protocol":"smt_lib_payload_wire_v2","executable":"{}","version":"declared","binary_sha256":"{}","fixed_arguments":["-in","-smt2"],"caps":{{"max_argv_count":8,"max_argv_bytes":4096,"max_stdin_bytes":4096,"max_stdout_bytes":65536,"max_stderr_bytes":16384,"max_timeout_ms":1000,"max_ram_bytes":8388608,"max_disk_bytes":0,"max_cleanup_bytes":1,"max_child_count":1}}}}]}}"#,
        if cfg!(windows) {
            "C:/declared/z3.exe"
        } else {
            "/declared/z3"
        },
        "11".repeat(32)
    )
}
#[test]
fn tlcx34_services_file_rejects_relative_network_device_and_traversal_paths() {
    for path in [
        "registry.json",
        "//server/share/config.json",
        r"\\server\share\config.json",
        r"\\?\C:\config.json",
        r"\\.\pipe\config",
    ] {
        assert!(ConfigFile::checked(Path::new(path)).is_err(), "{path}");
    }
    let path = std::env::temp_dir().join("..").join("registry.json");
    assert!(ConfigFile::checked(&path).is_err());
}

#[cfg(windows)]
#[test]
fn tlcx34_services_file_rejects_windows_devices_ads_and_ambiguous_names_before_open() {
    for name in [
        "COM1",
        "cOm1.json",
        "NUL.json",
        "CON",
        "PRN.txt",
        "AUX",
        "CONIN$",
        "CONOUT$.json",
        "COM¹.json",
        "COM²",
        "LPT³.json",
        "LPT9",
        "registry.json:stream",
        "name.",
        "name ",
        "bad|name",
        "bad?name",
        "bad*name",
        "bad<name",
        "bad>name",
        "bad\"name",
        "bad\u{1f}name",
    ] {
        let path = Path::new("C:/declared").join(name);
        assert!(ConfigFile::checked(&path).is_err(), "{name}");
    }
}
#[test]
fn tlcx34_services_file_reads_real_file_rejects_directory_oversize_and_hardlink() {
    let root = owned_dir();
    let path = root.join("registry.json");
    std::fs::write(&path, config()).unwrap();
    assert!(admitted(&path));
    assert!(!admitted(&root));
    let oversized = root.join("oversized.json");
    std::fs::write(&oversized, vec![b' '; 65537]).unwrap();
    assert!(!admitted(&oversized));
    let alias = root.join("alias.json");
    std::fs::hard_link(&path, &alias).unwrap();
    assert!(!admitted(&alias));
}
#[cfg(unix)]
#[test]
fn tlcx34_services_file_rejects_actual_symbolic_link() {
    let root = owned_dir();
    let target = root.join("registry.json");
    std::fs::write(&target, config()).unwrap();
    let link = root.join("link.json");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(!admitted(&link));
}
