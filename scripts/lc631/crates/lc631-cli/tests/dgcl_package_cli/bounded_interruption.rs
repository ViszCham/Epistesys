use lc631_process_supervisor::{
    required_ram_bytes, run_bounded_with_resources_observed, RunRequest,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

const CAP: usize = 16 * 1024;

pub(super) struct RepairFixture<'a> {
    pub base: &'a Path,
    pub package: &'a Path,
    pub bundle: &'a Path,
    pub host_root: &'a Path,
    pub author_root: &'a Path,
    pub journal: &'a Path,
    pub head: &'a Path,
    pub source: &'a str,
    pub correct: &'a str,
}

#[derive(Debug)]
struct Observation {
    exit_code: Option<i32>,
    cancelled: bool,
    timed_out: bool,
    kill_attempted: bool,
    cleanup_complete: bool,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

impl Observation {
    fn rejection(&self) -> Option<&'static str> {
        if !self.cleanup_complete {
            Some("process or pipe cleanup unverified")
        } else if !self.cancelled || self.timed_out || !self.kill_attempted {
            Some("cancellation and tree termination not observed")
        } else if !self.exit_code.is_some_and(|code| code != 0) {
            Some("root exit unobserved or natural success")
        } else if self.stdout_truncated || self.stderr_truncated {
            Some("output limit exceeded")
        } else {
            None
        }
    }
}

fn redacted_stderr(bytes: &[u8], truncated: bool, cleanup_complete: bool) -> String {
    if truncated || !cleanup_complete {
        "incomplete stderr; details redacted".to_string()
    } else {
        super::safe_cli_failure_summary(bytes)
            .unwrap_or_else(|| "stderr details redacted".to_string())
    }
}

impl RepairFixture<'_> {
    pub(super) fn run(&self) {
        let executable = OsStr::new(env!("CARGO_BIN_EXE_lc631"));
        let args: Vec<OsString> = vec![
            "lc631-dgcl-package-repair".into(),
            "--execute".into(),
            "--prompt".into(),
            self.source.into(),
            "--repo".into(),
            self.package.as_os_str().into(),
            "--evidence-file".into(),
            self.bundle.as_os_str().into(),
            "--receipt-root".into(),
            self.host_root.as_os_str().into(),
            "--author-receipt-root".into(),
            self.author_root.as_os_str().into(),
            "--ledger-root".into(),
            self.journal.as_os_str().into(),
            "--head-root".into(),
            self.head.as_os_str().into(),
        ];
        // This supervisor replaces the environment; preserve the test's Cargo
        // and rustup configuration without including environment values in logs.
        let environment: Vec<(OsString, OsString)> = std::env::vars_os().collect();
        let request = RunRequest {
            executable,
            arguments: &args,
            current_dir: self.base,
            environment: &environment,
            stdin: None,
            timeout: Duration::from_secs(30),
            stdout_limit: CAP,
            stderr_limit: CAP,
        };
        let ram = required_ram_bytes(&request).expect("bounded child launch plan");
        let output = (2 * CAP) as u64;
        let limits = ResourceLimits::uniform(ram.max(output).max(1)).unwrap();
        let broker = ResourceBroker::new(limits).unwrap();
        let cancel = Cancellation::default();
        let mut run = broker
            .begin_run(
                1,
                Digest32::from_bytes([1; 32]),
                limits,
                Deadline::after(Duration::from_secs(35)).unwrap(),
                cancel.clone(),
            )
            .unwrap();
        let amount = |n| ResourceAmount::checked(n).unwrap();
        let mut reservation = run
            .reserve(&[
                ResourceRequest::new(ResourceKind::Ram, amount(ram)),
                ResourceRequest::new(ResourceKind::Output, amount(output)),
                ResourceRequest::new(ResourceKind::Child, amount(1)),
                ResourceRequest::new(ResourceKind::Cleanup, amount(1)),
            ])
            .unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        let monitor_deadline = Instant::now() + Duration::from_secs(30);
        let mut saw_edit = false;
        let result = std::thread::scope(|scope| {
            let reservation_ref = &reservation;
            scope.spawn(move || {
                let view = reservation_ref.lease();
                let result = run_bounded_with_resources_observed(request, &view).map(|run| {
                    let out = run.outcome();
                    let observation = Observation {
                        exit_code: out.exit_code,
                        cancelled: run.cancelled(),
                        timed_out: out.timed_out,
                        kill_attempted: out.process_tree_kill_attempted,
                        cleanup_complete: run.cleanup_complete(),
                        stdout_truncated: out.stdout_truncated,
                        stderr_truncated: out.stderr_truncated,
                    };
                    let stderr = redacted_stderr(
                        &out.stderr,
                        out.stderr_truncated,
                        observation.cleanup_complete,
                    );
                    (observation, out.diagnostic, stderr)
                });
                let _ = tx.send(result);
            });
            loop {
                match rx.recv_timeout(Duration::from_millis(5)) {
                    Ok(result) => break result,
                    Err(RecvTimeoutError::Disconnected) => panic!("supervisor worker failed"),
                    Err(RecvTimeoutError::Timeout) => {}
                }
                if std::fs::read_to_string(self.package.join("src/lib.rs"))
                    .ok()
                    .as_deref()
                    == Some(self.correct)
                {
                    saw_edit = true;
                    cancel.cancel();
                    break rx
                        .recv()
                        .expect("supervisor worker failed after cancellation");
                }
                if Instant::now() >= monitor_deadline {
                    cancel.cancel();
                    break rx.recv().expect("supervisor worker failed at deadline");
                }
            }
        });
        let (observation, diagnostic, stderr) =
            result.unwrap_or_else(|error| panic!("supervisor start/resource failure: {error:?}"));
        // Unknown cleanup must not be refunded as confirmed.
        assert!(
            observation.cleanup_complete,
            "unverified cleanup: {observation:?}; {diagnostic:?}; {stderr}"
        );
        reservation
            .release_confirmed()
            .expect("confirmed reservation release");
        run.close_confirmed().expect("confirmed run close");
        assert!(
            saw_edit,
            "edit not observed before child stopped: {observation:?}; {diagnostic:?}; {stderr}"
        );
        if let Some(reason) = observation.rejection() {
            panic!("{reason}: {observation:?}; {diagnostic:?}; {stderr}");
        }
        assert_latest_action_started(self.journal);
    }
}

fn assert_latest_action_started(journal: &Path) {
    let mut paths = std::fs::read_dir(journal)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("checkpoint-"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty(), "no stored checkpoint");
    for (index, path) in paths.iter().enumerate() {
        assert_eq!(
            path.file_name().unwrap(),
            OsStr::new(&format!("checkpoint-{index:020}.json")),
            "unexpected checkpoint sequence"
        );
    }
    let latest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(paths.last().unwrap()).unwrap()).unwrap();
    assert_eq!(
        latest["body"]["event_kind"], "action_started",
        "repair already observed before interruption"
    );
}

#[cfg(test)]
mod tests {
    use super::{redacted_stderr, Observation};

    fn accepted() -> Observation {
        Observation {
            exit_code: Some(-1),
            cancelled: true,
            timed_out: false,
            kill_attempted: true,
            cleanup_complete: true,
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn interruption_requires_complete_observation() {
        assert!(accepted().rejection().is_none());
        let mut value = accepted();
        value.exit_code = Some(0);
        assert!(value.rejection().is_some());
        value.exit_code = None;
        assert!(value.rejection().is_some());
        value = accepted();
        value.cleanup_complete = false;
        assert!(value.rejection().is_some());
        value = accepted();
        value.cancelled = false;
        assert!(value.rejection().is_some());
        value = accepted();
        value.kill_attempted = false;
        assert!(value.rejection().is_some());
        value = accepted();
        value.timed_out = true;
        assert!(value.rejection().is_some());
        value = accepted();
        value.stdout_truncated = true;
        assert!(value.rejection().is_some());
        value = accepted();
        value.stderr_truncated = true;
        assert!(value.rejection().is_some());
    }

    #[test]
    fn interruption_diagnostics_never_render_untrusted_details() {
        let secret = "secret-test-sentinel";
        let data = format!(
            r#"{{"schema_version":"lc631-cli-error.v1","error":"repair_resume:{secret}","effect_state":{{"source_effect":"may_have_applied","persistence_effect":"unknown"}}}}"#
        );
        for (truncated, complete) in [(false, true), (true, true), (false, false)] {
            let rendered = redacted_stderr(data.as_bytes(), truncated, complete);
            assert!(!rendered.contains(secret));
        }
        assert!(!redacted_stderr(secret.as_bytes(), false, true).contains(secret));
        assert!(redacted_stderr(data.as_bytes(), false, true).contains("repair_resume"));
    }
}
