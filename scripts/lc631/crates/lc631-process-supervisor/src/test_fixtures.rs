//! Synthetic bounded lease fixtures for downstream integration tests.
//!
//! These views exercise per-lease claim accounting only. They are not backed by
//! a process-global quota broker and do not establish aggregate runtime limits.

use crate::{
    ResourceAmount, ResourceError, ResourceKind, ResourceRequest, ResourceReservationView,
};
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceLease, ResourceLeaseIssuer,
};
use lc631_substrate::Digest32;
use std::time::Duration;

const TEST_LEASE_BYTES_PER_KIND: u64 = 16 * 1024 * 1024;
const TEST_LEASE_TIMEOUT: Duration = Duration::from_secs(60);

pub struct BoundedTestLease {
    lease: ResourceLease,
}

impl BoundedTestLease {
    pub fn view(&self) -> ResourceReservationView<'_> {
        self.lease.view()
    }

    pub fn close_confirmed(&self) -> Result<(), ResourceError> {
        self.lease.close_confirmed()
    }
}

/// Construct a fixed-quota, locally-issued lease for downstream host tests.
/// `run_id` must be nonzero. This does not create a broker or aggregate quota.
pub fn bounded_test_lease(run_id: u64) -> Result<BoundedTestLease, ResourceError> {
    let issuer = ResourceLeaseIssuer::new()?;
    let amount = ResourceAmount::checked(TEST_LEASE_BYTES_PER_KIND)?;
    let requests = [
        ResourceRequest::new(ResourceKind::Ram, amount),
        ResourceRequest::new(ResourceKind::Disk, amount),
        ResourceRequest::new(ResourceKind::Output, amount),
    ];
    let report = ReportedResourceLease::checked(
        run_id,
        Digest32::from_bytes([0x7f; 32]),
        &requests,
        Deadline::after(TEST_LEASE_TIMEOUT)?,
        Cancellation::default(),
    )?;
    let lease = issuer.issue_admitted(run_id, report)?;
    Ok(BoundedTestLease { lease })
}
