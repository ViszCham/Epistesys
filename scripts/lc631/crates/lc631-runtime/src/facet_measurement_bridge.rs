//! Actual fixed-plan measurement ingress. Arithmetic is not source validation.
#![forbid(unsafe_code)]
mod identity;
mod paired;

use crate::{facet_ledger as l, formal_boolean as f, formal_measurement as m};
use identity::{digest, measurement_digest, query_digest, revision, validate_computed};
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextState, PremiseEdit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredFacetScope {
    pub baseline: l::Revision,
    pub root_goal: l::Revision,
    pub policy: l::Revision,
}
#[derive(Clone, Copy, Debug)]
pub struct ProbeObligation<'a> {
    pub identity: &'a [u8],
    pub root: l::ObligationId,
    pub property: l::PropertyId,
}
/// References remain declared metadata, never authenticated permissions/reasons.
#[derive(Clone, Copy, Debug)]
pub struct DeclaredDeltaReferences {
    pub reason: l::EvidenceRef,
    pub permission_basis: l::EvidenceRef,
    pub map_back: l::EvidenceRef,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressError {
    InvalidLimit,
    Capacity,
    AllocationFailed,
    ObligationMismatch,
    ProducerMismatch,
    ContextMismatch,
    DuplicateAttachment,
    PairMismatch,
    Ledger(l::LedgerError),
    Arithmetic(lc631_analysis::creativity_measurement::MeasurementError),
}

/// Private records can only arise from actual private-constructor measurements.
/// No ComponentReport/status/numeric payload is accepted as a substitute.
pub struct MeasuredFacetBatch<'a> {
    measured: &'a m::MeasuredFormalLoss<'a>,
    declared: DeclaredFacetScope,
    binding: l::TransformBinding,
    entries: Vec<l::Entry>,
    delta: Option<&'a ContextState>,
    baseline: Option<&'a m::MeasuredFormalLoss<'a>>,
    gain: Option<RationalInterval>,
}

impl<'a> MeasuredFacetBatch<'a> {
    pub fn from_loss(
        measured: &'a m::MeasuredFormalLoss<'a>,
        declared: DeclaredFacetScope,
        obligations: &[ProbeObligation<'_>],
    ) -> Result<Self, IngressError> {
        if obligations.len() != measured.probes().len() || obligations.is_empty() {
            return Err(IngressError::ObligationMismatch);
        }
        let scope = measured.plan().scope();
        let result = measurement_digest(measured)?;
        let id = l::TransformId(digest(
            b"epistesys.facet-ingress.transform.v1\0",
            &[
                &result,
                &declared.baseline.0,
                &declared.root_goal.0,
                &declared.policy.0,
            ],
        ));
        let binding = l::TransformBinding {
            id,
            source: revision(scope.source.digest()),
            target: revision(measured.candidate().revision().digest()),
            source_context: revision(scope.context.interpretation_digest()),
            target_context: revision(measured.candidate().context().interpretation_digest()),
            direction: l::Direction::Forward,
            scope: l::ComparisonScope {
                id: l::ScopeId(*scope.comparison.as_bytes()),
                baseline: declared.baseline,
                root_goal: declared.root_goal,
                probes: revision(scope.probe_set),
                policy: declared.policy,
            },
            producer: l::Revision(digest(
                b"epistesys.facet-ingress.producer.v1\0",
                &[b"fixed-plan-k1/finite-formal-v1.4"],
            )),
            query: l::Revision(query_digest(measured)?),
            result: l::Revision(result),
        };
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(obligations.len() + 1)
            .map_err(|_| IngressError::AllocationFailed)?;
        // This gap is independent of model counterexamples and exact arithmetic.
        entries.push(l::Entry {
            transform: id,
            facet: l::Facet::Uncertainty(l::Gap {
                property: l::PropertyId(digest(
                    b"epistesys.facet-ingress.source-unverified.v1\0",
                    &[],
                )),
                reason: l::GapReason::Unverified,
                required_evidence: l::Revision(digest(
                    b"epistesys.facet-ingress.source-interpretation-check.v1\0",
                    &[],
                )),
                next_action: l::Revision(digest(
                    b"epistesys.facet-ingress.independent-source-check.v1\0",
                    &[],
                )),
            }),
        });
        for (index, probe) in measured.probes().iter().enumerate() {
            let obligation = &obligations[index];
            if obligation.identity != probe.fixed().identity
                || obligations[..index].iter().any(|old| {
                    old.identity == obligation.identity || old.property == obligation.property
                })
            {
                return Err(IngressError::ObligationMismatch);
            }
            validate_computed(measured, probe)?;
            let facet = match probe.state() {
                m::ProbeState::Preserved => None,
                m::ProbeState::Countermodel(_) => Some(l::Facet::Defect(l::Defect {
                    key: l::DefectKey {
                        root: obligation.root,
                        property: obligation.property,
                        source: binding.source,
                        target: binding.target,
                    },
                    axis: l::LossAxis::Semantic,
                    witness: l::EvidenceRef(digest(
                        b"epistesys.facet-ingress.formal-countermodel.v1\0",
                        &[&result, obligation.identity],
                    )),
                })),
                m::ProbeState::Gap(gap) => Some(l::Facet::Uncertainty(l::Gap {
                    property: obligation.property,
                    reason: gap_reason(gap),
                    required_evidence: revision(scope.definition),
                    next_action: binding.query,
                })),
            };
            if let Some(facet) = facet {
                entries.push(l::Entry {
                    transform: id,
                    facet,
                });
            }
        }
        Ok(Self {
            measured,
            declared,
            binding,
            entries,
            delta: None,
            baseline: None,
            gain: None,
        })
    }

    pub fn measurement(&self) -> &'a m::MeasuredFormalLoss<'a> {
        self.measured
    }
    pub fn binding(&self) -> l::TransformBinding {
        self.binding
    }
    pub fn entries(&self) -> &[l::Entry] {
        &self.entries
    }
    pub fn gain_bounds(&self) -> Option<&RationalInterval> {
        self.gain.as_ref()
    }
    pub fn source_interpretation(&self) -> f::SourceInterpretation {
        self.measured.source_interpretation()
    }
    pub fn mapping_basis(&self) -> f::MappingBasis {
        self.measured.mapping_basis()
    }
    pub fn checker_independence(&self) -> f::CheckerIndependence {
        self.measured.checker_independence()
    }
    pub fn context_delta(&self) -> Option<&'a ContextState> {
        self.delta
    }
    pub fn paired_baseline(&self) -> Option<&'a m::MeasuredFormalLoss<'a>> {
        self.baseline
    }

    /// Consume actual checked context edits, retaining declared justification refs.
    /// Delta is structural and never erases a formal defect or SourceUnverified U.
    pub fn with_context_delta(
        mut self,
        context: &'a ContextState,
        references: DeclaredDeltaReferences,
    ) -> Result<Self, IngressError> {
        if self.delta.is_some() {
            return Err(IngressError::DuplicateAttachment);
        }
        let change = context
            .declared_delta()
            .ok_or(IngressError::ContextMismatch)?;
        if context.revision() != self.measured.candidate().context()
            || change.parent() != self.measured.plan().scope().context
            || change.scope().comparison().as_bytes() != &self.binding.scope.id.0
            || change.scope().goal().as_bytes() != &self.declared.root_goal.0
            || change.scope().probes() != self.measured.plan().scope().probe_set
        {
            return Err(IngressError::ContextMismatch);
        }
        if change.edits().len() > 4096 {
            return Err(IngressError::Capacity);
        }
        self.entries
            .try_reserve_exact(change.edits().len())
            .map_err(|_| IngressError::AllocationFailed)?;
        for edit in change.edits() {
            let (property, kind) = match edit {
                PremiseEdit::Add(premise) => (premise.id(), l::EditKind::AddPremise),
                PremiseEdit::Replace { id, .. } => (*id, l::EditKind::Substitution),
                PremiseEdit::Retract { id } => (*id, l::EditKind::RetractPremise),
            };
            self.entries.push(l::Entry {
                transform: self.binding.id,
                facet: l::Facet::Delta(l::Delta {
                    property: l::PropertyId(*property.as_bytes()),
                    before: self.binding.source_context,
                    after: self.binding.target_context,
                    kind,
                    reason: references.reason,
                    permission_basis: references.permission_basis,
                    map_back: references.map_back,
                }),
            });
        }
        self.delta = Some(context);
        Ok(self)
    }
}

/// Atomic append by validating a replacement ledger before publishing either
/// it or the borrowed producer record. No caller-built facet insertion API.
pub struct FacetMeasurementLedger<'a> {
    ledger: l::FacetLedger,
    records: Vec<MeasuredFacetBatch<'a>>,
    max_transforms: usize,
    max_entries: usize,
}
impl<'a> FacetMeasurementLedger<'a> {
    pub fn new(max_transforms: usize, max_entries: usize) -> Result<Self, IngressError> {
        if max_transforms == 0 || max_transforms > 256 || max_entries == 0 || max_entries > 4096 {
            return Err(IngressError::InvalidLimit);
        }
        let ledger =
            l::FacetLedger::new(max_transforms, max_entries).map_err(IngressError::Ledger)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(max_transforms)
            .map_err(|_| IngressError::AllocationFailed)?;
        Ok(Self {
            ledger,
            records,
            max_transforms,
            max_entries,
        })
    }
    pub fn ledger(&self) -> &l::FacetLedger {
        &self.ledger
    }
    pub fn records(&self) -> &[MeasuredFacetBatch<'a>] {
        &self.records
    }
    pub fn admit(&mut self, batch: MeasuredFacetBatch<'a>) -> Result<l::TransformId, IngressError> {
        if self.records.len() >= self.max_transforms
            || self
                .ledger
                .entries()
                .len()
                .checked_add(batch.entries.len())
                .is_none_or(|count| count > self.max_entries)
        {
            return Err(IngressError::Capacity);
        }
        let mut next = l::FacetLedger::new(self.max_transforms, self.max_entries)
            .map_err(IngressError::Ledger)?;
        for binding in self.ledger.transforms() {
            next.admit_transform(*binding)
                .map_err(IngressError::Ledger)?;
        }
        for entry in self.ledger.entries() {
            next.append(*entry).map_err(IngressError::Ledger)?;
        }
        next.admit_transform(batch.binding)
            .map_err(IngressError::Ledger)?;
        for entry in &batch.entries {
            next.append(*entry).map_err(IngressError::Ledger)?;
        }
        let id = batch.binding.id;
        self.records.push(batch);
        self.ledger = next;
        Ok(id)
    }
}

fn gap_reason(gap: m::Gap) -> l::GapReason {
    match gap {
        m::Gap::Missing => l::GapReason::Unobserved,
        m::Gap::Unknown => l::GapReason::Unverified,
        m::Gap::NoComparable(f::NoComparable::MissingTheoryScope) => l::GapReason::Unobserved,
        m::Gap::NoComparable(
            f::NoComparable::SourceContextInconsistent | f::NoComparable::TargetContextInconsistent,
        ) => l::GapReason::Conflict,
        m::Gap::Unsupported(f::Failure::ResourceExhausted) => l::GapReason::ResourceExhausted,
        m::Gap::Unsupported(_) => l::GapReason::Unsupported,
    }
}
