use lc631_process_supervisor::{required_ram_bytes, RunRequest};
use std::ffi::{OsStr, OsString};
use std::time::Duration;

#[test]
fn tlcx05_declared_child_ram_includes_arguments_environment_and_encoding_metadata() {
    let directory = std::env::current_dir().unwrap();
    let base = RunRequest {
        executable: OsStr::new("declared-executable"),
        arguments: &[],
        current_dir: &directory,
        environment: &[],
        stdin: None,
        timeout: Duration::from_secs(1),
        stdout_limit: 1024,
        stderr_limit: 1024,
    };
    let base_ram = required_ram_bytes(&base).unwrap();
    let arguments = [OsString::from("arg".repeat(100))];
    let environment = [(
        OsString::from("MODEL_KIND"),
        OsString::from("v".repeat(200)),
    )];
    let larger = RunRequest {
        arguments: &arguments,
        environment: &environment,
        ..base
    };
    assert!(required_ram_bytes(&larger).unwrap() > base_ram + 500);
    assert!(
        base_ram > 2048 + 32_768,
        "launch memory was omitted from byte-buffer-only bound"
    );
}

#[test]
fn tlcx05_invalid_environment_has_no_successful_launch_plan() {
    let directory = std::env::current_dir().unwrap();
    let environment = vec![(OsString::from("KEY"), OsString::from("value")); 257];
    let request = RunRequest {
        executable: OsStr::new("not-started"),
        arguments: &[],
        current_dir: &directory,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(1),
        stdout_limit: 8,
        stderr_limit: 8,
    };
    assert!(required_ram_bytes(&request).is_err());
}

#[cfg(windows)]
#[test]
fn tlcx05_windows_counts_utf16_per_variable_not_a_false_32767_block_ceiling() {
    let directory = std::env::current_dir().unwrap();
    let environment = [
        (OsString::from("FIRST"), OsString::from("日".repeat(20_000))),
        (OsString::from("SECOND"), OsString::from("v".repeat(20_000))),
    ];
    let request = RunRequest {
        executable: OsStr::new("not-started"),
        arguments: &[],
        current_dir: &directory,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(1),
        stdout_limit: 8,
        stderr_limit: 8,
    };
    // Each value is below the per-variable limit. Total UTF-16 block units
    // exceed32767 but remain inside the separately declared engineering cap.
    assert!(required_ram_bytes(&request).is_ok());
}

#[cfg(windows)]
#[test]
fn tlcx05_windows_rejects_a_variable_beyond_32767_utf16_units() {
    let directory = std::env::current_dir().unwrap();
    let environment = [(OsString::from("KEY"), OsString::from("v".repeat(32_768)))];
    let request = RunRequest {
        executable: OsStr::new("not-started"),
        arguments: &[],
        current_dir: &directory,
        environment: &environment,
        stdin: None,
        timeout: Duration::from_secs(1),
        stdout_limit: 8,
        stderr_limit: 8,
    };
    assert!(required_ram_bytes(&request).is_err());
}
