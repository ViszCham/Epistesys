//! Actual canonical owners versus local historical binary input, never host proof.
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "tlcx40_canonical_support/mod.rs"]
mod support;
use lc631_core::context::SourceRevision;
use lc631_runtime::{
    canonical_solve::{
        replay::{
            capture_current, persist_current, with_fresh_resume, with_replay_input, ArtifactKey,
            CanonicalArtifactReader, ReplayGoal, ReplayProfile,
        },
        solve_with_services, CanonicalServices, DecisionBudget, GpuPolicy, NativeEvidenceKind,
        SExprLimits,
    },
    resource::{
        Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
        ResourceRequest,
    },
};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn path(label: &str) -> PathBuf {
    std::env::temp_dir().canonicalize().unwrap().join(format!(
        "epistesys-tlcx95-canonical-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn amount(kind: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap())
}
fn profile() -> ReplayProfile {
    ReplayProfile::checked(ReplayGoal::NoGoal, false, false).unwrap()
}

#[test]
fn tlcx95_archive_key_is_exact_source_and_profile_not_a_path_or_authority() {
    let profile = profile();
    let key = profile.key(support::SAT);
    assert_eq!(ArtifactKey::parse(&key.to_string()).unwrap(), key);
    assert_ne!(profile.key("different source"), key);
    assert!(ReplayProfile::checked(ReplayGoal::NoGoal, false, true).is_err());
    for value in [
        "local:decision-1",
        "../../outside",
        "canonical:00:00",
        "true",
    ] {
        assert!(ArtifactKey::parse(value).is_err());
    }
}

#[test]
fn tlcx95_archive_reader_missing_root_is_read_only_and_source_preadmitted() {
    let root = path("absent");
    let key = profile().key(support::SAT);
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            95,
            SourceRevision::of(support::SAT).digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reserved = run
        .reserve(&[
            amount(ResourceKind::Ram, 16 * 1024 * 1024),
            amount(ResourceKind::Output, 16 * 1024 * 1024),
        ])
        .unwrap();
    {
        let view = reserved.lease();
        assert!(CanonicalArtifactReader::open(&root, key, &view).is_err());
        assert!(!root.exists());
        let wrong = profile().key("wrong expected source");
        assert!(CanonicalArtifactReader::open(&root, wrong, &view).is_err());
        assert!(!root.exists());
    }
    reserved.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
#[ignore = "actual pinned Z3: canonical save, old-owner teardown, historical read, fresh solve"]
fn tlcx95_actual_native_artifact_roundtrip_reexecutes_and_rejects_stale_read_owner() {
    let registry = support::native_registry();
    let root = path("native");
    let profile = profile();
    let key = profile.key(support::SAT);
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let mut first = broker
        .begin_run(
            95,
            SourceRevision::of(support::SAT).digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut first_reservation = first
        .reserve(&[
            amount(ResourceKind::Ram, 96 * 1024 * 1024),
            amount(ResourceKind::Output, 16 * 1024 * 1024),
            amount(ResourceKind::Disk, 1024 * 1024),
            amount(ResourceKind::Child, 1),
            amount(ResourceKind::Cleanup, 1024 * 1024),
        ])
        .unwrap();
    {
        let view = first_reservation.lease();
        with_replay_input(support::SAT, profile, &view, |request| {
            let decision = solve_with_services(
                request,
                CanonicalServices {
                    registry: Some(&registry),
                    broker: &broker,
                    current: &view,
                    execution: support::execution(),
                    parser: SExprLimits::checked(65536, 4096, 64).unwrap(),
                    working_directory: &cwd,
                    budget: DecisionBudget::checked(65536).unwrap(),
                    optional_gpu: GpuPolicy::NotRequested,
                },
            )
            .unwrap();
            assert_eq!(decision.native_kind(), NativeEvidenceKind::ModelRechecked);
            assert_eq!(decision.stored_results(), 1);
            let captured =
                capture_current(&decision, support::SAT, profile, &broker, &view).unwrap();
            assert_eq!(captured.key(), key);
            persist_current(&root, &captured, &view).unwrap();
            Ok(())
        })
        .unwrap();
    }
    // Every old parser, decision, actual stdout/model and lease is gone here.
    first_reservation.release_confirmed().unwrap();
    first.close_confirmed().unwrap();
    let cancellation = Cancellation::default();
    let fresh = broker
        .begin_run(
            96,
            SourceRevision::of(support::SAT).digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancellation.clone(),
        )
        .unwrap();
    let fresh_reservation = fresh
        .reserve(&[
            amount(ResourceKind::Ram, 96 * 1024 * 1024),
            amount(ResourceKind::Output, 16 * 1024 * 1024),
            amount(ResourceKind::Child, 1),
            amount(ResourceKind::Cleanup, 1024 * 1024),
        ])
        .unwrap();
    {
        let view = fresh_reservation.lease();
        let reader = CanonicalArtifactReader::open(&root, key, &view).unwrap();
        let artifact = reader.read(&view).unwrap();
        let historical = artifact.inspect_current(&view).unwrap();
        assert_eq!(historical.source(), support::SAT);
        assert!(historical.historical_text().contains("x=-3"));
        assert!(!historical.current_model());
        assert!(!historical.authority_created());
        assert!(!historical.host_delivery_observed());
        let seen = with_fresh_resume(
            &artifact,
            CanonicalServices {
                registry: Some(&registry),
                broker: &broker,
                current: &view,
                execution: support::execution(),
                parser: SExprLimits::checked(65536, 4096, 64).unwrap(),
                working_directory: &cwd,
                budget: DecisionBudget::checked(65536).unwrap(),
                optional_gpu: GpuPolicy::NotRequested,
            },
            |result| {
                let decision = result.unwrap();
                assert_eq!(decision.native_kind(), NativeEvidenceKind::ModelRechecked);
                assert_eq!(decision.stored_results(), 1);
                assert!(decision
                    .native_model()
                    .unwrap()
                    .observation()
                    .invocation()
                    .is_some());
                decision
                    .project_current(&broker, &view)
                    .unwrap()
                    .text()
                    .contains("x=-3")
            },
        )
        .unwrap();
        assert!(seen);
        cancellation.cancel();
        assert!(artifact.inspect_current(&view).is_err());
    }
    // Synthetic owned binary remains available for central bounded cleanup.
    eprintln!(
        "owned canonical artifact fixture retained: {}",
        root.display()
    );
}
