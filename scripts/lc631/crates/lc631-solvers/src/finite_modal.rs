//! Preparatory finite Kripke evaluation; no solver, proof, or authority promotion.
#![forbid(unsafe_code)]

use lc631_substrate::context_identity::ContextRevision;
use sha2::{Digest, Sha256};

pub const FRAGMENT_REVISION: &str = "finite-kripke-32x64x256.v1";

/// The single relation is modal accessibility, one agent's knowledge relation,
/// intuitionistic information growth, or ideal-world accessibility respectively.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    ModalK,
    EpistemicS5SingleAgent,
    IntuitionisticPreorder,
    DeonticSerialKd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    EmptyFrame,
    LimitExceeded,
    InvalidWorldBits,
    NotReflexive,
    NotSymmetric,
    NotTransitive,
    NotSerial,
    NonPersistentAtom,
    EmptyFormula,
    InvalidRoot,
    InvalidAtom,
    ForwardOrCyclicReference,
    UnsupportedOperator,
    AllocationFailed,
}

/// A checked borrowed frame, at most 32 worlds. Row w contains its successors.
/// Private fields prevent changing the relation after validation.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    profile: Profile,
    rows: &'a [u64],
    universe: u64,
}

impl<'a> Frame<'a> {
    pub fn new(profile: Profile, rows: &'a [u64]) -> Result<Self, Error> {
        if rows.is_empty() {
            return Err(Error::EmptyFrame);
        }
        if rows.len() > 32 {
            return Err(Error::LimitExceeded);
        }
        let universe = (1_u64 << rows.len()) - 1;
        if rows.iter().any(|row| row & !universe != 0) {
            return Err(Error::InvalidWorldBits);
        }
        let frame = Self {
            profile,
            rows,
            universe,
        };
        match profile {
            Profile::ModalK => {}
            Profile::DeonticSerialKd => {
                if rows.contains(&0) {
                    return Err(Error::NotSerial);
                }
            }
            Profile::EpistemicS5SingleAgent | Profile::IntuitionisticPreorder => {
                for (w, row) in rows.iter().enumerate() {
                    if row & (1_u64 << w) == 0 {
                        return Err(Error::NotReflexive);
                    }
                    for (v, next) in rows.iter().enumerate() {
                        if row & (1_u64 << v) != 0 {
                            if profile == Profile::EpistemicS5SingleAgent
                                && next & (1_u64 << w) == 0
                            {
                                return Err(Error::NotSymmetric);
                            }
                            if next & !row != 0 {
                                return Err(Error::NotTransitive);
                            }
                        }
                    }
                }
            }
        }
        Ok(frame)
    }

    pub fn profile(self) -> Profile {
        self.profile
    }

    pub fn world_count(self) -> usize {
        self.rows.len()
    }
}

/// Complete propositional valuation: atoms[a] is the set where atom a holds.
/// An omitted atom is invalid, never default-false. No NL lowering is supplied.
#[derive(Clone, Copy, Debug)]
pub struct Model<'a> {
    frame: Frame<'a>,
    atoms: &'a [u64],
}

impl<'a> Model<'a> {
    pub fn new(frame: Frame<'a>, atoms: &'a [u64]) -> Result<Self, Error> {
        if atoms.len() > 64 {
            return Err(Error::LimitExceeded);
        }
        for atom in atoms {
            if atom & !frame.universe != 0 {
                return Err(Error::InvalidWorldBits);
            }
            if frame.profile == Profile::IntuitionisticPreorder {
                for (w, successors) in frame.rows.iter().enumerate() {
                    if atom & (1_u64 << w) != 0 && successors & !atom != 0 {
                        return Err(Error::NonPersistentAtom);
                    }
                }
            }
        }
        Ok(Self { frame, atoms })
    }
}

/// Node references are local DAG indexes and must precede their parent.
/// Operators cannot silently change profiles: K uses Box/Diamond, S5 uses Know,
/// KD uses Ought, and intuitionistic formulas exclude all modal operators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Formula {
    Bottom,
    Top,
    Atom(u8),
    Not(u16),
    And(u16, u16),
    Or(u16, u16),
    Implies(u16, u16),
    Box(u16),
    Diamond(u16),
    Know(u16),
    Ought(u16),
}

/// Truth on precisely the supplied finite model. False-world bits are a
/// counterexample set within that model, not a proof of global invalidity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Evaluation {
    profile: Profile,
    root: u16,
    true_worlds: u64,
    false_worlds: u64,
}

/// Declared query identity attached to one typed context. The source and
/// interpretation are derived from that context, never independently supplied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallerLineage {
    pub context: ContextRevision,
    pub query_revision: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationBasis {
    EvaluatorReported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Termination {
    CompletedFiniteEvaluation,
}

/// Native finite observation, never an external-process execution report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeObservation {
    caller_lineage: CallerLineage,
    finite_world_count: u8,
    termination: Termination,
    validation_basis: ValidationBasis,
    evaluation: Evaluation,
    model_digest: [u8; 32],
    formula_digest: [u8; 32],
    input_digest: [u8; 32],
}

impl NativeObservation {
    pub fn caller_lineage(self) -> CallerLineage {
        self.caller_lineage
    }
    pub fn context(self) -> ContextRevision {
        self.caller_lineage.context
    }
    pub fn source_digest(self) -> [u8; 32] {
        *self.context().source().digest().as_bytes()
    }
    pub fn interpretation_digest(self) -> [u8; 32] {
        *self.context().interpretation_digest().as_bytes()
    }
    pub fn query_digest(self) -> [u8; 32] {
        self.caller_lineage.query_revision
    }
    pub fn finite_world_count(self) -> u8 {
        self.finite_world_count
    }
    pub fn termination(self) -> Termination {
        self.termination
    }
    pub fn validation_basis(self) -> ValidationBasis {
        self.validation_basis
    }
    pub fn evaluation(self) -> Evaluation {
        self.evaluation
    }
    pub fn model_digest(self) -> [u8; 32] {
        self.model_digest
    }
    pub fn formula_digest(self) -> [u8; 32] {
        self.formula_digest
    }
    pub fn input_digest(self) -> [u8; 32] {
        self.input_digest
    }
}

pub fn evaluate_with_lineage(
    caller_lineage: CallerLineage,
    model: Model<'_>,
    nodes: &[Formula],
    root: u16,
) -> Result<NativeObservation, Error> {
    let finite_world_count = model.frame.world_count() as u8;
    let evaluation = evaluate(model, nodes, root)?;
    let model_digest = digest_model(model);
    let formula_digest = digest_formula(nodes, root);
    let mut hash = Sha256::new();
    hash.update(b"epistesys.kripke.input.v1\0");
    hash.update(FRAGMENT_REVISION.as_bytes());
    hash.update(caller_lineage.context.source().digest().as_bytes());
    hash.update(caller_lineage.context.interpretation_digest().as_bytes());
    hash.update(caller_lineage.query_revision);
    hash.update(model_digest);
    hash.update(formula_digest);
    Ok(NativeObservation {
        caller_lineage,
        finite_world_count,
        termination: Termination::CompletedFiniteEvaluation,
        validation_basis: ValidationBasis::EvaluatorReported,
        evaluation,
        model_digest,
        formula_digest,
        input_digest: hash.finalize().into(),
    })
}

fn profile_tag(profile: Profile) -> u8 {
    match profile {
        Profile::ModalK => 0,
        Profile::EpistemicS5SingleAgent => 1,
        Profile::IntuitionisticPreorder => 2,
        Profile::DeonticSerialKd => 3,
    }
}

fn digest_model(model: Model<'_>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.kripke.model.v1\0");
    hash.update([
        profile_tag(model.frame.profile),
        model.frame.rows.len() as u8,
    ]);
    for row in model.frame.rows {
        hash.update(row.to_le_bytes());
    }
    hash.update([model.atoms.len() as u8]);
    for atom in model.atoms {
        hash.update(atom.to_le_bytes());
    }
    hash.finalize().into()
}

fn digest_formula(nodes: &[Formula], root: u16) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.kripke.formula.v1\0");
    hash.update((nodes.len() as u16).to_le_bytes());
    hash.update(root.to_le_bytes());
    for node in nodes {
        let (tag, args) = match *node {
            Formula::Bottom => (0, [0, 0]),
            Formula::Top => (1, [0, 0]),
            Formula::Atom(a) => (2, [u16::from(a), 0]),
            Formula::Not(a) => (3, [a, 0]),
            Formula::And(a, b) => (4, [a, b]),
            Formula::Or(a, b) => (5, [a, b]),
            Formula::Implies(a, b) => (6, [a, b]),
            Formula::Box(a) => (7, [a, 0]),
            Formula::Diamond(a) => (8, [a, 0]),
            Formula::Know(a) => (9, [a, 0]),
            Formula::Ought(a) => (10, [a, 0]),
        };
        hash.update([tag]);
        for arg in args {
            hash.update(arg.to_le_bytes());
        }
    }
    hash.finalize().into()
}

impl Evaluation {
    pub fn profile(self) -> Profile {
        self.profile
    }
    pub fn root(self) -> u16 {
        self.root
    }
    pub fn true_worlds(self) -> u64 {
        self.true_worlds
    }
    pub fn false_worlds(self) -> u64 {
        self.false_worlds
    }
    pub fn at_world(self, world: u8) -> Result<bool, Error> {
        let bit = 1_u64
            .checked_shl(u32::from(world))
            .ok_or(Error::InvalidWorldBits)?;
        if (self.true_worlds | self.false_worlds) & bit == 0 {
            return Err(Error::InvalidWorldBits);
        }
        Ok(self.true_worlds & bit != 0)
    }
}

fn universally(frame: Frame<'_>, values: u64) -> u64 {
    frame.rows.iter().enumerate().fold(0, |set, (w, row)| {
        set | if row & !values == 0 { 1_u64 << w } else { 0 }
    })
}

/// Evaluates every supplied node (including unused nodes) to reject hidden
/// malformed fragments. Bounded to 256 nodes and 32 worlds, with fallible scratch.
/// No cache, I/O, receipt issuer, mutation permit, or ResultStore is involved.
pub fn evaluate(model: Model<'_>, nodes: &[Formula], root: u16) -> Result<Evaluation, Error> {
    if nodes.is_empty() {
        return Err(Error::EmptyFormula);
    }
    if nodes.len() > 256 {
        return Err(Error::LimitExceeded);
    }
    if usize::from(root) >= nodes.len() {
        return Err(Error::InvalidRoot);
    }
    let frame = model.frame;
    let mut values = Vec::<u64>::new();
    values
        .try_reserve_exact(nodes.len())
        .map_err(|_| Error::AllocationFailed)?;
    for (index, node) in nodes.iter().enumerate() {
        let prior = |id: u16| -> Result<u64, Error> {
            if usize::from(id) >= index {
                return Err(Error::ForwardOrCyclicReference);
            }
            Ok(values[usize::from(id)])
        };
        let value = match *node {
            Formula::Bottom => 0,
            Formula::Top => frame.universe,
            Formula::Atom(id) => *model.atoms.get(usize::from(id)).ok_or(Error::InvalidAtom)?,
            Formula::And(a, b) => prior(a)? & prior(b)?,
            Formula::Or(a, b) => prior(a)? | prior(b)?,
            Formula::Not(a) => {
                let complement = frame.universe & !prior(a)?;
                if frame.profile == Profile::IntuitionisticPreorder {
                    universally(frame, complement)
                } else {
                    complement
                }
            }
            Formula::Implies(a, b) => {
                let material = frame.universe & (!prior(a)? | prior(b)?);
                if frame.profile == Profile::IntuitionisticPreorder {
                    universally(frame, material)
                } else {
                    material
                }
            }
            Formula::Box(a) if frame.profile == Profile::ModalK => universally(frame, prior(a)?),
            Formula::Know(a) if frame.profile == Profile::EpistemicS5SingleAgent => {
                universally(frame, prior(a)?)
            }
            Formula::Ought(a) if frame.profile == Profile::DeonticSerialKd => {
                universally(frame, prior(a)?)
            }
            Formula::Diamond(a) if frame.profile == Profile::ModalK => {
                frame.universe & !universally(frame, frame.universe & !prior(a)?)
            }
            _ => return Err(Error::UnsupportedOperator),
        };
        values.push(value);
    }
    let true_worlds = values[usize::from(root)];
    Ok(Evaluation {
        profile: frame.profile,
        root,
        true_worlds,
        false_worlds: frame.universe & !true_worlds,
    })
}

/// Projection correspondence obligations, not observed TL scores or grants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionObligation {
    FiniteDomainAndCompleteValuation,
    WorldAndAtomMapping,
    AccessibilityAndFrameConditions,
    IntuitionisticPersistenceAndImplication,
    SingleAgentKnowledgeScope,
    NormativeAccessibilityWithoutExecutionAuthority,
}

pub fn projection_obligations(profile: Profile) -> &'static [ProjectionObligation] {
    use ProjectionObligation::*;
    match profile {
        Profile::ModalK => &[
            FiniteDomainAndCompleteValuation,
            WorldAndAtomMapping,
            AccessibilityAndFrameConditions,
        ],
        Profile::EpistemicS5SingleAgent => &[
            FiniteDomainAndCompleteValuation,
            WorldAndAtomMapping,
            AccessibilityAndFrameConditions,
            SingleAgentKnowledgeScope,
        ],
        Profile::IntuitionisticPreorder => &[
            FiniteDomainAndCompleteValuation,
            WorldAndAtomMapping,
            AccessibilityAndFrameConditions,
            IntuitionisticPersistenceAndImplication,
        ],
        Profile::DeonticSerialKd => &[
            FiniteDomainAndCompleteValuation,
            WorldAndAtomMapping,
            AccessibilityAndFrameConditions,
            NormativeAccessibilityWithoutExecutionAuthority,
        ],
    }
}
