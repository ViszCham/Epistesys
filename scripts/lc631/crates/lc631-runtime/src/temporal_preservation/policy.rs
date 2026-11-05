//! 日本語: 比較前に固定する保護probe計画。digestは真正性・意味同値の証明ではない。
//! English: Protected probes are fixed before comparison; digests prove neither authenticity nor semantic equivalence.
use super::{TemporalModelInput, TemporalPreservationError, TemporalProbe};
use lc631_core::context::{ContextRevision, ContextScope};
use lc631_solvers::temporal::{ClockSpec, TimeScale};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredTimeTranslation {
    pub source_clock: ClockSpec,
    pub target_clock: ClockSpec,
    /// Translation-only a=1. This declaration is not a calibration receipt.
    pub offset_nanoseconds: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedTemporalRelation {
    /// Explicit source/target predicate pairing, not inferred formula equivalence.
    MappedPredicateTruth,
    Intervals {
        relation: TemporalProbe,
        source: [u16; 4],
        target: [u16; 4],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedTemporalProbe {
    pub id: Digest32,
    /// Positive integer weights are this adapter's bounded rational subset.
    pub weight: u32,
    pub relation: ProtectedTemporalRelation,
}

#[derive(Clone, Copy)]
struct ModelPin {
    context: ContextRevision,
    query: Digest32,
    trace: Digest32,
}
impl ModelPin {
    fn of(input: TemporalModelInput<'_>) -> Self {
        Self {
            context: input.query.context(),
            query: input.query.digest(),
            trace: input.trace.digest(),
        }
    }
    fn matches(self, input: TemporalModelInput<'_>) -> bool {
        self.context == input.query.context()
            && self.query == input.query.digest()
            && self.trace == input.trace.digest()
    }
    fn hash_into(self, hash: &mut Sha256) {
        for digest in [
            self.context.source().digest(),
            self.context.interpretation_digest(),
            self.query,
            self.trace,
        ] {
            hash.update(digest.as_bytes());
        }
    }
}

pub struct TemporalPreservationPolicy {
    source: ModelPin,
    target: ModelPin,
    scope: ContextScope,
    probes: Vec<ProtectedTemporalProbe>,
    mapping: Option<DeclaredTimeTranslation>,
    denominator: u64,
    digest: Digest32,
    binding_digest: Digest32,
}

impl TemporalPreservationPolicy {
    pub fn declared(
        source: TemporalModelInput<'_>,
        target: TemporalModelInput<'_>,
        probes: Vec<ProtectedTemporalProbe>,
        mapping: Option<DeclaredTimeTranslation>,
    ) -> Result<Self, TemporalPreservationError> {
        if source.scope != target.scope || probes.is_empty() || probes.len() > 256 {
            return Err(TemporalPreservationError::InvalidProbePlan);
        }
        let mut denominator = 0_u64;
        for (i, probe) in probes.iter().enumerate() {
            if probe.weight == 0
                || probe.weight > 1_000_000
                || probes[..i].iter().any(|p| p.id == probe.id)
            {
                return Err(TemporalPreservationError::InvalidProbePlan);
            }
            if let ProtectedTemporalRelation::Intervals {
                source: a,
                target: b,
                ..
            } = probe.relation
            {
                validate_indices(a, source.trace.steps().len())?;
                validate_indices(b, target.trace.steps().len())?;
            }
            denominator = denominator
                .checked_add(u64::from(probe.weight))
                .ok_or(TemporalPreservationError::InvalidProbePlan)?;
        }
        let mut policy = Self {
            source: ModelPin::of(source),
            target: ModelPin::of(target),
            scope: source.scope,
            probes,
            mapping,
            denominator,
            digest: Digest32::from_bytes([0; 32]),
            binding_digest: Digest32::from_bytes([0; 32]),
        };
        policy.digest = policy.compute_digest();
        let mut binding = Sha256::new();
        binding.update(b"epistesys.actual-temporal-preservation.binding.v1\0");
        binding.update(policy.digest.as_bytes());
        policy.source.hash_into(&mut binding);
        policy.target.hash_into(&mut binding);
        policy.binding_digest = Digest32::from_bytes(binding.finalize().into());
        Ok(policy)
    }
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
    pub const fn binding_digest(&self) -> Digest32 {
        self.binding_digest
    }
    pub const fn denominator(&self) -> u64 {
        self.denominator
    }
    pub fn probes(&self) -> &[ProtectedTemporalProbe] {
        &self.probes
    }
    pub const fn mapping(&self) -> Option<DeclaredTimeTranslation> {
        self.mapping
    }
    pub const fn scope(&self) -> ContextScope {
        self.scope
    }
    pub(super) fn matches(
        &self,
        source: TemporalModelInput<'_>,
        target: TemporalModelInput<'_>,
    ) -> bool {
        self.scope == source.scope
            && self.scope == target.scope
            && self.source.matches(source)
            && self.target.matches(target)
    }
    pub(super) fn legacy_binding(&self) -> lc631_analysis::spatiotemporal_loss::Binding {
        use lc631_analysis::spatiotemporal_loss::{Binding, Direction};
        Binding {
            scope: self.scope.comparison(),
            probe_set: self.scope.probes(),
            policy: self.digest,
            source_revision: self.source.context.source().digest(),
            target_revision: self.target.context.source().digest(),
            direction: Direction::Forward,
        }
    }
    fn compute_digest(&self) -> Digest32 {
        let mut hash = Sha256::new();
        hash.update(b"epistesys.actual-temporal-preservation.translation-binary.v1\0");
        for value in [
            self.scope.comparison(),
            self.scope.goal(),
            self.scope.probes(),
            self.scope.semantics(),
        ] {
            hash.update(value.as_bytes());
        }
        match self.mapping {
            None => hash.update([0]),
            Some(map) => {
                hash.update([1]);
                hash.update(clock_digest(map.source_clock).as_bytes());
                hash.update(clock_digest(map.target_clock).as_bytes());
                hash.update(map.offset_nanoseconds.to_le_bytes());
            }
        }
        hash.update((self.probes.len() as u16).to_le_bytes());
        for probe in &self.probes {
            hash.update(probe.id.as_bytes());
            hash.update(probe.weight.to_le_bytes());
            match probe.relation {
                ProtectedTemporalRelation::MappedPredicateTruth => hash.update([0]),
                ProtectedTemporalRelation::Intervals {
                    relation,
                    source,
                    target,
                } => {
                    hash.update([match relation {
                        TemporalProbe::Before => 1,
                        TemporalProbe::Meets => 2,
                        TemporalProbe::EqualDuration => 3,
                        TemporalProbe::MappedAbsoluteStart => 4,
                    }]);
                    for index in source.into_iter().chain(target) {
                        hash.update(index.to_le_bytes());
                    }
                }
            }
        }
        Digest32::from_bytes(hash.finalize().into())
    }
}

fn validate_indices(indices: [u16; 4], length: usize) -> Result<(), TemporalPreservationError> {
    if indices
        .into_iter()
        .any(|index| usize::from(index) >= length)
        || indices[0] >= indices[1]
        || indices[2] >= indices[3]
    {
        return Err(TemporalPreservationError::InvalidProbePlan);
    }
    Ok(())
}

pub(super) fn clock_digest(clock: ClockSpec) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.temporal-preservation.exact-clock.v1\0");
    for value in [clock.clock(), clock.origin(), clock.reference()] {
        hash.update(value.as_bytes());
    }
    hash.update([match clock.scale() {
        TimeScale::Monotonic => 0,
        TimeScale::Utc => 1,
        TimeScale::Tai => 2,
    }]);
    hash.update(clock.precision_nanoseconds().to_le_bytes());
    Digest32::from_bytes(hash.finalize().into())
}
