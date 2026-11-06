//! 日本語: 版付きの正確なpolicy・native観測・metric別の派生identity。
//! English: Versioned exact policy, native observations and derived per-metric identities.
use super::*;
use sha2::{Digest, Sha256};

pub(super) struct Hasher<'w, 'm> {
    hash: Sha256,
    work: &'w mut n::Work<'m>,
}
impl Hasher<'_, '_> {
    pub fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.work.charge(bytes.len() as u64 + 8).map_err(native)?;
        self.hash.update((bytes.len() as u64).to_le_bytes());
        self.hash.update(bytes);
        Ok(())
    }
    pub fn number(&mut self, v: u64) -> Result<(), Error> {
        self.bytes(&v.to_le_bytes())
    }
    fn ratio(&mut self, v: &Rational) -> Result<(), Error> {
        self.work
            .charge((v.numer().bits() + v.denom().bits()) / 8 + 64)
            .map_err(native)?;
        self.bytes(&v.numer().to_signed_bytes_le())?;
        self.bytes(&v.denom().to_signed_bytes_le())
    }
    fn end(self) -> Digest32 {
        Digest32::from_bytes(self.hash.finalize().into())
    }
}
pub(super) fn root(
    parent: &m::Root<'_, '_>,
    r: &Registration<'_>,
    w: &mut n::Work<'_>,
) -> Result<(Digest32, Digest32), Error> {
    let mut implementation = Hasher {
        hash: Sha256::new(),
        work: w,
    };
    implementation.bytes(b"epistesys.finite-framework.p3.producer.v1")?;
    for unit in [
        include_bytes!("../semantic.rs").as_slice(),
        include_bytes!("types.rs"),
        include_bytes!("root.rs"),
        include_bytes!("admission.rs"),
        include_bytes!("bank.rs"),
        include_bytes!("status.rs"),
        include_bytes!("bridge.rs"),
        include_bytes!("metrics.rs"),
        include_bytes!("current.rs"),
        include_bytes!("identity.rs"),
        include_bytes!("../../../../../../lc631-analysis/src/creativity_measurement/leaves.rs"),
        include_bytes!("../../../../../../lc631-substrate/src/measurement_numeric.rs"),
        include_bytes!("../../../../../../lc631-substrate/src/measurement_numeric/interval.rs"),
        include_bytes!("../../../../../../lc631-substrate/src/measurement_numeric/operations.rs"),
        include_bytes!(
            "../../../../../../lc631-substrate/src/measurement_numeric/transcendental.rs"
        ),
    ] {
        implementation.bytes(unit)?;
    }
    let producer = implementation.end();
    let mut h = Hasher {
        hash: Sha256::new(),
        work: w,
    };
    h.bytes(b"epistesys.finite-framework.p3.root.v1")?;
    for digest in [
        parent.historical_digest(),
        producer,
        r.definition_revision,
        r.oracle_revision,
        r.policy_revision,
    ] {
        h.bytes(digest.as_bytes())?;
    }
    h.number(u64::from(r.status_map.is_some()))?;
    h.number(u64::from(r.bridge_map.is_some()))?;
    // 日本語: 登録順序は固定集合とroleの意味を変更しない。
    // English: Registration order cannot change the fixed set or the role's semantic meaning.
    for role in ROLES {
        h.number(role as u64)?;
        if let Some(p) = r.semantic.iter().find(|p| p.role == role) {
            h.number(1)?;
            h.ratio(p.weight)?;
        } else {
            h.number(0)?;
        }
        if let Some(p) = r.bridges.iter().find(|p| p.role == role) {
            h.number(1)?;
            h.number(p.rule as u64)?;
            h.ratio(p.weight)?;
        } else {
            h.number(0)?;
        }
    }
    let l = r.limits;
    for v in [
        l.max_probes as u64,
        l.max_bridge_probes as u64,
        l.max_assessments as u64,
        l.max_current_checks as u64,
        l.max_native_dispatches as u64,
        l.max_lowered_nodes as u64,
        l.max_payload_bytes as u64,
        u64::from(l.max_weight_bits),
        u64::from(l.max_intermediate_bits),
        l.plan_ram,
        l.assessment_ram,
        l.scratch_ram,
        l.output_bytes,
        u64::from(l.precision),
    ] {
        h.number(v)?;
    }
    Ok((h.end(), producer))
}
fn state(h: &mut Hasher<'_, '_>, s: &ComponentState) -> Result<(), Error> {
    match s {
        ComponentState::Measured(v) => {
            h.number(0)?;
            h.ratio(v.lower())?;
            h.ratio(v.upper())
        }
        ComponentState::Incomparable { scope } => {
            h.number(1)?;
            h.bytes(scope.as_bytes())
        }
        ComponentState::Undefined(MeasurementError::EmptyDenominator) => h.number(2),
        ComponentState::ResourceExhausted { partial: None } => h.number(3),
        _ => Err(Error::ExpectedInputMismatch),
    }
}
pub(super) fn result(
    root: &Root<'_, '_>,
    pair: m::ExpectedPair<'_, '_, '_>,
    parent: &[ComponentReport; 2],
    bank: &bank::Bank<'_>,
    values: &metrics::Values,
    w: &mut n::Work<'_>,
) -> Result<Digest32, Error> {
    let mut h = Hasher {
        hash: Sha256::new(),
        work: w,
    };
    h.bytes(b"epistesys.finite-framework.p3.results.v1")?;
    h.bytes(root.digest.as_bytes())?;
    for report in parent {
        for pin in [
            report.binding.scope,
            report.binding.baseline,
            report.binding.candidate,
            report.binding.producer,
            report.binding.result,
            report.evidence_payload,
        ] {
            h.bytes(pin.as_bytes())?;
        }
    }
    for case in [pair.source, pair.target] {
        h.number(case.expected.kernel as u64)?;
        h.number(match case.expected.subject {
            f::Subject::Original => 0,
            f::Subject::Generated(_) => 1,
        })?;
    }
    h.number(values.status as u64)?;
    for n in [
        bank.counts.required_not_dispatches,
        bank.counts.native_not_dispatches,
        bank.counts.native_not_store_entries,
    ] {
        h.number(n as u64)?;
    }
    bank.bind_native(&mut h)?;
    h.ratio(&values.semantic_total)?;
    h.ratio(&values.bridge_total)?;
    for r in &values.bridges {
        h.number(r.role as u64)?;
        h.number(r.rule as u64)?;
        h.number(u64::from(r.passed()))?;
        h.number(r.mismatches().len() as u64)?;
        for witness in r.mismatches() {
            h.bytes(witness.target_states())?;
            h.bytes(witness.lost_future_atoms())?;
            h.number(u64::from(witness.source_root_forced()))?;
            h.number(u64::from(witness.target_root_forced()))?;
        }
    }
    for s in &values.states {
        state(&mut h, s)?;
    }
    Ok(h.end())
}
fn node_key(report: &ComponentReport, w: &mut n::Work<'_>) -> Result<Digest32, Error> {
    let mut h = Hasher {
        hash: Sha256::new(),
        work: w,
    };
    h.bytes(b"epistesys.finite-framework.p3.value-node.v1")?;
    h.number(report.metric as u64)?;
    h.bytes(report.definition.as_bytes())?;
    // 日本語: ValueNodeの完全なbindingとprobe setを、入力reportのまま固定する。
    // English: Bind the complete ValueNode binding and probe set from the actual input report.
    let b = report.binding;
    for pin in [
        b.scope,
        b.candidate,
        b.source,
        b.target,
        b.context,
        b.baseline,
        b.producer,
        b.transform,
        b.result,
        report.probe_set,
    ] {
        h.bytes(pin.as_bytes())?;
    }
    Ok(h.end())
}
fn metric_report(
    root: &Root<'_, '_>,
    dpar: &ComponentReport,
    metric: ComponentMetric,
    state: ComponentState,
    evidence: Digest32,
    w: &mut n::Work<'_>,
) -> Result<ComponentReport, Error> {
    w.charge(512).map_err(native)?;
    let mut h = Sha256::new();
    h.update(b"epistesys.finite-framework.p3.metric.v1\0");
    h.update(root.producer.as_bytes());
    h.update(root.digest.as_bytes());
    h.update((metric as u16).to_le_bytes());
    let producer = Digest32::from_bytes(h.finalize().into());
    let mut r = Sha256::new();
    r.update(b"epistesys.finite-framework.p3.metric-result.v1\0");
    r.update(producer.as_bytes());
    r.update(evidence.as_bytes());
    let metric_result = Digest32::from_bytes(r.finalize().into());
    // 日本語: 実reference/controlを含む共通subjectは、P2のsubjectをそのまま使う。
    // English: Retain P2's exact common subject, including its actual reference/control identity.
    Ok(ComponentReport {
        metric,
        definition: root.registration.definition_revision,
        binding: lc631_analysis::creativity_measurement::MeasurementBinding {
            producer,
            transform: root.registration.policy_revision,
            result: metric_result,
            ..dpar.binding
        },
        probe_set: root.digest,
        oracle: root.registration.oracle_revision,
        dependency_group: dpar.dependency_group,
        evidence_payload: evidence,
        basis: f::InputBasis::Given,
        validation: f::SemanticValidation::Unassessed,
        independence: f::IndependenceStatus::SharedDependency,
        state,
    })
}
pub(super) fn reports(
    root: &Root<'_, '_>,
    dpar: &ComponentReport,
    result: Digest32,
    states: [ComponentState; 3],
    w: &mut n::Work<'_>,
) -> Result<[ComponentReport; 3], Error> {
    let [sem, compat, tnc] = states;
    let semantic = metric_report(
        root,
        dpar,
        ComponentMetric::SemanticConfigurationDistance,
        sem,
        result,
        w,
    )?;
    let compatibility = metric_report(
        root,
        dpar,
        ComponentMetric::Compatibility,
        compat,
        result,
        w,
    )?;
    let inputs = [
        node_key(dpar, w)?,
        node_key(&semantic, w)?,
        node_key(&compatibility, w)?,
    ];
    let mut h = Hasher {
        hash: Sha256::new(),
        work: w,
    };
    h.bytes(b"epistesys.finite-framework.p3.tnc-derived-input-nodes.v1")?;
    h.bytes(root.digest.as_bytes())?;
    h.bytes(result.as_bytes())?;
    // 日本語: 実D_par・D_sem・A_compatの順で三入力nodeを固定し、共有依存を保持する。
    // English: Bind the actual D_par, D_sem and A_compat nodes in that order, retaining shared dependency.
    for input in inputs {
        h.bytes(input.as_bytes())?;
    }
    let derived = h.end();
    let novelty = metric_report(
        root,
        dpar,
        ComponentMetric::NoveltyCompatibility,
        tnc,
        derived,
        w,
    )?;
    Ok([semantic, compatibility, novelty])
}
