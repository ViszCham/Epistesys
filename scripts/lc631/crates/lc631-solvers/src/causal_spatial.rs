//! 準備用の有限 Boolean SCM と閉矩形関係。外界の因果・権限は認定しない。
//! Preparatory finite Boolean SCM and closed-rectangle relations, not causal truth or authority.
#![forbid(unsafe_code)]
use lc631_substrate::context_identity::ContextRevision;
use sha2::{Digest, Sha256};

pub const MAX_VARIABLES: usize = 8;
pub const MAX_WORLDS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Variable(pub u8);

/// Structural dependence only: neither temporal order nor semantic entailment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Equation {
    Missing,
    Exogenous(u8),
    /// Parent bits are packed in ascending variable order to index the table.
    Table {
        parents: u8,
        values: [u64; 4],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidDomain,
    InvalidVariable,
    InvalidTable,
    Cycle,
    DuplicateWorld,
    InvalidAssignment,
    HardConstraintViolation,
    FrameMismatch,
    InvalidRectangle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownReason {
    MissingEquation(Variable),
    MissingWorlds,
    NoCompatibleWorld,
    WorkBudgetExhausted,
    MissingSpatialObject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Evaluation<T> {
    Computed(T),
    Unknown(UnknownReason),
}

/// Context identity plus computed typed payload pins; never authenticated receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    context: ContextRevision,
    query: [u8; 32],
    model: [u8; 32],
}

impl Binding {
    pub const fn context(self) -> ContextRevision {
        self.context
    }
    pub const fn source(self) -> [u8; 32] {
        *self.context.source().digest().as_bytes()
    }
    pub const fn query(self) -> [u8; 32] {
        self.query
    }
    pub const fn model(self) -> [u8; 32] {
        self.model
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    BooleanAcyclicScmV1,
    ClosedIntegerRectangles2dV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Termination {
    Completed,
    NeedsEvidence,
    ResourceExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Report<T> {
    binding: Binding,
    profile: Profile,
    termination: Termination,
    outcome: Evaluation<T>,
}

impl<T: Copy> Report<T> {
    pub const fn binding(&self) -> Binding {
        self.binding
    }
    pub const fn profile(&self) -> Profile {
        self.profile
    }
    pub const fn termination(&self) -> Termination {
        self.termination
    }
    pub const fn outcome(&self) -> Evaluation<T> {
        self.outcome
    }
}

fn report<T>(binding: Binding, profile: Profile, outcome: Evaluation<T>) -> Report<T> {
    let termination = match &outcome {
        Evaluation::Computed(_) => Termination::Completed,
        Evaluation::Unknown(UnknownReason::WorkBudgetExhausted) => Termination::ResourceExhausted,
        Evaluation::Unknown(_) => Termination::NeedsEvidence,
    };
    Report {
        binding,
        profile,
        termination,
        outcome,
    }
}

/// Partial Boolean assignment; constructors reject values outside the mask.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Assignment {
    mask: u8,
    values: u8,
}

impl Assignment {
    pub fn new(mask: u8, values: u8) -> Result<Self, Error> {
        if values & !mask != 0 {
            return Err(Error::InvalidAssignment);
        }
        Ok(Self { mask, values })
    }

    pub const fn empty() -> Self {
        Self { mask: 0, values: 0 }
    }

    pub const fn mask(self) -> u8 {
        self.mask
    }
    pub const fn values(self) -> u8 {
        self.values
    }

    fn matches(self, state: u8) -> bool {
        state & self.mask == self.values
    }
}

/// Borrowed explicit exogenous support, without weights or inferred probability.
pub struct Scm<'a> {
    equations: &'a [Equation],
    worlds: &'a [u8],
    order: [u8; MAX_VARIABLES],
    domain: u8,
    hard: Assignment,
    model_digest: [u8; 32],
}

impl<'a> Scm<'a> {
    /// Read-only model content identity, not evidence authenticity or permission.
    pub const fn model_digest(&self) -> [u8; 32] {
        self.model_digest
    }

    pub fn evaluate(
        &self,
        context: ContextRevision,
        query: Query,
    ) -> Result<Report<CausalResult>, Error> {
        let mut hash = Sha256::new();
        hash.update(b"epistesys.tlcx16.scm-query.v1\0");
        hash.update([
            query.observed.mask,
            query.observed.values,
            query.intervention.mask,
            query.intervention.values,
            query.target.0,
            u8::from(query.value),
        ]);
        hash.update(query.work_limit.to_le_bytes());
        let binding = Binding {
            context,
            query: hash.finalize().into(),
            model: self.model_digest,
        };
        Ok(report(
            binding,
            Profile::BooleanAcyclicScmV1,
            self.query(query)?,
        ))
    }

    pub fn new(
        equations: &'a [Equation],
        exogenous_count: u8,
        worlds: &'a [u8],
        hard: Assignment,
    ) -> Result<Self, Error> {
        if equations.is_empty()
            || equations.len() > MAX_VARIABLES
            || exogenous_count > 8
            || worlds.len() > MAX_WORLDS
        {
            return Err(Error::InvalidDomain);
        }
        let domain = low_mask(equations.len());
        if hard.mask & !domain != 0 {
            return Err(Error::InvalidAssignment);
        }
        validate_worlds(worlds, low_mask(usize::from(exogenous_count)))?;
        for equation in equations {
            match *equation {
                Equation::Missing => {}
                Equation::Exogenous(index) if index < exogenous_count => {}
                Equation::Exogenous(_) => return Err(Error::InvalidVariable),
                Equation::Table { parents, values } => {
                    if parents & !domain != 0 {
                        return Err(Error::InvalidVariable);
                    }
                    let entries = 1usize << parents.count_ones();
                    for bit in entries..256 {
                        if values[bit / 64] & (1u64 << (bit % 64)) != 0 {
                            return Err(Error::InvalidTable);
                        }
                    }
                }
            }
        }
        let order = topological_order(equations, domain)?;
        let mut hash = Sha256::new();
        hash.update(b"epistesys.tlcx16.boolean-scm.v1\0");
        hash.update([
            equations.len() as u8,
            exogenous_count,
            hard.mask,
            hard.values,
        ]);
        for equation in equations {
            match equation {
                Equation::Missing => hash.update([0]),
                Equation::Exogenous(index) => hash.update([1, *index]),
                Equation::Table { parents, values } => {
                    hash.update([2, *parents]);
                    for value in values {
                        hash.update(value.to_le_bytes());
                    }
                }
            }
        }
        hash.update((worlds.len() as u16).to_le_bytes());
        hash.update(worlds);
        Ok(Self {
            equations,
            worlds,
            order,
            domain,
            hard,
            model_digest: hash.finalize().into(),
        })
    }

    /// Abduction on factual observations, then action and prediction in each
    /// SAME exogenous world. Empty intervention performs conditioning only.
    /// Every admitted factual/action state must preserve the fixed hard set.
    pub fn query(&self, query: Query) -> Result<Evaluation<CausalResult>, Error> {
        if usize::from(query.target.0) >= self.equations.len() {
            return Err(Error::InvalidVariable);
        }
        if (query.observed.mask | query.intervention.mask) & !self.domain != 0 {
            return Err(Error::InvalidAssignment);
        }
        if (query.intervention.values ^ self.hard.values) & query.intervention.mask & self.hard.mask
            != 0
        {
            return Err(Error::HardConstraintViolation);
        }
        if self.worlds.is_empty() {
            return Ok(Evaluation::Unknown(UnknownReason::MissingWorlds));
        }
        let mut result = CausalResult {
            true_worlds: 0,
            false_worlds: 0,
            true_witness: None,
            false_witness: None,
            work_used: 0,
        };
        for &world in self.worlds {
            let factual = match self.state(
                world,
                Assignment::empty(),
                &mut result.work_used,
                query.work_limit,
            ) {
                Evaluation::Computed(state) => state,
                Evaluation::Unknown(reason) => return Ok(Evaluation::Unknown(reason)),
            };
            if !self.hard.matches(factual) {
                return Err(Error::HardConstraintViolation);
            }
            if !query.observed.matches(factual) {
                continue;
            }
            let action = match self.state(
                world,
                query.intervention,
                &mut result.work_used,
                query.work_limit,
            ) {
                Evaluation::Computed(state) => state,
                Evaluation::Unknown(reason) => return Ok(Evaluation::Unknown(reason)),
            };
            if !self.hard.matches(action) {
                return Err(Error::HardConstraintViolation);
            }
            let witness = Witness {
                exogenous: world,
                factual,
                action,
            };
            if (action & (1u8 << query.target.0) != 0) == query.value {
                result.true_worlds += 1;
                if result.true_witness.is_none() {
                    result.true_witness = Some(witness);
                }
            } else {
                result.false_worlds += 1;
                if result.false_witness.is_none() {
                    result.false_witness = Some(witness);
                }
            }
        }
        if result.true_worlds + result.false_worlds == 0 {
            return Ok(Evaluation::Unknown(UnknownReason::NoCompatibleWorld));
        }
        Ok(Evaluation::Computed(result))
    }

    fn state(&self, world: u8, action: Assignment, work: &mut u16, limit: u16) -> Evaluation<u8> {
        let mut state = 0;
        for &index in &self.order[..self.equations.len()] {
            if *work >= limit {
                return Evaluation::Unknown(UnknownReason::WorkBudgetExhausted);
            }
            *work += 1;
            let bit = 1u8 << index;
            let value = if action.mask & bit != 0 {
                action.values & bit != 0
            } else {
                match self.equations[usize::from(index)] {
                    Equation::Missing => {
                        return Evaluation::Unknown(UnknownReason::MissingEquation(Variable(index)))
                    }
                    Equation::Exogenous(input) => world & (1u8 << input) != 0,
                    Equation::Table { parents, values } => {
                        let entry = pack_parents(parents, state);
                        values[entry / 64] & (1u64 << (entry % 64)) != 0
                    }
                }
            };
            if value {
                state |= bit;
            }
        }
        Evaluation::Computed(state)
    }
}

fn low_mask(count: usize) -> u8 {
    ((1u16 << count) - 1) as u8
}

fn validate_worlds(worlds: &[u8], domain: u8) -> Result<(), Error> {
    let mut seen = [false; MAX_WORLDS];
    for &world in worlds {
        if world & !domain != 0 {
            return Err(Error::InvalidDomain);
        }
        if seen[usize::from(world)] {
            return Err(Error::DuplicateWorld);
        }
        seen[usize::from(world)] = true;
    }
    Ok(())
}

fn topological_order(equations: &[Equation], domain: u8) -> Result<[u8; 8], Error> {
    let mut order = [0; 8];
    let mut resolved = 0;
    let mut count = 0;
    while resolved != domain {
        let previous = resolved;
        for (index, equation) in equations.iter().enumerate() {
            let bit = 1u8 << index;
            let parents = match equation {
                Equation::Table { parents, .. } => *parents,
                Equation::Missing | Equation::Exogenous(_) => 0,
            };
            if resolved & bit == 0 && parents & !resolved == 0 {
                order[count] = index as u8;
                count += 1;
                resolved |= bit;
            }
        }
        if resolved == previous {
            return Err(Error::Cycle);
        }
    }
    Ok(order)
}

fn pack_parents(parents: u8, state: u8) -> usize {
    let mut packed = 0;
    let mut shift = 0;
    for index in 0..8 {
        let bit = 1u8 << index;
        if parents & bit != 0 {
            if state & bit != 0 {
                packed |= 1usize << shift;
            }
            shift += 1;
        }
    }
    packed
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Query {
    pub observed: Assignment,
    pub intervention: Assignment,
    pub target: Variable,
    pub value: bool,
    pub work_limit: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Witness {
    pub exogenous: u8,
    pub factual: u8,
    pub action: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CausalResult {
    pub true_worlds: u16,
    pub false_worlds: u16,
    pub true_witness: Option<Witness>,
    pub false_witness: Option<Witness>,
    pub work_used: u16,
}

mod spatial;
pub use spatial::{evaluate_spatial, spatial, FrameId, Rectangle, SpatialRelation};
