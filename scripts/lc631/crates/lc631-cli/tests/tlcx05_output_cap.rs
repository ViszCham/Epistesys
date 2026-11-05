use lc631_cli::io::write_pretty_json;
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::time::Duration;

#[test]
fn tlcx05_oversized_serialization_writes_no_partial_public_output() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(64).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([4; 32]),
            ResourceLimits::uniform(64).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(64).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(64).unwrap()),
        ])
        .unwrap();
    let mut public = Vec::new();
    let result = write_pretty_json(
        &serde_json::json!({"text": "x".repeat(128)}),
        &reservation.lease(),
        &mut public,
    );
    assert_eq!(result, Err(ResourceError::Exhausted(ResourceKind::Output)));
    assert!(
        public.is_empty(),
        "no bytes may reach the sink before complete bounded encoding"
    );
}

#[test]
fn tlcx05_bounded_output_preserves_legacy_pretty_envelope_bytes_and_newline() {
    let broker = ResourceBroker::new(ResourceLimits::uniform(4096).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([4; 32]),
            ResourceLimits::uniform(4096).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(4096).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(4096).unwrap()),
        ])
        .unwrap();
    let envelope =
        lc631_wire::WireEnvelope::shadow("fixture", serde_json::json!({"text":"日\r\n"}));
    let mut expected = envelope.to_pretty_json().unwrap().into_bytes();
    expected.push(b'\n');
    let mut public = Vec::new();
    write_pretty_json(&envelope, &reservation.lease(), &mut public).unwrap();
    assert_eq!(public, expected);
    reservation.release_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Output).unwrap(), 0);
}

#[test]
fn tlcx05_encoding_failure_after_a_prefix_still_writes_no_public_bytes() {
    struct Broken;
    impl serde::Serialize for Broken {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeSeq;
            let mut sequence = serializer.serialize_seq(Some(2))?;
            sequence.serialize_element("encoded but not published")?;
            Err(<S::Error as serde::ser::Error>::custom(
                "deliberate fixture failure",
            ))
        }
    }
    let broker = ResourceBroker::new(ResourceLimits::uniform(4096).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            Digest32::from_bytes([4; 32]),
            ResourceLimits::uniform(4096).unwrap(),
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(4096).unwrap()),
            ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(4096).unwrap()),
        ])
        .unwrap();
    let mut public = Vec::new();
    assert!(write_pretty_json(&Broken, &reservation.lease(), &mut public).is_err());
    assert!(public.is_empty());
}
