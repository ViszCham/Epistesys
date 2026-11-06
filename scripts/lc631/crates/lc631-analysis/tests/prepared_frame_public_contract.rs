//! The additive SDK path must be reachable without bypassing shape checks.
use lc631_analysis::{
    run_prepared_tool_attempt, BoundedToolRequest, FrameDispatchBinding, FrameDispatchPurpose,
    PreparedToolFrame,
};
use lc631_substrate::Digest32;

#[test]
fn public_frame_constructor_keeps_rejection_and_binding() {
    let _entry = run_prepared_tool_attempt::<()>;
    let executable = std::env::current_exe().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let grant = format!("sha256:{}", "1".repeat(64));
    let request = || BoundedToolRequest {
        program: executable.to_str().unwrap(),
        args: &[],
        stdin: None,
        timeout_ms: 1000,
        stdout_limit: 1,
        stderr_limit: 1,
    };
    let binding = |source| FrameDispatchBinding {
        source,
        root_task: Digest32::from_bytes([2; 32]),
        frame: Digest32::from_bytes([3; 32]),
        purpose: FrameDispatchPurpose::CompilerIdentity,
        grant_receipt_digest: &grant,
    };
    assert!(PreparedToolFrame::checked(
        request(),
        &cwd,
        &[],
        binding(Digest32::from_bytes([0; 32]))
    )
    .is_err());
    let source = Digest32::from_bytes([1; 32]);
    let frame = PreparedToolFrame::checked(request(), &cwd, &[], binding(source)).unwrap();
    assert_eq!(frame.binding().source, source);
    let first = frame.invocation_digest().unwrap();
    let mut changed = binding(source);
    changed.frame = Digest32::from_bytes([4; 32]);
    let other = PreparedToolFrame::checked(request(), &cwd, &[], changed).unwrap();
    assert_ne!(first, other.invocation_digest().unwrap());
    // A structurally valid descriptor is not authenticated execution authority.
    // This test never invokes a child or invents a provider/permission receipt.
}
