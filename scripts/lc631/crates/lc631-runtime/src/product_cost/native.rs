//! 日本語: 実 owner 入力と課金 A/P/I の current join。元 provenance は借用のまま。
//! English: Current actual-owner join to charged execution, with original provenance borrowed.
#![forbid(unsafe_code)]
mod current;
mod errors;
mod ports;
mod raw;
mod validation;
mod verified;
use super::term::{app, int, sym};
use super::*;
use crate::{
    adversarial_measurement as risk, checked_tl_reduction as tl, description_measurement as desc,
    product_correspondence::{NativeOwnerRefs, ProductPlan, ReportedProduct},
};
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    Digest32,
};

/// Closed borrowed ports; reported products are only full expected comparands.
pub struct NativeProductInput<'a, 'v> {
    pub plan: &'a ProductPlan<'v>,
    pub baseline: &'a ReportedProduct<'v>,
    pub target: &'a ReportedProduct<'v>,
    pub owners: NativeOwnerRefs<'a, 'v>,
}
#[derive(Clone, Copy)]
struct BaselinePorts<'a, 'v> {
    description: &'a desc::OwnedDescription<'a, 'a, 'a, 'v>,
    risk: &'a risk::OwnedCounterRisk<'a, 'v>,
    responses: &'a risk::FrozenResponses<'a, 'v>,
    root: &'a tl::RootPlan<'a, 'a, 'v>,
}
impl<'a, 'v> BaselinePorts<'a, 'v> {
    fn from_input(input: &NativeProductInput<'a, 'v>) -> Self {
        let o = input.owners;
        Self {
            description: o.baseline_description,
            risk: o.baseline_risk,
            responses: o.baseline_responses,
            root: o.tl_root,
        }
    }
    fn matches(&self, input: &NativeProductInput<'_, '_>) -> bool {
        let o = input.owners;
        std::ptr::eq(self.description, o.baseline_description)
            && std::ptr::eq(self.risk, o.baseline_risk)
            && std::ptr::eq(self.responses, o.baseline_responses)
            && std::ptr::eq(self.root, o.tl_root)
    }
}
pub struct NativeIdentityAdmissionCost {
    raw: Vec<u8>,
    revision: Digest32,
    work: u64,
}
impl NativeIdentityAdmissionCost {
    pub fn raw_envelope_bytes(&self) -> &[u8] {
        &self.raw
    }
    pub fn adapter_revision(&self) -> Digest32 {
        self.revision
    }
    pub fn work(&self) -> u64 {
        self.work
    }
}
pub struct MeaningCorrespondence {
    kind: String,
    path: String,
    forward: Vec<u8>,
    dictionary: Vec<u8>,
}
impl MeaningCorrespondence {
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn role_path(&self) -> &str {
        &self.path
    }
    pub fn forward_meaning_bytes(&self) -> &[u8] {
        &self.forward
    }
    pub fn dictionary_meaning_bytes(&self) -> &[u8] {
        &self.dictionary
    }
}
struct Verified<'v> {
    report: CostObservation,
    ledger: NativeIdentityAdmissionCost,
    rows: Vec<MeaningCorrespondence>,
    lower: OwnedExecutableBundleCost<'v>,
    retained: ResourceClaim<'v>,
    rows_claim: ResourceClaim<'v>,
}
/// K0 stores ONLY source-side ports; no candidate tuple/report/target dependency.
pub struct OwnedSemanticProductBaseline<'a, 'v> {
    profile: &'a DeclaredCostProfile<'v>,
    plan: &'a ProductPlan<'v>,
    reported: &'a ReportedProduct<'v>,
    ports: BaselinePorts<'a, 'v>,
    catalog: risk::CatalogBinding,
    root: Digest32,
    verified: Verified<'v>,
}
pub struct OwnedSemanticProductCost<'a, 'v> {
    baseline: &'a OwnedSemanticProductBaseline<'a, 'v>,
    input: &'a NativeProductInput<'a, 'v>,
    verified: Verified<'v>,
}
pub struct CurrentCostObservation<'o> {
    cost: &'o CostObservation,
    ledger: &'o NativeIdentityAdmissionCost,
    rows: &'o [MeaningCorrespondence],
    baseline_description: &'o ComponentReport,
    target_description: &'o ComponentReport,
    baseline_risk: &'o ComponentReport,
    target_risk: &'o ComponentReport,
    translation: &'o tl::Report,
}
impl CurrentCostObservation<'_> {
    pub fn cost(&self) -> &CostObservation {
        self.cost
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
    pub fn native_identity_admission(&self) -> &NativeIdentityAdmissionCost {
        self.ledger
    }
    pub fn meaning_correspondence(&self) -> &[MeaningCorrespondence] {
        self.rows
    }
}
pub fn capture_baseline_current<'a, 'v>(
    profile: &'a DeclaredCostProfile<'v>,
    input: &'a NativeProductInput<'a, 'v>,
    package: InlinePackage<'_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedSemanticProductBaseline<'a, 'v>, CostError> {
    profile.check_current(view)?;
    let mut workspace = execution::reserve(view, profile.limits)?; // before any raw clone/allocation
    workspace.admit_native_host(8 * 1024 * 1024)?;
    let ports = BaselinePorts::from_input(input);
    let actual = ports::baseline(input.plan, input.baseline, ports, view)?;
    let catalog = actual.catalog;
    let root = actual.root;
    let verified = verified::execute(
        profile,
        actual,
        input.baseline,
        package,
        None,
        view,
        workspace,
    )?;
    let owner = OwnedSemanticProductBaseline {
        profile,
        plan: input.plan,
        reported: input.baseline,
        ports,
        catalog,
        root,
        verified,
    };
    owner.check_current(view)?;
    Ok(owner)
}
pub fn measure_candidate_current<'a, 'v>(
    baseline: &'a OwnedSemanticProductBaseline<'a, 'v>,
    input: &'a NativeProductInput<'a, 'v>,
    package: InlinePackage<'_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedSemanticProductCost<'a, 'v>, CostError> {
    baseline.check_expected(input)?;
    baseline.check_current(view)?;
    let mut workspace = execution::reserve(view, baseline.profile.limits)?;
    workspace.admit_native_host(8 * 1024 * 1024)?;
    let actual = ports::candidate(input, baseline.catalog, view)?;
    if actual.root != baseline.root {
        return Err(CostError::CorrespondenceMismatch);
    }
    let verified = verified::execute(
        baseline.profile,
        actual,
        input.target,
        package,
        Some(baseline.historical_report().bits()),
        view,
        workspace,
    )?;
    let owner = OwnedSemanticProductCost {
        baseline,
        input,
        verified,
    };
    owner.check_current(view)?;
    Ok(owner)
}
