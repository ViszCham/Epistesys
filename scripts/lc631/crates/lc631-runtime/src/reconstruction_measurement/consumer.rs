use super::{hash, Error, FeatureOutcome, Gap, OwnedReconstruction, ProtectedFeatureSet};
use crate::facet_ledger::*;
use crate::joint_selection::{AxisValue, MissingAxis};
use lc631_core::context::ContextRevision;
use lc631_substrate::resource::ResourceReservationView;

impl OwnedReconstruction<'_, '_> {
    /// Historical content binding only; not current eligibility or an authority.
    pub fn binding(&self, id: TransformId) -> TransformBinding {
        TransformBinding {
            id,
            source: Revision(*self.plan.context.source().digest().as_bytes()),
            target: Revision(*self.report.target.as_bytes()),
            source_context: Revision(*self.plan.context.interpretation_digest().as_bytes()),
            target_context: Revision(*self.plan.context.interpretation_digest().as_bytes()),
            direction: Direction::Reverse,
            scope: self.plan.scope,
            producer: Revision(*hash(super::decoder::CODE.as_bytes()).as_bytes()),
            query: Revision(*self.plan.query.as_bytes()),
            result: Revision(*self.report.result.as_bytes()),
        }
    }
    /// Borrowed actual-producer projection. No caller supplied scalar/status.
    pub fn axis_current(
        &self,
        plan: &ProtectedFeatureSet<'_>,
        target: &[u8],
        context: ContextRevision,
        resources: &ResourceReservationView<'_>,
    ) -> Result<AxisValue<'_>, Error> {
        self.check(plan, target, context, resources)?;
        Ok(match self.report.bounds.as_ref() {
            Some(bounds) if bounds.is_exact() => AxisValue::Exact(bounds),
            Some(bounds) => AxisValue::Bounded(bounds),
            None => AxisValue::Unavailable(match self.report.gap {
                Some(Gap::Unsupported) => MissingAxis::Unsupported,
                Some(Gap::MissingDecoderCost) => MissingAxis::MissingEvaluator,
                _ => MissingAxis::InvalidConfig,
            }),
        })
    }
    /// Retain ONE actual facet into the caller's existing canonical ledger.
    /// index=None retains semantic U; Some(i) retains only a checked mismatch.
    /// Separate appends make partial effects explicit, with no hidden rollback.
    #[allow(clippy::too_many_arguments)] // Every independent binding is checked.
    pub fn retain_current(
        &self,
        plan: &ProtectedFeatureSet<'_>,
        target: &[u8],
        context: ContextRevision,
        resources: &ResourceReservationView<'_>,
        ledger: &mut FacetLedger,
        transform: TransformId,
        index: Option<usize>,
    ) -> Result<(), Error> {
        self.check(plan, target, context, resources)?;
        if ledger.transform(transform) != Some(&self.binding(transform)) {
            return Err(Error::Binding);
        }
        let facet = match index {
            None => Facet::Uncertainty(crate::facet_ledger::Gap {
                property: PropertyId(*hash(b"tlcx18-semantic-unverified").as_bytes()),
                reason: if self.report.gap == Some(super::Gap::Unsupported) {
                    GapReason::Unsupported
                } else {
                    GapReason::Unverified
                },
                required_evidence: Revision(*plan.source_term.as_bytes()),
                next_action: Revision(*plan.query.as_bytes()),
            }),
            Some(i) => {
                if self.report.outcomes.get(i) != Some(&FeatureOutcome::Violated) {
                    return Err(Error::Binding);
                }
                Facet::Defect(Defect {
                    key: DefectKey {
                        root: ObligationId(plan.scope.root_goal.0),
                        property: PropertyId(*hash(&plan.features[i].canonical).as_bytes()),
                        source: self.binding(transform).source,
                        target: self.binding(transform).target,
                    },
                    axis: LossAxis::Reconstruction,
                    witness: EvidenceRef(*self.report.result.as_bytes()),
                })
            }
        };
        ledger
            .append(Entry { transform, facet })
            .map_err(Error::Ledger)
    }
}
