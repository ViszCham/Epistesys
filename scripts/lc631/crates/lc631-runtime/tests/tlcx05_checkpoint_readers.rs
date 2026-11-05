//! Journal record bytes are capped on the actual stream, never on metadata.
use lc631_analysis::read_checkpoint_record_with_resources;
use lc631_core::context::SourceRevision;
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use std::io::{Cursor, Read};
use std::time::Duration;

#[test]
fn tlcx05_checkpoint_record_actual_growth_is_rejected_and_exact_cap_remains_valid() {
    let source = SourceRevision::of("checkpoint source").digest();
    let limits = ResourceLimits::checked([32768, 0, 0, 0, 0, 0]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(32768).unwrap(),
        )])
        .unwrap();
    let view = reservation.lease();
    let mut grown = Cursor::new(vec![b' '; 16385]);
    assert!(matches!(
        read_checkpoint_record_with_resources(&mut grown, source, &view),
        Err(ResourceError::Exhausted(ResourceKind::Ram))
    ));
    assert_eq!(grown.position(), 16385);
    let mut exact = Cursor::new(vec![b' '; 16384]);
    let record = read_checkpoint_record_with_resources(&mut exact, source, &view).unwrap();
    assert_eq!(record.as_bytes().len(), 16384);
    drop(record);
    let mut untouched = Cursor::new(b"{}");
    assert!(matches!(
        read_checkpoint_record_with_resources(
            &mut untouched,
            SourceRevision::of("foreign").digest(),
            &view
        ),
        Err(ResourceError::SourceMismatch)
    ));
    assert_eq!(untouched.position(), 0);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

struct CancelsDuringRead {
    cancel: Cancellation,
    read: bool,
}
impl Read for CancelsDuringRead {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        if self.read {
            return Ok(0);
        }
        target[0] = b' ';
        self.read = true;
        self.cancel.cancel();
        Ok(1)
    }
}
#[test]
fn tlcx05_checkpoint_cancellation_during_actual_read_cannot_publish_partial_record() {
    let source = SourceRevision::of("checkpoint cancellation").digest();
    let limits = ResourceLimits::checked([16384, 0, 0, 0, 0, 0]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(16384).unwrap(),
        )])
        .unwrap();
    let view = reservation.lease();
    assert!(matches!(
        read_checkpoint_record_with_resources(
            &mut CancelsDuringRead {
                cancel,
                read: false
            },
            source,
            &view
        ),
        Err(ResourceError::Cancelled)
    ));
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
