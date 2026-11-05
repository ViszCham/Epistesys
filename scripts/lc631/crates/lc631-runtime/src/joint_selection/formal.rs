//! Current resource-owned bidirectional formal preservation, never creative D_sem.
//! Given AST/mapping and shared checker interpretation remain unverified.
#![forbid(unsafe_code)]
use super::{AxisKind, AxisValue, JointCandidate, SelectionPins, SelectionPolicy};
use crate::formal_boolean::{CheckerIndependence, MappingBasis, SourceInterpretation};
use crate::formal_measurement::{
    owned::{OwnedError, OwnedFormalLoss},
    Candidate, Direction, ProbePlan,
};
use lc631_analysis::creativity_measurement::{
    weighted_assessment, MeasurementError, WeightedProbe,
};
use lc631_core::measurement_numeric::{check, NumericError, Rational, RationalInterval};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

pub const REDUCTION_SCRATCH_RAM: u64 = 1024 * 1024;
pub const REDUCTION_RETAINED_RAM: u64 = 64 * 1024;
pub const REDUCTION_RETAINED_OUTPUT: u64 = 64 * 1024;
const SOUNDNESS_KEY: &[u8] = b"finite-formal-v1.4/semantic-preservation/soundness";
const COMPLETENESS_KEY: &[u8] = b"finite-formal-v1.4/semantic-preservation/completeness";

/// Content identity for this explicitly registered selection-reduction policy.
/// pins.policy is excluded to avoid a circular hash. No authentication is issued.
/// The caller must set SelectionPins.policy to this result before reduction.
pub fn reduction_policy_digest(
    pins: SelectionPins,
    definition: Digest32,
    soundness_weight: &Rational,
    completeness_weight: &Rational,
    resources: &ResourceReservationView<'_>,
) -> Result<Digest32, FormalAxisError> {
    resources.check().map_err(FormalAxisError::Resource)?;
    if resources.source_revision() != pins.source.digest() {
        return Err(FormalAxisError::BindingMismatch);
    }
    let scratch = resources
        .claim_resources(&[request(ResourceKind::Ram, REDUCTION_SCRATCH_RAM)?])
        .map_err(FormalAxisError::Resource)?;
    let digest = digest_under_claim(pins, definition, soundness_weight, completeness_weight)?;
    scratch.check().map_err(FormalAxisError::Resource)?;
    Ok(digest)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalAxisError {
    Owned(OwnedError),
    Resource(ResourceError),
    Numeric(NumericError),
    Arithmetic(MeasurementError),
    InvalidWeight,
    PolicyMismatch,
    BindingMismatch,
    ForeignReservation,
}

/// Explicit immutable declaration, not an authenticated registry or a default.
/// Validation (including canonical rational checks) occurs under scratch claims
/// in reduce_formal_owned; declaring a policy allocates no numeric buffers.
/// The public declaration constructor is intentional; direct field injection is not:
/// ```compile_fail
/// use lc631_runtime::joint_selection::{SelectionPins, formal::BothDirectionReduction};
/// use lc631_core::measurement_numeric::Rational;
/// use lc631_substrate::Digest32;
/// fn inject<'a>(pins: SelectionPins, weight: &'a Rational) -> BothDirectionReduction<'a> {
///     BothDirectionReduction {
///         pins, revision: pins.policy, definition: Digest32::from_bytes([0; 32]),
///         soundness_weight: weight, completeness_weight: weight,
///     }
/// }
/// ```
/// JSON input cannot deserialize directly into this declaration type:
/// ```compile_fail
/// use lc631_runtime::joint_selection::formal::BothDirectionReduction;
/// let _: BothDirectionReduction<'static> = serde_json::from_str("{}").unwrap();
/// ```
pub struct BothDirectionReduction<'a> {
    pins: SelectionPins,
    revision: Digest32,
    definition: Digest32,
    soundness_weight: &'a Rational,
    completeness_weight: &'a Rational,
}
impl<'a> BothDirectionReduction<'a> {
    pub fn declared_registered(
        pins: SelectionPins,
        revision: Digest32,
        definition: Digest32,
        soundness_weight: &'a Rational,
        completeness_weight: &'a Rational,
    ) -> Self {
        Self {
            pins,
            revision,
            definition,
            soundness_weight,
            completeness_weight,
        }
    }
    pub fn pins(&self) -> SelectionPins {
        self.pins
    }
    pub fn revision(&self) -> Digest32 {
        self.revision
    }
    pub fn definition(&self) -> Digest32 {
        self.definition
    }
    pub fn soundness_weight(&self) -> &Rational {
        self.soundness_weight
    }
    pub fn completeness_weight(&self) -> &Rational {
        self.completeness_weight
    }
}

/// Bounds drop before their RAM/output claim. Owners and immutable policy stay
/// borrowed; detached report injection and Deserialize are deliberately absent.
/// A formal measurement axis exposes no execution-permit issuer:
/// ```compile_fail
/// use lc631_runtime::joint_selection::formal::FormalSemanticAxis;
/// fn issue(axis: &FormalSemanticAxis<'_, '_, '_, '_>) {
///     axis.issue_execution_permit();
/// }
/// ```
pub struct FormalSemanticAxis<'owners, 'input, 'view, 'policy> {
    bounds: RationalInterval,
    soundness: &'owners OwnedFormalLoss<'input, 'view>,
    completeness: &'owners OwnedFormalLoss<'input, 'view>,
    policy: &'policy BothDirectionReduction<'policy>,
    retained: ResourceClaim<'view>,
}
impl<'owners, 'input, 'view, 'policy> FormalSemanticAxis<'owners, 'input, 'view, 'policy> {
    /// Historical arithmetic only; does not reactivate a cancelled reservation.
    pub fn historical_bounds(&self) -> &RationalInterval {
        &self.bounds
    }
    pub fn policy(&self) -> &BothDirectionReduction<'policy> {
        self.policy
    }
    pub fn soundness_owner(&self) -> &'owners OwnedFormalLoss<'input, 'view> {
        self.soundness
    }
    pub fn completeness_owner(&self) -> &'owners OwnedFormalLoss<'input, 'view> {
        self.completeness
    }
    pub fn mapping_basis(&self) -> MappingBasis {
        MappingBasis::Given
    }
    pub fn source_interpretation(&self) -> SourceInterpretation {
        SourceInterpretation::Unverified
    }
    pub fn checker_independence(&self) -> CheckerIndependence {
        CheckerIndependence::UnassessedSharedIr
    }

    /// Opt-in bridge for a row immediately before reported selection. This
    /// checks current owner/row correspondence, not reported goals, gates,
    /// key canonicalization, source interpretation or authority. The expected
    /// measurement/ledger binding is an explicit Given pin, not a candidate ID.
    /// Copying equal numeric endpoints is not current owner correspondence.
    /// Cancellation can still race after return; this is not a commit fence.
    #[allow(clippy::too_many_arguments)] // Keep each current binding explicit at this boundary.
    pub fn check_selection_row_current(
        &self,
        reduction: &BothDirectionReduction<'_>,
        selection: &SelectionPolicy<'_>,
        row: &JointCandidate<'_>,
        semantic_index: usize,
        expected_measurement_binding: Digest32,
        plan: &ProbePlan<'_>,
        candidate: &Candidate<'_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), FormalAxisError> {
        let (_, actual) =
            self.project_current(reduction, selection.pins, plan, candidate, resources)?;
        if row.pins != selection.pins
            || row.context != candidate.context()
            || row.measurement_binding != expected_measurement_binding
            || row.axes.len() != selection.axes.len()
            || selection.axes.get(semantic_index) != Some(&AxisKind::SemanticLoss)
        {
            return Err(FormalAxisError::BindingMismatch);
        }
        let attached = row.axes.get(semantic_index);
        let same_owner_view = match (attached, actual) {
            (Some(AxisValue::Exact(attached)), AxisValue::Exact(actual))
            | (Some(AxisValue::Bounded(attached)), AxisValue::Bounded(actual)) => {
                std::ptr::eq(*attached, actual)
            }
            _ => false,
        };
        if !same_owner_view {
            return Err(FormalAxisError::BindingMismatch);
        }
        Ok(())
    }

    pub fn project_current(
        &self,
        policy: &BothDirectionReduction<'_>,
        pins: SelectionPins,
        plan: &ProbePlan<'_>,
        candidate: &Candidate<'_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(AxisKind, AxisValue<'_>), FormalAxisError> {
        self.retained.check().map_err(FormalAxisError::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(FormalAxisError::ForeignReservation);
        }
        if !std::ptr::eq(policy, self.policy) || pins != self.policy.pins {
            return Err(FormalAxisError::PolicyMismatch);
        }
        validate_bindings(pins, plan, candidate, resources)?;
        if self.policy.definition != plan.scope().definition {
            return Err(FormalAxisError::PolicyMismatch);
        }
        self.soundness
            .project_current(plan, candidate, Direction::Soundness, resources)
            .map_err(FormalAxisError::Owned)?;
        self.completeness
            .project_current(plan, candidate, Direction::Completeness, resources)
            .map_err(FormalAxisError::Owned)?;
        let value = if self.bounds.is_exact() {
            AxisValue::Exact(&self.bounds)
        } else {
            AxisValue::Bounded(&self.bounds)
        };
        Ok((AxisKind::SemanticLoss, value))
    }
}

/// Two-input K1 uses canonical semantic-role keys, never direction IDs/order.
/// 64K-bit endpoints occupy <=32KiB for four numerator/denominator magnitudes;
/// retained 64KiB adds the small result/borrowed-owner wrapper and slack.
/// Scratch 1MiB models 24 double-cap 32KiB arithmetic buffers and 256KiB of
/// canonical-check/K1 bookkeeping slack for exactly two input roles.
/// These are logical payload bounds, excluding allocator headers, input owners,
/// caller policy/AST backing, stacks and total RSS; BigInt OOM is not fallible.
/// Cancellation checks are not an atomic revocation/publication fence.
pub fn reduce_formal_owned<'owners, 'input, 'view, 'policy>(
    policy: &'policy BothDirectionReduction<'policy>,
    soundness: &'owners OwnedFormalLoss<'input, 'view>,
    completeness: &'owners OwnedFormalLoss<'input, 'view>,
    plan: &ProbePlan<'_>,
    candidate: &Candidate<'_>,
    resources: &'view ResourceReservationView<'_>,
) -> Result<FormalSemanticAxis<'owners, 'input, 'view, 'policy>, FormalAxisError> {
    validate_bindings(policy.pins, plan, candidate, resources)?;
    let sound = soundness
        .project_current(plan, candidate, Direction::Soundness, resources)
        .map_err(FormalAxisError::Owned)?;
    let complete = completeness
        .project_current(plan, candidate, Direction::Completeness, resources)
        .map_err(FormalAxisError::Owned)?;
    let scratch = resources
        .claim_resources(&[request(ResourceKind::Ram, REDUCTION_SCRATCH_RAM)?])
        .map_err(FormalAxisError::Resource)?;
    let retained = resources
        .claim_resources(&[
            request(ResourceKind::Ram, REDUCTION_RETAINED_RAM)?,
            request(ResourceKind::Output, REDUCTION_RETAINED_OUTPUT)?,
        ])
        .map_err(FormalAxisError::Resource)?;
    if policy.definition != plan.scope().definition {
        return Err(FormalAxisError::PolicyMismatch);
    }
    let digest = digest_under_claim(
        policy.pins,
        policy.definition,
        policy.soundness_weight,
        policy.completeness_weight,
    )?;
    if policy.revision != policy.pins.policy || digest != policy.revision {
        return Err(FormalAxisError::PolicyMismatch);
    }
    let bounds = weighted_assessment(&[
        WeightedProbe {
            canonical_payload: SOUNDNESS_KEY,
            weight: policy.soundness_weight,
            value: sound.bounds(),
        },
        WeightedProbe {
            canonical_payload: COMPLETENESS_KEY,
            weight: policy.completeness_weight,
            value: complete.bounds(),
        },
    ])
    .map_err(FormalAxisError::Arithmetic)?;
    scratch.check().map_err(FormalAxisError::Resource)?;
    retained.check().map_err(FormalAxisError::Resource)?;
    // Recheck exact current owners immediately before publication.
    soundness
        .project_current(plan, candidate, Direction::Soundness, resources)
        .map_err(FormalAxisError::Owned)?;
    completeness
        .project_current(plan, candidate, Direction::Completeness, resources)
        .map_err(FormalAxisError::Owned)?;
    Ok(FormalSemanticAxis {
        bounds,
        soundness,
        completeness,
        policy,
        retained,
    })
}

fn positive_canonical(weight: &Rational) -> Result<(), FormalAxisError> {
    check(weight).map_err(FormalAxisError::Numeric)?;
    // check rejects zero/negative denominators and noncanonical new_raw values.
    if weight.numer() <= &lc631_core::measurement_numeric::BigInt::from(0) {
        return Err(FormalAxisError::InvalidWeight);
    }
    Ok(())
}
fn digest_under_claim(
    pins: SelectionPins,
    definition: Digest32,
    soundness: &Rational,
    completeness: &Rational,
) -> Result<Digest32, FormalAxisError> {
    positive_canonical(soundness)?;
    positive_canonical(completeness)?;
    let mut hash = Sha256::new();
    hash.update(b"epistesys.finite-formal-v1.4.both-direction-k1-reduction.v1\0");
    hash.update(definition.as_bytes());
    for digest in [
        pins.source.digest(),
        pins.scope.comparison(),
        pins.scope.goal(),
        pins.scope.probes(),
        pins.scope.semantics(),
        pins.baseline,
        pins.hard_invariants,
        pins.evidence_revision,
    ] {
        hash.update(digest.as_bytes());
    }
    match pins.goal_profile {
        None => hash.update([0]),
        Some(goal) => {
            hash.update([1]);
            hash.update(goal.as_bytes());
        }
    }
    for (role, weight) in [(SOUNDNESS_KEY, soundness), (COMPLETENESS_KEY, completeness)] {
        hash.update((role.len() as u64).to_le_bytes());
        hash.update(role);
        // Positive canonical numerator and denominator: shortest LE magnitudes,
        // explicit lengths; signs are fixed positive by positive_canonical.
        for integer in [weight.numer(), weight.denom()] {
            let (_, bytes) = integer.to_bytes_le();
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    Ok(Digest32::from_bytes(hash.finalize().into()))
}
fn validate_bindings(
    pins: SelectionPins,
    plan: &ProbePlan<'_>,
    candidate: &Candidate<'_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), FormalAxisError> {
    resources.check().map_err(FormalAxisError::Resource)?;
    let scope = plan.scope();
    if pins.source != scope.source
        || pins.scope.comparison() != scope.comparison
        || pins.scope.probes() != scope.probe_set
        || scope.context.source() != pins.source
        || candidate.revision() != pins.source
        || candidate.context().source() != pins.source
        || resources.source_revision() != pins.source.digest()
    {
        return Err(FormalAxisError::BindingMismatch);
    }
    Ok(())
}
fn request(kind: ResourceKind, amount: u64) -> Result<ResourceRequest, FormalAxisError> {
    Ok(ResourceRequest::new(
        kind,
        ResourceAmount::checked(amount).map_err(FormalAxisError::Resource)?,
    ))
}
