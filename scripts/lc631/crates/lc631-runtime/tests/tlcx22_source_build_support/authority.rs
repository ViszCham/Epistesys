//! Local configured test provider, NOT an observed host user or independent issuer.
use super::profiles::TASK;
use lc631_analysis::{source_build_origin as b, declared_build_capture::*};
use lc631_core::*;
use lc631_receipt_kernel::*;
use std::{cell::Cell, rc::Rc};
#[derive(Clone, Copy)]
pub enum Cut { None, Revoked, Expired, RollbackAfterFirst, RejectAtLink, RejectAfterLink }
pub struct Authority<'a> {
    pub issuer: &'a ReceiptIssuer, pub verifier: &'a ReceiptVerifier, pub cut: Rc<Cell<Cut>>,
    permit: Option<ExecutionPermit>, scope: String, source: String, revoked: Vec<String>, calls: usize,
}
impl<'a> Authority<'a> {
    pub fn new(issuer: &'a ReceiptIssuer, verifier: &'a ReceiptVerifier, cut: Rc<Cell<Cut>>) -> Self {
        Self { issuer, verifier, cut, permit: None, scope: String::new(), source: stable_sha256(TASK),
            revoked: Vec::new(), calls: 0 }
    }
    fn set_scope(&mut self, scope: String) {
        self.scope = scope;
        self.permit = Some(permit(&self.scope, self.issuer, self.verifier));
        self.revoked = if matches!(self.cut.get(), Cut::Revoked) {
            vec![self.permit.as_ref().unwrap().digest().to_owned()]
        } else { Vec::new() };
    }
    fn context(&self, epoch: u64) -> ExecutionPermitContext<'_> {
        ExecutionPermitContext { principal: self.permit.as_ref().unwrap().principal_binding(), action: Action::Test,
            scope: &self.scope, source_revision: &self.source,
            authority_revision: AuthorityRevision(1), now_epoch: epoch, revocation_revision: 0,
            revoked_permit_digests: &self.revoked, caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: self.verifier.key_fingerprint() }
    }
}
impl b::CurrentBuildAuthority for Authority<'_> {
    fn current(&mut self, plan: &b::BuildPlan<'_, '_>, stage: b::BuildStage)
        -> Result<b::BuildAuthorityView<'_>, b::BuildError> {
        self.calls += 1;
        if matches!(self.cut.get(), Cut::RejectAtLink) && stage == b::BuildStage::Link {
            return Err(b::BuildError::AuthorityUnavailable);
        }
        if matches!(self.cut.get(), Cut::RejectAfterLink) && stage == b::BuildStage::CurrentOrigin {
            return Err(b::BuildError::AuthorityUnavailable);
        }
        self.set_scope(plan.execution_scope(stage));
        let epoch = match self.cut.get() { Cut::Expired => 401,
            Cut::RollbackAfterFirst if self.calls > 1 => 99, _ => 100 };
        let context = self.context(epoch);
        Ok(b::BuildAuthorityView { permit: self.permit.as_ref().unwrap(), context,
            tool_verifier: self.verifier })
    }
}
pub struct CaptureAuthority<'a> { inner: Authority<'a>, source: String }
impl<'a> CaptureAuthority<'a> {
    pub fn new(binding: &SourceReadBinding<'_>, issuer: &'a ReceiptIssuer, verifier: &'a ReceiptVerifier) -> Self {
        let mut inner = Authority::new(issuer, verifier, Rc::new(Cell::new(Cut::None)));
        inner.set_scope(binding.execution_scope());
        Self { inner, source: stable_sha256(TASK) }
    }
}
impl CurrentSnapshotAuthority for CaptureAuthority<'_> {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        let old = self.inner.context(100);
        let context = ExecutionPermitContext { source_revision: &self.source, ..old };
        Ok(SnapshotAuthorityView { permit: self.inner.permit.as_ref().unwrap(),
            expected_principal: self.inner.permit.as_ref().unwrap().principal_binding(), context })
    }
}
fn permit(scope: &str, issuer: &ReceiptIssuer, verifier: &ReceiptVerifier) -> ExecutionPermit {
    let parent = stable_sha256("source-build configured test provider, not host observation");
    let event = AuthorityEvent { action: Action::Test, source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant, span: SourceSpan { start: 0, end: TASK.len() }, scope: scope.into() };
    let subject = SubjectRevision::checked(stable_sha256(TASK)).unwrap();
    let rs = ReceiptScope::checked(format!("authority/test/{scope}")).unwrap();
    let payload = authority_event_payload_digest(&event);
    let raw = issuer.issue(ReceiptClass::Authority, subject.clone(), rs.clone(), payload.clone(),
        100, Some(400), Some(parent.clone())).unwrap();
    let verified = verifier.verify(raw, &ReceiptPolicy::exact(ReceiptClass::Authority, subject, rs, payload, 100)
        .with_parent(&parent), &mut ReplayGuard::default()).unwrap();
    ExecutionPermit::from_verified_user_event(VerifiedAuthorityEvent::checked(event, verified).unwrap(),
        TASK, AuthorityRevision(1), 100, 400, 0).unwrap()
}
