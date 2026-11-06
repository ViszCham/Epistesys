//! Sealed adapter for the existing declared-build capture. Each current() obtains
//! a fresh Test grant and exactly one real sample from this role's capture actor.
use super::{grant::Grant, ClockFault, Error, RoleSession};
use lc631_analysis::declared_build_capture::{
    CurrentSnapshotAuthority, SnapshotAuthorityView, SnapshotCaptureError,
};
use lc631_substrate::resource::ResourceReservationView;

pub(super) struct CaptureAuthority<'a, 'o, 'p, 'v> {
    session: &'a RoleSession<'o, 'p, 'v>,
    resources: &'a ResourceReservationView<'a>,
    grant: Option<Grant>,
    pub(super) failure: Option<Error>,
}

impl<'a, 'o, 'p, 'v> CaptureAuthority<'a, 'o, 'p, 'v> {
    pub(super) fn new(
        session: &'a RoleSession<'o, 'p, 'v>,
        resources: &'a ResourceReservationView<'_>,
    ) -> Self {
        Self {
            session,
            resources,
            grant: None,
            failure: None,
        }
    }

    fn fresh(&self) -> Result<Grant, Error> {
        let session = self.session;
        session.check_current_binding(self.resources)?;
        let source = session
            .capture
            .snapshot_grant()
            .ok_or(Error::Clock(ClockFault::BindingMismatch))?;
        let mut grant = Grant::receive(
            source,
            &session.grant_scope,
            session.capture.binding().source,
            session.profile.metadata_limit(),
        )?;
        let epoch = session
            .capture
            .reader()
            .sample(self.resources, |sample| {
                session.record_sample(sample, grant.expires())
            })
            .map_err(Error::Clock)?;
        grant.at_sample(epoch)?;
        session.check_current_binding(self.resources)?;
        Ok(grant)
    }
}

impl CurrentSnapshotAuthority for CaptureAuthority<'_, '_, '_, '_> {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        if let Some(error) = &self.failure {
            return Err(snapshot_error(error));
        }
        // Release the preceding material before admitting a new provider delivery.
        self.grant = None;
        match self.fresh() {
            Ok(grant) => Ok(self.grant.insert(grant).view()),
            Err(error) => {
                let snapshot = snapshot_error(&error);
                self.failure = Some(error);
                Err(snapshot)
            }
        }
    }
}

// The actual producer has its own error vocabulary. Keep the exact provider
// error alongside its original SnapshotCaptureError in CaptureAttempt.
fn snapshot_error(error: &Error) -> SnapshotCaptureError {
    match error {
        Error::Snapshot(original) => original.clone(),
        Error::InputLimit => SnapshotCaptureError::InventoryLimit,
        Error::Clock(ClockFault::ClockRollback) => SnapshotCaptureError::ClockRollback,
        Error::Clock(ClockFault::Resource(resource)) => {
            SnapshotCaptureError::Resource(resource.clone())
        }
        Error::Clock(ClockFault::BindingMismatch) => SnapshotCaptureError::BindingMismatch,
        Error::Clock(ClockFault::ObservedExpiry) => SnapshotCaptureError::Unauthorized,
        _ => SnapshotCaptureError::AuthorityUnavailable,
    }
}
