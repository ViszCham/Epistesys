//! Windows anchor containment is not an executed successful root replacement.
#![cfg(windows)]
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "tlcx40_canonical_support/mod.rs"]
mod native;
#[allow(dead_code)]
#[path = "tlcx95_replay_dispatch_support/mod.rs"]
mod support;
use lc631_process_supervisor::observe_native_dispatches;
use lc631_runtime::canonical_solve::replay::CanonicalArtifactReader;
use std::time::Duration;

#[test]
#[ignore = "explicit actual pinned Z3 setup; Windows live anchor must deny root rename"]
fn tlcx95_replay_windows_pinned_root_denies_swap_without_native_dispatch() {
    let registry = native::native_registry();
    let root = support::root("windows-pinned");
    support::save(&root, &registry);
    let moved = root.with_extension("retained-old-root");
    assert!(!moved.exists());
    let session = support::Session::new(Duration::from_secs(60));
    let view = session.reservation.lease();
    let reader =
        CanonicalArtifactReader::open(&root, support::profile().key(native::SAT), &view).unwrap();
    let artifact = reader.read(&view).unwrap();
    let trace = observe_native_dispatches(|| {
        let error = std::fs::rename(&root, &moved)
            .expect_err("Windows anchor unexpectedly allowed root rename");
        assert!(
            matches!(error.raw_os_error(), Some(5 | 32)),
            "unexpected rename error: {error}"
        );
        assert!(root.is_dir());
        assert!(!moved.exists());
        let history = artifact.inspect_current(&view).unwrap();
        assert!(!history.current_model());
        assert!(!history.authority_created());
    })
    .unwrap();
    support::assert_zero(&trace);
    eprintln!(
        "Windows swap blocked by live anchor; successful-swap path NOT exercised; retained={}",
        root.display()
    );
}
