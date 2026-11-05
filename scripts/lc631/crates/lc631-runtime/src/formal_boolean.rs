//! Bounded evaluation of declared finite Boolean theories and protected probes.
//! Mapping is Given; source interpretation and checker independence stay unverified.
#![forbid(unsafe_code)]
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
use lc631_substrate::Digest32;
mod admission;
pub use admission::probe_map_digest;
use admission::{admit, hash_probe, hash_theory};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    pub source: SourceRevision,
    pub target: SourceRevision,
    pub source_context: ContextRevision,
    pub target_context: ContextRevision,
    pub probe_set: Digest32,
    pub probe_map: Digest32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnconstrainedPolicy {
    ExplicitlyGiven,
    MissingScope,
}

/// One nonempty protected probe. Symbol pairs are explicit semantic assumptions;
/// numeric IDs only select IR variables and never establish their correspondence.
#[derive(Debug)]
pub struct Request<'a> {
    pub source: &'a SmtStatusQuery,
    pub target: &'a SmtStatusQuery,
    pub source_predicate: &'a BoolExpr,
    pub target_predicate: &'a BoolExpr,
    pub symbols: &'a [(BoolSymbolId, BoolSymbolId)],
    pub unconstrained: UnconstrainedPolicy,
    pub binding: Binding,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidBudget,
    AtomLimit,
    IncompleteMap,
    DuplicateMap,
    SourceBindingMismatch,
    ProbeMapMismatch,
    UnsupportedInteger,
    DepthLimit,
    UndeclaredSymbol,
    EmptyConnective,
    ResourceExhausted,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoComparable {
    MissingTheoryScope,
    SourceContextInconsistent,
    TargetContextInconsistent,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectionResult {
    Preserved,
    Countermodel(u16),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalValidation {
    GivenMappingSourceInterpretationUnverified,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappingBasis {
    Given,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceInterpretation {
    Unverified,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckerIndependence {
    UnassessedSharedIr,
}
#[derive(Debug)]
pub enum Evaluation<'a> {
    Computed(ComputedReport<'a>),
    NoComparable {
        reason: NoComparable,
        ast_visits: u64,
    },
    Unknown {
        ast_visits: u64,
    },
}
/// Constructed exclusively after exhaustive evaluation; never deserialized.
#[derive(Debug)]
pub struct ComputedReport<'a> {
    request: &'a Request<'a>,
    source_digest: Digest32,
    target_digest: Digest32,
    soundness: DirectionResult,
    completeness: DirectionResult,
    assignments: u32,
    ast_visits: u64,
}
impl<'a> ComputedReport<'a> {
    pub fn request(&self) -> &'a Request<'a> {
        self.request
    }
    pub fn binding(&self) -> Binding {
        self.request.binding
    }
    pub fn source_theory_digest(&self) -> Digest32 {
        self.source_digest
    }
    pub fn target_theory_digest(&self) -> Digest32 {
        self.target_digest
    }
    pub fn soundness(&self) -> DirectionResult {
        self.soundness
    }
    pub fn completeness(&self) -> DirectionResult {
        self.completeness
    }
    pub fn assignments(&self) -> u32 {
        self.assignments
    }
    pub fn ast_visits(&self) -> u64 {
        self.ast_visits
    }
    pub fn validation(&self) -> FormalValidation {
        FormalValidation::GivenMappingSourceInterpretationUnverified
    }
    pub fn mapping_basis(&self) -> MappingBasis {
        MappingBasis::Given
    }
    pub fn source_interpretation(&self) -> SourceInterpretation {
        SourceInterpretation::Unverified
    }
    pub fn checker_independence(&self) -> CheckerIndependence {
        CheckerIndependence::UnassessedSharedIr
    }
}

struct Budget {
    limit: u64,
    used: u64,
}
impl Budget {
    fn new(limit: u64) -> Result<Self, Failure> {
        if limit == 0 || limit > 1_000_000 {
            return Err(Failure::InvalidBudget);
        }
        Ok(Self { limit, used: 0 })
    }
    fn visit(&mut self) -> Result<(), Failure> {
        if self.used >= self.limit {
            return Err(Failure::ResourceExhausted);
        }
        self.used += 1;
        Ok(())
    }
    fn admits(&self, visits: u64) -> Result<(), Failure> {
        if visits > self.limit - self.used {
            return Err(Failure::ResourceExhausted);
        }
        Ok(())
    }
}

pub fn evaluate<'a>(request: &'a Request<'a>, limit: u64) -> Result<Evaluation<'a>, Failure> {
    let mut budget = Budget::new(limit)?;
    match compute(request, &mut budget) {
        Err(Failure::ResourceExhausted) => Ok(Evaluation::Unknown {
            ast_visits: budget.used,
        }),
        result => result,
    }
}
fn compute<'a>(r: &'a Request<'a>, budget: &mut Budget) -> Result<Evaluation<'a>, Failure> {
    admit(r)?;
    if r.unconstrained == UnconstrainedPolicy::MissingScope
        && (r.source.assertions().is_empty() || r.target.assertions().is_empty())
    {
        return Ok(Evaluation::NoComparable {
            reason: NoComparable::MissingTheoryScope,
            ast_visits: budget.used,
        });
    }
    let source_digest = hash_theory(r.source, budget)?;
    let target_digest = hash_theory(r.target, budget)?;
    if hash_probe(r, budget)? != r.binding.probe_map {
        return Err(Failure::ProbeMapMismatch);
    }
    let assignments = 1u32 << r.symbols.len();
    // Every admitted AST node is visited once per assignment, without shortcuts.
    // Reserve all evaluation work before the first assignment is traversed.
    budget.admits(budget.used * u64::from(assignments))?;
    enumerate(r, source_digest, target_digest, assignments, budget)
}
fn enumerate<'a>(
    r: &'a Request<'a>,
    source_digest: Digest32,
    target_digest: Digest32,
    assignments: u32,
    budget: &mut Budget,
) -> Result<Evaluation<'a>, Failure> {
    let mut source_consistent = false;
    let mut target_consistent = false;
    let mut soundness = DirectionResult::Preserved;
    let mut completeness = DirectionResult::Preserved;
    for assignment in 0..assignments {
        let row = assignment as u16; // <=16 atoms gives 0..=65535.
        let source_context = theory_value(r.source, r.symbols, false, row, budget)?;
        let target_context = theory_value(r.target, r.symbols, true, row, budget)?;
        source_consistent |= source_context;
        target_consistent |= target_context;
        let source = value(r.source_predicate, r.symbols, false, row, budget)? & source_context;
        let target = value(r.target_predicate, r.symbols, true, row, budget)? & target_context;
        if target && !source && soundness == DirectionResult::Preserved {
            soundness = DirectionResult::Countermodel(row);
        }
        if source && !target && completeness == DirectionResult::Preserved {
            completeness = DirectionResult::Countermodel(row);
        }
    }
    let reason = if !source_consistent {
        Some(NoComparable::SourceContextInconsistent)
    } else if !target_consistent {
        Some(NoComparable::TargetContextInconsistent)
    } else {
        None
    };
    if let Some(reason) = reason {
        return Ok(Evaluation::NoComparable {
            reason,
            ast_visits: budget.used,
        });
    }
    Ok(Evaluation::Computed(ComputedReport {
        request: r,
        source_digest,
        target_digest,
        soundness,
        completeness,
        assignments,
        ast_visits: budget.used,
    }))
}
fn theory_value(
    query: &SmtStatusQuery,
    map: &[(BoolSymbolId, BoolSymbolId)],
    target: bool,
    assignment: u16,
    budget: &mut Budget,
) -> Result<bool, Failure> {
    let mut holds = true;
    for assertion in query.assertions() {
        holds &= value(assertion, map, target, assignment, budget)?;
    }
    Ok(holds)
}
fn value(
    expr: &BoolExpr,
    map: &[(BoolSymbolId, BoolSymbolId)],
    target: bool,
    assignment: u16,
    budget: &mut Budget,
) -> Result<bool, Failure> {
    budget.visit()?;
    match expr {
        BoolExpr::Constant(value) => Ok(*value),
        BoolExpr::Variable(symbol) => {
            let bit = map
                .iter()
                .position(|&(a, b)| if target { b == *symbol } else { a == *symbol })
                .ok_or(Failure::UndeclaredSymbol)?;
            Ok(assignment & (1u16 << bit) != 0)
        }
        BoolExpr::Not(child) => Ok(!value(child, map, target, assignment, budget)?),
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            let conjunction = matches!(expr, BoolExpr::And(_));
            let mut result = conjunction;
            for child in children {
                let next = value(child, map, target, assignment, budget)?;
                if conjunction {
                    result &= next;
                } else {
                    result |= next;
                }
            }
            Ok(result)
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            Err(Failure::UnsupportedInteger)
        }
    }
}
