//! Borrowed grant callbacks cannot outlive their issuer. Keep one bounded copy
//! per actual capture check, with the original permit and complete revocation set.
use super::{ClockFault, Error};
use lc631_analysis::declared_build_capture::{
    declared_capture_digest, SnapshotAuthorityView, SnapshotCaptureError,
};
use lc631_core::{
    Action, AuthorityRevision, CallerOrigin, ExecutionPermit, ExecutionPermitContext,
    PermitValidation, PrincipalBinding,
};
use lc631_substrate::{resource::ResourceError, Digest32};

pub struct SnapshotGrantView<'a> {
    pub permit: &'a ExecutionPermit,
    pub context: ExecutionPermitContext<'a>,
    pub expected_principal: &'a PrincipalBinding,
}

pub trait SnapshotGrantSource: Send + Sync {
    fn with_current(
        &self,
        use_view: &mut dyn FnMut(SnapshotGrantView<'_>) -> Result<(), Error>,
    ) -> Result<(), Error>;
}

pub(super) struct Grant {
    permit: ExecutionPermit,
    principal: PrincipalBinding,
    scope: String,
    source: String,
    host: String,
    authority: AuthorityRevision,
    revocation: u64,
    revoked: Vec<String>,
    epoch: u64,
}

impl Grant {
    /// Collect one delivery; a callback error remains an error even when the
    /// provider swallows it. An invalid delivery count cannot cause a clock read.
    pub(super) fn receive(
        source: &dyn SnapshotGrantSource,
        scope: &str,
        revision: Digest32,
        limit: usize,
    ) -> Result<Self, Error> {
        let mut seen = false;
        let mut received = None;
        let mut rejected = None;
        let outer = source.with_current(&mut |input| {
            let result = if seen {
                Err(Error::Clock(ClockFault::BindingMismatch))
            } else {
                seen = true;
                Self::from_view(input, scope, revision, limit)
            };
            match result {
                Ok(grant) => {
                    received = Some(grant);
                    Ok(())
                }
                Err(error) => {
                    if rejected.is_none() {
                        rejected = Some(error.clone());
                    }
                    Err(error)
                }
            }
        });
        if let Some(error) = rejected {
            return Err(error);
        }
        outer?;
        received.ok_or(Error::Snapshot(SnapshotCaptureError::AuthorityUnavailable))
    }

    fn from_view(
        input: SnapshotGrantView<'_>,
        expected_scope: &str,
        expected_source: Digest32,
        limit: usize,
    ) -> Result<Self, Error> {
        let context = &input.context;
        bound_material(&input, limit)?;
        if context.caller_origin != CallerOrigin::HostVerifiedUser
            || context.action != Action::Test
            || input.permit.action() != Action::Test
            || context.scope != expected_scope
            || input.permit.scope() != expected_scope
            || context.principal != input.expected_principal
            || input.permit.principal_binding() != input.expected_principal
            || declared_capture_digest(context.source_revision).map_err(Error::Snapshot)?
                != expected_source
            || input.permit.validate(context) != PermitValidation::ValidGrant
        {
            return Err(Error::Snapshot(SnapshotCaptureError::Unauthorized));
        }
        // The actual producer also requires canonical digest strings for both
        // values before it can construct a stamp. Reject them before sampling.
        declared_capture_digest(context.trusted_host_fingerprint).map_err(Error::Snapshot)?;
        declared_capture_digest(input.expected_principal.as_str()).map_err(Error::Snapshot)?;
        let mut revoked = Vec::new();
        revoked
            .try_reserve_exact(context.revoked_permit_digests.len())
            .map_err(|_| allocation_failed())?;
        for digest in context.revoked_permit_digests {
            declared_capture_digest(digest).map_err(Error::Snapshot)?;
            revoked.push(copy_text(digest)?);
        }
        Ok(Self {
            permit: input.permit.clone(),
            principal: input.expected_principal.clone(),
            scope: copy_text(context.scope)?,
            source: copy_text(context.source_revision)?,
            host: copy_text(context.trusted_host_fingerprint)?,
            authority: context.authority_revision,
            revocation: context.revocation_revision,
            revoked,
            epoch: context.now_epoch,
        })
    }

    pub(super) fn expires(&self) -> u64 {
        self.permit.expires_at_epoch()
    }

    pub(super) fn at_sample(&mut self, epoch: u64) -> Result<(), Error> {
        self.epoch = epoch;
        if self.permit.validate(&self.view().context) != PermitValidation::ValidGrant {
            return Err(Error::Snapshot(SnapshotCaptureError::Unauthorized));
        }
        Ok(())
    }

    pub(super) fn view(&self) -> SnapshotAuthorityView<'_> {
        SnapshotAuthorityView {
            permit: &self.permit,
            expected_principal: &self.principal,
            context: ExecutionPermitContext {
                principal: &self.principal,
                action: Action::Test,
                scope: &self.scope,
                source_revision: &self.source,
                authority_revision: self.authority,
                now_epoch: self.epoch,
                revocation_revision: self.revocation,
                revoked_permit_digests: &self.revoked,
                caller_origin: CallerOrigin::HostVerifiedUser,
                trusted_host_fingerprint: &self.host,
            },
        }
    }
}

fn bound_material(input: &SnapshotGrantView<'_>, limit: usize) -> Result<(), Error> {
    let c = &input.context;
    if c.revoked_permit_digests.len() > 256 {
        return Err(Error::InputLimit);
    }
    // Count slice backing as well as text so many empty entries cannot evade
    // admission. The role-local backing also bounds the fixed private permit.
    let backing_bytes = c
        .revoked_permit_digests
        .len()
        .checked_mul(std::mem::size_of::<String>())
        .ok_or(Error::InputLimit)?;
    let mut remaining = limit.checked_sub(backing_bytes).ok_or(Error::InputLimit)?;
    for value in [
        c.principal.as_str(),
        input.expected_principal.as_str(),
        c.scope,
        c.source_revision,
        c.trusted_host_fingerprint,
        input.permit.scope(),
        input.permit.digest(),
        input.permit.principal_binding().as_str(),
    ]
    .into_iter()
    .chain(c.revoked_permit_digests.iter().map(String::as_str))
    {
        remaining = remaining.checked_sub(value.len()).ok_or(Error::InputLimit)?;
    }
    Ok(())
}

fn allocation_failed() -> Error {
    Error::Clock(ClockFault::Resource(ResourceError::AllocationFailed))
}

fn copy_text(value: &str) -> Result<String, Error> {
    let mut out = String::new();
    out.try_reserve_exact(value.len())
        .map_err(|_| allocation_failed())?;
    out.push_str(value);
    Ok(out)
}
