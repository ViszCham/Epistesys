use super::output::OutputConsumer;
use super::ConnectionError;

#[test]
fn tlcx05_output_consumer_borrows_exact_bytes_and_returns_only_its_owned_result() {
    let bytes = b"a nonempty candidate";
    let mut slot = OutputConsumer::new(|observed: &[u8]| {
        assert_eq!(observed.as_ptr(), bytes.as_ptr());
        assert_eq!(observed, bytes);
        Ok(observed.len())
    });
    slot.invoke(bytes).unwrap();
    assert_eq!(slot.finish().unwrap(), bytes.len());
}
#[test]
fn tlcx05_skipped_or_repeated_output_callback_cannot_produce_a_result() {
    let skipped = OutputConsumer::new(|_bytes: &[u8]| Ok(1_usize));
    assert!(matches!(
        skipped.finish(),
        Err(ConnectionError::OutputInvalid)
    ));
    let mut repeated = OutputConsumer::new(|_bytes: &[u8]| Ok(1_usize));
    repeated.invoke(b"one").unwrap();
    assert!(matches!(
        repeated.invoke(b"two"),
        Err(ConnectionError::OutputInvalid)
    ));
    assert!(matches!(
        repeated.finish(),
        Err(ConnectionError::OutputInvalid)
    ));
}
#[test]
fn tlcx05_ignored_output_consumer_failure_is_not_a_successful_result() {
    let mut slot =
        OutputConsumer::new(|_bytes: &[u8]| Err::<usize, _>(ConnectionError::OutputInvalid));
    assert!(slot.invoke(b"candidate").is_err());
    assert!(matches!(slot.finish(), Err(ConnectionError::OutputInvalid)));
}
