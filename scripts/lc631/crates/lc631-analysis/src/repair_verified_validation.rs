//! Accepted revalidation observation only; never authority for another process.
use super::{DgclRepairApplication, DgclRepairRequest, DgclRepairValidation};
use lc631_receipt_kernel::VerifiedReceipt;

/// Borrowed only during the coordinator callback, after fresh authority,
/// application/validation binding, recomputed closure and receipt checks.
/// Additional probes require their own current Test permission.
/// No receipt replay, durable completion or host observation is created.
///
/// ```compile_fail
/// use lc631_analysis::DgclVerifiedRepairValidation;
/// let token = DgclVerifiedRepairValidation {};
/// ```
/// ```compile_fail
/// use lc631_analysis::DgclVerifiedRepairValidation;
/// fn copy(x: DgclVerifiedRepairValidation<'_>) { let _ = Clone::clone(&x); }
/// ```
/// ```compile_fail
/// use lc631_analysis::DgclVerifiedRepairValidation;
/// let _: DgclVerifiedRepairValidation<'_> = serde_json::from_str("{}").unwrap();
/// ```
pub struct DgclVerifiedRepairValidation<'a> {
    request: &'a DgclRepairRequest,
    application: &'a DgclRepairApplication,
    validation: &'a DgclRepairValidation,
    receipt: &'a VerifiedReceipt,
    digest: &'a str,
    checked_epoch: u64,
}
impl<'a> DgclVerifiedRepairValidation<'a> {
    pub(super) fn new(
        request: &'a DgclRepairRequest,
        application: &'a DgclRepairApplication,
        validation: &'a DgclRepairValidation,
        receipt: &'a VerifiedReceipt,
        digest: &'a str,
        checked_epoch: u64,
    ) -> Self {
        Self { request, application, validation, receipt, digest, checked_epoch }
    }
    pub const fn request(&self) -> &'a DgclRepairRequest { self.request }
    pub const fn application(&self) -> &'a DgclRepairApplication { self.application }
    pub const fn validation(&self) -> &'a DgclRepairValidation { self.validation }
    pub const fn validated_receipt(&self) -> &'a VerifiedReceipt { self.receipt }
    pub const fn validated_receipt_digest(&self) -> &'a str { self.digest }
    pub const fn checked_epoch(&self) -> u64 { self.checked_epoch }
}
