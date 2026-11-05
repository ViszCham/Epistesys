//! K2 over actual fixed-plan preservation outcomes, not aggregate-mean subtraction.
use super::identity::measurement_digest;
use super::{digest, f, l, m, IngressError, MeasuredFacetBatch};
use lc631_analysis::creativity_measurement::{paired_gain, ProbePair, RationalInterval};
use lc631_substrate::Digest32;

impl<'a> MeasuredFacetBatch<'a> {
    /// G is a measured formal-model change, still source-unverified/shared-IR.
    /// Definition is explicit; this is not the absolute CR-PRESERVE quantity.
    pub fn with_paired_preservation_gain(
        mut self,
        baseline: &'a m::MeasuredFormalLoss<'a>,
    ) -> Result<Self, IngressError> {
        if self.baseline.is_some() {
            return Err(IngressError::DuplicateAttachment);
        }
        if !std::ptr::eq(baseline.plan(), self.measured.plan())
            || baseline.direction() != self.measured.direction()
            || baseline.candidate().id().as_bytes() != &self.declared.baseline.0
            || baseline.probes().len() != self.measured.probes().len()
        {
            return Err(IngressError::PairMismatch);
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut pairs = Vec::new();
        let count = baseline.probes().len();
        before
            .try_reserve_exact(count)
            .map_err(|_| IngressError::AllocationFailed)?;
        after
            .try_reserve_exact(count)
            .map_err(|_| IngressError::AllocationFailed)?;
        pairs
            .try_reserve_exact(count)
            .map_err(|_| IngressError::AllocationFailed)?;
        for (base, candidate) in baseline.probes().iter().zip(self.measured.probes()) {
            super::validate_computed(baseline, base)?;
            super::validate_computed(self.measured, candidate)?;
            before.push(success(base.state())?);
            after.push(success(candidate.state())?);
        }
        let base_digest = measurement_digest(baseline)?;
        let candidate_digest = measurement_digest(self.measured)?;
        let definition = digest(
            b"epistesys.facet-ingress.definition.v1\0",
            &[
                b"PairedFormalPreservationChange/K2-v1",
                self.measured.plan().scope().definition.as_bytes(),
            ],
        );
        let mut complete = true;
        self.entries
            .try_reserve_exact(count + 1)
            .map_err(|_| IngressError::AllocationFailed)?;
        for (index, probe) in self.measured.probes().iter().enumerate() {
            let base = &baseline.probes()[index];
            let base_report = computed_report(base);
            let candidate_report = computed_report(probe);
            let same_observation = matches!(
                (base_report, candidate_report),
                (Some(left), Some(right)) if std::ptr::eq(left, right)
            );
            complete &= base_report.is_some() && candidate_report.is_some();
            if let m::ProbeState::Gap(gap) = base.state() {
                self.entries.push(l::Entry {
                    transform: self.binding.id,
                    facet: l::Facet::Uncertainty(l::Gap {
                        property: l::PropertyId(digest(
                            b"epistesys.facet-ingress.baseline-gap.v1\0",
                            &[probe.fixed().identity],
                        )),
                        reason: super::gap_reason(gap),
                        required_evidence: l::Revision(
                            *baseline.plan().scope().definition.as_bytes(),
                        ),
                        next_action: self.binding.query,
                    }),
                });
            }
            let node = |measured: &[u8; 32], side: &[u8]| {
                Digest32::from_bytes(digest(
                    b"epistesys.facet-ingress.k2-node.v1\0",
                    &[&definition, measured, probe.fixed().identity, side],
                ))
            };
            pairs.push(ProbePair {
                canonical_payload: probe.fixed().identity,
                weight: probe.fixed().weight,
                baseline: &before[index],
                candidate: &after[index],
                baseline_node: node(&base_digest, b"baseline"),
                candidate_node: if same_observation {
                    node(&base_digest, b"baseline")
                } else {
                    node(&candidate_digest, b"candidate")
                },
            });
        }
        let gain = paired_gain(&pairs).map_err(IngressError::Arithmetic)?;
        if complete {
            self.entries.push(l::Entry {
                transform: self.binding.id,
                facet: l::Facet::Gain(l::Gain {
                    primitive: l::PrimitiveId(*self.measured.plan().scope().probe_set.as_bytes()),
                    metric: 0x0101,
                    definition: l::Revision(definition),
                    unit: l::Revision(digest(
                        b"epistesys.facet-ingress.unit.v1\0",
                        &[b"dimensionless"],
                    )),
                    baseline_observation: l::EvidenceRef(base_digest),
                    candidate_observation: l::EvidenceRef(candidate_digest),
                    kind: l::GainKind::ObservedReported,
                }),
            });
        }
        self.gain = Some(gain);
        self.baseline = Some(baseline);
        Ok(self)
    }
}

fn computed_report<'a>(probe: &'a m::ProbeLineage<'a>) -> Option<&'a f::ComputedReport<'a>> {
    match probe.observation()?.value {
        m::ObservationValue::Computed(report) => Some(report),
        m::ObservationValue::Gap(_) => None,
    }
}
fn success(state: m::ProbeState) -> Result<RationalInterval, IngressError> {
    match state {
        m::ProbeState::Preserved => RationalInterval::exact_ratio(1, 1),
        m::ProbeState::Countermodel(_) => RationalInterval::exact_ratio(0, 1),
        m::ProbeState::Gap(_) => RationalInterval::from_ratio(0, 1, 1, 1),
    }
    .map_err(|error| IngressError::Arithmetic(error.into()))
}
