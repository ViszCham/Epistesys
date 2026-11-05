use lc631_substrate::context_identity::ContextRevision;
use lc631_substrate::Digest32;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AtomId(u16);

impl AtomId {
    pub const fn new(id: u16) -> Self {
        Self(id)
    }
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Supplied domain/grounding identities are declarations, not verified evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspBinding {
    context: ContextRevision,
    domain: Digest32,
    grounding_basis: Digest32,
}

impl AspBinding {
    pub const fn declared(
        context: ContextRevision,
        domain: Digest32,
        grounding_basis: Digest32,
    ) -> Self {
        Self {
            context,
            domain,
            grounding_basis,
        }
    }
    pub const fn context(self) -> ContextRevision {
        self.context
    }
    pub const fn domain(self) -> Digest32 {
        self.domain
    }
    pub const fn grounding_basis(self) -> Digest32 {
        self.grounding_basis
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AspSemantics {
    Unspecified,
    /// Absence from a stable model is default falsity, not explicit negation.
    StableModelsClosedWorld,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stratification {
    Stratified,
    NonStratified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroundRule {
    pub(super) head: Option<AtomId>,
    pub(super) positive: Vec<AtomId>,
    pub(super) negative: Vec<AtomId>,
}

impl GroundRule {
    /// `negative` contains explicit *default* negation, never classical literals.
    pub fn new(head: Option<AtomId>, positive: Vec<AtomId>, negative: Vec<AtomId>) -> Self {
        Self {
            head,
            positive,
            negative,
        }
    }
}

/// One nonnegative minimize cost; multiple priority levels are unsupported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomCost {
    pub atom: AtomId,
    pub weight: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AspError {
    SemanticsRequired,
    InvalidLimits,
    LimitsTooLarge,
    AtomLimit,
    RuleLimit,
    LiteralLimit,
    DuplicateAtom,
    UndeclaredAtom,
    DuplicateCost,
    ArithmeticOverflow,
    AllocationFailed,
    OutputLimit,
    OutputSyntax,
    UnsupportedOutput,
    InvalidAtomSyntax,
    DuplicateModelAtom,
    DuplicateModel,
    InvalidStableModel,
    IncompleteAnswerSet,
    StatusMismatch,
    BindingMismatch,
    ObservationIncomplete,
    OutputMismatch,
    Query,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspLimits {
    pub(super) atoms: usize,
    pub(super) rules: usize,
    pub(super) literals: usize,
    pub(super) models: usize,
    pub(super) steps: u64,
    pub(super) output: usize,
}

impl AspLimits {
    pub fn checked(
        atoms: usize,
        rules: usize,
        literals: usize,
        models: usize,
        steps: u64,
        output: usize,
    ) -> Result<Self, AspError> {
        if models == 0 || steps == 0 || output == 0 {
            return Err(AspError::InvalidLimits);
        }
        if atoms > 16
            || rules > 1024
            || literals > 16_384
            || models > 256
            || steps > 100_000_000
            || output > crate::registry::MAX_SOLVER_OUTPUT_BYTES
        {
            return Err(AspError::LimitsTooLarge);
        }
        Ok(Self {
            atoms,
            rules,
            literals,
            models,
            steps,
            output,
        })
    }
}

impl Default for AspLimits {
    fn default() -> Self {
        Self {
            atoms: 16,
            rules: 1024,
            literals: 16_384,
            models: 256,
            steps: 10_000_000,
            output: 256 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct AspProgram {
    pub(super) binding: AspBinding,
    pub(super) atoms: Vec<AtomId>,
    pub(super) rules: Vec<GroundRule>,
    pub(super) costs: Vec<AtomCost>,
    pub(super) limits: AspLimits,
}

impl AspProgram {
    pub fn checked(
        binding: AspBinding,
        semantics: AspSemantics,
        mut atoms: Vec<AtomId>,
        rules: Vec<GroundRule>,
        costs: Vec<AtomCost>,
        limits: AspLimits,
    ) -> Result<Self, AspError> {
        if semantics != AspSemantics::StableModelsClosedWorld {
            return Err(AspError::SemanticsRequired);
        }
        if atoms.len() > limits.atoms {
            return Err(AspError::AtomLimit);
        }
        if rules.len() > limits.rules {
            return Err(AspError::RuleLimit);
        }
        atoms.sort_unstable();
        if atoms.windows(2).any(|w| w[0] == w[1]) {
            return Err(AspError::DuplicateAtom);
        }
        let mut count = 0usize;
        for rule in &rules {
            count = count
                .checked_add(rule.positive.len())
                .and_then(|n| n.checked_add(rule.negative.len()))
                .and_then(|n| n.checked_add(usize::from(rule.head.is_some())))
                .ok_or(AspError::ArithmeticOverflow)?;
            if count > limits.literals {
                return Err(AspError::LiteralLimit);
            }
            for atom in rule.head.iter().chain(&rule.positive).chain(&rule.negative) {
                if atoms.binary_search(atom).is_err() {
                    return Err(AspError::UndeclaredAtom);
                }
            }
        }
        if costs.len() > atoms.len() {
            return Err(AspError::DuplicateCost);
        }
        let mut seen = [false; 16];
        let mut total = 0u64;
        for cost in &costs {
            let index = atoms
                .binary_search(&cost.atom)
                .map_err(|_| AspError::UndeclaredAtom)?;
            if seen[index] {
                return Err(AspError::DuplicateCost);
            }
            seen[index] = true;
            total = total
                .checked_add(cost.weight)
                .ok_or(AspError::ArithmeticOverflow)?;
        }
        Ok(Self {
            binding,
            atoms,
            rules,
            costs,
            limits,
        })
    }

    pub const fn binding(&self) -> AspBinding {
        self.binding
    }
    pub fn atoms(&self) -> &[AtomId] {
        &self.atoms
    }

    pub fn stratification(&self) -> Stratification {
        let mut reach = [[false; 16]; 16];
        for rule in &self.rules {
            if let Some(head) = rule.head {
                if let Ok(h) = self.atoms.binary_search(&head) {
                    for atom in rule.positive.iter().chain(&rule.negative) {
                        if let Ok(b) = self.atoms.binary_search(atom) {
                            reach[h][b] = true;
                        }
                    }
                }
            }
        }
        for k in 0..self.atoms.len() {
            for i in 0..self.atoms.len() {
                for j in 0..self.atoms.len() {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        for rule in &self.rules {
            if let Some(head) = rule.head {
                if let Ok(h) = self.atoms.binary_search(&head) {
                    for atom in &rule.negative {
                        if let Ok(b) = self.atoms.binary_search(atom) {
                            if b == h || reach[b][h] {
                                return Stratification::NonStratified;
                            }
                        }
                    }
                }
            }
        }
        Stratification::Stratified
    }

    pub(super) fn mask(&self, atoms: &[AtomId]) -> Result<u16, AspError> {
        let mut mask = 0u16;
        for atom in atoms {
            let index = self
                .atoms
                .binary_search(atom)
                .map_err(|_| AspError::UndeclaredAtom)?;
            mask |= 1u16 << index;
        }
        Ok(mask)
    }
}
