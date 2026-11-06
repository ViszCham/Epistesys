//! Closed concrete producer ports; no arbitrary report-returning trait/callback.
use super::Error;
use crate::{
    adversarial_measurement::{FrozenResponses, OwnedCounterRisk},
    description_measurement::OwnedDescription,
    design_measurement::{FrozenDesign, OwnedDesignGain},
    joint_selection::SelectionPins,
    memory_measurement::{OwnedBridgeCost, RegistryPlan},
    prediction_measurement::{FrozenPredictions, OwnedPredictionGain},
    structural_gain_measurement::OwnedPreservation,
};
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_substrate::resource::ResourceReservationView;

/// Borrow actual owners AND their exact native immutable input objects. Copying
/// these references does not clone reports or issue a new producer/receipt.
#[derive(Clone, Copy)]
pub enum ComponentOwnerRef<'a, 'v> {
    Description(&'a OwnedDescription<'a, 'a, 'a, 'v>),
    Preserve(&'a OwnedPreservation<'a, 'v>),
    Design {
        owner: &'a OwnedDesignGain<'a, 'v>,
        frozen: &'a FrozenDesign<'a, 'v>,
    },
    Prediction {
        owner: &'a OwnedPredictionGain<'a, 'v>,
        frozen: &'a FrozenPredictions<'v>,
    },
    CounterRisk {
        owner: &'a OwnedCounterRisk<'a, 'v>,
        frozen: &'a FrozenResponses<'a, 'v>,
    },
    BridgeCost {
        owner: &'a OwnedBridgeCost<'a, 'v>,
        plan: &'a RegistryPlan<'a>,
    },
}
impl<'a> ComponentOwnerRef<'a, '_> {
    pub(super) fn report_current(
        self,
        pins: SelectionPins,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&'a ComponentReport, Error> {
        match self {
            Self::Description(owner) => {
                let lineage = owner
                    .lineage_current(resources)
                    .map_err(Error::Description)?;
                for scope in [lineage.baseline.scope, lineage.candidate.scope] {
                    // Actual reconstruction probe_digest is NOT this declared
                    // scope.probes registry. Keep those different pins intact.
                    if scope.id.0 != *pins.scope.comparison().as_bytes()
                        || scope.baseline.0 != *pins.baseline.as_bytes()
                        || scope.root_goal.0 != *pins.scope.goal().as_bytes()
                        || scope.probes.0 != *pins.scope.probes().as_bytes()
                        || scope.policy.0 != *pins.scope.semantics().as_bytes()
                    {
                        return Err(Error::ScopeMismatch);
                    }
                }
                // Description has no native SelectionPins policy, goal_profile
                // option, hard-invariant or evidence-revision fields. The actual
                // root-goal revision above is distinct from goal_profile. These
                // absent pins remain unverified, not inferred from the source.
                owner.report_current(resources).map_err(Error::Description)
            }
            Self::Preserve(owner) => owner
                .report_for_pins_current(pins, resources)
                .map_err(Error::Preserve),
            Self::Design { owner, frozen } => owner
                .axis_current(frozen, pins, resources)
                .map(|axis| axis.report)
                .map_err(Error::Design),
            Self::Prediction { owner, frozen } => owner
                .axis_current(frozen, pins, resources)
                .map(|axis| axis.report)
                .map_err(Error::Prediction),
            Self::CounterRisk { owner, frozen } => owner
                .axis_current(frozen, pins, resources)
                .map(|axis| axis.report)
                .map_err(Error::CounterRisk),
            Self::BridgeCost { owner, plan } => owner
                .axis_current(plan, pins, resources)
                .map(|axis| axis.report)
                .map_err(Error::BridgeCost),
        }
    }
}
