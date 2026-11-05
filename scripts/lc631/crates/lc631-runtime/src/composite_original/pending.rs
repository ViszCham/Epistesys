//! 日本語: R1診断の互換view。実native tokenとは別型で、成功へ昇格しない。
//! English: R1 diagnostic compatibility view, distinct from the actual native token and never success.
use super::*;
use lc631_analysis::creativity_measurement::{ComponentMetric, RationalInterval};
use lc631_core::CandidateId;
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    source::SourceStore,
};

pub struct PendingComposite<'c, 'r, 'i, 'v> {
    root: &'r ConditionalRootPlan<'i, 'v>,
    candidate: &'c ConditionalCandidate<'r, 'i, 'v>,
    retained: ResourceClaim<'v>,
}
pub struct CurrentPending<'s> {
    source: &'s SourceStore,
    missing: &'s [ComponentMetric],
    unsupported: &'s [ComponentMetric],
}
const GAPS: &[MissingDependency] = &[
    MissingDependency::FullCompositeCost,
    MissingDependency::CompositeTotalTl,
    MissingDependency::ActualHardEvidence,
    MissingDependency::NativeOwnerCorrespondence,
    MissingDependency::SemanticApplicability,
];
impl<'c, 'r, 'i, 'v> PendingComposite<'c, 'r, 'i, 'v> {
    pub fn capture(
        root: &'r ConditionalRootPlan<'i, 'v>,
        candidate: &'c ConditionalCandidate<'r, 'i, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        root.check(view)?;
        candidate.check(view)?;
        if !std::ptr::eq(root, candidate.root) {
            return Err(Error::ForeignRoot);
        }
        let retained = resources::claim(view, 8 * 1024, 8 * 1024).map_err(Error::Resource)?;
        let pending = Self {
            root,
            candidate,
            retained,
        };
        pending.check(ExpectedPending { root, candidate }, view)?;
        Ok(pending)
    }
    fn check(
        &self,
        expected: ExpectedPending<'c, 'r, 'i, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        if !resources::current(&self.retained, view).map_err(Error::Resource)? {
            return Err(Error::ForeignReservation);
        }
        self.root.check(view)?;
        self.candidate.check(view)?;
        if !std::ptr::eq(expected.root, self.root) {
            return Err(Error::ForeignRoot);
        }
        if !std::ptr::eq(expected.candidate, self.candidate) {
            return Err(Error::ForeignCandidate);
        }
        Ok(())
    }
    pub fn with_current<R>(
        &self,
        formula: OriginalFormula,
        expected: ExpectedPending<'c, 'r, 'i, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'s> FnOnce(CurrentPending<'s>) -> R,
    ) -> Result<R, Error> {
        self.check(expected, view)?;
        let _scratch = resources::claim(view, 16 * 1024, 0).map_err(Error::Resource)?;
        let missing = required_metrics(formula);
        let mut unsupported = [ComponentMetric::DescriptionBits; 21];
        let mut count = 0usize;
        for &metric in missing {
            if self.root.spec.applicability.iter().any(|row| {
                row.metric == metric && matches!(row.claim, ApplicabilityClaim::Unsupported(_))
            }) {
                let slot = unsupported
                    .get_mut(count)
                    .ok_or(Error::InvalidDeclaration)?;
                *slot = metric;
                count += 1;
            }
        }
        let result = consume(CurrentPending {
            source: self.root.source,
            missing,
            unsupported: &unsupported[..count],
        });
        self.check(expected, view)?;
        Ok(result)
    }
}
impl<'s> CurrentPending<'s> {
    pub fn source(&self) -> &'s SourceStore {
        self.source
    }
    pub fn missing_leaves(&self) -> &[ComponentMetric] {
        self.missing
    }
    pub fn unsupported_leaves(&self) -> &[ComponentMetric] {
        self.unsupported
    }
    pub fn gaps(&self) -> &[MissingDependency] {
        GAPS
    }
    pub fn provenance(&self) -> ConditionalProvenance {
        ConditionalProvenance::GivenUnvalidated
    }
    pub fn causal_validation(&self) -> CausalValidation {
        CausalValidation::Unassessed
    }
    pub fn hard_status(&self) -> HardStatus {
        HardStatus::NeedsEvidence
    }
    pub fn scalar(&self) -> Option<&RationalInterval> {
        None
    }
    pub fn formula_evaluated(&self) -> bool {
        false
    }
    pub fn recommendation(&self) -> Option<CandidateId> {
        None
    }
}
