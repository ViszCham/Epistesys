#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "gpu-cubecl-wgpu")]
mod gpu;

use lc631_core::{
    stable_sha256, EpochId, FenceId, Generation, ReceiptId, ReceiptOwner, RevisionBinding,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, SubjectRevision, UntrustedReceipt,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const ACCELERATOR_SCHEMA: &str = "lc631-accelerator-broker.v1";
pub const MAX_QUEUE_CAPACITY: usize = 8;
pub const MAX_SUMMARY_ROWS: usize = 2_048;
pub const MAX_FINALIST_ROWS: usize = 16;
pub const NUMERIC_SUMMARY_SCHEMA: &str = "lc631-numeric-summary.v1";
pub const NUMERIC_SUMMARY_WORDS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    ProjectionLoss,
    DominanceConflict,
    SynchronousJacobi,
    QuboDelta,
    WorldDistance,
    WorldContradiction,
    WorldIncomparability,
    WorldPareto,
}

pub const MANDATORY_NUMERIC_LANES: [LaneKind; 4] = [
    LaneKind::ProjectionLoss,
    LaneKind::DominanceConflict,
    LaneKind::SynchronousJacobi,
    LaneKind::QuboDelta,
];

pub const REQUIRED_LOGICAL_LANES: [LaneKind; 8] = [
    LaneKind::ProjectionLoss,
    LaneKind::DominanceConflict,
    LaneKind::SynchronousJacobi,
    LaneKind::QuboDelta,
    LaneKind::WorldDistance,
    LaneKind::WorldContradiction,
    LaneKind::WorldIncomparability,
    LaneKind::WorldPareto,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericMask {
    Known,
    Unknown,
    NotApplicable,
    Incomparable,
}

impl NumericMask {
    pub fn code(self) -> u32 {
        match self {
            Self::Known => 0,
            Self::Unknown => 1,
            Self::NotApplicable => 2,
            Self::Incomparable => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationCode {
    LeftDominates,
    RightDominates,
    Equal,
    Incomparable,
    Unknown,
    NotApplicable,
    Conflict,
}

impl RelationCode {
    pub const fn code(self) -> u32 {
        match self {
            Self::LeftDominates => 0,
            Self::RightDominates => 1,
            Self::Equal => 2,
            Self::Incomparable => 3,
            Self::Unknown => 4,
            Self::NotApplicable => 5,
            Self::Conflict => 6,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct JacobiInput {
    pub offsets: Vec<u32>,
    pub indices: Vec<u32>,
    pub weights_x100: Vec<i16>,
    pub initial_x100: Vec<i16>,
    pub masks: Vec<NumericMask>,
    pub rounds: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct QuboInteraction {
    pub left: usize,
    pub right: usize,
    pub coefficient: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QuboInput {
    pub linear: Vec<i32>,
    pub interactions: Vec<QuboInteraction>,
    pub selected: Vec<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NumericEpochInput {
    pub context_digest: String,
    pub row_identity_digests: Vec<String>,
    pub axes_x100: Vec<[u16; 3]>,
    pub masks: Vec<NumericMask>,
    pub relation_offsets: Vec<u32>,
    pub relation_left: Vec<u32>,
    pub relation_right: Vec<u32>,
    pub jacobi: JacobiInput,
    pub qubo: QuboInput,
}

impl NumericEpochInput {
    pub fn validate(&self) -> Result<(), AcceleratorError> {
        let rows = self.axes_x100.len();
        if rows == 0 || self.masks.len() != rows {
            return Err(AcceleratorError::ShapeMismatch("projection"));
        }
        if self.row_identity_digests.len() != rows
            || self
                .row_identity_digests
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != rows
        {
            return Err(AcceleratorError::ShapeMismatch("row_identity"));
        }
        if self.relation_offsets.len() != rows + 1
            || self.relation_left.len() != self.relation_right.len()
            || self.relation_offsets.last().copied().unwrap_or_default() as usize
                != self.relation_left.len()
            || self
                .relation_offsets
                .windows(2)
                .any(|window| window[0] > window[1])
            || self
                .relation_left
                .iter()
                .chain(&self.relation_right)
                .any(|index| *index as usize >= rows)
            || (0..rows).any(|row| {
                let start = self.relation_offsets[row] as usize;
                let end = self.relation_offsets[row + 1] as usize;
                self.relation_left[start..end]
                    .iter()
                    .any(|left| *left as usize != row)
            })
        {
            return Err(AcceleratorError::ShapeMismatch("relations"));
        }
        let jacobi = &self.jacobi;
        if jacobi.initial_x100.is_empty()
            || jacobi.masks.len() != jacobi.initial_x100.len()
            || jacobi.offsets.len() != jacobi.initial_x100.len() + 1
            || jacobi.indices.len() != jacobi.weights_x100.len()
            || jacobi.offsets.last().copied().unwrap_or_default() as usize != jacobi.indices.len()
            || jacobi
                .indices
                .iter()
                .any(|index| *index as usize >= jacobi.initial_x100.len())
        {
            return Err(AcceleratorError::ShapeMismatch("jacobi"));
        }
        if self.qubo.linear.is_empty()
            || self.qubo.linear.len() != self.qubo.selected.len()
            || self.qubo.interactions.iter().any(|term| {
                term.left >= self.qubo.linear.len() || term.right >= self.qubo.linear.len()
            })
        {
            return Err(AcceleratorError::ShapeMismatch("qubo"));
        }
        Ok(())
    }

    pub fn rows(&self) -> usize {
        self.axes_x100.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    CpuScalar,
    CubeClWgpu,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LaneObservation {
    pub lane: LaneKind,
    pub rows: usize,
    pub output_digest: String,
    pub finalist_rows: Vec<usize>,
    pub finalist_values: Vec<u32>,
    pub cutoff_value: Option<u32>,
    pub cutoff_tie_count: usize,
    pub mask_digest: String,
    pub decision_digest: String,
    pub output_values: usize,
    pub summary_schema: &'static str,
    pub summary_words: Vec<u32>,
    pub readback_values: usize,
    pub full_output_materialized_on_host: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementState {
    Observed,
    Unavailable,
    #[default]
    NotRequested,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BackendTelemetry {
    pub device_timing: MeasurementState,
    pub device_elapsed_ns: Option<u64>,
    pub host_inflight_overlap_observed: bool,
    pub shader_occupancy: MeasurementState,
    pub device_loss_callback_registered: bool,
    pub device_loss_callback_observed: bool,
}

impl BackendTelemetry {
    fn cpu() -> Self {
        Self {
            device_timing: MeasurementState::NotRequested,
            device_elapsed_ns: None,
            host_inflight_overlap_observed: false,
            shader_occupancy: MeasurementState::NotRequested,
            device_loss_callback_registered: false,
            device_loss_callback_observed: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RawBackendObservation {
    pub backend: BackendKind,
    pub device_identity: String,
    pub lanes: Vec<LaneObservation>,
    pub native_completion_observed: bool,
    pub full_readback: bool,
    pub copy_operations: usize,
    pub workgroups: usize,
    pub overlap_observed: bool,
    pub readback_values: usize,
    pub telemetry: BackendTelemetry,
}

mod sealed {
    pub trait Sealed {}
}

pub trait NumericBackend: sealed::Sealed {
    fn kind(&self) -> BackendKind;
    fn execute(
        &mut self,
        input: &NumericEpochInput,
    ) -> Result<RawBackendObservation, AcceleratorError>;
}

#[derive(Default)]
pub struct CpuBackend;

impl sealed::Sealed for CpuBackend {}

impl NumericBackend for CpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::CpuScalar
    }

    fn execute(
        &mut self,
        input: &NumericEpochInput,
    ) -> Result<RawBackendObservation, AcceleratorError> {
        input.validate()?;
        let projection = cpu_projection(input);
        let relation = cpu_relations(input);
        let jacobi = cpu_jacobi(&input.jacobi)?;
        let qubo = cpu_qubo(&input.qubo)?;
        let world_distance = cpu_world_distance(input);
        let world_contradiction = cpu_world_contradiction(input, &relation);
        let world_incomparability = cpu_world_incomparability(input, &relation);
        let world_pareto = cpu_world_pareto(input, &relation);
        let lanes = vec![
            projection_lane_observation(input, &projection),
            lane_observation_u32(
                LaneKind::DominanceConflict,
                input.rows(),
                &relation,
                &input.masks,
            ),
            lane_observation_i32(
                LaneKind::SynchronousJacobi,
                jacobi.len(),
                &jacobi,
                &input.jacobi.masks,
            ),
            lane_observation_i32(LaneKind::QuboDelta, qubo.len(), &qubo, &[]),
            lane_observation_u32(
                LaneKind::WorldDistance,
                world_distance.len(),
                &world_distance,
                &input.masks,
            ),
            lane_observation_u32(
                LaneKind::WorldContradiction,
                world_contradiction.len(),
                &world_contradiction,
                &input.masks,
            ),
            lane_observation_u32(
                LaneKind::WorldIncomparability,
                world_incomparability.len(),
                &world_incomparability,
                &input.masks,
            ),
            lane_observation_u32(
                LaneKind::WorldPareto,
                world_pareto.len(),
                &world_pareto,
                &input.masks,
            ),
        ];
        Ok(RawBackendObservation {
            backend: BackendKind::CpuScalar,
            device_identity: "cpu-scalar".into(),
            lanes,
            native_completion_observed: true,
            full_readback: false,
            copy_operations: 0,
            workgroups: 0,
            overlap_observed: false,
            readback_values: 0,
            telemetry: BackendTelemetry::cpu(),
        })
    }
}

fn projection_lane_observation(input: &NumericEpochInput, values: &[u32]) -> LaneObservation {
    let mut ranked = values
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| (value, index))
        .collect::<Vec<_>>();
    ranked.sort_unstable();
    let finalist_rows = ranked
        .into_iter()
        .take(MAX_FINALIST_ROWS.min(values.len()))
        .map(|(_, index)| index)
        .collect::<Vec<_>>();
    let finalist_values = finalist_rows
        .iter()
        .map(|index| values[*index])
        .collect::<Vec<_>>();
    let cutoff_value = finalist_values.last().copied();
    let cutoff_tie_count = cutoff_value.map_or(0, |cutoff| {
        values.iter().filter(|value| **value == cutoff).count()
    });
    let summary_words = summary_words_u32(values);
    let mask_digest = numeric_mask_digest(&input.masks);
    let decision_digest = lane_decision_digest(
        LaneKind::ProjectionLoss,
        &summary_words,
        &finalist_rows,
        &finalist_values,
        cutoff_value,
        cutoff_tie_count,
        &mask_digest,
    );
    LaneObservation {
        lane: LaneKind::ProjectionLoss,
        rows: input.rows(),
        output_digest: summary_digest(LaneKind::ProjectionLoss, &summary_words),
        finalist_rows,
        finalist_values,
        cutoff_value,
        cutoff_tie_count,
        mask_digest,
        decision_digest,
        output_values: values.len(),
        summary_schema: NUMERIC_SUMMARY_SCHEMA,
        summary_words,
        readback_values: 0,
        full_output_materialized_on_host: true,
    }
}

#[cfg(feature = "gpu-cubecl-wgpu")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn projection_lane_from_summary(
    rows: usize,
    summary_words: Vec<u32>,
    finalist_rows: Vec<usize>,
    finalist_values: Vec<u32>,
    cutoff_value: Option<u32>,
    cutoff_tie_count: usize,
    mask_digest: String,
    readback_values: usize,
) -> LaneObservation {
    let decision_digest = lane_decision_digest(
        LaneKind::ProjectionLoss,
        &summary_words,
        &finalist_rows,
        &finalist_values,
        cutoff_value,
        cutoff_tie_count,
        &mask_digest,
    );
    LaneObservation {
        lane: LaneKind::ProjectionLoss,
        rows,
        output_digest: summary_digest(LaneKind::ProjectionLoss, &summary_words),
        finalist_rows,
        finalist_values,
        cutoff_value,
        cutoff_tie_count,
        mask_digest,
        decision_digest,
        output_values: rows,
        summary_schema: NUMERIC_SUMMARY_SCHEMA,
        summary_words,
        readback_values,
        full_output_materialized_on_host: false,
    }
}

fn lane_observation_u32(
    lane: LaneKind,
    rows: usize,
    values: &[u32],
    masks: &[NumericMask],
) -> LaneObservation {
    let summary_words = summary_words_u32(values);
    let mask_digest = numeric_mask_digest(masks);
    let decision_digest =
        lane_decision_digest(lane, &summary_words, &[], &[], None, 0, &mask_digest);
    LaneObservation {
        lane,
        rows,
        output_digest: summary_digest(lane, &summary_words),
        finalist_rows: Vec::new(),
        finalist_values: Vec::new(),
        cutoff_value: None,
        cutoff_tie_count: 0,
        mask_digest,
        decision_digest,
        output_values: values.len(),
        summary_schema: NUMERIC_SUMMARY_SCHEMA,
        summary_words,
        readback_values: 0,
        full_output_materialized_on_host: true,
    }
}

fn lane_observation_i32(
    lane: LaneKind,
    rows: usize,
    values: &[i32],
    masks: &[NumericMask],
) -> LaneObservation {
    let summary_words = summary_words_i32(values);
    let mask_digest = numeric_mask_digest(masks);
    let decision_digest =
        lane_decision_digest(lane, &summary_words, &[], &[], None, 0, &mask_digest);
    LaneObservation {
        lane,
        rows,
        output_digest: summary_digest(lane, &summary_words),
        finalist_rows: Vec::new(),
        finalist_values: Vec::new(),
        cutoff_value: None,
        cutoff_tie_count: 0,
        mask_digest,
        decision_digest,
        output_values: values.len(),
        summary_schema: NUMERIC_SUMMARY_SCHEMA,
        summary_words,
        readback_values: 0,
        full_output_materialized_on_host: true,
    }
}

#[cfg(feature = "gpu-cubecl-wgpu")]
pub(crate) fn lane_from_summary(
    lane: LaneKind,
    rows: usize,
    output_values: usize,
    summary_words: Vec<u32>,
    mask_digest: String,
    readback_values: usize,
) -> LaneObservation {
    let decision_digest =
        lane_decision_digest(lane, &summary_words, &[], &[], None, 0, &mask_digest);
    LaneObservation {
        lane,
        rows,
        output_digest: summary_digest(lane, &summary_words),
        finalist_rows: Vec::new(),
        finalist_values: Vec::new(),
        cutoff_value: None,
        cutoff_tie_count: 0,
        mask_digest,
        decision_digest,
        output_values,
        summary_schema: NUMERIC_SUMMARY_SCHEMA,
        summary_words,
        readback_values,
        full_output_materialized_on_host: false,
    }
}

pub(crate) fn summary_digest(lane: LaneKind, words: &[u32]) -> String {
    stable_sha256(&format!("{NUMERIC_SUMMARY_SCHEMA}:{lane:?}:{words:?}"))
}

pub fn decision_summary_u32(values: &[u32]) -> Vec<u32> {
    summary_words_u32(values)
}

fn numeric_mask_digest(masks: &[NumericMask]) -> String {
    stable_sha256(&format!(
        "lc631-numeric-mask.v1:{:?}",
        masks.iter().map(|mask| mask.code()).collect::<Vec<_>>()
    ))
}

#[allow(clippy::too_many_arguments)]
fn lane_decision_digest(
    lane: LaneKind,
    summary_words: &[u32],
    finalist_rows: &[usize],
    finalist_values: &[u32],
    cutoff_value: Option<u32>,
    cutoff_tie_count: usize,
    mask_digest: &str,
) -> String {
    stable_sha256(&format!(
        "lc631-lane-decision.v2:{lane:?}:{summary_words:?}:{finalist_rows:?}:{finalist_values:?}:{cutoff_value:?}:{cutoff_tie_count}:{mask_digest}:value_then_row_ascending",
    ))
}

pub(crate) fn summary_words_u32(values: &[u32]) -> Vec<u32> {
    let mut minimum = u32::MAX;
    let mut maximum = u32::MIN;
    let mut hash_1 = 0_u32;
    let mut hash_2 = 0_u32;
    let mut hash_3 = 0_u32;
    let mut zeroes = 0_u32;
    for (index, value) in values.iter().copied().enumerate() {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
        let position = index as u32 + 1;
        hash_1 = hash_1
            .wrapping_add((value ^ position.wrapping_mul(0x9e37_79b9)).wrapping_mul(0x85eb_ca6b));
        hash_2 ^= value
            .wrapping_add(position.wrapping_mul(0xc2b2_ae35))
            .wrapping_mul(0x27d4_eb2d);
        hash_3 = hash_3.wrapping_add(
            value
                .wrapping_add(0x1656_67b1)
                .wrapping_mul(position.wrapping_mul(0x7feb_352d)),
        );
        zeroes = zeroes.wrapping_add(u32::from(value == 0));
    }
    vec![
        5,
        values.len() as u32,
        minimum,
        maximum,
        hash_1,
        hash_2,
        hash_3,
        zeroes,
    ]
}

pub(crate) fn summary_words_i32(values: &[i32]) -> Vec<u32> {
    let mut minimum = i32::MAX;
    let mut maximum = i32::MIN;
    let mut hash_1 = 0_u32;
    let mut hash_2 = 0_u32;
    let mut hash_3 = 0_u32;
    let mut zeroes = 0_u32;
    for (index, value) in values.iter().copied().enumerate() {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
        let bits = value as u32;
        let position = index as u32 + 1;
        hash_1 = hash_1
            .wrapping_add((bits ^ position.wrapping_mul(0x9e37_79b9)).wrapping_mul(0x85eb_ca6b));
        hash_2 ^= bits
            .wrapping_add(position.wrapping_mul(0xc2b2_ae35))
            .wrapping_mul(0x27d4_eb2d);
        hash_3 = hash_3.wrapping_add(
            bits.wrapping_add(0x1656_67b1)
                .wrapping_mul(position.wrapping_mul(0x7feb_352d)),
        );
        zeroes = zeroes.wrapping_add(u32::from(value == 0));
    }
    vec![
        6,
        values.len() as u32,
        minimum as u32,
        maximum as u32,
        hash_1,
        hash_2,
        hash_3,
        zeroes,
    ]
}

fn cpu_projection(input: &NumericEpochInput) -> Vec<u32> {
    input
        .axes_x100
        .iter()
        .zip(&input.masks)
        .map(|(axes, mask)| match mask {
            NumericMask::Known => axes.iter().map(|value| u32::from(*value)).sum(),
            NumericMask::Unknown => u32::MAX,
            NumericMask::NotApplicable => u32::MAX - 1,
            NumericMask::Incomparable => u32::MAX - 2,
        })
        .collect()
}

fn cpu_relations(input: &NumericEpochInput) -> Vec<u32> {
    let mut output = Vec::with_capacity(input.relation_left.len());
    for (left, right) in input
        .relation_left
        .iter()
        .zip(&input.relation_right)
        .map(|(left, right)| (*left as usize, *right as usize))
    {
        let relation = if input.masks[left] == NumericMask::NotApplicable
            || input.masks[right] == NumericMask::NotApplicable
        {
            RelationCode::NotApplicable.code()
        } else if input.masks[left] == NumericMask::Unknown
            || input.masks[right] == NumericMask::Unknown
        {
            RelationCode::Unknown.code()
        } else if input.masks[left] == NumericMask::Incomparable
            || input.masks[right] == NumericMask::Incomparable
        {
            RelationCode::Incomparable.code()
        } else if left == right || input.axes_x100[left] == input.axes_x100[right] {
            RelationCode::Equal.code()
        } else {
            let lower =
                (0..3).all(|axis| input.axes_x100[left][axis] <= input.axes_x100[right][axis]);
            let higher =
                (0..3).all(|axis| input.axes_x100[right][axis] <= input.axes_x100[left][axis]);
            if lower {
                RelationCode::LeftDominates.code()
            } else if higher {
                RelationCode::RightDominates.code()
            } else {
                RelationCode::Incomparable.code()
            }
        };
        output.push(relation);
    }
    output
}

fn cpu_jacobi(input: &JacobiInput) -> Result<Vec<i32>, AcceleratorError> {
    if input.rounds > 1_024 {
        return Err(AcceleratorError::BudgetExceeded("jacobi_rounds"));
    }
    let mut old = input
        .initial_x100
        .iter()
        .map(|value| i32::from(*value))
        .collect::<Vec<_>>();
    let mut next = old.clone();
    for _ in 0..input.rounds {
        for node in 0..old.len() {
            if input.masks[node] != NumericMask::Known {
                next[node] = old[node];
                continue;
            }
            let start = input.offsets[node] as usize;
            let end = input.offsets[node + 1] as usize;
            let contribution = (start..end)
                .filter(|position| {
                    input.masks[input.indices[*position] as usize] == NumericMask::Known
                })
                .map(|position| {
                    old[input.indices[position] as usize] * i32::from(input.weights_x100[position])
                        / 100
                })
                .sum::<i32>();
            next[node] = (old[node] + contribution).clamp(-100, 100);
        }
        std::mem::swap(&mut old, &mut next);
    }
    Ok(old)
}

fn cpu_qubo(input: &QuboInput) -> Result<Vec<i32>, AcceleratorError> {
    let mut output = Vec::with_capacity(input.linear.len());
    for variable in 0..input.linear.len() {
        let mut local = input.linear[variable];
        for term in &input.interactions {
            let other = if term.left == variable {
                Some(term.right)
            } else if term.right == variable {
                Some(term.left)
            } else {
                None
            };
            if other.is_some_and(|other| input.selected[other]) {
                local = local
                    .checked_add(term.coefficient)
                    .ok_or(AcceleratorError::ArithmeticOverflow("qubo"))?;
            }
        }
        output.push(if input.selected[variable] {
            -local
        } else {
            local
        });
    }
    Ok(output)
}

fn cpu_world_distance(input: &NumericEpochInput) -> Vec<u32> {
    let base = input.axes_x100[0];
    input
        .axes_x100
        .iter()
        .map(|axes| {
            (0..3)
                .map(|axis| axes[axis].abs_diff(base[axis]) as u32)
                .sum()
        })
        .collect()
}

fn cpu_world_contradiction(input: &NumericEpochInput, relations: &[u32]) -> Vec<u32> {
    (0..input.rows())
        .map(|left| {
            let start = input.relation_offsets[left] as usize;
            let end = input.relation_offsets[left + 1] as usize;
            (start..end)
                .filter(|position| relations[*position] == RelationCode::Conflict.code())
                .count() as u32
        })
        .collect()
}

fn cpu_world_incomparability(input: &NumericEpochInput, relations: &[u32]) -> Vec<u32> {
    (0..input.rows())
        .map(|left| {
            let start = input.relation_offsets[left] as usize;
            let end = input.relation_offsets[left + 1] as usize;
            (start..end)
                .filter(|position| relations[*position] == RelationCode::Incomparable.code())
                .count() as u32
        })
        .collect()
}

fn cpu_world_pareto(input: &NumericEpochInput, relations: &[u32]) -> Vec<u32> {
    (0..input.rows())
        .map(|left| {
            let start = input.relation_offsets[left] as usize;
            let end = input.relation_offsets[left + 1] as usize;
            u32::from(
                (start..end)
                    .all(|position| relations[position] != RelationCode::RightDominates.code()),
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    DeviceLost,
    OutOfMemory,
    Panic,
    Timeout,
    PartialReadback,
    StaleCompletion,
    HostCancellation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FaultDisposition {
    pub fault: FaultKind,
    pub published: bool,
    pub partial_values_reused: bool,
    pub arena_quarantined: bool,
    pub fresh_cpu_fallback_required: bool,
}

pub fn abort_fault(fault: FaultKind) -> FaultDisposition {
    FaultDisposition {
        fault,
        published: false,
        partial_values_reused: false,
        arena_quarantined: true,
        fresh_cpu_fallback_required: true,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrivacyContract {
    pub cross_context_batch_allowed: bool,
    pub raw_surface_persistence_allowed: bool,
    pub generation_ttl_required: bool,
    pub best_effort_clear_required: bool,
}

impl Default for PrivacyContract {
    fn default() -> Self {
        Self {
            cross_context_batch_allowed: false,
            raw_surface_persistence_allowed: false,
            generation_ttl_required: true,
            best_effort_clear_required: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompletionToken {
    pub epoch: EpochId,
    pub generation: Generation,
    pub fence: FenceId,
    pub binding_digest: String,
    pub native_completion_observed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutionReceipt {
    pub schema_version: &'static str,
    pub receipt_id: ReceiptId,
    pub owner: ReceiptOwner,
    pub backend: BackendKind,
    pub context_digest: String,
    pub binding_digest: String,
    pub semantic_binding_digest: String,
    pub row_identity_digest: String,
    pub mask_digest: String,
    pub completion: CompletionToken,
    pub lanes: Vec<LaneObservation>,
    pub mandatory_lane_coverage: usize,
    pub logical_lane_coverage: usize,
    pub required_logical_lane_count: usize,
    pub full_readback: bool,
    pub copy_operations: usize,
    pub workgroups: usize,
    pub overlap_observed: bool,
    pub readback_values: usize,
    pub telemetry: BackendTelemetry,
    pub gpu_creates_authority: bool,
    pub gpu_creates_evidence: bool,
    pub gpu_creates_output_commit: bool,
    pub privacy: PrivacyContract,
    pub residual_risks: Vec<String>,
    pub authenticity_receipt_digest: Option<String>,
    pub authenticity_attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AcceleratorJob {
    pub epoch: EpochId,
    pub context_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkConservingScheduler {
    capacity: usize,
    queue: VecDeque<AcceleratorJob>,
}

impl WorkConservingScheduler {
    pub fn new(capacity: usize) -> Result<Self, AcceleratorError> {
        if capacity == 0 || capacity > MAX_QUEUE_CAPACITY {
            return Err(AcceleratorError::BudgetExceeded("queue_capacity"));
        }
        Ok(Self {
            capacity,
            queue: VecDeque::with_capacity(capacity),
        })
    }

    pub fn submit(&mut self, job: AcceleratorJob) -> Result<(), AcceleratorError> {
        if self.queue.len() == self.capacity {
            return Err(AcceleratorError::BackpressureVisible);
        }
        if self
            .queue
            .iter()
            .any(|queued| queued.context_digest != job.context_digest)
        {
            return Err(AcceleratorError::CrossContextBatchDenied);
        }
        self.queue.push_back(job);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<AcceleratorJob> {
        self.queue.pop_front()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

pub struct AcceleratorBroker<B> {
    backend: B,
    issuer: Option<ReceiptIssuer>,
    next_receipt: u64,
    next_fence: u64,
    generation: Generation,
    minted: BTreeSet<ReceiptId>,
    quarantined: bool,
}

impl<B: NumericBackend> AcceleratorBroker<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            issuer: None,
            next_receipt: 0,
            next_fence: 0,
            generation: Generation(0),
            minted: BTreeSet::new(),
            quarantined: false,
        }
    }

    pub fn with_issuer(backend: B, issuer: ReceiptIssuer) -> Self {
        Self {
            backend,
            issuer: Some(issuer),
            next_receipt: 0,
            next_fence: 0,
            generation: Generation(0),
            minted: BTreeSet::new(),
            quarantined: false,
        }
    }

    pub fn execute_epoch(
        &mut self,
        epoch: EpochId,
        binding: &RevisionBinding,
        input: &NumericEpochInput,
    ) -> Result<ExecutionReceipt, AcceleratorError> {
        if self.quarantined {
            return Err(AcceleratorError::ArenaQuarantined);
        }
        input.validate()?;
        let raw = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.backend.execute(input)
        })) {
            Ok(Ok(raw)) => raw,
            Ok(Err(error @ AcceleratorError::BackendFault(_))) => {
                self.quarantined = true;
                self.generation = Generation(self.generation.0.saturating_add(1));
                return Err(error);
            }
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                self.quarantined = true;
                return Err(AcceleratorError::BackendFault(FaultKind::Panic));
            }
        };
        if raw.backend != self.backend.kind() {
            self.quarantined = true;
            return Err(AcceleratorError::BackendIdentityMismatch);
        }
        let observed = raw
            .lanes
            .iter()
            .map(|lane| lane.lane)
            .collect::<BTreeSet<_>>();
        let mandatory_lane_coverage = MANDATORY_NUMERIC_LANES
            .iter()
            .filter(|lane| observed.contains(lane))
            .count();
        if mandatory_lane_coverage != MANDATORY_NUMERIC_LANES.len() {
            self.quarantined = true;
            return Err(AcceleratorError::MandatoryLaneMissing(
                mandatory_lane_coverage,
            ));
        }
        let logical_lane_coverage = REQUIRED_LOGICAL_LANES
            .iter()
            .filter(|lane| observed.contains(lane))
            .count();
        if logical_lane_coverage != REQUIRED_LOGICAL_LANES.len() {
            self.quarantined = true;
            return Err(AcceleratorError::LogicalLaneMissing(logical_lane_coverage));
        }
        if !raw.native_completion_observed {
            self.quarantined = true;
            return Err(AcceleratorError::BackendFault(FaultKind::StaleCompletion));
        }
        self.next_receipt = self
            .next_receipt
            .checked_add(1)
            .ok_or(AcceleratorError::IdExhausted)?;
        self.next_fence = self
            .next_fence
            .checked_add(1)
            .ok_or(AcceleratorError::IdExhausted)?;
        let receipt_id = ReceiptId(self.next_receipt);
        if !self.minted.insert(receipt_id) {
            return Err(AcceleratorError::DuplicateReceipt(receipt_id));
        }
        let binding_digest = binding.digest();
        let semantic_binding_digest = binding.semantic_digest();
        let row_identity_digest = stable_sha256(&input.row_identity_digests.join(":"));
        let mask_digest = numeric_mask_digest(&input.masks);
        let mut residual_risks = Vec::new();
        if raw.full_readback {
            residual_risks.push("full_gpu_readback_still_observed".to_string());
        }
        if !raw.overlap_observed {
            residual_risks.push("backend_overlap_unobserved".to_string());
        }
        if raw.backend == BackendKind::CubeClWgpu
            && raw.telemetry.shader_occupancy != MeasurementState::Observed
        {
            residual_risks.push("shader_occupancy_counter_unavailable".to_string());
        }
        if !raw.telemetry.device_loss_callback_registered && raw.backend == BackendKind::CubeClWgpu
        {
            residual_risks.push("device_loss_callback_unregistered".to_string());
        }
        let mut receipt = ExecutionReceipt {
            schema_version: ACCELERATOR_SCHEMA,
            receipt_id,
            owner: ReceiptOwner::AcceleratorBroker,
            backend: raw.backend,
            context_digest: input.context_digest.clone(),
            binding_digest: binding_digest.clone(),
            semantic_binding_digest: semantic_binding_digest.clone(),
            row_identity_digest,
            mask_digest,
            completion: CompletionToken {
                epoch,
                generation: self.generation,
                fence: FenceId(self.next_fence),
                binding_digest,
                native_completion_observed: true,
            },
            lanes: raw.lanes,
            mandatory_lane_coverage,
            logical_lane_coverage,
            required_logical_lane_count: REQUIRED_LOGICAL_LANES.len(),
            full_readback: raw.full_readback,
            copy_operations: raw.copy_operations,
            workgroups: raw.workgroups,
            overlap_observed: raw.overlap_observed,
            readback_values: raw.readback_values,
            telemetry: raw.telemetry,
            gpu_creates_authority: false,
            gpu_creates_evidence: false,
            gpu_creates_output_commit: false,
            privacy: PrivacyContract::default(),
            residual_risks,
            authenticity_receipt_digest: None,
            authenticity_attestation: None,
        };
        if let Some(issuer) = &self.issuer {
            let scope =
                ReceiptScope::checked(format!("accelerator/{:?}/{}", receipt.backend, epoch.0))
                    .map_err(|_| AcceleratorError::ReceiptIssueFailed)?;
            let subject = SubjectRevision::checked(semantic_binding_digest)
                .map_err(|_| AcceleratorError::ReceiptIssueFailed)?;
            let wire = issuer
                .issue(
                    ReceiptClass::Accelerator,
                    subject,
                    scope,
                    execution_receipt_payload_digest(&receipt),
                    epoch.0,
                    None,
                    None,
                )
                .map_err(|_| AcceleratorError::ReceiptIssueFailed)?;
            receipt.authenticity_receipt_digest = Some(wire.receipt_digest());
            receipt.authenticity_attestation = Some(wire);
        }
        Ok(receipt)
    }

    pub fn quarantine(&mut self, fault: FaultKind) -> FaultDisposition {
        self.quarantined = true;
        self.generation = Generation(self.generation.0.saturating_add(1));
        abort_fault(fault)
    }
}

#[cfg(feature = "gpu-cubecl-wgpu")]
pub use gpu::CubeClWgpuBackend;

pub fn execution_receipt_payload_digest(receipt: &ExecutionReceipt) -> String {
    stable_sha256(&format!(
        "{}\0{:?}\0{}\0{}\0{}\0{}\0{:?}",
        receipt.schema_version,
        receipt.backend,
        receipt.context_digest,
        receipt.semantic_binding_digest,
        receipt.row_identity_digest,
        receipt.mask_digest,
        receipt
            .lanes
            .iter()
            .map(|lane| (lane.lane, lane.decision_digest.as_str()))
            .collect::<Vec<_>>()
    ))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum AcceleratorError {
    ArenaQuarantined,
    ArithmeticOverflow(&'static str),
    BackendFault(FaultKind),
    BackendIdentityMismatch,
    BackendUnavailable(String),
    BackpressureVisible,
    BudgetExceeded(&'static str),
    CrossContextBatchDenied,
    DuplicateReceipt(ReceiptId),
    IdExhausted,
    MandatoryLaneMissing(usize),
    LogicalLaneMissing(usize),
    ReceiptIssueFailed,
    ShapeMismatch(&'static str),
}

pub fn fixture_input(rows: usize) -> NumericEpochInput {
    let rows = rows.max(2);
    let axes_x100 = (0..rows)
        .map(|index| {
            [
                (index % 101) as u16,
                ((index * 3 + 7) % 101) as u16,
                ((index * 5 + 11) % 101) as u16,
            ]
        })
        .collect::<Vec<_>>();
    let masks = vec![NumericMask::Known; rows];
    let mut relation_offsets = Vec::with_capacity(rows + 1);
    let mut relation_left = Vec::with_capacity(rows * 2);
    let mut relation_right = Vec::with_capacity(rows * 2);
    relation_offsets.push(0);
    for left in 0..rows {
        let previous = (left + rows - 1) % rows;
        let next = (left + 1) % rows;
        relation_left.push(left as u32);
        relation_right.push(previous as u32);
        if next != previous {
            relation_left.push(left as u32);
            relation_right.push(next as u32);
        }
        relation_offsets.push(relation_left.len() as u32);
    }
    let mut offsets = Vec::with_capacity(rows + 1);
    let mut indices = Vec::with_capacity(rows * 2);
    let mut weights = Vec::with_capacity(rows * 2);
    offsets.push(0);
    for node in 0..rows {
        indices.push(((node + rows - 1) % rows) as u32);
        indices.push(((node + 1) % rows) as u32);
        weights.extend_from_slice(&[10, -10]);
        offsets.push(indices.len() as u32);
    }
    let interactions = (0..rows)
        .map(|left| QuboInteraction {
            left,
            right: (left + 1) % rows,
            coefficient: (left as i32 % 7) - 3,
        })
        .collect::<Vec<_>>();
    NumericEpochInput {
        context_digest: stable_sha256("lc631-fixture-context"),
        row_identity_digests: (0..rows)
            .map(|index| stable_sha256(&format!("lc631-fixture-row:{index}")))
            .collect(),
        axes_x100,
        masks: masks.clone(),
        relation_offsets,
        relation_left,
        relation_right,
        jacobi: JacobiInput {
            offsets,
            indices,
            weights_x100: weights,
            initial_x100: (0..rows).map(|index| (index as i16 % 21) - 10).collect(),
            masks,
            rounds: 2,
        },
        qubo: QuboInput {
            linear: (0..rows).map(|index| (index as i32 % 11) - 5).collect(),
            interactions,
            selected: (0..rows).map(|index| index % 3 == 0).collect(),
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundedRelationPlan {
    pub offsets: Vec<u32>,
    pub left_rows: Vec<u32>,
    pub right_rows: Vec<u32>,
}

pub fn bounded_relation_plan(rows: usize) -> Result<BoundedRelationPlan, AcceleratorError> {
    if rows == 0 || rows > MAX_SUMMARY_ROWS {
        return Err(AcceleratorError::BudgetExceeded("relation_rows"));
    }
    let mut offsets = Vec::with_capacity(rows + 1);
    let mut left_rows = Vec::with_capacity(rows * 2);
    let mut right_rows = Vec::with_capacity(rows * 2);
    offsets.push(0);
    for left in 0..rows {
        let previous = (left + rows - 1) % rows;
        let next = (left + 1) % rows;
        left_rows.push(left as u32);
        right_rows.push(previous as u32);
        if next != previous {
            left_rows.push(left as u32);
            right_rows.push(next as u32);
        }
        offsets.push(left_rows.len() as u32);
    }
    Ok(BoundedRelationPlan {
        offsets,
        left_rows,
        right_rows,
    })
}

pub fn input_from_worlds(
    worlds: &[lc631_world::PositedWorld],
) -> Result<NumericEpochInput, AcceleratorError> {
    if worlds.len() != lc631_world::BASE_WORLD_COUNT {
        return Err(AcceleratorError::ShapeMismatch("world_arena"));
    }
    let unique = worlds
        .iter()
        .map(|world| world.key.canonical_digest())
        .collect::<BTreeSet<_>>();
    if unique.len() != lc631_world::BASE_WORLD_COUNT {
        return Err(AcceleratorError::ShapeMismatch("world_distinctness"));
    }
    let mut input = fixture_input(worlds.len());
    input.axes_x100 = worlds
        .iter()
        .map(|world| {
            [
                world.key.logic.code() * 10,
                world.key.temporal.code() * 25 + world.key.causal.code() * 5,
                u16::from(world.key.ontology_variant) * 20,
            ]
        })
        .collect();
    input.row_identity_digests = worlds
        .iter()
        .map(|world| world.key.canonical_digest())
        .collect();
    input.context_digest = stable_sha256(
        &worlds
            .iter()
            .map(|world| world.key.canonical_digest())
            .collect::<Vec<_>>()
            .join(":"),
    );
    Ok(input)
}

pub fn input_from_evaluations(
    evaluations: &[lc631_world::WorldEvaluation],
) -> Result<NumericEpochInput, AcceleratorError> {
    if evaluations.len() != lc631_world::MAX_EVALUATIONS {
        return Err(AcceleratorError::ShapeMismatch("world_evaluations"));
    }
    let identities = evaluations
        .iter()
        .map(|evaluation| evaluation.evaluation_digest.clone())
        .collect::<Vec<_>>();
    if identities.iter().collect::<BTreeSet<_>>().len() != evaluations.len() {
        return Err(AcceleratorError::ShapeMismatch(
            "world_evaluation_distinctness",
        ));
    }
    let mut input = fixture_input(evaluations.len());
    input.axes_x100 = evaluations
        .iter()
        .map(|evaluation| evaluation.axes_x100)
        .collect();
    input.row_identity_digests = identities.clone();
    input.context_digest = stable_sha256(&identities.join(":"));
    input.validate()?;
    Ok(input)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DecisionParityReport {
    pub context_match: bool,
    pub semantic_binding_match: bool,
    pub row_identity_match: bool,
    pub mask_match: bool,
    pub lane_decisions: BTreeMap<LaneKind, bool>,
    pub all_decision_parity: bool,
}

pub fn compare_decision_witnesses(
    left: &ExecutionReceipt,
    right: &ExecutionReceipt,
) -> DecisionParityReport {
    let context_match = left.context_digest == right.context_digest;
    let semantic_binding_match = left.semantic_binding_digest == right.semantic_binding_digest;
    let row_identity_match = left.row_identity_digest == right.row_identity_digest;
    let mask_match = left.mask_digest == right.mask_digest;
    let envelope_match =
        context_match && semantic_binding_match && row_identity_match && mask_match;
    let right = right
        .lanes
        .iter()
        .map(|lane| (lane.lane, lane))
        .collect::<BTreeMap<_, _>>();
    let lane_decisions = left
        .lanes
        .iter()
        .map(|lane| {
            (
                lane.lane,
                envelope_match
                    && right.get(&lane.lane).is_some_and(|other| {
                        other.rows == lane.rows
                            && other.output_values == lane.output_values
                            && other.summary_schema == lane.summary_schema
                            && other.summary_words == lane.summary_words
                            && other.finalist_rows == lane.finalist_rows
                            && other.finalist_values == lane.finalist_values
                            && other.cutoff_value == lane.cutoff_value
                            && other.cutoff_tie_count == lane.cutoff_tie_count
                            && other.mask_digest == lane.mask_digest
                            && other.decision_digest == lane.decision_digest
                    }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let all_decision_parity = envelope_match
        && lane_decisions.len() == REQUIRED_LOGICAL_LANES.len()
        && REQUIRED_LOGICAL_LANES
            .iter()
            .all(|lane| lane_decisions.get(lane) == Some(&true));
    DecisionParityReport {
        context_match,
        semantic_binding_match,
        row_identity_match,
        mask_match,
        lane_decisions,
        all_decision_parity,
    }
}

pub fn compare_lane_digests(
    left: &ExecutionReceipt,
    right: &ExecutionReceipt,
) -> BTreeMap<LaneKind, bool> {
    compare_decision_witnesses(left, right).lane_decisions
}

#[cfg(test)]
mod tests {
    use super::*;
    use lc631_core::{ArtifactId, AuthorityRevision};

    fn binding() -> RevisionBinding {
        RevisionBinding {
            artifact: ArtifactId(1),
            source_sha256: stable_sha256("source"),
            program_sha256: stable_sha256("program"),
            authority_revision: AuthorityRevision(1),
            engine_version: env!("CARGO_PKG_VERSION").into(),
            kernel_revision: "cpu-v1".into(),
            device_identity: "cpu".into(),
        }
    }

    #[test]
    fn broker_mints_one_receipt_covering_four_mandatory_lanes() {
        let mut broker = AcceleratorBroker::new(CpuBackend);
        let receipt = broker
            .execute_epoch(EpochId(1), &binding(), &fixture_input(16))
            .unwrap();
        assert_eq!(receipt.owner, ReceiptOwner::AcceleratorBroker);
        assert_eq!(receipt.mandatory_lane_coverage, 4);
        assert_eq!(receipt.logical_lane_coverage, 8);
        assert!(!receipt.gpu_creates_authority);
        assert!(!receipt.gpu_creates_evidence);
        assert!(!receipt.gpu_creates_output_commit);
    }

    #[test]
    fn scheduler_rejects_cross_context_and_visible_backpressure() {
        let mut scheduler = WorkConservingScheduler::new(1).unwrap();
        scheduler
            .submit(AcceleratorJob {
                epoch: EpochId(1),
                context_digest: "a".into(),
            })
            .unwrap();
        assert_eq!(
            scheduler.submit(AcceleratorJob {
                epoch: EpochId(2),
                context_digest: "a".into(),
            }),
            Err(AcceleratorError::BackpressureVisible)
        );
        scheduler.pop();
        scheduler
            .submit(AcceleratorJob {
                epoch: EpochId(3),
                context_digest: "b".into(),
            })
            .unwrap();
        assert_eq!(scheduler.len(), 1);
    }

    #[test]
    fn every_fault_forbids_partial_stitching() {
        for fault in [
            FaultKind::DeviceLost,
            FaultKind::OutOfMemory,
            FaultKind::Panic,
            FaultKind::Timeout,
            FaultKind::PartialReadback,
            FaultKind::StaleCompletion,
            FaultKind::HostCancellation,
        ] {
            let disposition = abort_fault(fault);
            assert!(!disposition.published);
            assert!(!disposition.partial_values_reused);
            assert!(disposition.arena_quarantined);
            assert!(disposition.fresh_cpu_fallback_required);
        }
    }

    #[test]
    fn world_arena_input_covers_distinct_logical_lanes_without_synthetic_padding() {
        let arena = lc631_world::WorldArena::new("world-backed accelerator input").unwrap();
        let input = input_from_worlds(arena.worlds()).unwrap();
        assert_eq!(input.rows(), 256);
        let mut broker = AcceleratorBroker::new(CpuBackend);
        let receipt = broker
            .execute_epoch(EpochId(2), &binding(), &input)
            .unwrap();
        assert_eq!(receipt.logical_lane_coverage, 8);
        assert_eq!(receipt.required_logical_lane_count, 8);
    }

    #[test]
    fn numeric_summary_is_bounded_and_order_sensitive() {
        let left = summary_words_u32(&[1, 2, 3, 4]);
        let right = summary_words_u32(&[4, 3, 2, 1]);
        assert_eq!(left.len(), NUMERIC_SUMMARY_WORDS);
        assert_ne!(left, right);
        let input = fixture_input(256);
        let receipt = projection_lane_observation(&input, &(0..256).collect::<Vec<_>>());
        assert_eq!(receipt.finalist_rows.len(), MAX_FINALIST_ROWS);
        assert_eq!(receipt.readback_values, 0);
    }

    #[test]
    fn backend_device_loss_quarantines_before_any_followup_epoch() {
        struct LostBackend;
        impl sealed::Sealed for LostBackend {}
        impl NumericBackend for LostBackend {
            fn kind(&self) -> BackendKind {
                BackendKind::CubeClWgpu
            }

            fn execute(
                &mut self,
                _input: &NumericEpochInput,
            ) -> Result<RawBackendObservation, AcceleratorError> {
                Err(AcceleratorError::BackendFault(FaultKind::DeviceLost))
            }
        }
        let mut broker = AcceleratorBroker::new(LostBackend);
        assert_eq!(
            broker.execute_epoch(EpochId(1), &binding(), &fixture_input(16)),
            Err(AcceleratorError::BackendFault(FaultKind::DeviceLost))
        );
        assert_eq!(
            broker.execute_epoch(EpochId(2), &binding(), &fixture_input(16)),
            Err(AcceleratorError::ArenaQuarantined)
        );
    }

    #[test]
    fn broker_rejects_backend_identity_mismatch_before_receipt_minting() {
        struct MismatchedBackend;
        impl sealed::Sealed for MismatchedBackend {}
        impl NumericBackend for MismatchedBackend {
            fn kind(&self) -> BackendKind {
                BackendKind::CpuScalar
            }

            fn execute(
                &mut self,
                input: &NumericEpochInput,
            ) -> Result<RawBackendObservation, AcceleratorError> {
                let mut cpu = CpuBackend;
                let mut raw = cpu.execute(input)?;
                raw.backend = BackendKind::CubeClWgpu;
                Ok(raw)
            }
        }
        let mut broker = AcceleratorBroker::new(MismatchedBackend);
        assert_eq!(
            broker.execute_epoch(EpochId(1), &binding(), &fixture_input(16)),
            Err(AcceleratorError::BackendIdentityMismatch)
        );
    }
}
