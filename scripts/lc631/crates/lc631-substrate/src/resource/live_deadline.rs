//! One operational deadline shared by already-issued views and their run.
#![forbid(unsafe_code)]
use super::*;
pub struct LiveDeadline {
    value: Mutex<Deadline>,
}
pub struct DeadlineNarrowing {
    previous: Deadline,
    effective: Deadline,
}
impl DeadlineNarrowing {
    pub fn was_shortened(&self) -> bool {
        self.effective.0 < self.previous.0
    }
    pub fn effective(&self) -> Deadline {
        self.effective
    }
}
impl LiveDeadline {
    pub fn new(value: Deadline) -> Self {
        Self {
            value: Mutex::new(value),
        }
    }
    pub fn snapshot(&self) -> Result<Deadline, ResourceError> {
        self.value
            .lock()
            .map(|d| *d)
            .map_err(|_| ResourceError::Poisoned)
    }
    pub fn check(&self) -> Result<(), ResourceError> {
        self.snapshot()?.check()
    }
    pub fn remaining(&self) -> Result<Duration, ResourceError> {
        self.snapshot()?.remaining()
    }
    pub fn narrow(&self, external: Deadline) -> Result<DeadlineNarrowing, ResourceError> {
        let mut deadline = self.value.lock().map_err(|_| ResourceError::Poisoned)?;
        let previous = *deadline;
        if external.0 < deadline.0 {
            *deadline = external;
        }
        Ok(DeadlineNarrowing {
            previous,
            effective: *deadline,
        })
    }
}
impl ResourceLeaseIssuer {
    /// The broker supplies its actual shared run owner after quota admission.
    /// Creating another issuer/deadline cannot mutate that broker's old leases.
    pub fn issue_admitted_with_deadline(
        &self,
        reservation_id: u64,
        report: ReportedResourceLease,
        deadline: Arc<LiveDeadline>,
    ) -> Result<ResourceLease, ResourceError> {
        deadline.check()?;
        let mut lease = self.issue_admitted(reservation_id, report)?;
        lease.live_deadline = deadline;
        Ok(lease)
    }
}
