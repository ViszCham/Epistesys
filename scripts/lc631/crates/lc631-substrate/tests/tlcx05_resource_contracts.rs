use lc631_substrate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceError, ResourceKind, ResourceRequest,
};
use std::time::{Duration, Instant};

#[test]
fn tlcx05_zero_amount_and_zero_deadline_are_visible_invalid_config() {
    assert_eq!(
        ResourceAmount::checked(0).unwrap_err(),
        ResourceError::InvalidAmount
    );
    assert_eq!(
        Deadline::after(Duration::ZERO).unwrap_err(),
        ResourceError::InvalidDeadline
    );
    let amount = ResourceAmount::checked(8).unwrap();
    let request = ResourceRequest::new(ResourceKind::Ram, amount);
    assert_eq!(request.kind(), ResourceKind::Ram);
    assert_eq!(request.amount().get(), 8);
}

#[test]
fn tlcx05_cancel_is_shared_and_not_a_success_or_authority_state() {
    let cancellation = Cancellation::default();
    let observer = cancellation.clone();
    assert!(!observer.is_cancelled());
    cancellation.cancel();
    assert!(observer.is_cancelled());
}

#[test]
fn tlcx05_deadline_checks_the_actual_current_instant() {
    let deadline = Deadline::at(Instant::now() - Duration::from_millis(1));
    assert_eq!(
        deadline.check().unwrap_err(),
        ResourceError::DeadlineExceeded
    );
    let current = Deadline::after(Duration::from_secs(30)).unwrap();
    current.check().unwrap();
}
