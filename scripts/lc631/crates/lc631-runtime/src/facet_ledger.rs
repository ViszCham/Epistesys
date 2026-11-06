//! Preparatory REC-D/DELTA/U/G event store, not a metric evaluator or authority.
//! Caller supplied pins are references, never authenticated evidence.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TransformId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ScopeId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Revision(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EvidenceRef(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ObligationId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PropertyId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PrimitiveId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Forward,
    Reverse,
}

/// Shared comparison identity excludes candidate-specific revisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonScope {
    pub id: ScopeId,
    pub baseline: Revision,
    pub root_goal: Revision,
    pub probes: Revision,
    pub policy: Revision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransformBinding {
    pub id: TransformId,
    pub source: Revision,
    pub target: Revision,
    pub source_context: Revision,
    pub target_context: Revision,
    pub direction: Direction,
    pub scope: ComparisonScope,
    pub producer: Revision,
    pub query: Revision,
    pub result: Revision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefectKey {
    pub root: ObligationId,
    pub property: PropertyId,
    pub source: Revision,
    pub target: Revision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LossAxis {
    Reconstruction,
    Semantic,
    Temporal,
    Spatial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Defect {
    pub key: DefectKey,
    pub axis: LossAxis,
    pub witness: EvidenceRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditKind {
    Abstraction,
    Substitution,
    AddPremise,
    RetractPremise,
    Rebind,
    Representation,
}

/// Declared change does not discharge preservation obligations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Delta {
    pub property: PropertyId,
    pub before: Revision,
    pub after: Revision,
    pub kind: EditKind,
    pub reason: EvidenceRef,
    pub permission_basis: EvidenceRef,
    pub map_back: EvidenceRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GapReason {
    Unobserved,
    Ambiguous,
    Incomparable,
    Unsupported,
    Unverified,
    Timeout,
    Uncalibrated,
    Conflict,
    Undefined,
    NotApplicable,
    InvalidConfig,
    ResourceExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gap {
    pub property: PropertyId,
    pub reason: GapReason,
    pub required_evidence: Revision,
    pub next_action: Revision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GainKind {
    Predicted,
    ObservedReported,
}

/// A pair of references is not independent verification or numeric gain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gain {
    pub primitive: PrimitiveId,
    pub metric: u16,
    pub definition: Revision,
    pub unit: Revision,
    pub baseline_observation: EvidenceRef,
    pub candidate_observation: EvidenceRef,
    pub kind: GainKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Facet {
    Defect(Defect),
    Delta(Delta),
    Uncertainty(Gap),
    Gain(Gain),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry {
    pub transform: TransformId,
    pub facet: Facet,
}

/// Native finite-result-store pins, never external-process execution receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FiniteObservationBinding {
    pub source: Revision,
    pub context: Revision,
    pub query: Revision,
    pub result: Revision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedgerError {
    InvalidLimit,
    Capacity,
    AllocationFailed,
    UnknownTransform,
    DuplicateTransform,
    DuplicateEvent,
    ScopeMismatch,
    RevisionMismatch,
    BrokenPath,
    EmptyPath,
    IncomparableMeasure,
}

/// Fixed-size entries; no text payload allocation or mutation of admitted entries.
pub struct FacetLedger {
    transforms: Vec<TransformBinding>,
    entries: Vec<Entry>,
    max_transforms: usize,
    max_entries: usize,
}

impl FacetLedger {
    pub fn new(max_transforms: usize, max_entries: usize) -> Result<Self, LedgerError> {
        if max_transforms == 0
            || max_entries == 0
            || max_transforms > 100_000
            || max_entries > 100_000
        {
            return Err(LedgerError::InvalidLimit);
        }
        let mut transforms = Vec::new();
        let mut entries = Vec::new();
        transforms
            .try_reserve_exact(max_transforms)
            .map_err(|_| LedgerError::AllocationFailed)?;
        entries
            .try_reserve_exact(max_entries)
            .map_err(|_| LedgerError::AllocationFailed)?;
        Ok(Self {
            transforms,
            entries,
            max_transforms,
            max_entries,
        })
    }

    pub fn transforms(&self) -> &[TransformBinding] {
        &self.transforms
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn transform(&self, id: TransformId) -> Option<&TransformBinding> {
        self.transforms.iter().find(|value| value.id == id)
    }

    pub fn admit_transform(&mut self, binding: TransformBinding) -> Result<(), LedgerError> {
        if self.transform(binding.id).is_some() {
            return Err(LedgerError::DuplicateTransform);
        }
        if self.transforms.len() == self.max_transforms {
            return Err(LedgerError::Capacity);
        }
        self.transforms.push(binding);
        Ok(())
    }

    /// Atomically admit declared bindings/events; these remain advisory refs.
    /// Empty slices are an explicit no-op, never an observation/completion.
    /// All fallible validation precedes publication into pre-reserved storage.
    pub fn admit_batch(
        &mut self,
        transforms: &[TransformBinding],
        entries: &[Entry],
    ) -> Result<(), LedgerError> {
        if self
            .transforms
            .len()
            .checked_add(transforms.len())
            .is_none_or(|n| n > self.max_transforms)
            || self
                .entries
                .len()
                .checked_add(entries.len())
                .is_none_or(|n| n > self.max_entries)
        {
            return Err(LedgerError::Capacity);
        }
        for (index, binding) in transforms.iter().enumerate() {
            if self.transform(binding.id).is_some()
                || transforms[..index].iter().any(|old| old.id == binding.id)
            {
                return Err(LedgerError::DuplicateTransform);
            }
        }
        for (index, entry) in entries.iter().enumerate() {
            let binding = self
                .transform(entry.transform)
                .or_else(|| transforms.iter().find(|b| b.id == entry.transform))
                .ok_or(LedgerError::UnknownTransform)?;
            if let Facet::Defect(defect) = entry.facet {
                if defect.key.source != binding.source || defect.key.target != binding.target {
                    return Err(LedgerError::RevisionMismatch);
                }
            }
            if self
                .entries
                .iter()
                .chain(entries[..index].iter())
                .any(|old| old.transform == entry.transform && same_event(old.facet, entry.facet))
            {
                return Err(LedgerError::DuplicateEvent);
            }
        }
        self.transforms.extend_from_slice(transforms);
        self.entries.extend_from_slice(entries);
        Ok(())
    }

    pub fn append(&mut self, entry: Entry) -> Result<(), LedgerError> {
        let binding = self
            .transform(entry.transform)
            .ok_or(LedgerError::UnknownTransform)?;
        if let Facet::Defect(defect) = entry.facet {
            if defect.key.source != binding.source || defect.key.target != binding.target {
                return Err(LedgerError::RevisionMismatch);
            }
        }
        if self
            .entries
            .iter()
            .any(|old| old.transform == entry.transform && same_event(old.facet, entry.facet))
        {
            return Err(LedgerError::DuplicateEvent);
        }
        if self.entries.len() == self.max_entries {
            return Err(LedgerError::Capacity);
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Pure integration seam: a native evaluation alone leaves preservation
    /// unverified. Logical truth/Both/Neither are retained by the result store.
    /// This does not reinterpret any logic's truth values as defects or gains.
    pub fn retain_finite_observation(
        &mut self,
        transform: TransformId,
        observation: FiniteObservationBinding,
        property: PropertyId,
        required_evidence: Revision,
        next_action: Revision,
    ) -> Result<(), LedgerError> {
        let binding = self
            .transform(transform)
            .ok_or(LedgerError::UnknownTransform)?;
        if binding.source != observation.source
            || binding.source_context != observation.context
            || binding.query != observation.query
            || binding.result != observation.result
        {
            return Err(LedgerError::RevisionMismatch);
        }
        self.append(Entry {
            transform,
            facet: Facet::Uncertainty(Gap {
                property,
                reason: GapReason::Unverified,
                required_evidence,
                next_action,
            }),
        })
    }

    /// Checks correspondence only; never sums edge losses or infers restoration.
    /// O(path length * admitted transforms); no allocation and bounded path size.
    pub fn check_path(&self, path: &[TransformId]) -> Result<ComparisonScope, LedgerError> {
        if path.is_empty() {
            return Err(LedgerError::EmptyPath);
        }
        if path.len() > self.max_transforms {
            return Err(LedgerError::Capacity);
        }
        let first = self
            .transform(path[0])
            .ok_or(LedgerError::UnknownTransform)?;
        let mut previous = first;
        for id in &path[1..] {
            let next = self.transform(*id).ok_or(LedgerError::UnknownTransform)?;
            if next.scope != first.scope {
                return Err(LedgerError::ScopeMismatch);
            }
            if previous.target != next.source
                || previous.target_context != next.source_context
                || previous.direction != next.direction
            {
                return Err(LedgerError::BrokenPath);
            }
            previous = next;
        }
        Ok(first.scope)
    }

    /// Borrowed view preserves provenance; count is not a loss or gain score.
    pub fn for_transform(&self, id: TransformId) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(move |entry| entry.transform == id)
    }

    /// Logical REC-D view deduplicates repeated paths in the same scope.
    /// Raw entries retain every transform witness. Count is never numeric TL.
    pub fn unique_defects(&self, scope: ComparisonScope) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(move |(index, entry)| {
                let Facet::Defect(defect) = entry.facet else {
                    return None;
                };
                if self.transform(entry.transform)?.scope != scope {
                    return None;
                }
                let repeated = self.entries[..index].iter().any(|prior| {
                    matches!(prior.facet, Facet::Defect(old) if old.key == defect.key)
                        && self
                            .transform(prior.transform)
                            .is_some_and(|binding| binding.scope == scope)
                });
                if repeated {
                    None
                } else {
                    Some(entry)
                }
            })
    }

    /// Shared-primitive view for one candidate binding; observation aliases do
    /// not create independent gain events. Different candidates remain distinct.
    pub fn unique_gains(&self, scope: ComparisonScope) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(move |(index, entry)| {
                let Facet::Gain(gain) = entry.facet else {
                    return None;
                };
                let binding = self.transform(entry.transform)?;
                if binding.scope != scope {
                    return None;
                }
                let repeated = self.entries[..index].iter().any(|prior| {
                    let Facet::Gain(old) = prior.facet else {
                        return false;
                    };
                    old.primitive == gain.primitive
                        && old.metric == gain.metric
                        && old.definition == gain.definition
                        && old.unit == gain.unit
                        && old.kind == gain.kind
                        && self.transform(prior.transform).is_some_and(|other| {
                            other.scope == scope
                                && other.source == binding.source
                                && other.target == binding.target
                                && other.source_context == binding.source_context
                                && other.target_context == binding.target_context
                                && other.direction == binding.direction
                        })
                });
                if repeated {
                    None
                } else {
                    Some(entry)
                }
            })
    }

    /// Reject a mixed measurement before any downstream arithmetic.
    pub fn comparable_gains(&self, left: Entry, right: Entry) -> Result<(), LedgerError> {
        let a = self
            .transform(left.transform)
            .ok_or(LedgerError::UnknownTransform)?;
        let b = self
            .transform(right.transform)
            .ok_or(LedgerError::UnknownTransform)?;
        if a.scope != b.scope {
            return Err(LedgerError::ScopeMismatch);
        }
        match (left.facet, right.facet) {
            (Facet::Gain(x), Facet::Gain(y))
                if x.metric == y.metric
                    && x.definition == y.definition
                    && x.unit == y.unit
                    && a.direction == b.direction =>
            {
                Ok(())
            }
            _ => Err(LedgerError::IncomparableMeasure),
        }
    }
}

fn same_event(a: Facet, b: Facet) -> bool {
    match (a, b) {
        (Facet::Defect(x), Facet::Defect(y)) => x.key == y.key,
        (Facet::Delta(x), Facet::Delta(y)) => {
            x.property == y.property
                && x.before == y.before
                && x.after == y.after
                && x.kind == y.kind
        }
        (Facet::Uncertainty(x), Facet::Uncertainty(y)) => {
            x.property == y.property && x.reason == y.reason
        }
        (Facet::Gain(x), Facet::Gain(y)) => {
            x.primitive == y.primitive
                && x.metric == y.metric
                && x.definition == y.definition
                && x.unit == y.unit
                && x.baseline_observation == y.baseline_observation
                && x.candidate_observation == y.candidate_observation
                && x.kind == y.kind
        }
        _ => false,
    }
}
