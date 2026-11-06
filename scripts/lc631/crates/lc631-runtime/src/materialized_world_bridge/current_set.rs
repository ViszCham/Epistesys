//! 日本語: 実ownerを毎回再検査する有限world集合。意味distinctnessや権限ではない。
//! English: Recheck actual owners on every finite-world set consumption.
#![forbid(unsafe_code)]
use super::{CurrentObservedTemporalWorld, ObservedWorldCoverage, SourceMeaning, WorldBridgeError};
use lc631_core::context::ContextScope;
use lc631_solvers::temporal::Truth;
use lc631_substrate::resource::ResourceReservationView;

#[derive(Debug)]
pub enum CurrentSetError {
    World(WorldBridgeError),
    InvalidPlan,
    Empty,
    Limit,
    ScopeMismatch,
    ParentMismatch,
    ConflictingInput,
    InsufficientCoverage,
}

/// Explicit Given truth query and minimum actual-input coverage. No implicit
/// root goal, scalar utility, caller-reported result or eligibility is accepted.
#[derive(Clone, Copy)]
pub struct TruthWitnessPlan {
    scope: ContextScope,
    desired: Truth,
    minimum_inputs: usize,
}
impl TruthWitnessPlan {
    pub fn checked(
        scope: ContextScope,
        desired: Truth,
        minimum_inputs: usize,
    ) -> Result<Self, CurrentSetError> {
        if desired == Truth::Unknown || minimum_inputs == 0 || minimum_inputs > 2048 {
            return Err(CurrentSetError::InvalidPlan);
        }
        Ok(Self {
            scope,
            desired,
            minimum_inputs,
        })
    }
}

/// A finite trace witness, never a final recommendation or universal entailment.
/// Unknown blocks a unique/absent conclusion rather than becoming false.
#[derive(Debug, Eq, PartialEq)]
pub enum TruthWitness {
    Unique { row: usize },
    Multiple { matches: usize },
    Absent,
    NeedsEvidence { unknown_inputs: usize },
}

/// Borrow the actual immutable world/store/prepared owners. No detached report,
/// Clone, Deserialize, mutable view or scalar/status constructor is provided.
pub struct CurrentWorldSet<'s, 'a, 'i, 'v, 'w> {
    rows: &'s [&'s CurrentObservedTemporalWorld<'a, 'i, 'v, 'w>],
    scope: ContextScope,
}
impl<'s, 'a, 'i, 'v, 'w> CurrentWorldSet<'s, 'a, 'i, 'v, 'w> {
    pub fn capture(
        rows: &'s [&'s CurrentObservedTemporalWorld<'a, 'i, 'v, 'w>],
        scope: ContextScope,
        resources: &ResourceReservationView<'_>,
    ) -> Result<Self, CurrentSetError> {
        if rows.is_empty() {
            return Err(CurrentSetError::Empty);
        }
        if rows.len() > 2048 {
            return Err(CurrentSetError::Limit);
        }
        let set = Self { rows, scope };
        set.recheck(resources)?;
        Ok(set)
    }

    fn recheck(&self, resources: &ResourceReservationView<'_>) -> Result<(), CurrentSetError> {
        for row in self.rows {
            if row.historical.scope != self.scope {
                return Err(CurrentSetError::ScopeMismatch);
            }
            if !std::ptr::eq(row.worlds, self.rows[0].worlds)
                || row.historical.context.revision() != self.rows[0].historical.context.revision()
            {
                return Err(CurrentSetError::ParentMismatch);
            }
            row.report_current(resources)
                .map_err(CurrentSetError::World)?;
        }
        resources
            .check()
            .map_err(|error| CurrentSetError::World(WorldBridgeError::Resource(error)))
    }

    fn duplicate(&self, index: usize) -> Result<bool, CurrentSetError> {
        let row = self.rows[index];
        for prior in &self.rows[..index] {
            if prior.historical.input == row.historical.input {
                if prior.historical.report.truth() != row.historical.report.truth() {
                    return Err(CurrentSetError::ConflictingInput);
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Point-in-time checked counts; the returned values are historical data,
    /// not a current permit or semantic-distinctness certificate.
    pub fn coverage_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<ObservedWorldCoverage, CurrentSetError> {
        self.recheck(resources)?;
        let mut executed_live = 0;
        let mut unknown_outcomes = 0;
        for (index, row) in self.rows.iter().enumerate() {
            if !self.duplicate(index)? {
                executed_live += 1;
                if row.historical.report.truth() == Truth::Unknown {
                    unknown_outcomes += 1;
                }
            }
        }
        self.recheck(resources)?;
        Ok(ObservedWorldCoverage {
            executed_live,
            unknown_outcomes,
            source_meaning: SourceMeaning::NeedsEvidence,
        })
    }

    /// Concrete consumer of actual query results. Every use rechecks the set;
    /// aliases cannot satisfy a minimum-coverage requirement or cast extra votes.
    pub fn consume_truth_current(
        &self,
        plan: TruthWitnessPlan,
        resources: &ResourceReservationView<'_>,
    ) -> Result<TruthWitness, CurrentSetError> {
        if plan.scope != self.scope {
            return Err(CurrentSetError::ScopeMismatch);
        }
        let coverage = self.coverage_current(resources)?;
        if coverage.executed_live < plan.minimum_inputs {
            return Err(CurrentSetError::InsufficientCoverage);
        }
        let mut matches = 0;
        let mut selected = 0;
        for (index, row) in self.rows.iter().enumerate() {
            if !self.duplicate(index)? && row.historical.report.truth() == plan.desired {
                matches += 1;
                selected = index;
            }
        }
        self.recheck(resources)?;
        if coverage.unknown_outcomes > 0 {
            return Ok(TruthWitness::NeedsEvidence {
                unknown_inputs: coverage.unknown_outcomes,
            });
        }
        Ok(match matches {
            0 => TruthWitness::Absent,
            1 => TruthWitness::Unique { row: selected },
            _ => TruthWitness::Multiple { matches },
        })
    }
}
