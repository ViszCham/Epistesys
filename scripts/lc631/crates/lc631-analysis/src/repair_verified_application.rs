//! Borrowed observation boundary; not an execution permit or durable receipt.
use super::{DgclRepairApplication, DgclRepairRequest};
use lc631_receipt_kernel::VerifiedReceipt;

/// Created only by the repair coordinator after its application-binding,
/// current-target-lease and exact application-receipt checks succeed.
/// The lifetime prevents retaining the token beyond this callback. It issues
/// no authority, completion, persistence or delivery guarantee.
///
/// Public callers cannot construct it:
/// ```compile_fail
/// use lc631_analysis::DgclVerifiedRepairApplication;
/// let token = DgclVerifiedRepairApplication {
///     request: todo!(), application: todo!(), validated_receipt: todo!(),
///     accepted_digest: "unverified", checked_epoch: 0,
/// };
/// ```
/// It deliberately has no Clone implementation:
/// ```compile_fail
/// use lc631_analysis::DgclVerifiedRepairApplication;
/// fn duplicate(token: DgclVerifiedRepairApplication<'_>) {
///     let _ = Clone::clone(&token);
/// }
/// ```
/// Public reports cannot deserialize into it:
/// ```compile_fail
/// use lc631_analysis::DgclVerifiedRepairApplication;
/// let _: DgclVerifiedRepairApplication<'_> = serde_json::from_str("{}").unwrap();
/// ```
pub struct DgclVerifiedRepairApplication<'a> {
    request: &'a DgclRepairRequest,
    application: &'a DgclRepairApplication,
    validated_receipt: &'a VerifiedReceipt,
    accepted_digest: &'a str,
    checked_epoch: u64,
}

impl<'a> DgclVerifiedRepairApplication<'a> {
    pub(super) fn new(
        request: &'a DgclRepairRequest,
        application: &'a DgclRepairApplication,
        validated_receipt: &'a VerifiedReceipt,
        accepted_digest: &'a str,
        checked_epoch: u64,
    ) -> Self {
        Self {
            request,
            application,
            validated_receipt,
            accepted_digest,
            checked_epoch,
        }
    }

    pub const fn request(&self) -> &'a DgclRepairRequest {
        self.request
    }
    pub const fn application(&self) -> &'a DgclRepairApplication {
        self.application
    }
    pub const fn validated_receipt(&self) -> &'a VerifiedReceipt {
        self.validated_receipt
    }
    pub const fn validated_receipt_digest(&self) -> &'a str {
        self.accepted_digest
    }
    pub const fn checked_epoch(&self) -> u64 {
        self.checked_epoch
    }
}
