//! 日本語: 単一時点Boolean形の厳格照合とtrace由来modelの既存再検査。
//! English: Exact one-point Boolean shape matching and existing trace-model rechecking.
use super::*;
use lc631_solvers::{
    protocol::sexpr::{self, SExprLimits},
    smt::validate_model,
    temporal::TemporalNode,
    typed_query::BoolExpr,
};
use std::io::Write;

pub(super) struct Work {
    limit: u64,
    used: u64,
}
impl Work {
    pub(super) fn new(limit: u64) -> Self {
        Self { limit, used: 0 }
    }
    pub(super) fn used(&self) -> u64 {
        self.used
    }
    fn visit(&mut self, depth: usize) -> Result<(), PointError> {
        if depth > MAX_DEPTH {
            return Err(PointError::Unsupported(UnsupportedPoint::DepthLimit));
        }
        if self.used >= self.limit {
            return Err(PointError::WorkBudget);
        }
        self.used += 1;
        Ok(())
    }
}
pub(super) fn admit(request: PointRequest<'_, '_, '_>, work: &mut Work) -> Result<(), PointError> {
    let native = request.native;
    let target = request.candidate.target_query();
    let probe = request
        .plan
        .fixed_probes()
        .get(request.probe)
        .ok_or(PointError::MissingProbe)?;
    if probe.template.unconstrained == crate::formal_boolean::UnconstrainedPolicy::MissingScope
        && (probe.template.source.assertions().is_empty() || target.assertions().is_empty())
    {
        return Err(PointError::NeedsEvidence(MissingPoint::MissingTheoryScope));
    }
    if native.trace.steps().len() != 1 {
        return Err(PointError::Unsupported(UnsupportedPoint::MultipleSteps));
    }
    if native.trace.atom_count() == 0
        || native.trace.atom_count() > MAX_ATOMS
        || target.boolean_symbols().is_empty()
        || target.boolean_symbols().len() > MAX_ATOMS
    {
        return Err(PointError::Unsupported(UnsupportedPoint::AtomLimit));
    }
    if native.query.nodes().len() > MAX_TEMPORAL_NODES {
        return Err(PointError::Unsupported(UnsupportedPoint::TemporalNodeLimit));
    }
    if native.trace.steps()[0]
        .instant()
        .clock()
        .precision_nanoseconds()
        != 0
    {
        return Err(PointError::Unsupported(UnsupportedPoint::InexactClock));
    }
    if !target.integer_symbols().is_empty() {
        return Err(PointError::Unsupported(UnsupportedPoint::TheoryFragment));
    }
    if request.map.len() != native.trace.atom_count()
        || request.map.len() != target.boolean_symbols().len()
    {
        return Err(PointError::Unsupported(UnsupportedPoint::IncompleteMap));
    }
    for (i, pair) in request.map.iter().enumerate() {
        if usize::from(pair.temporal) >= native.trace.atom_count()
            || target
                .boolean_symbols()
                .binary_search(&pair.formal)
                .is_err()
            || request.map[..i]
                .iter()
                .any(|p| p.temporal == pair.temporal || p.formal == pair.formal)
        {
            return Err(PointError::InvalidMap);
        }
    }
    for node in native.query.nodes() {
        work.visit(0)?;
        match node {
            TemporalNode::Constant(_)
            | TemporalNode::Atom(_)
            | TemporalNode::Not(_)
            | TemporalNode::And(_, _)
            | TemporalNode::Or(_, _) => {}
            TemporalNode::StrongNext(_)
            | TemporalNode::Eventually(_)
            | TemporalNode::Always(_)
            | TemporalNode::Until { .. } => {
                return Err(PointError::Unsupported(UnsupportedPoint::TemporalOperator))
            }
        }
    }
    for assertion in target.assertions() {
        boolean_fragment(assertion, work, 0)?;
    }
    let predicate = request
        .plan
        .fixed_probes()
        .get(request.probe)
        .ok_or(PointError::MissingProbe)?
        .template
        .target_predicate;
    boolean_fragment(predicate, work, 0)?;
    if !same_shape(native.query.root(), predicate, request, work, 0)? {
        return Err(PointError::PredicateMismatch);
    }
    // A defined point assignment needs every mapped observation. A determinate
    // root alone cannot fill unknown inputs needed by the SMT theory.
    for pair in request.map {
        if native.trace.steps()[0].values()[usize::from(pair.temporal)] == Truth::Unknown {
            return Err(PointError::NeedsEvidence(MissingPoint::UnknownAtom(
                pair.temporal,
            )));
        }
    }
    Ok(())
}
fn boolean_fragment(expr: &BoolExpr, work: &mut Work, depth: usize) -> Result<(), PointError> {
    work.visit(depth)?;
    match expr {
        BoolExpr::Constant(_) | BoolExpr::Variable(_) => Ok(()),
        BoolExpr::Not(child) => boolean_fragment(child, work, depth + 1),
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            if children.is_empty() {
                return Err(PointError::Unsupported(UnsupportedPoint::TheoryFragment));
            }
            for child in children {
                boolean_fragment(child, work, depth + 1)?;
            }
            Ok(())
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            Err(PointError::Unsupported(UnsupportedPoint::TheoryFragment))
        }
    }
}
fn same_shape(
    node: u16,
    expr: &BoolExpr,
    request: PointRequest<'_, '_, '_>,
    work: &mut Work,
    depth: usize,
) -> Result<bool, PointError> {
    work.visit(depth)?;
    let actual = request
        .native
        .query
        .nodes()
        .get(usize::from(node))
        .ok_or(PointError::PredicateMismatch)?;
    match (actual, expr) {
        (TemporalNode::Constant(a), BoolExpr::Constant(b)) => Ok(a == b),
        (TemporalNode::Atom(atom), BoolExpr::Variable(symbol)) => Ok(request
            .map
            .iter()
            .any(|pair| pair.temporal == *atom && pair.formal == *symbol)),
        (TemporalNode::Not(a), BoolExpr::Not(b)) => same_shape(*a, b, request, work, depth + 1),
        (TemporalNode::And(a, b), BoolExpr::And(children))
        | (TemporalNode::Or(a, b), BoolExpr::Or(children))
            if children.len() == 2 =>
        {
            let left = same_shape(*a, &children[0], request, work, depth + 1)?;
            let right = same_shape(*b, &children[1], request, work, depth + 1)?;
            Ok(left && right)
        }
        _ => Ok(false), // Shape mismatch is never a zero loss or semantic equivalence.
    }
}
pub(super) fn model<'input>(
    request: PointRequest<'_, 'input, '_>,
) -> Result<(Vec<u8>, CheckedModel<'input>), PointError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_MODEL_BYTES)
        .map_err(|_| PointError::AllocationFailed)?;
    if bytes.capacity() > MAX_MODEL_BYTES {
        return Err(PointError::AllocationFailed);
    }
    bytes.push(b'(');
    for symbol in request.candidate.target_query().boolean_symbols() {
        let pair = request
            .map
            .iter()
            .find(|p| p.formal == *symbol)
            .ok_or(PointError::InvalidMap)?;
        let value = match request.native.trace.steps()[0].values()[usize::from(pair.temporal)] {
            Truth::True => "true",
            Truth::False => "false",
            Truth::Unknown => {
                return Err(PointError::NeedsEvidence(MissingPoint::UnknownAtom(
                    pair.temporal,
                )))
            }
        };
        // <=8 symbols, <=10 decimal digits/id: payload is <512 bytes. The
        // full 1024-byte buffer was claimed/reserved before any formatting.
        write!(
            &mut bytes,
            "(define-fun b{} () Bool {})",
            symbol.get(),
            value
        )
        .map_err(|_| PointError::ModelEncoding)?;
    }
    bytes.push(b')');
    if bytes.len() > MAX_MODEL_BYTES {
        return Err(PointError::ModelEncoding);
    }
    let limits = SExprLimits::checked(MAX_MODEL_BYTES, 128, 4).map_err(PointError::ModelSyntax)?;
    let document = sexpr::parse(&bytes, limits).map_err(PointError::ModelSyntax)?;
    let checked = validate_model(request.candidate.target_query(), &document)
        .map_err(PointError::NotModel)?;
    Ok((bytes, checked))
}
pub(super) fn evaluate(
    expr: &BoolExpr,
    model: &CheckedModel<'_>,
    work: &mut Work,
    depth: usize,
) -> Result<bool, PointError> {
    work.visit(depth)?;
    match expr {
        BoolExpr::Constant(v) => Ok(*v),
        BoolExpr::Variable(id) => model.boolean(*id).ok_or(PointError::InvalidMap),
        BoolExpr::Not(child) => Ok(!evaluate(child, model, work, depth + 1)?),
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            let conjunction = matches!(expr, BoolExpr::And(_));
            let mut value = conjunction;
            for child in children {
                let next = evaluate(child, model, work, depth + 1)?;
                if conjunction {
                    value &= next;
                } else {
                    value |= next;
                }
            }
            Ok(value)
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            Err(PointError::Unsupported(UnsupportedPoint::TheoryFragment))
        }
    }
}
