//! Actual composition and root-use ASTs are built here, never caller results.
use super::{claim, finite, vector, Error, MapBackInput, RebindPlan, RoleModel, UnitId, UseExpr};
use lc631_core::CandidateId;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};
use sha2::{Digest, Sha256};

pub(super) struct UnitProgram {
    pub unit: UnitId,
    pub composition: BoolExpr,
    pub root_use: BoolExpr,
    pub map_at_false: BoolExpr,
    pub map_at_true: BoolExpr,
}
pub struct PreparedRebindProgram<'p, 'i, 'v> {
    pub(super) plan: &'p RebindPlan<'i, 'v>,
    pub(super) candidate: &'p RoleModel<'i>,
    pub(super) maps: &'p [MapBackInput<'i>],
    pub(super) id: CandidateId,
    pub(super) units: Vec<UnitProgram>,
    pub(super) digest: Digest32,
    pub(super) symbol_map: [(BoolSymbolId, BoolSymbolId); 2],
    pub(super) symbol_count: usize,
    retained: ResourceClaim<'v>,
}
impl<'p, 'i, 'v> PreparedRebindProgram<'p, 'i, 'v> {
    pub fn prepare(
        plan: &'p RebindPlan<'i, 'v>,
        candidate: &'p RoleModel<'i>,
        maps: &'p [MapBackInput<'i>],
        id: CandidateId,
        resources: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        plan.check(resources)?;
        let retained = claim(resources, plan.limits.program_ram)?;
        if id.0 == 0 {
            return Err(Error::InvalidCandidate);
        }
        if !std::ptr::eq(candidate.source, plan.baseline.source) {
            return Err(Error::ForeignSourceOwner);
        }
        if !std::ptr::eq(candidate.theory, plan.baseline.theory)
            || candidate.context != plan.baseline.context
        {
            return Err(Error::TheoryMismatch);
        }
        if candidate.bindings.len() != plan.probes.len()
            || plan
                .probes
                .iter()
                .any(|p| candidate.binding(p.unit).is_err())
        {
            return Err(Error::UnitSetMismatch);
        }
        if maps.len() != plan.probes.len()
            || maps.iter().enumerate().any(|(i, m)| {
                maps[..i].iter().any(|x| x.unit == m.unit)
                    || !plan.probes.iter().any(|p| p.unit == m.unit)
            })
        {
            return Err(Error::MapSetMismatch);
        }
        plan.charge(candidate.nodes)?;
        let storage = plan
            .probes
            .len()
            .checked_mul(std::mem::size_of::<UnitProgram>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .and_then(|n| n.checked_add(plan.limits.max_payload_bytes))
            .ok_or(Error::ArithmeticOverflow)?;
        if storage as u64 > plan.limits.program_ram {
            return Err(Error::PayloadLimit);
        }
        let mut units = vector(plan.probes.len())?;
        let mut total = 0u64;
        for p in plan.probes {
            let b = candidate.binding(p.unit)?;
            if b.parameter != plan.baseline.binding(p.unit)?.parameter {
                return Err(Error::ParameterMismatch);
            }
            let m = maps
                .iter()
                .find(|m| m.unit == p.unit)
                .ok_or(Error::MapSetMismatch)?;
            match candidate.use_input(p.unit)?.body {
                UseExpr::MappedUnit(u) if *u == p.unit => {}
                UseExpr::MappedUnit(_) => return Err(Error::UseUnitMismatch),
                UseExpr::RawInput(_) => return Err(Error::MissingActualUse),
            }
            // Conservative registered admission charge BEFORE scanning an
            // untrusted map AST. Work stays spent if shape admission fails.
            plan.charge(8 * plan.limits.max_ast_nodes)?;
            let rn = finite::scan(
                b.body,
                &[b.parameter],
                plan.limits.max_ast_nodes,
                plan.limits.max_ast_depth,
            )?;
            let mn = finite::scan(
                m.body,
                &[m.parameter],
                plan.limits.max_ast_nodes,
                plan.limits.max_ast_depth,
            )?;
            let expanded = finite::expanded_nodes(m.body, m.parameter, rn)?;
            if expanded > plan.limits.max_ast_nodes
                || finite::expanded_depth(m.body, m.parameter, finite::depth(b.body))
                    > plan.limits.max_ast_depth
            {
                return Err(Error::ShapeLimit);
            }
            let owned_nodes = expanded
                .checked_mul(2)
                .and_then(|n| mn.checked_mul(2).and_then(|m| n.checked_add(m)))
                .ok_or(Error::ArithmeticOverflow)?;
            total = total
                .checked_add(owned_nodes)
                .ok_or(Error::ArithmeticOverflow)?;
            if total.checked_mul(48).ok_or(Error::ArithmeticOverflow)?
                > plan.limits.max_payload_bytes as u64
            {
                return Err(Error::PayloadLimit);
            }
            let extra_work = mn.checked_mul(8).ok_or(Error::ArithmeticOverflow)?;
            plan.charge(
                (4 * (rn + mn) + 8 * expanded)
                    .checked_add(extra_work)
                    .ok_or(Error::ArithmeticOverflow)?,
            )?;
            let composition = finite::substitute(m.body, Some((m.parameter, b.body)))?;
            finite::scan(
                &composition,
                candidate.theory.boolean_symbols(),
                plan.limits.max_ast_nodes,
                plan.limits.max_ast_depth,
            )?;
            // Lowering MappedUnit executes the same composition; no unrelated proof path.
            let root_use = lower_use(candidate.use_input(p.unit)?.body, p.unit, &composition)?;
            // Counterfactual input values are registered Bool-domain artifacts,
            // not caller proof flags. Preserve AST shape; do not constant-fold.
            let map_at_false =
                finite::substitute(m.body, Some((m.parameter, &BoolExpr::Constant(false))))?;
            let map_at_true =
                finite::substitute(m.body, Some((m.parameter, &BoolExpr::Constant(true))))?;
            units.push(UnitProgram {
                unit: p.unit,
                composition,
                root_use,
                map_at_false,
                map_at_true,
            });
        }
        let mut h = Sha256::new();
        h.update(b"epistesys.rebind.prepared-program.v2-input-dependence\0");
        h.update(plan.digest.as_bytes());
        h.update(candidate.digest.as_bytes());
        h.update(id.0.to_le_bytes());
        h.update((units.len() as u64).to_le_bytes());
        for u in &units {
            h.update(u.unit.0.to_le_bytes());
            finite::expression(&mut h, &u.composition);
            finite::expression(&mut h, &u.root_use);
            finite::expression(&mut h, &u.map_at_false);
            finite::expression(&mut h, &u.map_at_true);
            let map = maps
                .iter()
                .find(|m| m.unit == u.unit)
                .ok_or(Error::MapSetMismatch)?;
            h.update(map.parameter.get().to_le_bytes());
            finite::expression(&mut h, map.body);
        }
        let symbol_count = plan.baseline.theory.boolean_symbols().len();
        let mut symbol_map = [(BoolSymbolId::new(0), BoolSymbolId::new(0)); 2];
        for (slot, symbol) in symbol_map
            .iter_mut()
            .zip(plan.baseline.theory.boolean_symbols())
        {
            *slot = (*symbol, *symbol);
        }
        let program = Self {
            plan,
            candidate,
            maps,
            id,
            units,
            digest: finite::finish(h),
            symbol_map,
            symbol_count,
            retained,
        };
        program.check(resources)?;
        Ok(program)
    }
    pub fn composition(&self, unit: UnitId) -> Result<&BoolExpr, Error> {
        self.units
            .iter()
            .find(|u| u.unit == unit)
            .map(|u| &u.composition)
            .ok_or(Error::UnitSetMismatch)
    }
    pub fn root_use(&self, unit: UnitId) -> Result<&BoolExpr, Error> {
        self.units
            .iter()
            .find(|u| u.unit == unit)
            .map(|u| &u.root_use)
            .ok_or(Error::UnitSetMismatch)
    }
    pub(super) fn input_pair(&self, unit: UnitId) -> Result<(&BoolExpr, &BoolExpr), Error> {
        let u = self
            .units
            .iter()
            .find(|u| u.unit == unit)
            .ok_or(Error::UnitSetMismatch)?;
        Ok((&u.map_at_false, &u.map_at_true))
    }
    pub(super) fn map_input(&self, unit: UnitId) -> Result<&MapBackInput<'i>, Error> {
        self.maps
            .iter()
            .find(|m| m.unit == unit)
            .ok_or(Error::MapSetMismatch)
    }
    pub(super) fn check(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !self.retained.same_reservation(resources) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check()?;
        self.plan.check(resources)?;
        if self.maps.len() != self.units.len() || self.candidate.bindings.len() != self.units.len()
        {
            return Err(Error::UnitSetMismatch);
        }
        Ok(())
    }
}
fn lower_use(use_expr: &UseExpr, unit: UnitId, composition: &BoolExpr) -> Result<BoolExpr, Error> {
    match use_expr {
        UseExpr::MappedUnit(actual) if *actual == unit => finite::substitute(composition, None),
        UseExpr::MappedUnit(_) => Err(Error::UseUnitMismatch),
        UseExpr::RawInput(_) => Err(Error::MissingActualUse),
    }
}
