//! Actual temporal indices/root/trace and complete rectangles; no semantic lowering.
use super::*;
use crate::{
    spatial_preservation::{SpatialModelInput, SpatialUnit},
    temporal_preservation::TemporalModelInput,
};
use lc631_solvers::temporal::{TemporalNode, TemporalProfile, TimeScale, Truth};
pub(in crate::product_cost::native) fn temporal(
    data: &PlanData,
    input: TemporalModelInput<'_>,
    r: &Registry,
) -> Result<Term, CostError> {
    if input.query.context() != data.context
        || input.scope != data.scope
        || input.query.profile() != TemporalProfile::ClosedFiniteKleeneV1
        || input.query.nodes().len() > data.limits.max_temporal_nodes
        || input.trace.steps().is_empty()
        || input.trace.steps().len() > data.limits.max_trace_steps
        || input.trace.atom_count() != data.temporal_atom_meanings.len()
    {
        return Err(CostError::UnsupportedProfile);
    }
    let clock = input.trace.steps()[0].instant().clock();
    if clock.precision_nanoseconds() != 0 {
        return Err(CostError::UnsupportedProfile);
    }
    let mut nodes = Vec::new();
    for n in input.query.nodes() {
        let (name, children) = match *n {
            TemporalNode::Constant(b) => ("time-constant-v1", vec![Term::Bool(b)]),
            TemporalNode::Atom(i) => ("time-atom-v1", vec![int(i as i128)]),
            TemporalNode::Not(i) => ("time-not-v1", vec![int(i as i128)]),
            TemporalNode::StrongNext(i) => ("time-strong-next-v1", vec![int(i as i128)]),
            TemporalNode::Eventually(i) => ("time-eventually-v1", vec![int(i as i128)]),
            TemporalNode::Always(i) => ("time-always-v1", vec![int(i as i128)]),
            TemporalNode::And(a, b) => ("time-and-v1", vec![int(a as i128), int(b as i128)]),
            TemporalNode::Or(a, b) => ("time-or-v1", vec![int(a as i128), int(b as i128)]),
            TemporalNode::Until { left: a, right: b } => {
                ("time-until-v1", vec![int(a as i128), int(b as i128)])
            }
        };
        nodes.push(app(name, children));
    }
    let mut steps = Vec::new();
    let mut previous = None;
    for step in input.trace.steps() {
        let at = step.instant();
        if at.clock() != clock
            || step.values().len() != data.temporal_atom_meanings.len()
            || previous.is_some_and(|p| p >= at.nanoseconds())
        {
            return Err(CostError::UnsupportedProfile);
        }
        previous = Some(at.nanoseconds());
        let values = step
            .values()
            .iter()
            .map(|v| {
                sym(match v {
                    Truth::False => "False",
                    Truth::Unknown => "Unknown",
                    Truth::True => "True",
                })
            })
            .collect();
        steps.push(app(
            "step-fields-v1",
            vec![
                int(at.nanoseconds()),
                r.existing("Clock", at.clock().clock().as_bytes())?,
                app("truths-v1", values),
            ],
        ));
    }
    let scale = match clock.scale() {
        TimeScale::Monotonic => "Monotonic",
        TimeScale::Utc => "Utc",
        TimeScale::Tai => "Tai",
    };
    let clock = app(
        "clock-fields-v1",
        vec![
            r.existing("Clock", clock.clock().as_bytes())?,
            sym(scale),
            r.existing("Origin", clock.origin().as_bytes())?,
            r.existing("Reference", clock.reference().as_bytes())?,
            int(0),
        ],
    );
    Ok(app(
        "temporal-fields-v1",
        vec![
            sym("ClosedFiniteKleeneV1"),
            app("nodes-v1", nodes),
            int(input.query.root() as i128),
            // FiniteTrace::checked rejects OpenPrefix/LoopTo; no hidden open state exists.
            int(input.trace.atom_count() as i128),
            Term::Bool(true),
            clock,
            app("steps-fields-v1", steps),
        ],
    ))
}
pub(in crate::product_cost::native) fn spatial(
    data: &PlanData,
    input: SpatialModelInput,
    r: &Registry,
) -> Result<Term, CostError> {
    if input.context != data.context || input.scope != data.scope {
        return Err(CostError::CorrespondenceMismatch);
    }
    let rect = |v: Option<lc631_solvers::causal_spatial::Rectangle>| -> Result<Term, CostError> {
        match v {
            None => Ok(app("none-v1", vec![])),
            Some(v) => {
                if v.frame() != input.frame {
                    return Err(CostError::CorrespondenceMismatch);
                }
                let a = v.min();
                let b = v.max();
                Ok(app(
                    "some-v1",
                    vec![app(
                        "rectangle-fields-v1",
                        vec![
                            r.existing("Frame", &v.frame().0)?,
                            int(a[0] as i128),
                            int(a[1] as i128),
                            int(b[0] as i128),
                            int(b[1] as i128),
                        ],
                    )],
                ))
            }
        }
    };
    Ok(app(
        "spatial-fields-v1",
        vec![
            r.existing("Frame", &input.frame.0)?,
            sym(match input.unit {
                SpatialUnit::Pixel => "Pixel",
                SpatialUnit::Millimeter => "Millimeter",
            }),
            rect(input.left)?,
            rect(input.right)?,
        ],
    ))
}
