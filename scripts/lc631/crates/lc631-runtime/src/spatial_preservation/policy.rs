//! 日本語: 固定frame/unit/mapと保護probe。candidate pinsはshared policyから分離する。
//! English: Pin frames/units/maps/probes; separate candidate pins from shared policy.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredSpatialMap {
    pub source_frame: FrameId,
    pub target_frame: FrameId,
    pub source_unit: SpatialUnit,
    pub target_unit: SpatialUnit,
    pub signs: (AxisSign, AxisSign),
    pub translation: (i64, i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedSpatialRelation {
    Kernel(SpatialRelation),
    /// Both rectangle corner pairs mapped by the existing grid comparator.
    MappedPlacement,
    /// No registered continuous comparator in this bounded binary profile.
    ContinuousDistance,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedSpatialProbe {
    pub id: Digest32,
    pub weight: u32,
    pub relation: ProtectedSpatialRelation,
}

pub struct SpatialPreservationPolicy {
    source: SpatialModelInput,
    target: SpatialModelInput,
    probes: Vec<ProtectedSpatialProbe>,
    mapping: Option<DeclaredSpatialMap>,
    denominator: u64,
    digest: Digest32,
    binding: Digest32,
}
impl SpatialPreservationPolicy {
    pub fn declared(
        source: SpatialModelInput,
        target: SpatialModelInput,
        probes: Vec<ProtectedSpatialProbe>,
        mapping: Option<DeclaredSpatialMap>,
    ) -> Result<Self, SpatialPreservationError> {
        if source.scope != target.scope || probes.is_empty() || probes.len() > 6 {
            return Err(SpatialPreservationError::InvalidProbePlan);
        }
        let mut denominator = 0_u64;
        for (i, probe) in probes.iter().enumerate() {
            if probe.weight == 0
                || probe.weight > 1_000_000
                || probes[..i]
                    .iter()
                    .any(|old| old.id == probe.id || old.relation == probe.relation)
            {
                return Err(SpatialPreservationError::InvalidProbePlan);
            }
            denominator = denominator
                .checked_add(u64::from(probe.weight))
                .ok_or(SpatialPreservationError::InvalidProbePlan)?;
        }
        let mut hash = Sha256::new();
        hash.update(b"epistesys.actual-spatial-preservation.grid-binary.v1\0");
        for digest in [
            source.scope.comparison(),
            source.scope.goal(),
            source.scope.probes(),
            source.scope.semantics(),
        ] {
            hash.update(digest.as_bytes());
        }
        for input in [source, target] {
            hash.update(input.frame.0);
            hash.update([unit_code(input.unit)]);
        }
        match mapping {
            None => hash.update([0]),
            Some(map) => {
                hash.update([1]);
                hash.update(map.source_frame.0);
                hash.update(map.target_frame.0);
                hash.update([
                    unit_code(map.source_unit),
                    unit_code(map.target_unit),
                    sign_code(map.signs.0),
                    sign_code(map.signs.1),
                ]);
                hash.update(map.translation.0.to_le_bytes());
                hash.update(map.translation.1.to_le_bytes());
            }
        }
        hash.update((probes.len() as u16).to_le_bytes());
        for probe in &probes {
            hash.update(probe.id.as_bytes());
            hash.update(probe.weight.to_le_bytes());
            hash.update([relation_code(probe.relation)]);
        }
        let digest = Digest32::from_bytes(hash.finalize().into());
        let mut hash = Sha256::new();
        hash.update(b"epistesys.actual-spatial-preservation.binding.v1\0");
        hash.update(digest.as_bytes());
        hash_input(source, &mut hash);
        hash_input(target, &mut hash);
        let binding = Digest32::from_bytes(hash.finalize().into());
        Ok(Self {
            source,
            target,
            probes,
            mapping,
            denominator,
            digest,
            binding,
        })
    }
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
    pub const fn binding_digest(&self) -> Digest32 {
        self.binding
    }
    pub const fn denominator(&self) -> u64 {
        self.denominator
    }
    pub fn probes(&self) -> &[ProtectedSpatialProbe] {
        &self.probes
    }
    pub const fn mapping(&self) -> Option<DeclaredSpatialMap> {
        self.mapping
    }
    pub(super) fn matches(&self, source: SpatialModelInput, target: SpatialModelInput) -> bool {
        self.source == source && self.target == target
    }
    pub(super) fn legacy_binding(&self) -> lc631_analysis::spatiotemporal_loss::Binding {
        use lc631_analysis::spatiotemporal_loss::{Binding, Direction};
        Binding {
            scope: self.source.scope.comparison(),
            probe_set: self.source.scope.probes(),
            policy: self.digest,
            source_revision: self.source.context.source().digest(),
            target_revision: self.target.context.source().digest(),
            direction: Direction::Forward,
        }
    }
}

fn hash_input(input: SpatialModelInput, hash: &mut Sha256) {
    for value in [
        input.context.source().digest(),
        input.context.interpretation_digest(),
    ] {
        hash.update(value.as_bytes());
    }
    for rect in [input.left, input.right] {
        match rect {
            None => hash.update([0]),
            Some(rect) => {
                hash.update([1]);
                hash.update(rect.frame().0);
                for coordinate in rect.min().into_iter().chain(rect.max()) {
                    hash.update(coordinate.to_le_bytes());
                }
            }
        }
    }
}
fn unit_code(unit: SpatialUnit) -> u8 {
    match unit {
        SpatialUnit::Pixel => 0,
        SpatialUnit::Millimeter => 1,
    }
}
fn sign_code(sign: AxisSign) -> u8 {
    match sign {
        AxisSign::Positive => 0,
        AxisSign::Negative => 1,
    }
}
fn relation_code(relation: ProtectedSpatialRelation) -> u8 {
    match relation {
        ProtectedSpatialRelation::Kernel(SpatialRelation::Contains) => 0,
        ProtectedSpatialRelation::Kernel(SpatialRelation::Connected) => 1,
        ProtectedSpatialRelation::Kernel(SpatialRelation::Touches) => 2,
        ProtectedSpatialRelation::Kernel(SpatialRelation::StrictlyLeftOf) => 3,
        ProtectedSpatialRelation::MappedPlacement => 4,
        ProtectedSpatialRelation::ContinuousDistance => 5,
    }
}
