//! Complete bounded temporal tree/closed-trace payload, including Unknown.
use super::{app, fields, int, integer, list, roles, sym};
use crate::{
    product_correspondence::{reserved, PlanData, ProductError},
    temporal_preservation::TemporalModelInput,
};
use lc631_solvers::temporal::{TemporalNode, TemporalProfile, Truth};
use lc631_substrate::semantic_term::Term;

pub(crate) fn project(
    data: &PlanData,
    input: TemporalModelInput<'_>,
) -> Result<Term, ProductError> {
    if input.query.context() != data.context
        || input.scope != data.scope
        || input.query.profile() != TemporalProfile::ClosedFiniteKleeneV1
        || input.query.nodes().len() > data.limits.max_temporal_nodes
        || input.trace.steps().is_empty()
        || input.trace.steps().len() > data.limits.max_trace_steps
        || input.trace.atom_count() != data.temporal_atom_meanings.len()
    {
        return Err(ProductError::UnsupportedProjection);
    }
    let mut seen = reserved(input.query.nodes().len())?;
    seen.resize(input.query.nodes().len(), false);
    let tree = node(data, input.query.nodes(), input.query.root(), &mut seen, 0)?;
    if seen.iter().any(|seen| !seen) {
        return Err(ProductError::UnsupportedProjection);
    }
    let clock = input.trace.steps()[0].instant().clock();
    let clock_term = roles::clock(data, clock)?;
    let clock_role = roles::validate_clock(data, &clock_term)?;
    let mut steps = reserved(input.trace.steps().len())?;
    let mut previous = None;
    for step in input.trace.steps() {
        let instant = step.instant();
        if instant.clock() != clock
            || step.values().len() != data.temporal_atom_meanings.len()
            || previous.is_some_and(|p| p >= instant.nanoseconds())
        {
            return Err(ProductError::UnsupportedProjection);
        }
        previous = Some(instant.nanoseconds());
        let mut truths = reserved(step.values().len())?;
        for truth in step.values() {
            truths.push(sym(match truth {
                Truth::False => "False",
                Truth::Unknown => "Unknown",
                Truth::True => "True",
            }));
        }
        steps.push(app(
            "step-v1",
            vec![
                int(instant.nanoseconds()),
                int(i128::from(clock_role)),
                app("truths-v1", truths),
            ],
        ));
    }
    Ok(app(
        "temporal-model-v1",
        vec![
            sym("ClosedFiniteKleeneV1"),
            clock_term,
            app("atoms-v1", data.temporal_atom_meanings.clone()),
            tree,
            app("closed-trace-v1", steps),
        ],
    ))
}
fn node(
    data: &PlanData,
    nodes: &[TemporalNode],
    index: u16,
    seen: &mut [bool],
    depth: usize,
) -> Result<Term, ProductError> {
    if depth > data.limits.max_depth {
        return Err(ProductError::UnsupportedProjection);
    }
    let index = usize::from(index);
    let was_seen = seen
        .get_mut(index)
        .ok_or(ProductError::UnsupportedProjection)?;
    if *was_seen {
        return Err(ProductError::UnsupportedProjection);
    }
    *was_seen = true;
    let current = nodes
        .get(index)
        .ok_or(ProductError::UnsupportedProjection)?;
    match *current {
        TemporalNode::Constant(value) => Ok(Term::Bool(value)),
        TemporalNode::Atom(atom) => Ok(app(
            "atom-v1",
            vec![data
                .temporal_atom_meanings
                .get(usize::from(atom))
                .ok_or(ProductError::MissingMeaningDeclaration)?
                .clone()],
        )),
        TemporalNode::Not(a)
        | TemporalNode::StrongNext(a)
        | TemporalNode::Eventually(a)
        | TemporalNode::Always(a) => {
            let name = match current {
                TemporalNode::Not(_) => "not-v1",
                TemporalNode::StrongNext(_) => "strong-next-v1",
                TemporalNode::Eventually(_) => "eventually-v1",
                TemporalNode::Always(_) => "always-v1",
                _ => return Err(ProductError::UnsupportedProjection),
            };
            Ok(app(name, vec![node(data, nodes, a, seen, depth + 1)?]))
        }
        TemporalNode::And(a, b)
        | TemporalNode::Or(a, b)
        | TemporalNode::Until { left: a, right: b } => {
            let name = match current {
                TemporalNode::And(..) => "and-v1",
                TemporalNode::Or(..) => "or-v1",
                TemporalNode::Until { .. } => "until-v1",
                _ => return Err(ProductError::UnsupportedProjection),
            };
            Ok(app(
                name,
                vec![
                    node(data, nodes, a, seen, depth + 1)?,
                    node(data, nodes, b, seen, depth + 1)?,
                ],
            ))
        }
    }
}
pub(crate) fn validate(data: &PlanData, term: &Term) -> Result<(), ProductError> {
    let f = fields(term, "temporal-model-v1", 5)?;
    if f[0] != sym("ClosedFiniteKleeneV1")
        || list(&f[2], "atoms-v1")? != data.temporal_atom_meanings.as_slice()
    {
        return Err(ProductError::UnsupportedProjection);
    }
    let clock_role = roles::validate_clock(data, &f[1])?;
    let mut remaining = data.limits.max_temporal_nodes;
    validate_node(data, &f[3], &mut remaining, 0)?;
    let steps = list(&f[4], "closed-trace-v1")?;
    if steps.is_empty() || steps.len() > data.limits.max_trace_steps {
        return Err(ProductError::UnsupportedProjection);
    }
    let mut previous = None;
    for step in steps {
        let s = fields(step, "step-v1", 3)?;
        let time = integer(&s[0])?;
        if integer(&s[1])? != i128::from(clock_role) || previous.is_some_and(|p| p >= time) {
            return Err(ProductError::IdentityMismatch);
        }
        previous = Some(time);
        let truths = list(&s[2], "truths-v1")?;
        if truths.len() != data.temporal_atom_meanings.len()
            || truths.iter().any(
                |t| !matches!(t,Term::Symbol(s) if matches!(s.as_str(),"False"|"Unknown"|"True")),
            )
        {
            return Err(ProductError::UnsupportedProjection);
        }
    }
    Ok(())
}
fn validate_node(
    data: &PlanData,
    term: &Term,
    remaining: &mut usize,
    depth: usize,
) -> Result<(), ProductError> {
    if *remaining == 0 || depth > data.limits.max_depth {
        return Err(ProductError::UnsupportedProjection);
    }
    *remaining -= 1;
    match term {
        Term::Bool(_) => Ok(()),
        Term::Apply(name, _) if name == "atom-v1" => {
            if !data
                .temporal_atom_meanings
                .contains(&fields(term, "atom-v1", 1)?[0])
            {
                return Err(ProductError::MissingMeaningDeclaration);
            }
            Ok(())
        }
        Term::Apply(name, _)
            if matches!(
                name.as_str(),
                "not-v1" | "strong-next-v1" | "eventually-v1" | "always-v1"
            ) =>
        {
            validate_node(data, &fields(term, name, 1)?[0], remaining, depth + 1)
        }
        Term::Apply(name, _) if matches!(name.as_str(), "and-v1" | "or-v1" | "until-v1") => {
            for child in fields(term, name, 2)? {
                validate_node(data, child, remaining, depth + 1)?;
            }
            Ok(())
        }
        _ => Err(ProductError::UnsupportedProjection),
    }
}
