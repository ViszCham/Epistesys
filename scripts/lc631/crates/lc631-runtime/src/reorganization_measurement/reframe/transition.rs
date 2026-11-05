//! Actual source/state/Delta correspondence, not a generic Replace success flag.
use super::{
    claim, syntax, Cause, Error, FrameProfile, FrameSpec, FrameTransitionInput, ReframePlan,
};
use crate::contextual_distillation::{consumer::revalidate_next_query, ContextualDistillation};
use lc631_core::context::{ContextDelta, ContextState, PremiseBasis, PremiseEdit};
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
pub struct CheckedFrameMap<'p, 'i, 's, 'v> {
    pub(super) plan: &'p ReframePlan<'i, 's, 'v>,
    pub(super) frame: &'i FrameSpec<'i, 's>,
    pub(super) baseline: bool,
    parent: &'i ContextState,
    child: Option<&'i ContextState>,
    delta: Option<&'i ContextDelta>,
    state: &'s ContextualDistillation,
    retained: ResourceClaim<'v>,
}
impl<'p, 'i, 's, 'v> CheckedFrameMap<'p, 'i, 's, 'v> {
    pub fn baseline_current(
        plan: &'p ReframePlan<'i, 's, 'v>,
        r: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        plan.check(r)?;
        let retained = claim(r, plan.limits.retained_ram)?;
        let map = Self {
            plan,
            frame: plan.input.baseline_frame,
            baseline: true,
            parent: plan.input.original.state(),
            child: None,
            delta: None,
            state: plan.input.state,
            retained,
        };
        map.check(r)?;
        Ok(map)
    }
    pub fn produce_exact(
        plan: &'p ReframePlan<'i, 's, 'v>,
        input: FrameTransitionInput<'i, 's>,
        r: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        plan.check(r)?;
        let retained = claim(r, plan.limits.retained_ram)?;
        let actual = input
            .child
            .declared_delta()
            .ok_or(Error::FrameDeltaMismatch)?;
        if !std::ptr::eq(actual, input.delta) {
            return Err(Error::ForeignDeltaOwner);
        }
        let map = Self {
            plan,
            frame: input.frame,
            baseline: false,
            parent: input.parent,
            child: Some(input.child),
            delta: Some(input.delta),
            state: input.state,
            retained,
        };
        map.check(r)?;
        Ok(map)
    }
    pub(super) fn check(&self, r: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !self.retained.same_reservation(r) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check()?;
        self.plan.check(r)?;
        if !std::ptr::eq(self.parent, self.plan.input.original.state())
            || !std::ptr::eq(self.state, self.plan.input.state)
        {
            return Err(Error::FrameDeltaMismatch);
        }
        if !self.baseline {
            let child = self.child.ok_or(Error::FrameDeltaMismatch)?;
            let d = self.delta.ok_or(Error::FrameDeltaMismatch)?;
            if !std::ptr::eq(child.declared_delta().ok_or(Error::FrameDeltaMismatch)?, d) {
                return Err(Error::ForeignDeltaOwner);
            }
            if d.parent() != self.parent.revision()
                || child.parent() != Some(self.parent.revision())
                || d.scope() != self.parent.scope()
                || child.scope() != self.parent.scope()
                || d.authority() != self.plan.input.original.authority()
            {
                return Err(Error::FrameDeltaMismatch);
            }
            if d.edits().len() != 1 {
                return Err(Error::FrameDeltaMismatch);
            }
            match &d.edits()[0] {
                PremiseEdit::Replace {
                    id,
                    statement,
                    basis,
                } if *id == self.plan.input.frame_premise
                    && statement.as_ref() == self.frame.declaration
                    && *basis == PremiseBasis::Proposed => {}
                _ => return Err(Error::FrameDeltaMismatch),
            }
            if child.premises().len() != self.parent.premises().len() {
                return Err(Error::FrameDeltaMismatch);
            }
            for before in self.parent.premises() {
                let after = child
                    .premises()
                    .iter()
                    .find(|p| p.id() == before.id())
                    .ok_or(Error::FrameDeltaMismatch)?;
                if before.id() == self.plan.input.frame_premise {
                    if !before.changeable()
                        || after.statement() != self.frame.declaration
                        || after.basis() != PremiseBasis::Proposed
                    {
                        return Err(Error::FrameDeltaMismatch);
                    }
                } else if before != after || before.source_span() != after.source_span() {
                    return Err(Error::FrameDeltaMismatch);
                }
            }
        }
        validate_frame(self.plan, self.frame, self.state)?;
        self.retained.check()?;
        r.check()?;
        Ok(())
    }
    pub(super) fn context(&self) -> &ContextState {
        self.child.unwrap_or(self.parent)
    }
}
pub(super) fn validate_frame(
    plan: &ReframePlan<'_, '_, '_>,
    frame: &FrameSpec<'_, '_>,
    state: &ContextualDistillation,
) -> Result<(), Error> {
    if !plan.input.family.supports(frame.profile) {
        return Err(Error::Unsupported(Cause::UnsupportedProfile));
    }
    plan.charge((frame.declaration.len() + frame.macros.len() + 1) as u64)?;
    let decl = syntax::declaration(frame.declaration, plan.input.base.node_count())?;
    if decl.profile != frame.profile
        || decl.entries.len() != frame.macros.len()
        || (frame.profile == FrameProfile::DirectEdgeV1 && !frame.macros.is_empty())
    {
        return Err(Error::FramePayloadMismatch);
    }
    for (i, m) in frame.macros.iter().enumerate() {
        if frame.macros[..i].iter().any(|p| p.edge == m.edge) {
            return Err(Error::FramePayloadMismatch);
        }
        let path = m.path.ok_or(Error::MissingPathOwner)?;
        if !std::ptr::eq(path.backing_state(), state) || !std::ptr::eq(state, plan.input.state) {
            return Err(Error::ForeignPathState);
        }
        let nodes = path.nodes();
        if nodes.len() < 3
            || nodes.len() > 16
            || nodes.first().map(|n| u16::from(n.0)) != Some(m.edge.0 .0)
            || nodes.last().map(|n| u16::from(n.0)) != Some(m.edge.1 .0)
        {
            return Err(Error::MapBackEndpoints);
        }
        let recheck_bound = u64::from(state.basis().node_count())
            .checked_mul(state.basis().edges().len() as u64)
            .ok_or(Error::ArithmeticOverflow)?;
        plan.charge(recheck_bound)?;
        let checked =
            revalidate_next_query(state, path.request(), plan.limits.max_root_work as u32)
                .map_err(Error::Relation)?;
        let fresh = crate::contextual_distillation::consumer::consume_dependency_path(checked)
            .map_err(Error::Relation)?;
        if nodes != fresh.nodes() {
            return Err(Error::FramePayloadMismatch);
        }
        let (e, decl_nodes) = decl
            .entries
            .iter()
            .find(|(e, _)| *e == m.edge)
            .ok_or(Error::FramePayloadMismatch)?;
        if *e != m.edge
            || decl_nodes.len() != nodes.len()
            || decl_nodes
                .iter()
                .zip(nodes)
                .any(|(a, b)| *a != u16::from(b.0))
        {
            return Err(Error::FramePayloadMismatch);
        }
        for pair in nodes.windows(2) {
            plan.charge(1)?;
            if !plan
                .input
                .base
                .edges()
                .iter()
                .any(|e| e.0 .0 == u16::from(pair[0].0) && e.1 .0 == u16::from(pair[1].0))
            {
                return Err(Error::FramePayloadMismatch);
            }
        }
    }
    Ok(())
}
