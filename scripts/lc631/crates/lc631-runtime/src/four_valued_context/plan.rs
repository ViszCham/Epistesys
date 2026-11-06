//! 日本語: 全前提を明示文法へlowerし、元のbasisと未検証境界を保持する。
//! English: Lower every premise under explicit grammar, retaining basis and unverified boundaries.
use super::*;
use lc631_core::context::PremiseBasis;
use sha2::{Digest, Sha256};

const MAX_PREMISES: usize = 256;
const MAX_TEXT: usize = 16 * 1024;
pub(super) enum Parsed {
    Fact(Literal),
    Rule {
        body: [Literal; 64],
        len: u8,
        head: Literal,
    },
}

/// Explicit finite MODEL grammar only: `+N`, `-N`, or `+N <- +M,-K`.
/// Canonical decimal N is 0..atoms-1, atoms<=64. A comma may have surrounding
/// ASCII spaces; other whitespace/prose/default negation is unsupported.
/// Given/Assumed/Proposed stay input classifications, never source truth.
/// Every premise is parsed; an unsupported premise rejects the whole plan.
pub struct BranchPlan<'graph, 'view> {
    pub(super) graph: &'graph ContextGraph,
    pub(super) state: &'graph ContextState,
    pub(super) parsed: Vec<Parsed>,
    pub(super) atoms: usize,
    pub(super) query: Atom,
    pub(super) query_digest: [u8; 32],
    held: ResourceClaim<'view>,
}
impl<'graph, 'view> BranchPlan<'graph, 'view> {
    pub fn prepare(
        graph: &'graph ContextGraph,
        context: ContextRevision,
        scope: ContextScope,
        atoms: usize,
        query: Atom,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, BranchError> {
        view.check().map_err(BranchError::Resource)?;
        let state = graph.get(context).ok_or(BranchError::UnknownContext)?;
        if state.scope() != scope {
            return Err(BranchError::ScopeMismatch);
        }
        if context.source().digest() != view.source_revision() {
            return Err(BranchError::Resource(ResourceError::SourceMismatch));
        }
        if atoms == 0 || atoms > 64 {
            return Err(BranchError::InvalidAtomCount);
        }
        if usize::from(query.index()) >= atoms {
            return Err(BranchError::UndeclaredAtom);
        }
        if state.premises().len() > MAX_PREMISES {
            return Err(BranchError::PremiseBudget);
        }
        if state
            .declared_delta()
            .is_some_and(|delta| delta.edits().len() > MAX_PREMISES)
        {
            return Err(BranchError::PremiseBudget);
        }
        let bytes = state
            .premises()
            .len()
            .checked_mul(std::mem::size_of::<Parsed>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .ok_or(BranchError::ArithmeticOverflow)?;
        let held = claim(view, bytes)?;
        let mut parsed = Vec::new();
        parsed
            .try_reserve_exact(state.premises().len())
            .map_err(|_| BranchError::AllocationFailed)?;
        if parsed.capacity() > state.premises().len() {
            return Err(BranchError::AllocationFailed);
        }
        let mut text_bytes = 0usize;
        for premise in state.premises() {
            match premise.basis() {
                PremiseBasis::Given | PremiseBasis::Assumed | PremiseBasis::Proposed => {}
                PremiseBasis::Observed
                | PremiseBasis::Derived
                | PremiseBasis::Refuted
                | PremiseBasis::Conflicted => return Err(BranchError::UnsupportedBasis),
            }
            text_bytes = text_bytes
                .checked_add(premise.statement().len())
                .ok_or(BranchError::PremiseBudget)?;
            if text_bytes > MAX_TEXT {
                return Err(BranchError::PremiseBudget);
            }
            parsed.push(parse(premise.statement(), atoms, query)?);
        }
        let query_digest = query_digest(state, atoms, query);
        let result = Self {
            graph,
            state,
            parsed,
            atoms,
            query,
            query_digest,
            held,
        };
        result.check(view)?;
        Ok(result)
    }
    pub const fn query_digest(&self) -> [u8; 32] {
        self.query_digest
    }
    pub const fn context(&self) -> ContextRevision {
        self.state.revision()
    }
    pub const fn context_state(&self) -> &ContextState {
        self.state
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), BranchError> {
        check_claim(&self.held, view)?;
        if self
            .graph
            .get(self.state.revision())
            .is_none_or(|s| !std::ptr::eq(s, self.state))
        {
            return Err(BranchError::UnknownContext);
        }
        if self.state.revision().source().digest() != view.source_revision() {
            return Err(BranchError::Resource(ResourceError::SourceMismatch));
        }
        if let Some(parent) = self.state.parent() {
            self.graph.get(parent).ok_or(BranchError::UnknownParent)?;
        }
        Ok(())
    }
}

fn parse(text: &str, atoms: usize, fill: Atom) -> Result<Parsed, BranchError> {
    if text.is_empty()
        || !text.is_ascii()
        || text.trim() != text
        || text.bytes().any(|b| b.is_ascii_whitespace() && b != b' ')
    {
        return Err(BranchError::UnsupportedSyntax);
    }
    match text.split_once(" <- ") {
        None => Ok(Parsed::Fact(literal(text, atoms)?)),
        Some((head, tail)) => {
            let head = literal(head, atoms)?;
            let mut body = [Literal::positive(fill); 64];
            let mut len = 0usize;
            for item in tail.split(',') {
                if len == body.len() {
                    return Err(BranchError::PremiseBudget);
                }
                body[len] = literal(item.trim_matches(' '), atoms)?;
                len += 1;
            }
            Ok(Parsed::Rule {
                body,
                len: len as u8,
                head,
            })
        }
    }
}
fn literal(text: &str, atoms: usize) -> Result<Literal, BranchError> {
    let bytes = text.as_bytes();
    if !(2..=3).contains(&bytes.len())
        || !matches!(bytes[0], b'+' | b'-')
        || !bytes[1..].iter().all(u8::is_ascii_digit)
        || (bytes.len() == 3 && bytes[1] == b'0')
    {
        return Err(BranchError::UnsupportedSyntax);
    }
    let index = bytes[1..]
        .iter()
        .fold(0u8, |value, digit| value * 10 + digit - b'0');
    if usize::from(index) >= atoms {
        return Err(BranchError::UndeclaredAtom);
    }
    let atom = Atom::new(index).map_err(|_| BranchError::UndeclaredAtom)?;
    Ok(if bytes[0] == b'-' {
        Literal::negative(atom)
    } else {
        Literal::positive(atom)
    })
}
fn query_digest(state: &ContextState, atoms: usize, query: Atom) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.context-four-valued.explicit-query.v1\0");
    hash.update(PROFILE.as_bytes());
    hash.update(lc631_solvers::four_valued::PROFILE_REVISION.as_bytes());
    for pin in [
        state.revision().source().digest(),
        state.revision().interpretation_digest(),
        state.scope().comparison(),
        state.scope().goal(),
        state.scope().probes(),
        state.scope().semantics(),
    ] {
        hash.update(pin.as_bytes());
    }
    hash.update([atoms as u8, query.index()]);
    hash.update((state.premises().len() as u64).to_le_bytes());
    for premise in state.premises() {
        hash.update(premise.id().as_bytes());
        hash.update((premise.statement().len() as u64).to_le_bytes());
        hash.update(premise.statement().as_bytes());
        let basis = match premise.basis() {
            PremiseBasis::Given => 0,
            PremiseBasis::Assumed => 1,
            PremiseBasis::Proposed => 2,
            PremiseBasis::Observed => 3,
            PremiseBasis::Derived => 4,
            PremiseBasis::Refuted => 5,
            PremiseBasis::Conflicted => 6,
        };
        hash.update([basis, u8::from(premise.changeable())]);
    }
    hash.finalize().into()
}
