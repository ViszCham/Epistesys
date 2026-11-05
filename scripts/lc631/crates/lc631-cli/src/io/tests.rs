use super::*;

fn owner() -> ResourceBroker {
    ResourceBroker::new(
        ResourceLimits::checked([
            512 * 1024 * 1024,
            1024 * 1024 * 1024,
            128 * 1024 * 1024,
            64 * 1024 * 1024,
            2,
            4 * 1024 * 1024,
        ])
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn tlcx34_canonical_services_share_root_and_source_without_refunding_unknown_work() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    let source = "(set-logic QF_LIA) (assert true) (check-sat)";
    let result = resources.with_canonical_services(source, |shared, run, view| {
        assert_eq!(
            view.source_revision(),
            lc631_core::context::SourceRevision::of(source).digest()
        );
        assert!(shared.used(ResourceKind::Ram).unwrap() > 0);
        assert_eq!(
            shared.used(ResourceKind::Ram).unwrap(),
            owner.used(ResourceKind::Ram).unwrap()
        );
        let extra = run.reserve(&[request(ResourceKind::Child, 1).unwrap()]);
        assert!(matches!(
            extra,
            Err(ResourceError::Exhausted(ResourceKind::Child))
        ));
        view.record_cleanup_unknown();
        Err("canonical native cleanup unobserved".to_owned())
    });
    assert!(result
        .unwrap_err()
        .contains("canonical native cleanup unobserved"));
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 2);
    assert!(resources.finish().is_err());
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 2);
}

#[test]
fn tlcx34_canonical_output_cap_is_checked_before_reservation_refund() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    let result = resources.with_canonical_services("source", |_, _, _| {
        Ok(vec![0; crate::tlcx_commands::MAX_SUMMARY_BYTES + 1])
    });
    assert!(result.unwrap_err().contains("canonical_summary_limit"));
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 0);
    resources.finish().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx_path_services_pin_raw_config_and_reject_oversized_output() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    let config = "{\"profile\":\"given-k6-path-v1\"}";
    let bytes = resources
        .with_path_services(config, |view| {
            assert_eq!(
                view.source_revision(),
                lc631_core::context::SourceRevision::of(config).digest()
            );
            Ok(b"bounded summary".to_vec())
        })
        .unwrap();
    assert_eq!(bytes, b"bounded summary");
    let error = resources
        .with_path_services(config, |_| Ok(vec![0; 65_537]))
        .unwrap_err();
    assert!(error.contains("path_summary_limit"));
    resources.finish().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx_checked_tl_services_pin_raw_source_and_enforce_summary_limit() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    let source = "{\"profile\":\"given-finite-tuple\"}";
    let output = resources
        .with_checked_tl_view(source, |view| {
            assert_eq!(
                view.source_revision(),
                lc631_core::context::SourceRevision::of(source).digest()
            );
            Ok(b"bounded".to_vec())
        })
        .unwrap();
    assert_eq!(output, b"bounded");
    let error = resources
        .with_checked_tl_view(source, |_| Ok(vec![0; 65_537]))
        .unwrap_err();
    assert!(error.contains("checked_tl_summary_limit"));
    resources.finish().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_sequential_child_fixture() {
    println!("bounded child completed");
}

#[test]
fn tlcx05_three_sequential_confirmed_children_reuse_two_same_root_slots() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    for _ in 0..3 {
        resources
            .with_source_process_view("actual child source", 65_536, |view| {
                let executable = std::env::current_exe().unwrap();
                let cwd = std::env::current_dir().unwrap();
                let args = [
                    "--exact",
                    "io::tests::tlcx05_sequential_child_fixture",
                    "--nocapture",
                ]
                .map(std::ffi::OsString::from);
                let observed = lc631_process_supervisor::run_bounded_with_resources(
                    lc631_process_supervisor::RunRequest {
                        executable: executable.as_os_str(),
                        arguments: &args,
                        current_dir: &cwd,
                        environment: &[],
                        stdin: None,
                        timeout: Duration::from_secs(5),
                        stdout_limit: 4096,
                        stderr_limit: 4096,
                    },
                    view,
                )
                .map_err(resource_error)?;
                assert!(observed.cleanup_complete());
                assert_eq!(observed.outcome().exit_code, Some(0));
                assert!(String::from_utf8_lossy(&observed.outcome().stdout)
                    .contains("bounded child completed"));
                Ok(())
            })
            .unwrap();
    }
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 0);
    // Returned DTO payload accounting is still live until command finish.
    assert!(owner.used(ResourceKind::Ram).unwrap() > 0);
    resources.finish().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_unknown_child_cleanup_does_not_reuse_a_slot_or_refund_payload() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    for _ in 0..2 {
        let result = resources.with_source_process_view("unconfirmed child", 8192, |view| {
            view.record_cleanup_unknown();
            Err::<(), _>("child cleanup unobserved".to_string())
        });
        assert!(result.is_err());
    }
    let charged = owner.used(ResourceKind::Ram).unwrap();
    let result = resources.with_source_process_view("new child", 8192, |_| Ok(()));
    assert!(result.unwrap_err().contains("Exhausted(Child)"));
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 2);
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), charged);
    assert!(resources.finish().is_err());
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 2);
}

#[test]
fn tlcx05_cancelled_command_does_not_admit_another_child() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    resources.cancellation.cancel();
    let mut called = false;
    let result = resources.with_source_process_view("cancelled source", 8192, |_| {
        called = true;
        Ok(())
    });
    assert!(result.unwrap_err().contains("Cancelled"));
    assert!(!called);
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 0);
}

#[test]
fn tlcx05_journal_failure_preserves_primary_error_with_unknown_cleanup() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    let result = resources.with_journal_view("journal source", 1024, 0, |view| {
        view.record_cleanup_unknown();
        Err::<(), _>("primary journal handoff failure".to_string())
    });

    let error = result.unwrap_err();
    assert!(error.contains("primary journal handoff failure"), "{error}");
    assert!(
        error.contains("io_resource_cleanup:CleanupUnverified"),
        "{error}"
    );
    assert!(resources.finish().is_err());
    assert_ne!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_sample_payload_remains_charged_after_child_cleanup() {
    let owner = owner();
    let resources = CommandIo::begin(&owner).unwrap();
    // No child is launched in this local lifecycle test: the unused child
    // reservation is known reclaimable. Actual tree/pipe cleanup is tested
    // by the three sequential real-child test above.
    let mut child = resources
        .run
        .reserve(&[
            request(ResourceKind::Ram, 1024 * 1024).unwrap(),
            request(ResourceKind::Output, 80 * 1024).unwrap(),
            request(ResourceKind::Child, 1).unwrap(),
            request(ResourceKind::Cleanup, 65_536).unwrap(),
        ])
        .unwrap();
    let samples = resources.sample_storage().unwrap();
    samples.lock().unwrap().push([1, 2, 3, 4]);
    child.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 0);
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 4096 * 32);
    assert_eq!(samples.lock().unwrap().len(), 1);
    drop(samples);
    resources.finish().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}
