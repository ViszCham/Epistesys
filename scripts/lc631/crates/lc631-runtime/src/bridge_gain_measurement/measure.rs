//! 日本語: privateなactual useのK2とcurrent binding。親scalarを改名しない。
//! English: K2 over private actual uses/current bindings, never parent scalar relabeling.
use super::*;
use lc631_analysis::creativity_measurement::{
    paired_gain, ComponentMetric, ComponentState, IndependenceStatus, InputBasis, ProbePair,
    Rational, SemanticValidation,
};
use lc631_substrate::measurement_numeric::BigInt;
use sha2::{Digest, Sha256};

pub struct OwnedBridgeGain<'a, 'b, 'o, 'p, 'i, 'v> {
    report: ComponentReport,
    baseline: &'a BridgeUseBatch<'b, 'o, 'p, 'i, 'v>,
    candidate: &'a BridgeUseBatch<'b, 'o, 'p, 'i, 'v>,
    root: &'p BridgeRoot<'i, 'v>,
    retained: ResourceClaim<'v>,
}
pub fn measure_pair<'a, 'b, 'o, 'p, 'i, 'v>(
    root: &'p BridgeRoot<'i, 'v>,
    baseline: &'a BridgeUseBatch<'b, 'o, 'p, 'i, 'v>,
    candidate: &'a BridgeUseBatch<'b, 'o, 'p, 'i, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedBridgeGain<'a, 'b, 'o, 'p, 'i, 'v>, BridgeError> {
    check_pair(root, baseline, candidate, view)?;
    let retained = claim(view, 32 * 1024)?;
    let mut a = Vec::new();
    let mut b = Vec::new();
    a.try_reserve_exact(4)
        .map_err(|_| BridgeError::AllocationFailed)?;
    b.try_reserve_exact(4)
        .map_err(|_| BridgeError::AllocationFailed)?;
    let seed = Digest32::from_bytes([0; 32]);
    let mut keys = [seed; 4];
    let mut an = [seed; 4];
    let mut bn = [seed; 4];
    let mut result = Sha256::new();
    result.update(b"CR-BRIDGE/actual-model-uses/result/v1\0");
    for (i, case) in root.order.into_iter().enumerate() {
        let ar = baseline.record(case, view)?;
        let br = candidate.record(case, view)?;
        let av = execute::checked_use(baseline.branch, ar, view)?;
        let bv = execute::checked_use(candidate.branch, br, view)?;
        a.push(
            RationalInterval::exact_ratio(i64::from(av), 1)
                .map_err(|error| BridgeError::Measurement(error.into()))?,
        );
        b.push(
            RationalInterval::exact_ratio(i64::from(bv), 1)
                .map_err(|error| BridgeError::Measurement(error.into()))?,
        );
        let line = ar.owner.lineage_current(view).map_err(from_c)?;
        keys[i] = line.declared_inputs[case];
        an[i] = node(ar, 0, view)?;
        bn[i] = node(br, 1, view)?;
        result.update(keys[i].as_bytes());
        result.update(an[i].as_bytes());
        result.update(bn[i].as_bytes());
        result.update([u8::from(av), u8::from(bv)]);
    }
    let one = Rational::from_integer(BigInt::from(1));
    let probes: [ProbePair<'_>; 4] = std::array::from_fn(|i| ProbePair {
        canonical_payload: keys[i].as_bytes(),
        weight: &one,
        baseline: &a[i],
        candidate: &b[i],
        baseline_node: an[i],
        candidate_node: bn[i],
    });
    let value = paired_gain(&probes).map_err(BridgeError::Measurement)?;
    let result = Digest32::from_bytes(result.finalize().into());
    let parent = candidate.branch.parent_report_current(view)?;
    let before = baseline.branch.parent_report_current(view)?;
    let mut transform = Sha256::new();
    transform.update(b"CR-BRIDGE/fixed-baseline-candidate/v1\0");
    transform.update(before.binding.candidate.as_bytes());
    transform.update(parent.binding.candidate.as_bytes());
    transform.update(before.binding.transform.as_bytes());
    transform.update(parent.binding.transform.as_bytes());
    let mut binding = parent.binding;
    binding.producer = digest(b"finite-formal-v1.4/CR-BRIDGE/ThreeBooleanPortServices-v1");
    binding.transform = Digest32::from_bytes(transform.finalize().into());
    binding.result = result;
    let report = ComponentReport {
        metric: ComponentMetric::Bridge,
        definition: root.input.pins.definition,
        binding,
        probe_set: root.input.scope.probes(),
        oracle: root.input.pins.oracle,
        dependency_group: digest(b"Given-C-Boolean-shared-IR-checker-v1"),
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: ComponentState::Measured(value),
    };
    check_pair(root, baseline, candidate, view)?;
    check_claim(&retained, view)?;
    Ok(OwnedBridgeGain {
        report,
        baseline,
        candidate,
        root,
        retained,
    })
}
impl OwnedBridgeGain<'_, '_, '_, '_, '_, '_> {
    pub fn report_current(
        &self,
        root: &BridgeRoot<'_, '_>,
        candidate: &BridgeProgram<'_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&ComponentReport, BridgeError> {
        root.check(view)?;
        if !std::ptr::eq(root, self.root) || !std::ptr::eq(candidate, self.candidate.branch.program)
        {
            return Err(BridgeError::BindingMismatch);
        }
        check_pair(self.root, self.baseline, self.candidate, view)?;
        check_claim(&self.retained, view)?;
        Ok(&self.report)
    }
}
fn check_pair(
    root: &BridgeRoot<'_, '_>,
    baseline: &BridgeUseBatch<'_, '_, '_, '_, '_>,
    candidate: &BridgeUseBatch<'_, '_, '_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), BridgeError> {
    root.check(view)?;
    baseline.branch.program.check(root, view)?;
    candidate.branch.program.check(root, view)?;
    if baseline.branch.program.role != plan::Role::Baseline
        || candidate.branch.program.role != plan::Role::Candidate
    {
        return Err(BridgeError::RoleMismatch);
    }
    baseline.check(view)?;
    candidate.check(view)?;
    root.check(view)
}
fn node(
    record: &execute::UseRecord<'_, '_, '_, '_, '_>,
    role: u8,
    view: &ResourceReservationView<'_>,
) -> Result<Digest32, BridgeError> {
    let report = record.owner.report_current(view).map_err(from_c)?;
    let line = record.owner.lineage_current(view).map_err(from_c)?;
    let mut h = Sha256::new();
    h.update(b"CR-BRIDGE/actual-use-value-node/v1\0");
    h.update([role]);
    for d in [
        report.definition,
        report.binding.producer,
        report.binding.transform,
        report.binding.result,
        report.probe_set,
        line.budget_series,
        line.queries[3],
        line.queries[4],
        line.declared_inputs[record.case],
    ] {
        h.update(d.as_bytes());
    }
    for frames in [
        line.output_frames,
        line.inverse_frames,
        line.source_model_frames,
        line.target_model_frames,
    ] {
        let frame =
            frames[record.case].ok_or(BridgeError::MissingExecution { case: record.case })?;
        h.update(frame.as_bytes());
    }
    h.update(record.work_end.to_le_bytes());
    h.update((record.case as u64).to_le_bytes());
    h.update(view.owner_id().to_le_bytes());
    h.update(view.run_id().to_le_bytes());
    h.update(view.reservation_id().to_le_bytes());
    Ok(Digest32::from_bytes(h.finalize().into()))
}
fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
