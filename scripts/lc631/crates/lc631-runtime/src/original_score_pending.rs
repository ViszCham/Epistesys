//! 日本語: 実TL ownerを消費する原式preflight。未実装leafはPendingのまま。
//! English: Original-score preflight consuming actual TL owners; missing leaves stay pending.
//! This is NOT an evaluated F1/F2 score, eligibility, recommendation or authority.
//! No caller report, scalar, component mask or gate can be injected.
#![forbid(unsafe_code)]
pub mod owned_components;
use crate::checked_tl_reduction as tl;
use lc631_analysis::creativity_measurement::{
    formula::{FormulaProfile, OriginalFormula},
    ComponentMetric as M, RationalInterval,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

const RETAINED_RAM: u64 = 64 * 1024;
const F1: &[M] = &[
    M::DescriptionBits,
    M::ContextDistance,
    M::SemanticConfigurationDistance,
    M::Compatibility,
    M::Knownness,
];
const F2: &[M] = &[
    M::DescriptionBits,
    M::FrameworkDistance,
    M::SemanticConfigurationDistance,
    M::Compatibility,
    M::Coverage,
    M::Compression,
    M::Bridge,
    M::Prediction,
    M::Redundancy,
    M::Preserve,
    M::Rebind,
    M::Reframe,
    M::BreakCost,
    M::Explain,
    M::Design,
    M::Transfer,
    M::Solve,
    M::CounterexampleRisk,
    M::BridgeCost,
];

/// The strict registry, shared without constructing unrelated evidence owners.
pub(crate) fn required_metrics(formula: OriginalFormula) -> &'static [M] {
    match formula {
        OriginalFormula::F1V1 => F1,
        OriginalFormula::F2V1 => F2,
    }
}

#[derive(Debug)]
pub enum Error {
    Resource(ResourceError),
    Translation(tl::Error),
    ProfileMismatch,
    CandidateMismatch,
    InvalidCandidates,
}

/// A fixed declared preference captured without reading a target candidate.
/// The tuple TL reduction policy is an exact root digest, not a caller mask.
pub struct RootPlan<'f, 'r, 'i, 'p, 'v> {
    root: &'r tl::RootPlan<'i, 'p, 'v>,
    profile: &'f FormulaProfile,
    formula: OriginalFormula,
    retained: ResourceClaim<'v>,
}
impl<'f, 'r, 'i, 'p, 'v> RootPlan<'f, 'r, 'i, 'p, 'v> {
    pub fn capture(
        root: &'r tl::RootPlan<'i, 'p, 'v>,
        profile: &'f FormulaProfile,
        formula: OriginalFormula,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let retained = claim(view)?;
        let binding = profile.binding();
        if binding.comparison_scope != root.scope().comparison()
            || binding.baseline_revision != root.digest()
            || binding.tl_reduction_policy != root.digest()
            || root.context().source().digest() != view.source_revision()
            || profile.ranking_allowed(formula).is_err()
        {
            return Err(Error::ProfileMismatch);
        }
        retained.check().map_err(Error::Resource)?;
        Ok(Self {
            root,
            profile,
            formula,
            retained,
        })
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        view.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ProfileMismatch);
        }
        Ok(())
    }
    pub fn profile(&self) -> &FormulaProfile {
        self.profile
    }
}

/// No clone/deserialize/from-report path: original TL provenance remains the
/// borrowed actual reduction, actual root, exact target tuple and live claims.
/// ```compile_fail
/// use lc631_runtime::original_score_pending::Pending;
/// use lc631_analysis::creativity_measurement::ComponentReport;
/// fn inject(report: ComponentReport) { Pending::from_report(report); }
/// ```
/// ```compile_fail
/// use lc631_runtime::original_score_pending::Pending;
/// let _: Pending<'_, '_, '_, '_, '_, '_> = serde_json::from_str("{}").unwrap();
/// ```
pub struct Pending<'a, 'f, 'r, 'i, 'p, 'v> {
    plan: &'a RootPlan<'f, 'r, 'i, 'p, 'v>,
    candidate: &'a tl::TupleCandidate<'r, 'i, 'p, 'v>,
    reduction: &'a tl::OwnedReduction<'a, 'r, 'i, 'p, 'v>,
    retained: ResourceClaim<'v>,
}
pub fn capture_current<'a, 'f, 'r, 'i, 'p, 'v>(
    plan: &'a RootPlan<'f, 'r, 'i, 'p, 'v>,
    candidate: &'a tl::TupleCandidate<'r, 'i, 'p, 'v>,
    reduction: &'a tl::OwnedReduction<'a, 'r, 'i, 'p, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<Pending<'a, 'f, 'r, 'i, 'p, 'v>, Error> {
    plan.check(view)?;
    let retained = claim(view)?;
    reduction
        .current(plan.root, candidate, view)
        .map_err(Error::Translation)?;
    let result = Pending {
        plan,
        candidate,
        reduction,
        retained,
    };
    result.current(candidate, view)?;
    Ok(result)
}
impl<'a, 'f, 'r, 'i, 'p, 'v> Pending<'a, 'f, 'r, 'i, 'p, 'v> {
    pub fn current(
        &self,
        candidate: &tl::TupleCandidate<'r, 'i, 'p, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<Observation<'_>, Error> {
        self.plan.check(view)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) || !std::ptr::eq(candidate, self.candidate) {
            return Err(Error::CandidateMismatch);
        }
        let translation = self
            .reduction
            .current(self.plan.root, candidate, view)
            .map_err(Error::Translation)?;
        Ok(Observation {
            translation,
            missing: required_metrics(self.plan.formula),
        })
    }
    /// Provenance is retained by reference, not rewritten to a formula-wide binding.
    pub fn translation_owner(&self) -> &tl::OwnedReduction<'a, 'r, 'i, 'p, 'v> {
        self.reduction
    }
}
pub struct Observation<'a> {
    translation: &'a tl::Report,
    missing: &'static [M],
}
impl Observation<'_> {
    pub fn translation_loss(&self) -> Option<&RationalInterval> {
        self.translation.total()
    }
    pub fn translation_gaps(&self) -> [Option<tl::NoScalar>; 2] {
        self.translation.gaps()
    }
    pub fn missing_leaves(&self) -> &[M] {
        self.missing
    }
    pub fn scalar(&self) -> Option<&RationalInterval> {
        None
    }
    /// Preflight deliberately does not call the arithmetic evaluator with invented leaves.
    pub const fn formula_evaluated(&self) -> bool {
        false
    }
}

/// Historical bounded summary, NOT a current permit or a cached recommendation.
pub struct Selection {
    pending: usize,
}
impl Selection {
    pub const fn recommendation(&self) -> Option<usize> {
        None
    }
    pub const fn pending_count(&self) -> usize {
        self.pending
    }
}
/// Consume every actual owner again. Pending candidates are retained by count,
/// never assigned zero utility, silently filtered to an empty success, or ranked.
pub fn select_current<'a, 'f, 'r, 'i, 'p, 'v>(
    plan: &RootPlan<'f, 'r, 'i, 'p, 'v>,
    rows: &[&Pending<'a, 'f, 'r, 'i, 'p, 'v>],
    view: &ResourceReservationView<'_>,
) -> Result<Selection, Error> {
    plan.check(view)?;
    if rows.is_empty() || rows.len() > 2048 {
        return Err(Error::InvalidCandidates);
    }
    for row in rows {
        if !std::ptr::eq(row.plan, plan) {
            return Err(Error::ProfileMismatch);
        }
        let observation = row.current(row.candidate, view)?;
        if observation.missing_leaves().is_empty() {
            return Err(Error::InvalidCandidates);
        }
    }
    plan.check(view)?;
    Ok(Selection {
        pending: rows.len(),
    })
}
fn claim<'v>(view: &'v ResourceReservationView<'_>) -> Result<ResourceClaim<'v>, Error> {
    view.check().map_err(Error::Resource)?;
    view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(RETAINED_RAM).map_err(Error::Resource)?,
    )])
    .map_err(Error::Resource)
}
