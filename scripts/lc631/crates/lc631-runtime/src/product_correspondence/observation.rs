//! 日本語: 実ownerを借用する有限payload対応検査。full-cost Kや選択認証ではない。
//! English: Borrowed native finite-payload correspondence, never full-cost K or eligibility.
//! Source inspection is separate from target-only decoding. Original reports stay borrowed.
#![forbid(unsafe_code)]
mod description;
mod risk;
mod tuple;
use super::{digest, DeferredReason, ProductError, ProductPlan, ReportedProduct};
use crate::{
    adversarial_measurement as adversarial, checked_tl_reduction as tl,
    description_measurement as description_owner,
};
use lc631_analysis::creativity_measurement::{ComponentReport, RationalInterval};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};

/// Closed actual owner ports. No detached report, replica or validation flag ingress.
#[derive(Clone, Copy)]
pub struct NativeOwnerRefs<'a, 'v> {
    pub baseline_description: &'a description_owner::OwnedDescription<'a, 'a, 'a, 'v>,
    pub target_description: &'a description_owner::OwnedDescription<'a, 'a, 'a, 'v>,
    pub baseline_risk: &'a adversarial::OwnedCounterRisk<'a, 'v>,
    pub baseline_responses: &'a adversarial::FrozenResponses<'a, 'v>,
    pub target_risk: &'a adversarial::OwnedCounterRisk<'a, 'v>,
    pub target_responses: &'a adversarial::FrozenResponses<'a, 'v>,
    pub tl_root: &'a tl::RootPlan<'a, 'a, 'v>,
    pub tl_candidate: &'a tl::TupleCandidate<'a, 'a, 'a, 'v>,
    pub tl_reduction: &'a tl::OwnedReduction<'a, 'a, 'a, 'a, 'v>,
}

/// Private construction retains exact ports, plan and products; current uses recheck them.
/// This token establishes only finite native correspondence under declared meanings.
pub struct OwnedNativeInspection<'a, 'v> {
    plan: &'a ProductPlan<'v>,
    baseline: &'a ReportedProduct<'v>,
    target: &'a ReportedProduct<'v>,
    owners: NativeOwnerRefs<'a, 'v>,
    retained: ResourceClaim<'v>,
}
/// Callback-scoped borrowed native reports. Basis, validation, gaps and gates are unmodified.
pub struct CurrentNativeInspection<'a> {
    baseline_description: &'a ComponentReport,
    target_description: &'a ComponentReport,
    baseline_risk: &'a ComponentReport,
    target_risk: &'a ComponentReport,
    translation: &'a tl::Report,
    product_baseline: Digest32,
    actual_tl_root: Digest32,
}
impl CurrentNativeInspection<'_> {
    pub const fn scalar_status(&self) -> DeferredReason {
        DeferredReason::FullCostOwnerMissing
    }
    pub const fn scalar(&self) -> Option<&RationalInterval> {
        None
    }
    pub const fn formula_evaluated(&self) -> bool {
        false
    }
    pub fn baseline_description_report(&self) -> &ComponentReport {
        self.baseline_description
    }
    pub fn target_description_report(&self) -> &ComponentReport {
        self.target_description
    }
    pub fn baseline_risk_report(&self) -> &ComponentReport {
        self.baseline_risk
    }
    pub fn target_risk_report(&self) -> &ComponentReport {
        self.target_risk
    }
    pub fn translation_report(&self) -> &tl::Report {
        self.translation
    }
    pub const fn product_baseline_digest(&self) -> Digest32 {
        self.product_baseline
    }
    pub const fn actual_tl_root_digest(&self) -> Digest32 {
        self.actual_tl_root
    }
}

pub fn inspect_native_current<'a, 'v>(
    plan: &'a ProductPlan<'v>,
    baseline: &'a ReportedProduct<'v>,
    target: &'a ReportedProduct<'v>,
    owners: NativeOwnerRefs<'a, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedNativeInspection<'a, 'v>, ProductError> {
    plan.check_current(view)?;
    let retained = claim(view, 4096)?;
    let inspection = OwnedNativeInspection {
        plan,
        baseline,
        target,
        owners,
        retained,
    };
    inspection.validate(view)?;
    Ok(inspection)
}
impl OwnedNativeInspection<'_, '_> {
    fn validate(&self, view: &ResourceReservationView<'_>) -> Result<Digest32, ProductError> {
        view.check().map_err(ProductError::Resource)?;
        self.retained.check().map_err(ProductError::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(ProductError::Resource(ResourceError::ForeignOwner));
        }
        self.plan.check_current(view)?;
        self.baseline.check_current(self.plan, view)?;
        self.target.check_current(self.plan, view)?;
        // Bounded codec/AST workspace is admitted before constructing comparison terms.
        // This is a logical reservation, not an allocator/RSS or 128MiB closure claim.
        let scratch = claim(view, 2 * 1024 * 1024)?;
        description::check(self.plan, self.baseline, self.target, self.owners, view)?;
        let source = risk::responses(
            self.plan,
            self.owners.baseline_responses,
            self.owners.baseline_risk,
            view,
        )?;
        let target = risk::responses(
            self.plan,
            self.owners.target_responses,
            self.owners.target_risk,
            view,
        )?;
        let root = tuple::check(
            self.plan,
            self.baseline,
            self.target,
            self.owners,
            source,
            target,
            view,
        )?;
        scratch.check().map_err(ProductError::Resource)?;
        self.retained.check().map_err(ProductError::Resource)?;
        Ok(root)
    }
    pub fn with_current<R>(
        &self,
        view: &ResourceReservationView<'_>,
        consume: impl for<'p> FnOnce(CurrentNativeInspection<'p>) -> R,
    ) -> Result<R, ProductError> {
        let root = self.validate(view)?;
        let o = self.owners;
        let current = CurrentNativeInspection {
            baseline_description: o
                .baseline_description
                .report_current(view)
                .map_err(description::error)?,
            target_description: o
                .target_description
                .report_current(view)
                .map_err(description::error)?,
            baseline_risk: o
                .baseline_risk
                .report_current(o.baseline_responses, view)
                .map_err(risk::error)?,
            target_risk: o
                .target_risk
                .report_current(o.target_responses, view)
                .map_err(risk::error)?,
            translation: o
                .tl_reduction
                .current(o.tl_root, o.tl_candidate, view)
                .map_err(tuple::error)?,
            product_baseline: digest(
                b"epistesys.stage-b.product-semantic-baseline.v1\0",
                self.baseline.semantic_bytes(),
            ),
            actual_tl_root: root,
        };
        let result = consume(current);
        // Revocation/cancellation during consumption cannot return a successful current read.
        // This is not rollback or an atomic revocation fence.
        self.validate(view)?;
        Ok(result)
    }
}
fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, ProductError> {
    view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes).map_err(ProductError::Resource)?,
    )])
    .map_err(ProductError::Resource)
}
fn allocation() -> ProductError {
    ProductError::Resource(ResourceError::AllocationFailed)
}
