//! Actual typed rational batch -> independent CPU frontier -> joint selection.
//! Numeric parity only: mapping/interpretation/provenance remain upstream duties.
#![forbid(unsafe_code)]

#[cfg(feature = "semantic-gpu")]
pub mod gpu;
#[cfg(feature = "semantic-gpu")]
pub mod given_points;
#[cfg(feature = "semantic-gpu")]
pub mod tiled;
use crate::joint_selection::{
    select_joint_reported, AxisKind, AxisValue, Disposition, JointCandidate, JointSelectionReport,
    MissingAxis, ReportedGate, SearchCoverage, SelectionError, SelectionPolicy,
};
use lc631_core::measurement_numeric::{checked_compare, NumericError, Rational};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

const MAX_PAYLOAD_BYTES: u64 = 1024 * 1024;
const SCRATCH_RAM: u64 = 2 * 1024 * 1024;
const RETAINED_RAM: u64 = 256 * 1024;
const TILED_PAYLOAD_BYTES: u64 = 4 * 1024 * 1024;

/// A bounded accounting profile, not evidence of world provenance or truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericPayloadProfile {
    LegacyV1,
    TiledWorld2048V1,
}
impl NumericPayloadProfile {
    fn payload_limit(self) -> u64 {
        match self {
            Self::LegacyV1 => MAX_PAYLOAD_BYTES,
            Self::TiledWorld2048V1 => TILED_PAYLOAD_BYTES,
        }
    }

    fn retained_ram(self) -> u64 {
        match self {
            Self::LegacyV1 => RETAINED_RAM,
            Self::TiledWorld2048V1 => RETAINED_RAM + TILED_PAYLOAD_BYTES,
        }
    }
}

/// Caller reported publication labels, never a hardware fence observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochStamp {
    pub epoch: u64,
    pub generation: u64,
    pub fence: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuState {
    Unavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParityBasis {
    FreshCpuOnly,
    ReportedFullVectorCompared,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsumerError {
    Resource(ResourceError),
    Selection(SelectionError),
    Numeric(NumericError),
    WorkLimit,
    PayloadLimit,
    SourceMismatch,
    ForeignReservation,
    Ineligible {
        row: usize,
    },
    MissingNumeric {
        row: usize,
        axis: usize,
        reason: MissingAxis,
    },
    StaleCandidate,
    PartialVector,
    VectorMismatch {
        row: usize,
    },
    ConsumerMismatch {
        row: usize,
    },
    AllocationFailed,
}

/// Borrows actual selection/measurement payloads, retaining their full labels.
/// Inputs are not cloned or flattened to float/ID-derived coordinates.
pub struct NumericBatch<'a, 'p, 'v, 'r> {
    policy: &'a SelectionPolicy<'p>,
    rows: &'a [JointCandidate<'v>],
    coverage: SearchCoverage,
    stamp: EpochStamp,
    token: Digest32,
    claim: ResourceClaim<'r>,
    profile: NumericPayloadProfile,
}
impl NumericBatch<'_, '_, '_, '_> {
    pub fn token(&self) -> Digest32 {
        self.token
    }
    pub fn stamp(&self) -> EpochStamp {
        self.stamp
    }
    pub fn payload_profile(&self) -> NumericPayloadProfile {
        self.profile
    }
}

/// Every input row is present, including numeric aliases. Not a GPU receipt.
pub struct FrontierCandidate<'a> {
    pub token: Digest32,
    pub stamp: EpochStamp,
    pub retained: &'a [bool],
}

/// Private publication requires both independent numeric and actual consumer checks.
/// Report drops before its retained accounting claim. No detached current getter.
pub struct CurrentSelection<'a, 'v, 'r> {
    report: JointSelectionReport<'a, 'v>,
    claim: ResourceClaim<'r>,
    parity: ParityBasis,
}
impl CurrentSelection<'_, '_, '_> {
    pub fn gpu_state(&self) -> GpuState {
        GpuState::Unavailable
    }
    pub fn parity_basis(&self) -> ParityBasis {
        self.parity
    }
    pub fn project_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&JointSelectionReport<'_, '_>, ConsumerError> {
        current(&self.claim, resources)?;
        Ok(&self.report)
    }
}

/// Logical payload/accounting caps exclude caller-owned AST/numeric backing,
/// allocator headers, stacks and total RSS. BigInt OOM is not made fallible here.
/// Non-numeric or ineligible rows stop numeric batching; no silent filtering.
pub fn prepare_numeric_batch<'a, 'p, 'v, 'r>(
    policy: &'a SelectionPolicy<'p>,
    rows: &'a [JointCandidate<'v>],
    coverage: SearchCoverage,
    stamp: EpochStamp,
    resources: &'r ResourceReservationView<'_>,
    comparison_budget: usize,
) -> Result<NumericBatch<'a, 'p, 'v, 'r>, ConsumerError> {
    prepare_numeric_batch_with_profile(
        policy,
        rows,
        coverage,
        stamp,
        resources,
        comparison_budget,
        NumericPayloadProfile::LegacyV1,
    )
}

/// Explicit expanded logical-input budget. Both batch and resulting selection
/// retain separate charges; peak accounting includes both while they coexist.
pub fn prepare_numeric_batch_with_profile<'a, 'p, 'v, 'r>(
    policy: &'a SelectionPolicy<'p>,
    rows: &'a [JointCandidate<'v>],
    coverage: SearchCoverage,
    stamp: EpochStamp,
    resources: &'r ResourceReservationView<'_>,
    comparison_budget: usize,
    profile: NumericPayloadProfile,
) -> Result<NumericBatch<'a, 'p, 'v, 'r>, ConsumerError> {
    resources.check().map_err(ConsumerError::Resource)?;
    if resources.source_revision() != policy.pins.source.digest() {
        return Err(ConsumerError::SourceMismatch);
    }
    let work = rows
        .len()
        .checked_mul(rows.len())
        .and_then(|pairs| pairs.checked_mul(policy.axes.len()))
        .ok_or(ConsumerError::WorkLimit)?;
    if rows.is_empty()
        || rows.len() > super::MAX_ROWS
        || policy.axes.is_empty()
        || policy.axes.len() > super::MAX_AXES
        || comparison_budget > super::MAX_COMPARISONS
        || work > comparison_budget
    {
        return Err(ConsumerError::WorkLimit);
    }
    let scratch = resources
        .claim_resources(&[request(ResourceKind::Ram, SCRATCH_RAM)?])
        .map_err(ConsumerError::Resource)?;
    let claim = resources
        .claim_resources(&[request(ResourceKind::Ram, profile.retained_ram())?])
        .map_err(ConsumerError::Resource)?;
    admit_rows(policy, rows, profile.payload_limit())?;
    let base_token = payload_token(policy, rows, coverage, stamp)?;
    let token = match profile {
        NumericPayloadProfile::LegacyV1 => base_token,
        NumericPayloadProfile::TiledWorld2048V1 => {
            let mut hash = Sha256::new();
            hash.update(b"epistesys.tlcx33.numeric-payload-profile.v1\0");
            hash.update(b"tiled-world-2048-v1\0");
            for value in [TILED_PAYLOAD_BYTES, 2048, 32, 8_388_608] {
                hash.update(value.to_le_bytes());
            }
            hash.update(base_token.as_bytes());
            Digest32::from_bytes(hash.finalize().into())
        }
    };
    scratch.check().map_err(ConsumerError::Resource)?;
    current(&claim, resources)?;
    Ok(NumericBatch {
        policy,
        rows,
        coverage,
        stamp,
        token,
        claim,
        profile,
    })
}

fn admit_rows(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
    payload_limit: u64,
) -> Result<(), ConsumerError> {
    let mut bytes = 0u64;
    for (index, row) in rows.iter().enumerate() {
        if row.pins != policy.pins
            || row.context.source() != policy.pins.source
            || row.hard_gate != ReportedGate::Pass
            || row.map_back != ReportedGate::Pass
        {
            return Err(ConsumerError::Ineligible { row: index });
        }
        if row.axes.len() != policy.axes.len() {
            return Err(ConsumerError::Selection(SelectionError::AxisCountMismatch));
        }
        if rows[..index].iter().any(|other| other.id == row.id) {
            return Err(ConsumerError::Selection(
                SelectionError::DuplicateCandidateId,
            ));
        }
        bytes = bytes
            .checked_add(row.canonical_key.len() as u64)
            .and_then(|n| n.checked_add(256))
            .ok_or(ConsumerError::PayloadLimit)?;
        for (axis, value) in row
            .axes
            .iter()
            .chain(std::iter::once(&row.total_loss))
            .enumerate()
        {
            let interval = match value {
                AxisValue::Exact(v) | AxisValue::Bounded(v) | AxisValue::Estimated(v) => *v,
                AxisValue::Unavailable(reason) => {
                    return Err(ConsumerError::MissingNumeric {
                        row: index,
                        axis,
                        reason: *reason,
                    })
                }
            };
            for q in [interval.lower(), interval.upper()] {
                lc631_core::measurement_numeric::check(q).map_err(ConsumerError::Numeric)?;
                bytes = bytes
                    .checked_add(q.numer().bits().div_ceil(8))
                    .and_then(|n| n.checked_add(q.denom().bits().div_ceil(8)))
                    .and_then(|n| n.checked_add(17))
                    .ok_or(ConsumerError::PayloadLimit)?;
            }
            if bytes > payload_limit {
                return Err(ConsumerError::PayloadLimit);
            }
        }
    }
    Ok(())
}

/// All-pairs scalar oracle uses exact rational endpoints and explicit directions.
/// It does not invoke the selection consumer's compare_rows or legacy GPU kernels.
pub fn independent_cpu_frontier(
    batch: &NumericBatch<'_, '_, '_, '_>,
) -> Result<Vec<bool>, ConsumerError> {
    batch.claim.check().map_err(ConsumerError::Resource)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(batch.rows.len())
        .map_err(|_| ConsumerError::AllocationFailed)?;
    for right in 0..batch.rows.len() {
        let mut dominated = false;
        for left in 0..batch.rows.len() {
            batch.claim.check().map_err(ConsumerError::Resource)?;
            if left != right {
                dominated |= dominates(batch, left, right)?;
            }
        }
        result.push(!dominated);
    }
    batch.claim.check().map_err(ConsumerError::Resource)?;
    Ok(result)
}

fn dominates(
    batch: &NumericBatch<'_, '_, '_, '_>,
    left: usize,
    right: usize,
) -> Result<bool, ConsumerError> {
    let (mut weak, mut strict) = (true, false);
    for (axis, kind) in batch.policy.axes.iter().enumerate() {
        let a = batch.rows[left].axes[axis]
            .interval()
            .ok_or(ConsumerError::ConsumerMismatch { row: left })?;
        let b = batch.rows[right].axes[axis]
            .interval()
            .ok_or(ConsumerError::ConsumerMismatch { row: right })?;
        let comparison = if kind.maximize() {
            checked_compare(a.lower(), b.upper())
        } else {
            checked_compare(b.lower(), a.upper())
        }
        .map_err(ConsumerError::Numeric)?;
        weak &= !comparison.is_lt();
        strict |= comparison.is_gt();
    }
    Ok(weak && strict)
}

/// Current production-callable consumer. None means a fresh CPU path, never a
/// GPU fallback observation. Some means a reported vector, not GPU provenance.
pub fn finish_selection<'a, 'v, 'r>(
    batch: &NumericBatch<'a, '_, 'v, '_>,
    candidate: Option<FrontierCandidate<'_>>,
    resources: &'r ResourceReservationView<'_>,
) -> Result<CurrentSelection<'a, 'v, 'r>, ConsumerError> {
    current(&batch.claim, resources)?;
    let claim = resources
        .claim_resources(&[
            request(ResourceKind::Ram, batch.profile.retained_ram())?,
            request(ResourceKind::Output, RETAINED_RAM)?,
        ])
        .map_err(ConsumerError::Resource)?;
    let expected = independent_cpu_frontier(batch)?;
    let parity = if let Some(proposal) = candidate {
        if proposal.token != batch.token || proposal.stamp != batch.stamp {
            return Err(ConsumerError::StaleCandidate);
        }
        if proposal.retained.len() != expected.len() {
            return Err(ConsumerError::PartialVector);
        }
        for (row, (actual, wanted)) in proposal.retained.iter().zip(&expected).enumerate() {
            if actual != wanted {
                return Err(ConsumerError::VectorMismatch { row });
            }
        }
        ParityBasis::ReportedFullVectorCompared
    } else {
        ParityBasis::FreshCpuOnly
    };
    let report = select_joint_reported(batch.policy, batch.rows, batch.coverage)
        .map_err(ConsumerError::Selection)?;
    for (row, wanted) in expected.iter().enumerate() {
        if canonical_frontier(&report, row)? != *wanted {
            return Err(ConsumerError::ConsumerMismatch { row });
        }
    }
    current(&batch.claim, resources)?;
    current(&claim, resources)?;
    Ok(CurrentSelection {
        report,
        claim,
        parity,
    })
}

fn canonical_frontier(
    report: &JointSelectionReport<'_, '_>,
    mut row: usize,
) -> Result<bool, ConsumerError> {
    for _ in 0..report.candidates.len() {
        match report.dispositions.get(row) {
            Some(Disposition::Frontier) => return Ok(true),
            Some(Disposition::Dominated { .. }) => return Ok(false),
            Some(Disposition::Duplicate { representative }) if *representative < row => {
                row = *representative
            }
            _ => return Err(ConsumerError::ConsumerMismatch { row }),
        }
    }
    Err(ConsumerError::ConsumerMismatch { row })
}
fn current(
    claim: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), ConsumerError> {
    claim.check().map_err(ConsumerError::Resource)?;
    view.check().map_err(ConsumerError::Resource)?;
    if !claim.same_reservation(view) {
        return Err(ConsumerError::ForeignReservation);
    }
    Ok(())
}
fn request(kind: ResourceKind, bytes: u64) -> Result<ResourceRequest, ConsumerError> {
    Ok(ResourceRequest::new(
        kind,
        ResourceAmount::checked(bytes).map_err(ConsumerError::Resource)?,
    ))
}

fn payload_token(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
    coverage: SearchCoverage,
    stamp: EpochStamp,
) -> Result<Digest32, ConsumerError> {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx33.exact-rational-numeric-batch.v1\0");
    let p = policy.pins;
    for d in [
        p.source.digest(),
        p.scope.comparison(),
        p.scope.goal(),
        p.scope.probes(),
        p.scope.semantics(),
        p.baseline,
        p.hard_invariants,
        p.policy,
        p.evidence_revision,
    ] {
        hash.update(d.as_bytes());
    }
    match p.goal_profile {
        Some(d) => {
            hash.update([1]);
            hash.update(d.as_bytes());
        }
        None => hash.update([0]),
    }
    for n in [
        stamp.epoch,
        stamp.generation,
        stamp.fence,
        policy.max_candidates as u64,
        policy.max_canonical_bytes as u64,
    ] {
        hash.update(n.to_le_bytes());
    }
    hash.update([
        u8::from(policy.declared_goal_solve_overlap),
        match coverage {
            SearchCoverage::EvaluatedSet => 0,
            SearchCoverage::BudgetExhausted => 1,
        },
    ]);
    match policy.representative_order {
        None => hash.update([0]),
        Some(order) => {
            hash.update([1]);
            hash.update((order.len() as u64).to_le_bytes());
            for n in order {
                hash.update((*n as u64).to_le_bytes());
            }
        }
    }
    hash.update((policy.axes.len() as u64).to_le_bytes());
    for axis in policy.axes {
        hash_axis(&mut hash, *axis);
    }
    hash.update((rows.len() as u64).to_le_bytes());
    for row in rows {
        hash.update(row.id.0.to_le_bytes());
        hash.update(row.context.interpretation_digest().as_bytes());
        hash.update(row.measurement_binding.as_bytes());
        hash.update((row.canonical_key.len() as u64).to_le_bytes());
        hash.update(row.canonical_key);
        for value in row.axes.iter().chain(std::iter::once(&row.total_loss)) {
            hash_value(&mut hash, value)?;
        }
    }
    Ok(Digest32::from_bytes(hash.finalize().into()))
}
fn hash_value(hash: &mut Sha256, value: &AxisValue<'_>) -> Result<(), ConsumerError> {
    let (tag, interval) = match value {
        AxisValue::Exact(v) => (0, *v),
        AxisValue::Bounded(v) => (1, *v),
        AxisValue::Estimated(v) => (2, *v),
        AxisValue::Unavailable(_) => {
            return Err(ConsumerError::ConsumerMismatch { row: usize::MAX })
        }
    };
    hash.update([tag]);
    for q in [interval.lower(), interval.upper()] {
        hash_rational(hash, q);
    }
    Ok(())
}
fn hash_rational(hash: &mut Sha256, q: &Rational) {
    for integer in [q.numer(), q.denom()] {
        let sign = if integer < &0.into() {
            1
        } else if integer.bits() == 0 {
            0
        } else {
            2
        };
        let (_, bytes) = integer.to_bytes_le();
        hash.update([sign]);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(&bytes);
    }
}
fn hash_axis(hash: &mut Sha256, axis: AxisKind) {
    let tag = match axis {
        AxisKind::Goal(d) => {
            hash.update([0]);
            hash.update(d.as_bytes());
            return;
        }
        AxisKind::ReconstructionLoss => 1,
        AxisKind::SemanticLoss => 2,
        AxisKind::TemporalLoss => 3,
        AxisKind::SpatialLoss => 4,
        AxisKind::CoverageGain => 5,
        AxisKind::CompressionGain => 6,
        AxisKind::BridgeGain => 7,
        AxisKind::PredictionGain => 8,
        AxisKind::Preserve => 9,
        AxisKind::Rebind => 10,
        AxisKind::Reframe => 11,
        AxisKind::ExplainGain => 12,
        AxisKind::DesignGain => 13,
        AxisKind::TransferGain => 14,
        AxisKind::SolveGain => 15,
        AxisKind::RedundancyCost => 16,
        AxisKind::BreakCost => 17,
        AxisKind::CounterexampleCost => 18,
        AxisKind::MigrationCost => 19,
    };
    hash.update([tag]);
}
