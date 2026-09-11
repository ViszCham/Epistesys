#![forbid(unsafe_code)]

use lc631_core::{stable_sha256, Generation, WorldId};
use serde::Serialize;
use std::collections::BTreeSet;

pub const BASE_WORLD_COUNT: usize = 256;
pub const PROJECTION_COUNT: usize = 8;
pub const MAX_EVALUATIONS: usize = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct WorldBudget {
    pub worlds: usize,
    pub projections: usize,
    pub maximum_evaluations: usize,
}

impl WorldBudget {
    pub fn exact() -> Self {
        Self {
            worlds: BASE_WORLD_COUNT,
            projections: PROJECTION_COUNT,
            maximum_evaluations: MAX_EVALUATIONS,
        }
    }

    pub fn checked(worlds: usize, projections: usize) -> Result<Self, WorldError> {
        let evaluations = worlds
            .checked_mul(projections)
            .ok_or(WorldError::EvaluationOverflow)?;
        if worlds != BASE_WORLD_COUNT || projections != PROJECTION_COUNT {
            return Err(WorldError::NonCanonicalBudget {
                worlds,
                projections,
            });
        }
        if evaluations > MAX_EVALUATIONS {
            return Err(WorldError::EvaluationBudgetExceeded(evaluations));
        }
        Ok(Self {
            worlds,
            projections,
            maximum_evaluations: evaluations,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicFragment {
    Classical,
    Intuitionistic,
    Paraconsistent,
    Modal,
    Temporal,
    Deontic,
    Epistemic,
    CausalCounterfactual,
}

impl LogicFragment {
    pub const ALL: [Self; 8] = [
        Self::Classical,
        Self::Intuitionistic,
        Self::Paraconsistent,
        Self::Modal,
        Self::Temporal,
        Self::Deontic,
        Self::Epistemic,
        Self::CausalCounterfactual,
    ];

    pub const fn code(self) -> u16 {
        match self {
            Self::Classical => 0,
            Self::Intuitionistic => 1,
            Self::Paraconsistent => 2,
            Self::Modal => 3,
            Self::Temporal => 4,
            Self::Deontic => 5,
            Self::Epistemic => 6,
            Self::CausalCounterfactual => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalFrame {
    Absolute,
    Relative,
}

impl TemporalFrame {
    pub const fn code(self) -> u16 {
        match self {
            Self::Absolute => 0,
            Self::Relative => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalFrame {
    AbsoluteTemporal,
    RelativeTemporal,
    AbsoluteSemantic,
    RelativeSemantic,
}

impl CausalFrame {
    pub const fn code(self) -> u16 {
        match self {
            Self::AbsoluteTemporal => 0,
            Self::RelativeTemporal => 1,
            Self::AbsoluteSemantic => 2,
            Self::RelativeSemantic => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionProgram {
    Identity,
    Counterexample,
    Confounder,
    CausalReverse,
    TemporalShift,
    AlternativeOntology,
    EvidenceWeakening,
    AxiomMutation,
}

impl ProjectionProgram {
    pub const ALL: [Self; PROJECTION_COUNT] = [
        Self::Identity,
        Self::Counterexample,
        Self::Confounder,
        Self::CausalReverse,
        Self::TemporalShift,
        Self::AlternativeOntology,
        Self::EvidenceWeakening,
        Self::AxiomMutation,
    ];

    pub const fn code(self) -> u16 {
        match self {
            Self::Identity => 0,
            Self::Counterexample => 1,
            Self::Confounder => 2,
            Self::CausalReverse => 3,
            Self::TemporalShift => 4,
            Self::AlternativeOntology => 5,
            Self::EvidenceWeakening => 6,
            Self::AxiomMutation => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectionProgramIr {
    pub program: ProjectionProgram,
    pub premise_delta: i16,
    pub causal_delta: i16,
    pub temporal_delta: i16,
    pub ontology_delta: i16,
    pub evidence_delta: i16,
    pub axiom_delta: i16,
}

impl ProjectionProgramIr {
    pub const fn for_program(program: ProjectionProgram) -> Self {
        match program {
            ProjectionProgram::Identity => Self::new(program, 0, 0, 0, 0, 0, 0),
            ProjectionProgram::Counterexample => Self::new(program, -1, 0, 0, 0, 0, 0),
            ProjectionProgram::Confounder => Self::new(program, 1, 1, 0, 0, -1, 0),
            ProjectionProgram::CausalReverse => Self::new(program, 0, -2, 0, 0, 0, 0),
            ProjectionProgram::TemporalShift => Self::new(program, 0, 0, 2, 0, 0, 0),
            ProjectionProgram::AlternativeOntology => Self::new(program, 0, 0, 0, 2, 0, 0),
            ProjectionProgram::EvidenceWeakening => Self::new(program, 0, 0, 0, 0, -2, 0),
            ProjectionProgram::AxiomMutation => Self::new(program, 1, 0, 0, 1, 0, 2),
        }
    }

    const fn new(
        program: ProjectionProgram,
        premise_delta: i16,
        causal_delta: i16,
        temporal_delta: i16,
        ontology_delta: i16,
        evidence_delta: i16,
        axiom_delta: i16,
    ) -> Self {
        Self {
            program,
            premise_delta,
            causal_delta,
            temporal_delta,
            ontology_delta,
            evidence_delta,
            axiom_delta,
        }
    }

    pub fn digest(&self) -> String {
        stable_sha256(&format!(
            "{:?}:{}:{}:{}:{}:{}:{}",
            self.program,
            self.premise_delta,
            self.causal_delta,
            self.temporal_delta,
            self.ontology_delta,
            self.evidence_delta,
            self.axiom_delta
        ))
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WorldKey {
    pub premise_digest: String,
    pub logic: LogicFragment,
    pub temporal: TemporalFrame,
    pub causal: CausalFrame,
    pub ontology_variant: u8,
}

impl WorldKey {
    pub fn canonical_digest(&self) -> String {
        stable_sha256(&format!(
            "{}:{:?}:{:?}:{:?}:{}",
            self.premise_digest, self.logic, self.temporal, self.causal, self.ontology_variant
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PositedWorld {
    pub id: WorldId,
    pub key: WorldKey,
    pub generation: Generation,
    pub temporary_posit: bool,
}

pub fn generate_distinct_worlds(premise: &str) -> Result<Vec<PositedWorld>, WorldError> {
    let premise_digest = stable_sha256(premise);
    let mut worlds = Vec::with_capacity(BASE_WORLD_COUNT);
    let mut digests = BTreeSet::new();
    for index in 0..BASE_WORLD_COUNT {
        let logic = LogicFragment::ALL[index % 8];
        let temporal = if (index / 8) % 2 == 0 {
            TemporalFrame::Absolute
        } else {
            TemporalFrame::Relative
        };
        let causal = match (index / 16) % 4 {
            0 => CausalFrame::AbsoluteTemporal,
            1 => CausalFrame::RelativeTemporal,
            2 => CausalFrame::AbsoluteSemantic,
            _ => CausalFrame::RelativeSemantic,
        };
        let ontology_variant = ((index / 64) % 4) as u8;
        let key = WorldKey {
            premise_digest: premise_digest.clone(),
            logic,
            temporal,
            causal,
            ontology_variant,
        };
        let digest = key.canonical_digest();
        if !digests.insert(digest) {
            return Err(WorldError::DuplicateWorld(index));
        }
        worlds.push(PositedWorld {
            id: WorldId(index as u64),
            key,
            generation: Generation(0),
            temporary_posit: true,
        });
    }
    if worlds.len() != BASE_WORLD_COUNT {
        return Err(WorldError::InsufficientDistinctWorlds(worlds.len()));
    }
    Ok(worlds)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvaluationSlot {
    pub slot: usize,
    pub world_id: WorldId,
    pub projection: ProjectionProgram,
    pub generation: Generation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorldEvaluation {
    pub slot: usize,
    pub world_id: WorldId,
    pub projection: ProjectionProgram,
    pub generation: Generation,
    pub world_digest: String,
    pub effect_digest: String,
    pub evaluation_digest: String,
    pub axes_x100: [u16; 3],
}

pub fn materialize_evaluations(
    worlds: &[PositedWorld],
) -> Result<Vec<WorldEvaluation>, WorldError> {
    if worlds.len() != BASE_WORLD_COUNT {
        return Err(WorldError::InsufficientDistinctWorlds(worlds.len()));
    }
    let unique = worlds
        .iter()
        .map(|world| world.key.canonical_digest())
        .collect::<BTreeSet<_>>();
    if unique.len() != BASE_WORLD_COUNT {
        return Err(WorldError::InsufficientDistinctWorlds(unique.len()));
    }
    let mut evaluations = Vec::with_capacity(MAX_EVALUATIONS);
    let mut digests = BTreeSet::new();
    for world in worlds {
        let world_digest = world.key.canonical_digest();
        for projection in ProjectionProgram::ALL {
            let slot = evaluations.len();
            if slot >= MAX_EVALUATIONS {
                return Err(WorldError::EvaluationBudgetExceeded(slot + 1));
            }
            let program = ProjectionProgramIr::for_program(projection);
            let effect_digest = program.digest();
            let evaluation_digest = stable_sha256(&format!(
                "{}:{}:{}:{}:{}",
                slot, world.id.0, world_digest, effect_digest, world.generation.0
            ));
            if !digests.insert(evaluation_digest.clone()) {
                return Err(WorldError::DuplicateEvaluation(slot));
            }
            let slot_axis = u16::try_from(slot).map_err(|_| WorldError::EvaluationOverflow)?;
            let structural_axis = world.key.logic.code() * 256
                + world.key.causal.code() * 32
                + world.key.temporal.code() * 16
                + projection.code();
            let effect_axis = u16::from(world.key.ontology_variant) * 512
                + projection.code() * 32
                + u16::try_from(world.generation.0.min(31)).unwrap_or(31);
            evaluations.push(WorldEvaluation {
                slot,
                world_id: world.id,
                projection,
                generation: world.generation,
                world_digest: world_digest.clone(),
                effect_digest,
                evaluation_digest,
                axes_x100: [slot_axis, structural_axis, effect_axis],
            });
        }
    }
    if evaluations.len() != MAX_EVALUATIONS {
        return Err(WorldError::InsufficientEvaluations(evaluations.len()));
    }
    Ok(evaluations)
}

pub fn evaluation_matrix(worlds: &[PositedWorld]) -> Result<Vec<EvaluationSlot>, WorldError> {
    if worlds.len() != BASE_WORLD_COUNT {
        return Err(WorldError::InsufficientDistinctWorlds(worlds.len()));
    }
    let unique = worlds
        .iter()
        .map(|world| world.key.canonical_digest())
        .collect::<BTreeSet<_>>();
    if unique.len() != BASE_WORLD_COUNT {
        return Err(WorldError::InsufficientDistinctWorlds(unique.len()));
    }
    let mut slots = Vec::with_capacity(MAX_EVALUATIONS);
    for world in worlds {
        for projection in ProjectionProgram::ALL {
            if slots.len() == MAX_EVALUATIONS {
                return Err(WorldError::EvaluationBudgetExceeded(slots.len() + 1));
            }
            slots.push(EvaluationSlot {
                slot: slots.len(),
                world_id: world.id,
                projection,
                generation: world.generation,
            });
        }
    }
    Ok(slots)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorldArena {
    worlds: Vec<PositedWorld>,
    replacement_count: u64,
}

impl WorldArena {
    pub fn new(premise: &str) -> Result<Self, WorldError> {
        Ok(Self {
            worlds: generate_distinct_worlds(premise)?,
            replacement_count: 0,
        })
    }

    pub fn worlds(&self) -> &[PositedWorld] {
        &self.worlds
    }

    pub fn replace(&mut self, slot: usize, key: WorldKey) -> Result<Generation, WorldError> {
        if slot >= self.worlds.len() {
            return Err(WorldError::WorldSlotOutOfBounds(slot));
        }
        if self
            .worlds
            .iter()
            .enumerate()
            .any(|(index, world)| index != slot && world.key == key)
        {
            return Err(WorldError::DuplicateWorld(slot));
        }
        let generation = Generation(
            self.worlds[slot]
                .generation
                .0
                .checked_add(1)
                .ok_or(WorldError::GenerationExhausted)?,
        );
        self.worlds[slot] = PositedWorld {
            id: WorldId(slot as u64),
            key,
            generation,
            temporary_posit: true,
        };
        self.replacement_count = self.replacement_count.saturating_add(1);
        Ok(generation)
    }

    pub fn replacement_count(&self) -> u64 {
        self.replacement_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum WorldError {
    DuplicateEvaluation(usize),
    DuplicateWorld(usize),
    EvaluationBudgetExceeded(usize),
    EvaluationOverflow,
    GenerationExhausted,
    InsufficientDistinctWorlds(usize),
    InsufficientEvaluations(usize),
    NonCanonicalBudget { worlds: usize, projections: usize },
    WorldSlotOutOfBounds(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_256_distinct_worlds_and_2048_evaluations() {
        let worlds = generate_distinct_worlds("2 + 2 = 4 under an unspecified algebra").unwrap();
        assert_eq!(worlds.len(), 256);
        assert_eq!(
            worlds
                .iter()
                .map(|world| world.key.canonical_digest())
                .collect::<BTreeSet<_>>()
                .len(),
            256
        );
        assert_eq!(evaluation_matrix(&worlds).unwrap().len(), 2_048);
    }

    #[test]
    fn budget_rejects_noncanonical_or_overwide_requests() {
        assert!(matches!(
            WorldBudget::checked(257, 8),
            Err(WorldError::NonCanonicalBudget { .. })
        ));
        assert!(matches!(
            WorldBudget::checked(256, 9),
            Err(WorldError::NonCanonicalBudget { .. })
        ));
    }

    #[test]
    fn reprojection_replaces_in_place_without_growing() {
        let mut arena = WorldArena::new("premise").unwrap();
        let before = arena.worlds().len();
        let mut key = arena.worlds()[0].key.clone();
        key.premise_digest = stable_sha256("reprojected premise");
        let generation = arena.replace(0, key).unwrap();
        assert_eq!(generation, Generation(1));
        assert_eq!(arena.worlds().len(), before);
        assert_eq!(arena.replacement_count(), 1);
    }
}
