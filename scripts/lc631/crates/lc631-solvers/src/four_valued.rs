//! Preparatory finite signed-Horn least model, revision four-valued-horn-v1.
//! Both and Neither are retained. No classical satisfiability or authority is issued.
#![forbid(unsafe_code)]
use lc631_substrate::context_identity::ContextRevision;
use sha2::{Digest, Sha256};

pub const MAX_ATOMS: usize = 64;
pub const MAX_RULES: usize = 1024;
pub const MAX_BODY: usize = 64;
pub const PROFILE_REVISION: &str = "four-valued-horn-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Truth {
    Neither,
    Supported,
    Refuted,
    Both,
}

impl Truth {
    pub const fn from_bits(support: bool, refutation: bool) -> Self {
        match (support, refutation) {
            (false, false) => Self::Neither,
            (true, false) => Self::Supported,
            (false, true) => Self::Refuted,
            (true, true) => Self::Both,
        }
    }
    pub const fn support(self) -> bool {
        matches!(self, Self::Supported | Self::Both)
    }
    pub const fn refutation(self) -> bool {
        matches!(self, Self::Refuted | Self::Both)
    }
    pub const fn negated(self) -> Self {
        Self::from_bits(self.refutation(), self.support())
    }
    pub const fn conjunction(self, other: Self) -> Self {
        Self::from_bits(
            self.support() && other.support(),
            self.refutation() || other.refutation(),
        )
    }
    pub const fn disjunction(self, other: Self) -> Self {
        Self::from_bits(
            self.support() || other.support(),
            self.refutation() && other.refutation(),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Atom(u8);
impl Atom {
    pub fn new(index: u8) -> Result<Self, Error> {
        if usize::from(index) >= MAX_ATOMS {
            Err(Error::InvalidAtom)
        } else {
            Ok(Self(index))
        }
    }
    pub const fn index(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Literal {
    atom: Atom,
    negative: bool,
}
impl Literal {
    pub const fn positive(atom: Atom) -> Self {
        Self {
            atom,
            negative: false,
        }
    }
    pub const fn negative(atom: Atom) -> Self {
        Self {
            atom,
            negative: true,
        }
    }
    pub const fn atom(self) -> Atom {
        self.atom
    }
    pub const fn is_negative(self) -> bool {
        self.negative
    }
    const fn slot(self) -> usize {
        self.atom.0 as usize * 2 + self.negative as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rule<'a> {
    pub body: &'a [Literal],
    pub head: Literal,
}

/// Context identity and exact typed-program content binding, not an authenticated
/// receipt. The query remains caller-declared; the program digest is computed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    context: ContextRevision,
    query: [u8; 32],
    formula: [u8; 32],
}
impl Binding {
    pub const fn context(self) -> ContextRevision {
        self.context
    }
    pub fn source_digest(self) -> [u8; 32] {
        *self.context.source().digest().as_bytes()
    }
    pub fn interpretation_digest(self) -> [u8; 32] {
        *self.context.interpretation_digest().as_bytes()
    }
    pub const fn query_digest(self) -> [u8; 32] {
        self.query
    }
    pub const fn formula_digest(self) -> [u8; 32] {
        self.formula
    }
    pub fn input_digest(self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"epistesys.four-valued-horn.input.v1\0");
        hash.update(self.source_digest());
        hash.update(self.interpretation_digest());
        hash.update(self.query);
        hash.update(self.formula);
        hash.finalize().into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidAtom,
    InvalidAtomCount,
    TooManyRules,
    BodyTooLarge,
    UndeclaredAtom,
    DuplicateFact,
    ResourceExhausted,
}

pub struct Program<'a> {
    atoms: usize,
    facts: &'a [Literal],
    rules: &'a [Rule<'a>],
    binding: Binding,
}

impl<'a> Program<'a> {
    pub fn new(
        atoms: usize,
        facts: &'a [Literal],
        rules: &'a [Rule<'a>],
        context: ContextRevision,
        query: [u8; 32],
    ) -> Result<Self, Error> {
        if atoms == 0 || atoms > MAX_ATOMS {
            return Err(Error::InvalidAtomCount);
        }
        if rules.len() > MAX_RULES {
            return Err(Error::TooManyRules);
        }
        if facts.len() > atoms * 2 {
            return Err(Error::DuplicateFact);
        }
        let mut seen = [false; MAX_ATOMS * 2];
        for fact in facts {
            check_atom(fact.atom, atoms)?;
            if seen[fact.slot()] {
                return Err(Error::DuplicateFact);
            }
            seen[fact.slot()] = true;
        }
        for rule in rules {
            if rule.body.len() > MAX_BODY {
                return Err(Error::BodyTooLarge);
            }
            check_atom(rule.head.atom, atoms)?;
            for literal in rule.body {
                check_atom(literal.atom, atoms)?;
            }
        }
        let binding = Binding {
            context,
            query,
            formula: program_digest(atoms, &seen, rules),
        };
        Ok(Self {
            atoms,
            facts,
            rules,
            binding,
        })
    }

    /// Logical budget counts body inspections and head updates. Exhaustion
    /// returns an error, never a complete or false result. No heap allocation.
    pub fn evaluate(&self, max_steps: u64) -> Result<Evaluation, Error> {
        let mut result = Evaluation {
            atoms: self.atoms,
            present: [false; MAX_ATOMS * 2],
            dependencies: [0; MAX_ATOMS * 2],
            binding: self.binding,
            steps: 0,
        };
        for fact in self.facts {
            spend(&mut result.steps, max_steps)?;
            result.present[fact.slot()] = true;
            result.dependencies[fact.slot()] = 1u64 << fact.atom.0;
        }
        loop {
            let mut changed = false;
            for rule in self.rules {
                let mut enabled = true;
                let mut dependencies = 0;
                for literal in rule.body {
                    spend(&mut result.steps, max_steps)?;
                    if !result.present[literal.slot()] {
                        enabled = false;
                        break;
                    }
                    dependencies |= result.dependencies[literal.slot()];
                }
                if enabled {
                    spend(&mut result.steps, max_steps)?;
                    let slot = rule.head.slot();
                    let previous = result.dependencies[slot];
                    result.dependencies[slot] |= dependencies;
                    if !result.present[slot] || previous != result.dependencies[slot] {
                        changed = true;
                        result.present[slot] = true;
                    }
                }
            }
            if !changed {
                return Ok(result);
            }
        }
    }
}

/// Domain-separated canonical typed encoding; rule and body order are retained
/// as declared input syntax. This is content binding, not semantic equivalence.
fn program_digest(atoms: usize, facts: &[bool; MAX_ATOMS * 2], rules: &[Rule<'_>]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.four-valued-horn.program.v1\0");
    hash.update([atoms as u8]);
    for present in facts {
        hash.update([u8::from(*present)]);
    }
    hash.update((rules.len() as u16).to_le_bytes());
    for rule in rules {
        hash.update([
            rule.head.atom.0,
            u8::from(rule.head.negative),
            rule.body.len() as u8,
        ]);
        for literal in rule.body {
            hash.update([literal.atom.0, u8::from(literal.negative)]);
        }
    }
    hash.finalize().into()
}

fn spend(steps: &mut u64, limit: u64) -> Result<(), Error> {
    if *steps >= limit {
        return Err(Error::ResourceExhausted);
    }
    *steps += 1;
    Ok(())
}
fn check_atom(atom: Atom, count: usize) -> Result<(), Error> {
    if usize::from(atom.0) < count {
        Ok(())
    } else {
        Err(Error::UndeclaredAtom)
    }
}

#[derive(Debug)]
pub struct Evaluation {
    atoms: usize,
    present: [bool; MAX_ATOMS * 2],
    dependencies: [u64; MAX_ATOMS * 2],
    binding: Binding,
    steps: u64,
}
impl Evaluation {
    pub const fn atom_count(&self) -> usize {
        self.atoms
    }
    pub const fn binding(&self) -> Binding {
        self.binding
    }
    pub const fn steps(&self) -> u64 {
        self.steps
    }
    pub fn truth(&self, atom: Atom) -> Result<Truth, Error> {
        check_atom(atom, self.atoms)?;
        let slot = usize::from(atom.0) * 2;
        Ok(Truth::from_bits(self.present[slot], self.present[slot + 1]))
    }
    /// Union of fact-atom dependencies across all enabled derivations. This
    /// conservative local scope is not a minimal unsat core or a proof token.
    pub fn dependencies(&self, literal: Literal) -> Result<Option<u64>, Error> {
        check_atom(literal.atom, self.atoms)?;
        if self.present[literal.slot()] {
            Ok(Some(self.dependencies[literal.slot()]))
        } else {
            Ok(None)
        }
    }
    pub fn contradiction_scope(&self, atom: Atom) -> Result<Option<u64>, Error> {
        if self.truth(atom)? != Truth::Both {
            return Ok(None);
        }
        let slot = usize::from(atom.0) * 2;
        Ok(Some(self.dependencies[slot] | self.dependencies[slot + 1]))
    }
}
