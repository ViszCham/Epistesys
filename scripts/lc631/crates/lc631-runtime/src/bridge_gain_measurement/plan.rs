//! Fixed endpoint/checker admission and actual programs; one existing C work series.
use super::*;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Role {
    Baseline,
    Candidate,
}
pub struct BridgeRoot<'i, 'v> {
    pub(super) native: c::Plan<'i, 'v>,
    pub(super) input: c::PlanInput<'i>,
    baseline: ProgramSpec<'i>,
    pub(super) order: [usize; 4],
    pub(super) use_symbol: BoolSymbolId,
    retained: ResourceClaim<'v>,
}
pub struct BridgeProgram<'p, 'i, 'v> {
    pub(super) native: c::Program<'p, 'i, 'v>,
    pub(super) root: &'p BridgeRoot<'i, 'v>,
    pub(super) role: Role,
    retained: ResourceClaim<'v>,
}
impl<'i, 'v> BridgeRoot<'i, 'v> {
    pub fn given(
        input: BridgeRootInput<'i>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, BridgeError> {
        view.check().map_err(BridgeError::Resource)?;
        match input.endpoints {
            EndpointPair::InputRegisters3ToControlPorts3V1 => {}
        }
        if !(1..=48).contains(&input.limits.max_work) {
            return Err(BridgeError::InvalidLimits);
        }
        // These bind this closed executable checker version, not caller-supplied oracle code.
        if input.compatibility.pins.definition != Digest32::from_bytes([0xb1; 32])
            || input.compatibility.pins.oracle != Digest32::from_bytes([0xb2; 32])
        {
            return Err(BridgeError::OracleMismatch);
        }
        let native = c::Plan::given(
            input.compatibility,
            c::Limits {
                max_work: input.limits.max_work,
                max_frame_bytes: 1024,
                max_program_steps: 3,
            },
            view,
        )
        .map_err(from_c)?;
        let retained = claim(view, 64 * 1024)?;
        // Validate the frozen executable/inverse now through the existing C
        // constructor. This temporary owns no work and drops before root moves.
        {
            let _baseline = native
                .permutation(
                    input.frozen_baseline.forward,
                    input.frozen_baseline.inverse,
                    view,
                )
                .map_err(from_c)?;
        }
        let (use_symbol, order) = canonical_order(input.compatibility)?;
        check_claim(&retained, view)?;
        Ok(Self {
            native,
            input: input.compatibility,
            baseline: input.frozen_baseline,
            order,
            use_symbol,
            retained,
        })
    }
    pub fn baseline_program<'p>(
        &'p self,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<BridgeProgram<'p, 'i, 'v>, BridgeError> {
        self.program(self.baseline, Role::Baseline, view)
    }
    pub fn candidate_program<'p>(
        &'p self,
        spec: ProgramSpec<'_>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<BridgeProgram<'p, 'i, 'v>, BridgeError> {
        self.program(spec, Role::Candidate, view)
    }
    fn program<'p>(
        &'p self,
        spec: ProgramSpec<'_>,
        role: Role,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<BridgeProgram<'p, 'i, 'v>, BridgeError> {
        self.check(view)?;
        let native = self
            .native
            .permutation(spec.forward, spec.inverse, view)
            .map_err(from_c)?;
        let retained = claim(view, 16 * 1024)?;
        self.check(view)?;
        Ok(BridgeProgram {
            native,
            root: self,
            role,
            retained,
        })
    }
    pub fn work_used_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<u64, BridgeError> {
        self.check(view)?;
        self.native.work_used_current(view).map_err(from_c)
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), BridgeError> {
        check_claim(&self.retained, view)?;
        self.native.work_used_current(view).map_err(from_c)?;
        check_claim(&self.retained, view)
    }
}
impl BridgeProgram<'_, '_, '_> {
    pub(super) fn check(
        &self,
        root: &BridgeRoot<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), BridgeError> {
        root.check(view)?;
        if !std::ptr::eq(root, self.root) {
            return Err(BridgeError::BindingMismatch);
        }
        check_claim(&self.retained, view)
    }
}
impl<'p, 'i, 'v> BridgeProgram<'p, 'i, 'v> {
    /// Borrow the exact privately owned C plan/program, not an equal replica.
    pub(crate) fn inspect_composite_current<R>(
        &self,
        root: &BridgeRoot<'i, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'s> FnOnce(
            &'s c::Plan<'i, 'v>, &'s c::Program<'p, 'i, 'v>, c::PlanInput<'i>,
            &'s [c::PortMap; 3], &'s [c::PortMap; 3],
        ) -> R,
    ) -> Result<R, BridgeError> {
        self.check(root, view)?;
        let result = self.native
            .inspect_composite_current(&root.native, view, consume)
            .map_err(from_c)?;
        self.check(root, view)?;
        Ok(result)
    }
}

fn canonical_order(input: c::PlanInput<'_>) -> Result<(BoolSymbolId, [usize; 4]), BridgeError> {
    let [BoolExpr::Variable(p)] = input.source_use.assertions() else {
        return Err(BridgeError::Compatibility(c::Error::Unsupported(
            c::Unsupported::UseFormula,
        )));
    };
    let q = input
        .source_theory
        .boolean_symbols()
        .iter()
        .copied()
        .find(|s| *s != input.source_hard && s != p)
        .ok_or(BridgeError::BindingMismatch)?;
    let mut order = [0; 4];
    let mut seen = [false; 4];
    for (case, probe) in input.cases.iter().enumerate() {
        let value = |symbol| {
            probe
                .input
                .iter()
                .find(|a| a.symbol == symbol)
                .map(|a| a.value)
                .ok_or(BridgeError::BindingMismatch)
        };
        let canonical = 2 * usize::from(value(*p)?) + usize::from(value(q)?);
        if seen[canonical] {
            return Err(BridgeError::Compatibility(c::Error::InvalidInput));
        }
        seen[canonical] = true;
        order[canonical] = case;
    }
    if seen != [true; 4] {
        return Err(BridgeError::Compatibility(c::Error::InvalidInput));
    }
    Ok((*p, order))
}
