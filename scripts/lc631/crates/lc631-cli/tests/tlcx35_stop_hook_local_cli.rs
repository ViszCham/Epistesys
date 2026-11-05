//! A locally launched binary and synthetic stdin, not an actual host callback.
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn tlcx35_synthetic_stop_hook_persists_local_observation_without_host_claim() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("tlcx35-local-cli-{}-{unique}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for raw in [
        br#"{"session_id":"synthetic-1","cwd":".","hook_event_name":"Stop","model":"fixture","turn_id":"turn-1","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"local-output"}"#.as_slice(),
        br#"{"session_id":"synthetic-2","cwd":".","hook_event_name":"Stop","model":"fixture","turn_id":"turn-2","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"local-output"}"#.as_slice(),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_lc631"))
            .arg("lc631-host-stop-hook").env("PLUGIN_DATA", &root)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().unwrap();
        child.stdin.take().unwrap().write_all(raw).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(response["continue"], true);
    }
    let observations = root.join("host-output-hook").join("observations");
    let files: Vec<_> = std::fs::read_dir(&observations)
        .unwrap()
        .map(|item| item.unwrap().path())
        .collect();
    assert_eq!(files.len(), 2);
    for file in files {
        let observation: serde_json::Value =
            serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        assert_eq!(observation["automatic_host_callback_observed"], false);
        assert_eq!(observation["persistent_supervision_observed"], false);
    }
    // The uniquely created fixture directory is intentionally retained; no deletion.
}
