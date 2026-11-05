use super::{AspBinding, AspError, AspProgram, AtomId, ConsequenceMode};
use crate::protocol::clingo::ClingoTerminalStatus;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AspCompleteness {
    Exhaustive,
    ModelLimit,
    StepLimit,
    SolverUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Consequence {
    Holds,
    DoesNotHold,
    NoStableModels,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StableModel {
    pub(super) mask: u16,
    cost: u64,
    pub(super) reported_answer: Option<u32>,
}

impl StableModel {
    pub const fn cost(self) -> u64 {
        self.cost
    }
    pub const fn reported_answer(self) -> Option<u32> {
        self.reported_answer
    }
}

#[derive(Debug)]
pub struct AspEvaluation {
    pub(super) binding: AspBinding,
    pub(super) atoms: Vec<AtomId>,
    pub(super) models: Vec<StableModel>,
    pub(super) completeness: AspCompleteness,
    pub(super) steps: u64,
    pub(super) reported_status: Option<ClingoTerminalStatus>,
}

impl AspEvaluation {
    pub const fn binding(&self) -> AspBinding {
        self.binding
    }
    pub fn models(&self) -> &[StableModel] {
        &self.models
    }
    pub const fn completeness(&self) -> AspCompleteness {
        self.completeness
    }
    pub const fn steps(&self) -> u64 {
        self.steps
    }
    /// Solver status and local membership/completeness are separate axes.
    pub const fn reported_status(&self) -> Option<ClingoTerminalStatus> {
        self.reported_status
    }
    pub const fn clingo_native_optimization(&self) -> bool {
        false
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "finite typed-program evaluation only; no general grounding/lowering accuracy, proof, independent corroboration, authority, output commit or host delivery"
    }

    pub fn model_atoms(&self, index: usize) -> Option<Vec<AtomId>> {
        let model = self.models.get(index)?;
        Some(
            self.atoms
                .iter()
                .enumerate()
                .filter_map(|(i, atom)| (model.mask & (1u16 << i) != 0).then_some(*atom))
                .collect(),
        )
    }

    pub fn consequence(
        &self,
        atom: AtomId,
        mode: ConsequenceMode,
    ) -> Result<Consequence, AspError> {
        let index = self
            .atoms
            .binary_search(&atom)
            .map_err(|_| AspError::UndeclaredAtom)?;
        if self.completeness != AspCompleteness::Exhaustive {
            return Ok(Consequence::Unknown);
        }
        if self.models.is_empty() {
            return Ok(Consequence::NoStableModels);
        }
        let bit = 1u16 << index;
        let holds = match mode {
            ConsequenceMode::Brave => self.models.iter().any(|m| m.mask & bit != 0),
            ConsequenceMode::Cautious => self.models.iter().all(|m| m.mask & bit != 0),
        };
        Ok(if holds {
            Consequence::Holds
        } else {
            Consequence::DoesNotHold
        })
    }

    /// A minimum over the declared finite program, never a native solver claim.
    pub fn minimum_cost(&self) -> Option<u64> {
        if self.completeness != AspCompleteness::Exhaustive {
            return None;
        }
        self.models.iter().map(|m| m.cost).min()
    }

    pub fn optimal_model_indices(&self) -> Vec<usize> {
        let Some(minimum) = self.minimum_cost() else {
            return Vec::new();
        };
        self.models
            .iter()
            .enumerate()
            .filter_map(|(i, m)| (m.cost == minimum).then_some(i))
            .collect()
    }
}

#[derive(Clone, Copy)]
pub(super) struct CompiledRule {
    head: u16,
    positive: u16,
    negative: u16,
}

impl AspProgram {
    /// GL reduct plus its least positive fixed point, bounded independently of wall time.
    pub fn enumerate(&self) -> Result<AspEvaluation, AspError> {
        let mut steps = 0;
        self.enumerate_with_steps(&mut steps)
    }

    pub(super) fn compiled_rules(&self) -> Result<Vec<CompiledRule>, AspError> {
        let mut compiled = Vec::new();
        compiled
            .try_reserve_exact(self.rules.len())
            .map_err(|_| AspError::AllocationFailed)?;
        for rule in &self.rules {
            compiled.push(CompiledRule {
                head: match rule.head {
                    Some(atom) => self.mask(&[atom])?,
                    None => 0,
                },
                positive: self.mask(&rule.positive)?,
                negative: self.mask(&rule.negative)?,
            });
        }
        Ok(compiled)
    }

    pub(super) fn enumerate_with_steps(&self, steps: &mut u64) -> Result<AspEvaluation, AspError> {
        let compiled = self.compiled_rules()?;
        let mut models = Vec::new();
        models
            .try_reserve_exact(self.limits.models)
            .map_err(|_| AspError::AllocationFailed)?;
        let mut completeness = AspCompleteness::Exhaustive;
        let assignments = 1u32 << self.atoms.len();
        for assignment in 0..assignments {
            if !charge(steps, self.limits.steps) {
                completeness = AspCompleteness::StepLimit;
                break;
            }
            let candidate = u16::try_from(assignment).map_err(|_| AspError::ArithmeticOverflow)?;
            match is_stable(&compiled, candidate, steps, self.limits.steps) {
                None => {
                    completeness = AspCompleteness::StepLimit;
                    break;
                }
                Some(false) => continue,
                Some(true) => {}
            }
            if models.len() == self.limits.models {
                completeness = AspCompleteness::ModelLimit;
                break;
            }
            models.push(self.model(candidate, None)?);
        }
        self.evaluation(models, completeness, *steps, None)
    }

    pub(super) fn evaluation(
        &self,
        models: Vec<StableModel>,
        completeness: AspCompleteness,
        steps: u64,
        reported_status: Option<ClingoTerminalStatus>,
    ) -> Result<AspEvaluation, AspError> {
        let mut atoms = Vec::new();
        atoms
            .try_reserve_exact(self.atoms.len())
            .map_err(|_| AspError::AllocationFailed)?;
        atoms.extend_from_slice(&self.atoms);
        Ok(AspEvaluation {
            binding: self.binding,
            atoms,
            models,
            completeness,
            steps,
            reported_status,
        })
    }

    pub(super) fn model(
        &self,
        mask: u16,
        reported_answer: Option<u32>,
    ) -> Result<StableModel, AspError> {
        Ok(StableModel {
            mask,
            cost: self.model_cost(mask)?,
            reported_answer,
        })
    }

    fn model_cost(&self, candidate: u16) -> Result<u64, AspError> {
        let mut total = 0u64;
        for cost in &self.costs {
            if candidate & self.mask(&[cost.atom])? != 0 {
                total = total
                    .checked_add(cost.weight)
                    .ok_or(AspError::ArithmeticOverflow)?;
            }
        }
        Ok(total)
    }
}

pub(super) fn charge(steps: &mut u64, maximum: u64) -> bool {
    if *steps >= maximum {
        return false;
    }
    *steps += 1;
    true
}

pub(super) fn is_stable(
    rules: &[CompiledRule],
    candidate: u16,
    steps: &mut u64,
    maximum: u64,
) -> Option<bool> {
    let mut closure = 0u16;
    loop {
        let previous = closure;
        for rule in rules {
            if !charge(steps, maximum) {
                return None;
            }
            if rule.negative & candidate == 0 && rule.positive & closure == rule.positive {
                if rule.head == 0 {
                    return Some(false);
                }
                closure |= rule.head;
            }
        }
        if closure == previous {
            return Some(closure == candidate);
        }
    }
}
