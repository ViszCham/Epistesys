//! 日本語: 登録compilerとは別に、各stageのfresh Testを検査する。
//! Fresh stage authority, independently checked from the registered compiler.
use super::super::{
    compiler_registration_payload, compiler_registration_scope, BuildError, BuildPlan,
    BuildServices, BuildStage,
};
use super::super::types::AuthorityStamp;
use crate::declared_build_capture::declared_capture_digest;
use lc631_core::{Action, CallerOrigin, PermitRejection, PermitValidation};
use lc631_receipt_kernel::{ReceiptClass, ReceiptPolicy, ReceiptScope, ReplayGuard, SubjectRevision};
use lc631_substrate::{resource::ResourceReservationView, Digest32};

pub(super) fn check(
    plan: &BuildPlan<'_, '_>, services: &mut BuildServices<'_, '_>, stage: BuildStage,
    view: &ResourceReservationView<'_>,
) -> Result<AuthorityStamp, BuildError> {
    plan.check(view)?;
    let current = services.authority.current(plan, stage)?;
    let context = &current.context;
    let limit = plan.input.limits.max_metadata_bytes;
    if context.revoked_permit_digests.len() > 256 { return Err(BuildError::InputLimit); }
    let mut bytes = context.revoked_permit_digests.len()
        .checked_mul(std::mem::size_of::<String>()).ok_or(BuildError::InputLimit)?;
    for text in [context.principal.as_str(), current.permit.principal_binding().as_str(),
        context.scope, current.permit.scope(), context.source_revision,
        context.trusted_host_fingerprint, current.tool_verifier.key_fingerprint(),
        current.permit.digest()]
        .into_iter().chain(context.revoked_permit_digests.iter().map(String::as_str))
    {
        bytes = bytes.checked_add(text.len()).ok_or(BuildError::InputLimit)?;
        if bytes > limit { return Err(BuildError::InputLimit); }
    }
    let scope = plan.execution_scope(stage);
    if context.action != Action::Test || current.permit.action() != Action::Test
        || context.caller_origin != CallerOrigin::HostVerifiedUser
        || context.scope != scope || current.permit.scope() != scope
        || context.principal != current.permit.principal_binding()
        || declared_capture_digest(context.source_revision)? != plan.input.binding.source()
        || context.trusted_host_fingerprint != current.tool_verifier.key_fingerprint()
    { return Err(BuildError::Unauthorized); }
    let stamp = AuthorityStamp {
        epoch: context.now_epoch, expires: current.permit.expires_at_epoch(),
        principal: declared_capture_digest(context.principal.as_str())?,
        host: declared_capture_digest(context.trusted_host_fingerprint)?,
        revocation: context.revocation_revision,
    };
    for digest in context.revoked_permit_digests { declared_capture_digest(digest)?; }
    match current.permit.validate(context) {
        PermitValidation::ValidGrant => {}
        PermitValidation::Rejected(PermitRejection::Expired) => {
            // The observed expiry cannot be replaced by a later lower epoch.
            plan.observe_authority(stamp)?;
            return Err(BuildError::Expired);
        }
        PermitValidation::Rejected(PermitRejection::NotYetValid) => {
            // Compare only to a previously trusted sample; never admit this grant.
            if plan.work.get().authority.is_some_and(|previous| stamp.epoch < previous.epoch) {
                return Err(BuildError::ClockRollback);
            }
            return Err(BuildError::Unauthorized);
        }
        _ => return Err(BuildError::Unauthorized),
    }
    let compiler = plan.input.compiler;
    let subject = SubjectRevision::checked(digest_text(compiler.expected_image))
        .map_err(|_| BuildError::CompilerRegistrationMismatch)?;
    let scope = ReceiptScope::checked(compiler_registration_scope(compiler))
        .map_err(|_| BuildError::CompilerRegistrationMismatch)?;
    let policy = ReceiptPolicy::exact(ReceiptClass::ToolExecution, subject, scope,
        compiler_registration_payload(compiler), stamp.epoch);
    current.tool_verifier.verify(plan.input.compiler_registration.clone(), &policy,
        &mut ReplayGuard::default()).map_err(|_| BuildError::CompilerRegistrationMismatch)?;
    plan.observe_authority(stamp)?;
    plan.check(view)?;
    Ok(stamp)
}

fn digest_text(digest: Digest32) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(71);
    text.push_str("sha256:");
    for byte in digest.as_bytes() { let _ = write!(text, "{byte:02x}"); }
    text
}
