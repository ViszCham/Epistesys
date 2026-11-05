//! Broker-backed analysis boundaries. No analysis -> runtime reverse dependency.
use lc631_analysis::{
    bounded_input::AnalysisText, run_bounded_tool_with_resources, BoundedToolRequest, ToolRunState,
};
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest, RunResources,
};
use std::io::{self, Cursor, Read};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

fn source() -> Digest32 {
    SourceRevision::of("reader task source").digest()
}
fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
fn owner() -> (ResourceBroker, RunResources) {
    let limits = ResourceLimits::checked([4 * 1024 * 1024, 0, 1024 * 1024, 0, 2, 2]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            source(),
            limits,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    (broker, run)
}

struct GrowingReader {
    calls: Arc<AtomicUsize>,
    bytes: Cursor<Vec<u8>>,
}
impl GrowingReader {
    fn stale_metadata_len(&self) -> usize {
        1
    }
}
impl Read for GrowingReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.bytes.read(output)
    }
}

#[test]
fn tlcx05_analysis_reader_rejects_growth_after_a_small_metadata_hint() {
    let (broker, mut run) = owner();
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, 16)]).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut reader = GrowingReader {
        calls: calls.clone(),
        bytes: Cursor::new(b"larger-than-cap".to_vec()),
    };
    assert_eq!(reader.stale_metadata_len(), 1);
    let view = reservation.lease();
    assert!(matches!(
        AnalysisText::read(&mut reader, &view, source(), 4),
        Err(ResourceError::Exhausted(ResourceKind::Ram))
    ));
    assert!(calls.load(Ordering::SeqCst) > 0);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_analysis_reader_retains_the_live_claim_until_the_text_drops() {
    let (_, mut run) = owner();
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, 16)]).unwrap();
    let view = reservation.lease();
    let text = AnalysisText::read(
        &mut Cursor::new("日本語".as_bytes()),
        &view,
        source(),
        "日本語".len(),
    )
    .unwrap();
    assert_eq!(text.as_str().unwrap(), "日本語");
    assert!(matches!(
        view.claim_buffer(ResourceKind::Ram),
        Err(ResourceError::Exhausted(ResourceKind::Ram))
    ));
    drop(text);
    drop(view.claim_buffer(ResourceKind::Ram).unwrap());
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_analysis_two_texts_share_one_larger_reservation_without_claiming_it_all() {
    let (_, mut run) = owner();
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, 16)]).unwrap();
    let view = reservation.lease();
    let first = AnalysisText::read(&mut Cursor::new(b"abcd"), &view, source(), 4).unwrap();
    let second = AnalysisText::read(&mut Cursor::new(b"efgh"), &view, source(), 4).unwrap();
    assert_eq!(first.as_str().unwrap(), "abcd");
    assert_eq!(second.as_str().unwrap(), "efgh");
    drop(first);
    drop(second);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_analysis_reader_rejects_foreign_source_before_reading() {
    let (_, mut run) = owner();
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, 16)]).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut reader = GrowingReader {
        calls: calls.clone(),
        bytes: Cursor::new(b"text".to_vec()),
    };
    let view = reservation.lease();
    assert!(matches!(
        AnalysisText::read(
            &mut reader,
            &view,
            SourceRevision::of("different task").digest(),
            8
        ),
        Err(ResourceError::SourceMismatch)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_analysis_reader_rejects_invalid_utf8_without_returning_partial_text() {
    let (_, mut run) = owner();
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, 16)]).unwrap();
    let view = reservation.lease();
    assert!(matches!(
        AnalysisText::read(&mut Cursor::new([0xe3, 0x81]), &view, source(), 8),
        Err(ResourceError::Io(io::ErrorKind::InvalidData))
    ));
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_analysis_process_rejects_an_unreserved_child() {
    let (_, mut run) = owner();
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 1024 * 1024),
            request(ResourceKind::Output, 8192),
            request(ResourceKind::Cleanup, 1),
        ])
        .unwrap();
    let executable = std::env::current_exe().unwrap();
    let executable = executable.to_str().unwrap();
    let view = reservation.lease();
    assert!(matches!(
        run_bounded_tool_with_resources(
            BoundedToolRequest {
                program: executable,
                args: &["--exact", "tlcx05_analysis_process_helper", "--nocapture"],
                stdin: Some(b"emit"),
                timeout_ms: 1000,
                stdout_limit: 4096,
                stderr_limit: 4096,
            },
            &view
        ),
        Err(ResourceError::Unreserved(ResourceKind::Child))
    ));
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_analysis_process_has_a_real_nonempty_reserved_positive() {
    let (_, mut run) = owner();
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 1024 * 1024),
            request(ResourceKind::Output, 8192),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 1),
        ])
        .unwrap();
    #[cfg(windows)]
    let executable = std::env::current_exe().unwrap();
    #[cfg(windows)]
    let executable = executable.to_str().unwrap();
    #[cfg(windows)]
    let arguments: &[&str] = &["--exact", "tlcx05_analysis_process_helper", "--nocapture"];
    // Preserve the real nonempty process observation without requiring a large
    // Rust test binary to be fingerprinted on a cold mounted filesystem.
    #[cfg(unix)]
    let (executable, arguments): (&str, &[&str]) = (
        "/bin/sh",
        &["-c", "IFS= read -r role; [ \"$role\" = emit ] || exit 7; printf 'broker-backed-analysis-observation\\n'"],
    );
    let view = reservation.lease();
    let output = run_bounded_tool_with_resources(
        BoundedToolRequest {
            program: executable,
            args: arguments,
            stdin: Some(b"emit"),
            timeout_ms: 2000,
            stdout_limit: 4096,
            stderr_limit: 4096,
        },
        &view,
    )
    .unwrap();
    assert_eq!(output.reported().state, ToolRunState::Observed);
    assert!(!output.stdout().is_empty());
    assert!(String::from_utf8_lossy(output.stdout()).contains("broker-backed-analysis-observation"));
    assert!(matches!(
        view.claim_buffer(ResourceKind::Ram),
        Err(ResourceError::Exhausted(ResourceKind::Ram))
    ));
    drop(output);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_analysis_process_helper() {
    if !std::env::args().any(|arg| arg == "--exact") {
        return;
    }
    let mut role = [0; 4];
    io::stdin().read_exact(&mut role).unwrap();
    assert_eq!(&role, b"emit");
    println!("broker-backed-analysis-observation");
}
