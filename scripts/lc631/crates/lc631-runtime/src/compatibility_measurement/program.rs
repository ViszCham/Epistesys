//! Actual bounded copy/permutation programs; no inverse-equality or status shortcut.
use super::*;

pub struct Program<'p, 'i, 'v> {
    pub(super) plan: &'p Plan<'i, 'v>,
    pub(super) forward: [PortMap; 3],
    pub(super) inverse: [PortMap; 3],
    forward_moves: [usize; 3],
    inverse_moves: [usize; 3],
    retained: ResourceClaim<'v>,
}
impl<'p, 'i, 'v> Program<'p, 'i, 'v> {
    pub(super) fn checked(
        plan: &'p Plan<'i, 'v>,
        forward: &[PortMap],
        inverse: &[PortMap],
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        plan.check(view)?;
        if plan.limits.max_program_steps < 3 {
            return Err(Error::ProgramBudget);
        }
        let retained = claim(view, 64 * 1024, 0)?;
        let source = plan.input.source_theory.boolean_symbols();
        let target = plan.input.target_theory.boolean_symbols();
        let (forward, forward_moves) = mapping(source, target, forward)?;
        let (inverse, inverse_moves) = mapping(target, source, inverse)?;
        retained.check().map_err(Error::Resource)?;
        Ok(Self {
            plan,
            forward,
            inverse,
            forward_moves,
            inverse_moves,
            retained,
        })
    }
    pub(super) fn check(
        &self,
        plan: &Plan<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.plan.check(view)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(view) {
            return Err(Error::ForeignReservation);
        }
        if !same_owner(plan, self.plan) {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
    /// Complete actual occurrence references and original native payload.
    pub(crate) fn inspect_composite_current<R>(
        &self,
        expected_plan: &Plan<'i, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'s> FnOnce(
            &'s Plan<'i, 'v>,
            &'s Program<'p, 'i, 'v>,
            PlanInput<'i>,
            &'s [PortMap; 3],
            &'s [PortMap; 3],
        ) -> R,
    ) -> Result<R, Error> {
        self.check(expected_plan, view)?;
        let result = consume(
            self.plan,
            self,
            self.plan.input,
            &self.forward,
            &self.inverse,
        );
        self.check(expected_plan, view)?;
        Ok(result)
    }

    pub(super) fn forward_frame(&self, frame: &Frame) -> Frame {
        execute(
            frame,
            self.plan.input.target_theory.boolean_symbols(),
            self.forward_moves,
        )
    }
    pub(super) fn inverse_frame(&self, frame: &Frame) -> Frame {
        execute(
            frame,
            self.plan.input.source_theory.boolean_symbols(),
            self.inverse_moves,
        )
    }
}
pub(super) fn mapping(
    source: &[BoolSymbolId],
    target: &[BoolSymbolId],
    maps: &[PortMap],
) -> Result<([PortMap; 3], [usize; 3]), Error> {
    if source.len() != 3 || target.len() != 3 || maps.len() != 3 {
        return Err(Error::InvalidInput);
    }
    let mut canonical = [maps[0]; 3];
    let mut moves = [0usize; 3];
    let mut source_seen = [false; 3];
    let mut target_seen = [false; 3];
    for pair in maps {
        let from = source
            .iter()
            .position(|s| *s == pair.source)
            .ok_or(Error::InvalidInput)?;
        let to = target
            .iter()
            .position(|s| *s == pair.target)
            .ok_or(Error::InvalidInput)?;
        if source_seen[from] || target_seen[to] {
            return Err(Error::InvalidInput);
        }
        source_seen[from] = true;
        target_seen[to] = true;
        canonical[from] = *pair;
        moves[to] = from;
    }
    Ok((canonical, moves))
}
fn execute(input: &Frame, target: &[BoolSymbolId], moves: [usize; 3]) -> Frame {
    // Checked program construction guarantees all three indices are in 0..3.
    std::array::from_fn(|to| Assignment {
        symbol: target[to],
        value: input[moves[to]].value,
    })
}
