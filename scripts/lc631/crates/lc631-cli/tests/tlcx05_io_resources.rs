use lc631_cli::io::read_text;
use lc631_runtime::resource::Digest32;
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::io::{self, Cursor, Read};
use std::time::Duration;

#[test]
fn tlcx05_cli_text_reader_consumes_the_injected_runtime_reservation() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(16).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(16).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(16).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(16).unwrap()),
        ])
        .unwrap();
    {
        let lease = reservation.lease();
        let text = read_text(
            &mut Cursor::new("日\r\n".as_bytes()),
            &lease,
            ResourceKind::Output,
        )
        .unwrap();
        assert_eq!(text.as_utf8().unwrap(), "日\r\n");
        assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 16);
        assert_eq!(broker.used(ResourceKind::Output).unwrap(), 16);
    }
    reservation.release_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_cli_invalid_utf8_is_typed_failure_without_implicit_refund() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(8).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(8).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(8).unwrap()),
        ])
        .unwrap();
    match read_text(
        &mut Cursor::new([0xff]),
        &reservation.lease(),
        ResourceKind::Output,
    ) {
        Err(error) => assert_eq!(error, ResourceError::Io(io::ErrorKind::InvalidData)),
        Ok(_) => panic!("invalid UTF-8 became a successful text read"),
    }
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 8);
    assert_eq!(broker.used(ResourceKind::Output).unwrap(), 8);
}

struct GrowingInput;

impl Read for GrowingInput {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        output.fill(b'x');
        Ok(output.len())
    }
}

#[test]
fn tlcx05_cli_input_growth_is_rejected_by_the_actual_read_not_metadata() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(8).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(8).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(8).unwrap()),
        ])
        .unwrap();
    match read_text(
        &mut GrowingInput,
        &reservation.lease(),
        ResourceKind::Output,
    ) {
        Err(error) => assert_eq!(error, ResourceError::Exhausted(ResourceKind::Output)),
        Ok(_) => panic!("growing input bypassed the byte reservation"),
    }
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 8);
}

#[test]
fn tlcx05_production_sampler_has_no_unbounded_command_output_or_sample_push() {
    let production = include_str!("../src/lib.rs");
    assert!(
        !production.contains(".output()"),
        "the production sampler still bypasses the supervised process owner"
    );
    assert!(
        production.contains("MAX_NVIDIA_SAMPLES"),
        "sampler retention must declare a finite cap before pushing samples"
    );
}

#[test]
fn tlcx05_host_reader_uses_the_same_actual_runtime_owner() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(16).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([9; 32]),
            ResourceLimits::uniform(16).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(16).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(16).unwrap()),
        ])
        .unwrap();
    {
        let view = reservation.lease();
        let text = lc631_host::bounded_reader::read_text(
            &mut Cursor::new("日\r\n".as_bytes()),
            &view,
            ResourceKind::Output,
        )
        .unwrap();
        assert_eq!(text.as_slice(), "日\r\n".as_bytes());
        assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 16);
    }
    reservation.release_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_host_invalid_utf8_and_growth_retain_the_actual_owner_charge() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(16).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([9; 32]),
            ResourceLimits::uniform(16).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    for invalid_utf8 in [true, false] {
        let reservation = run
            .reserve(&[
                ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(8).unwrap()),
                ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(8).unwrap()),
            ])
            .unwrap();
        let expected = if invalid_utf8 {
            ResourceError::Io(io::ErrorKind::InvalidData)
        } else {
            ResourceError::Exhausted(ResourceKind::Output)
        };
        let result = if invalid_utf8 {
            lc631_host::bounded_reader::read_text(
                &mut Cursor::new([0xff]),
                &reservation.lease(),
                ResourceKind::Output,
            )
            .map(|_| ())
        } else {
            lc631_host::bounded_reader::read_text(
                &mut GrowingInput,
                &reservation.lease(),
                ResourceKind::Output,
            )
            .map(|_| ())
        };
        assert_eq!(result, Err(expected));
        drop(reservation);
    }
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 16);
}

#[test]
fn tlcx05_host_key_creation_rejects_an_underfunded_view_before_side_effects() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([9; 32]),
            ResourceLimits::uniform(8).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(8).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(8).unwrap()),
        ])
        .unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "epistesys-tlcx05-underfunded-key-{}-{nonce}",
        std::process::id()
    ));
    assert!(!root.exists());
    let rejected =
        lc631_host::HostReceiptContext::load_or_create_with_resources(&root, &reservation.lease())
            .is_err();
    assert!(rejected, "live shape alone cannot fund a host key creation");
    assert!(
        !root.exists(),
        "rejected key creation must have no filesystem effects"
    );
}

#[test]
fn tlcx05_host_ledger_rejects_underfunded_view_before_nonce_or_filesystem_effects() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([9; 32]),
            ResourceLimits::uniform(8).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(8).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(8).unwrap()),
        ])
        .unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Bounded legacy fixture setup is separate from the ledger under test;
    // production CLI uses an injected runtime reservation instead.
    let host_root = std::env::temp_dir().join(format!(
        "epistesys-tlcx05-ledger-fixture-root-{}-{nonce}",
        std::process::id()
    ));
    let host = lc631_host::HostReceiptContext::load_or_create(&host_root).unwrap();
    let receipt = lc631_host::HostOutputReceipt::legacy_fixture_for_test(
        lc631_core::ArtifactId(1),
        lc631_core::TurnId(1),
        lc631_core::stable_sha256("fixture"),
    );
    let root = std::env::temp_dir().join(format!(
        "epistesys-tlcx05-ledger-noeffect-{}-{nonce}",
        std::process::id()
    ));
    let mut replay = lc631_receipt_kernel::ReplayGuard::default();
    let result = lc631_host::append_durable_replay_verified_with_resources(
        &root,
        &root.join("ledger.jsonl"),
        &receipt,
        host.verifier(),
        &mut replay,
        1,
        &reservation.lease(),
    );
    match result {
        Err(lc631_host::HostError::Io(reason)) => {
            assert!(reason.contains("host_resource"), "{reason}")
        }
        _ => panic!("ledger resource admission must reject an underfunded view first"),
    }
    assert_eq!(replay.observed_count(), 0);
    assert!(!root.exists());
}
