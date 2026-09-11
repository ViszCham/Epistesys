use crate::{AnchorState, StructuralKernel};
#[cfg(feature = "gpu-cubecl-wgpu")]
use lc631_accelerator::{compare_lane_digests, CubeClWgpuBackend};
use lc631_accelerator::{
    AcceleratorBroker, CpuBackend, ExecutionReceipt, JacobiInput, LaneKind, NumericEpochInput,
    NumericMask, QuboInput,
};
use lc631_core::{ArtifactId, AuthorityRevision, EpochId, RevisionBinding};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TldgGeometryAcceleratorReport {
    pub cpu_receipt: ExecutionReceipt,
    pub gpu_receipt: Option<ExecutionReceipt>,
    pub lane_parity: BTreeMap<LaneKind, bool>,
    pub real_gpu_observed: bool,
    pub all_lane_parity: bool,
    pub gpu_error: Option<String>,
    pub geometry_creates_authority: bool,
    pub geometry_creates_output_commit: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum TldgGeometryAcceleratorError {
    EmptyKernel,
    Execution(String),
}

pub fn run_tldg_geometry_accelerator(
    kernel: &StructuralKernel,
    execute_gpu: bool,
) -> Result<TldgGeometryAcceleratorReport, TldgGeometryAcceleratorError> {
    let input = numeric_input(kernel)?;
    let mut cpu = AcceleratorBroker::new(CpuBackend);
    let cpu_receipt = cpu
        .execute_epoch(EpochId(16), &binding(kernel, "cpu-scalar"), &input)
        .map_err(|error| TldgGeometryAcceleratorError::Execution(format!("{error:?}")))?;
    let (gpu_receipt, gpu_error, lane_parity) = run_gpu(kernel, execute_gpu, &input, &cpu_receipt);
    let real_gpu_observed = gpu_receipt.is_some();
    let all_lane_parity = real_gpu_observed
        && lane_parity.len() == cpu_receipt.required_logical_lane_count
        && lane_parity.values().all(|matched| *matched);
    Ok(TldgGeometryAcceleratorReport {
        cpu_receipt,
        gpu_receipt,
        lane_parity,
        real_gpu_observed,
        all_lane_parity,
        gpu_error,
        geometry_creates_authority: false,
        geometry_creates_output_commit: false,
        claim_boundary:
            "real GPU receipt covers the encoded numeric shadow only; discrete grammar validation remains canonical"
                .into(),
    })
}

fn numeric_input(
    kernel: &StructuralKernel,
) -> Result<NumericEpochInput, TldgGeometryAcceleratorError> {
    if kernel.anchors.is_empty() {
        return Err(TldgGeometryAcceleratorError::EmptyKernel);
    }
    let axes_x100 = kernel
        .anchors
        .iter()
        .map(|anchor| {
            [
                anchor
                    .relation_degree
                    .saturating_mul(100)
                    .min(u32::from(u16::MAX)) as u16,
                anchor.order.saturating_mul(10).min(u32::from(u16::MAX)) as u16,
                anchor.profile_count.saturating_mul(100),
            ]
        })
        .collect::<Vec<_>>();
    let masks = kernel
        .anchors
        .iter()
        .map(|anchor| match anchor.state {
            AnchorState::Verified => NumericMask::Known,
            AnchorState::Unknown => NumericMask::Unknown,
            AnchorState::Incomparable => NumericMask::Incomparable,
        })
        .collect::<Vec<_>>();
    let rows = axes_x100.len();
    let relation_plan = lc631_accelerator::bounded_relation_plan(rows)
        .map_err(|error| TldgGeometryAcceleratorError::Execution(format!("{error:?}")))?;
    Ok(NumericEpochInput {
        context_digest: kernel.kernel_digest.clone(),
        row_identity_digests: kernel
            .anchors
            .iter()
            .map(|anchor| {
                lc631_core::stable_sha256(&format!("{}:{}", kernel.kernel_digest, anchor.node_id.0))
            })
            .collect(),
        axes_x100,
        masks: masks.clone(),
        relation_offsets: relation_plan.offsets,
        relation_left: relation_plan.left_rows,
        relation_right: relation_plan.right_rows,
        jacobi: JacobiInput {
            offsets: vec![0; rows + 1],
            indices: Vec::new(),
            weights_x100: Vec::new(),
            initial_x100: kernel
                .anchors
                .iter()
                .map(|anchor| i16::try_from(anchor.order).unwrap_or(i16::MAX))
                .collect(),
            masks,
            rounds: 1,
        },
        qubo: QuboInput {
            linear: kernel
                .anchors
                .iter()
                .map(|anchor| i32::from(anchor.profile_count))
                .collect(),
            interactions: Vec::new(),
            selected: vec![false; rows],
        },
    })
}

fn binding(kernel: &StructuralKernel, device: &str) -> RevisionBinding {
    RevisionBinding {
        artifact: ArtifactId(631_160),
        source_sha256: kernel.source_revision.clone(),
        program_sha256: kernel.kernel_digest.clone(),
        authority_revision: AuthorityRevision(0),
        engine_version: env!("CARGO_PKG_VERSION").into(),
        kernel_revision: "lc631-tldg-numeric-shadow.v1".into(),
        device_identity: device.into(),
    }
}

#[cfg(feature = "gpu-cubecl-wgpu")]
fn run_gpu(
    kernel: &StructuralKernel,
    execute_gpu: bool,
    input: &NumericEpochInput,
    cpu: &ExecutionReceipt,
) -> (
    Option<ExecutionReceipt>,
    Option<String>,
    BTreeMap<LaneKind, bool>,
) {
    if !execute_gpu {
        return (None, None, BTreeMap::new());
    }
    let mut gpu = AcceleratorBroker::new(CubeClWgpuBackend::profiled());
    match gpu.execute_epoch(EpochId(16), &binding(kernel, "cubecl-wgpu"), input) {
        Ok(receipt) => {
            let parity = compare_lane_digests(cpu, &receipt);
            (Some(receipt), None, parity)
        }
        Err(error) => (None, Some(format!("{error:?}")), BTreeMap::new()),
    }
}

#[cfg(not(feature = "gpu-cubecl-wgpu"))]
fn run_gpu(
    _kernel: &StructuralKernel,
    execute_gpu: bool,
    _input: &NumericEpochInput,
    _cpu: &ExecutionReceipt,
) -> (
    Option<ExecutionReceipt>,
    Option<String>,
    BTreeMap<LaneKind, bool>,
) {
    (
        None,
        execute_gpu.then(|| "gpu_feature_not_compiled".into()),
        BTreeMap::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze, build_structural_kernel};

    #[test]
    fn cpu_numeric_shadow_is_observed_but_non_authoritative() {
        let artifact = analyze("fn main() {}").unwrap();
        let kernel = build_structural_kernel(&artifact).unwrap();
        let report = run_tldg_geometry_accelerator(&kernel, false).unwrap();
        assert!(!report.real_gpu_observed);
        assert!(!report.geometry_creates_authority);
        assert_eq!(
            report.cpu_receipt.logical_lane_coverage,
            report.cpu_receipt.required_logical_lane_count
        );
        assert_ne!(lc631_core::stable_sha256(&report.claim_boundary), "sha256:");
    }
}
