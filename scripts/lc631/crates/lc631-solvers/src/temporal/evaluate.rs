//! 日本語: 有限suffixの実評価。reportを独立検証・証明へ昇格しない。
//! English: Evaluate real finite suffixes; a report is not independent validation or proof.
use super::{FiniteTrace, TemporalError, TemporalNode, TemporalProfile, TemporalQuery, Truth};
use crate::{LogicalResult, Support, TraceDomain};
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
use lc631_substrate::Digest32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvaluationBudget {
    max_node_steps: usize,
}

impl EvaluationBudget {
    pub fn checked(max_node_steps: usize) -> Result<Self, TemporalError> {
        if max_node_steps > 1024 * 256 {
            return Err(TemporalError::InvalidBudget);
        }
        Ok(Self { max_node_steps })
    }
}
impl Default for EvaluationBudget {
    fn default() -> Self {
        Self {
            max_node_steps: 1024 * 256,
        }
    }
}

/// Only this evaluator can construct the report. Its observation is in-memory;
/// no persistence, host receipt, proof, authority or shared-store completion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalEvaluation {
    context: ContextRevision,
    query: Digest32,
    trace: Digest32,
    truth: Truth,
    counterexample: Option<FiniteTrace>,
    node_steps: usize,
}

impl TemporalEvaluation {
    pub const fn context(&self) -> ContextRevision {
        self.context
    }
    pub const fn source(&self) -> SourceRevision {
        self.context.source()
    }
    pub const fn query_digest(&self) -> Digest32 {
        self.query
    }
    pub const fn trace_digest(&self) -> Digest32 {
        self.trace
    }
    pub const fn profile(&self) -> TemporalProfile {
        TemporalProfile::ClosedFiniteKleeneV1
    }
    pub fn profile_digest(&self) -> Digest32 {
        self.profile().digest()
    }
    pub const fn truth(&self) -> Truth {
        self.truth
    }
    pub const fn evaluated_at(&self) -> usize {
        0
    }
    pub const fn node_steps(&self) -> usize {
        self.node_steps
    }
    pub fn counterexample(&self) -> Option<&FiniteTrace> {
        self.counterexample.as_ref()
    }
    pub fn logical_result(&self) -> LogicalResult {
        let support = match self.truth {
            Truth::True => Support::Supported,
            Truth::False => Support::Refuted,
            Truth::Unknown => Support::Neither,
        };
        LogicalResult::ModelCheck {
            support,
            traces: TraceDomain::Finite,
        }
    }
}

pub fn evaluate(
    query: &TemporalQuery,
    trace: &FiniteTrace,
    budget: EvaluationBudget,
) -> Result<TemporalEvaluation, TemporalError> {
    let width = trace.steps().len();
    let cells = query
        .nodes()
        .len()
        .checked_mul(width)
        .ok_or(TemporalError::ResourceExhausted)?;
    if cells > budget.max_node_steps {
        return Err(TemporalError::ResourceExhausted);
    }
    for node in query.nodes() {
        if let TemporalNode::Atom(atom) = node {
            if usize::from(*atom) >= trace.atom_count() {
                return Err(TemporalError::MissingAtom);
            }
        }
    }
    let mut table = Vec::new();
    table
        .try_reserve_exact(cells)
        .map_err(|_| TemporalError::AllocationFailed)?;
    if table.capacity() > cells {
        return Err(TemporalError::AllocationFailed);
    }
    table.resize(cells, Truth::Unknown);
    for (index, node) in query.nodes().iter().enumerate() {
        for time in (0..width).rev() {
            table[index * width + time] = evaluate_cell(*node, index, time, width, &table, trace);
        }
    }
    let truth = table[usize::from(query.root()) * width];
    Ok(TemporalEvaluation {
        context: query.context(),
        query: query.digest(),
        trace: trace.digest(),
        truth,
        counterexample: if truth == Truth::False {
            Some(trace.clone())
        } else {
            None
        },
        node_steps: cells,
    })
}

fn evaluate_cell(
    node: TemporalNode,
    index: usize,
    time: usize,
    width: usize,
    table: &[Truth],
    trace: &FiniteTrace,
) -> Truth {
    // Checked constructors establish all node/time/atom bounds before the
    // table allocation; each child row is complete before its parent row.
    let at = |child: u16| table[usize::from(child) * width + time];
    let next_self = |end: Truth| {
        if time + 1 < width {
            table[index * width + time + 1]
        } else {
            end
        }
    };
    match node {
        TemporalNode::Constant(false) => Truth::False,
        TemporalNode::Constant(true) => Truth::True,
        TemporalNode::Atom(atom) => trace.steps()[time].values()[usize::from(atom)],
        TemporalNode::Not(a) => at(a).not(),
        TemporalNode::And(a, b) => at(a).and(at(b)),
        TemporalNode::Or(a, b) => at(a).or(at(b)),
        TemporalNode::StrongNext(a) => {
            if time + 1 < width {
                table[usize::from(a) * width + time + 1]
            } else {
                Truth::False
            }
        }
        TemporalNode::Eventually(a) => at(a).or(next_self(Truth::False)),
        TemporalNode::Always(a) => at(a).and(next_self(Truth::True)),
        TemporalNode::Until { left, right } => at(right).or(at(left).and(next_self(Truth::False))),
    }
}
