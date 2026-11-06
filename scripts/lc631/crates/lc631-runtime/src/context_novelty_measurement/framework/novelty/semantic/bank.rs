//! 日本語: 全role・全合法付値でnative Not(query)を評価する。
//! 両側を含むbank全体の上限をdispatch前に検査する。
//! English: Evaluate native Not(query) for every role and every legal valuation.
//! Check bounds for the whole bank before dispatch, including both sides.
use super::*;
use lc631_solvers::typed_query::BoolExpr;

pub(super) struct Bank<'v> {
    sides: Option<[SideData<'v>; 2]>,
    pub counts: Counts,
}
struct SideData<'v> {
    queries: Vec<Query>,
    chain: Option<f::native::NativeHistory<'v>>,
    actual_dispatches: usize,
}
impl<'v> Bank<'v> {
    pub fn evaluate(root: &Root<'_, 'v>, pair: m::ExpectedPair<'_, '_, 'v>,
        view: &'v ResourceReservationView<'_>, work: &mut n::Work<'_>) -> Result<Self, Error> {
        let rows = pair.source.owner.data.rows.len().checked_add(pair.target.owner.data.rows.len())
            .ok_or(Error::PayloadBudget)?;
        let required = rows.checked_mul(ROLES.len()).ok_or(Error::PayloadBudget)?;
        let mut counts = Counts { required_not_dispatches: required, ..Counts::default() };
        let payload = required.checked_mul(160).and_then(|v| v.checked_add(4096))
            .ok_or(Error::PayloadBudget)?;
        let l = root.registration.limits;
        if payload > l.max_payload_bytes { return Err(Error::PayloadBudget); }
        if payload as u64 > l.output_bytes { return Err(Error::OutputBudget); }
        if required > root.dispatches_left.get() {
            return Ok(Self { sides: None, counts });
        }
        let source = root.parent.framework.native.state.source;
        let prepay = required.checked_mul(source.text().len().checked_add(512).ok_or(Error::WorkBudget)?)
            .ok_or(Error::WorkBudget)?;
        work.charge(prepay as u64).map_err(native)?;
        root.dispatches_left.set(root.dispatches_left.get() - required);
        let left = SideData::evaluate(root, pair.source, view, work)?;
        let right = SideData::evaluate(root, pair.target, view, work)?;
        counts.native_not_dispatches = left.actual_dispatches.checked_add(right.actual_dispatches)
            .ok_or(Error::WorkBudget)?;
        counts.native_not_store_entries = left.store_entries().checked_add(right.store_entries())
            .ok_or(Error::WorkBudget)?;
        if counts.native_not_dispatches != required { return Err(Error::ExpectedInputMismatch); }
        Ok(Self { sides: Some([left, right]), counts })
    }
    pub fn is_complete(&self) -> bool { self.sides.is_some() }
    pub fn queries(&self, side: Side) -> &[Query] {
        self.sides.as_ref().map_or(&[], |s| s[side as usize].queries.as_slice())
    }
    pub fn query(&self, side: Side, role: f::QueryRole) -> Option<&Query> {
        self.sides.as_ref()?.get(side as usize)?.queries.get(role as usize)
    }
    pub fn check(&self, pair: m::ExpectedPair<'_, '_, '_>, view: &ResourceReservationView<'_>,
        work: &mut n::Work<'_>) -> Result<(), Error> {
        if let Some(sides) = &self.sides {
            sides[0].check(pair.source, view, work)?;
            sides[1].check(pair.target, view, work)?;
        }
        Ok(())
    }
    pub(super) fn bind_native(&self, h: &mut identity::Hasher<'_, '_>) -> Result<(), Error> {
        if let Some(sides) = &self.sides {
            for side in sides {
                h.number(side.actual_dispatches as u64)?;
                if let Some(chain) = &side.chain {
                    for r in &chain.records {
                        h.number(r.model as u64)?;
                        h.number(r.formula as u64)?;
                        h.bytes(&r.input)?;
                        h.number(r.bits)?;
                    }
                }
                for q in &side.queries {
                    h.number(q.role as u64)?;
                    h.number(q.status() as u64)?;
                    for row in q.rows() {
                        h.bytes(row.valuation_states())?;
                        h.number(u64::from(row.admitted()))?;
                        h.number(row.query_bits())?;
                        h.number(row.negation_bits())?;
                    }
                }
            }
        }
        Ok(())
    }
}
impl<'v> SideData<'v> {
    fn store_entries(&self) -> usize { self.chain.as_ref().map_or(0, |h| h.store.len()) }
    fn evaluate(root: &Root<'_, 'v>, case: m::CaseRef<'_, '_, 'v>,
        view: &'v ResourceReservationView<'_>, work: &mut n::Work<'_>) -> Result<Self, Error> {
        let frame = &root.parent.framework;
        let atoms = frame.native.state.declaration.atoms;
        let map = frame.declaration.map.atoms;
        let rows = &case.owner.data.rows;
        let kernel = case.expected.kernel;
        if rows.len() != f::bounds::models(kernel, atoms.len()).map_err(framework)?
            || !case.owner.data.hard_ok || case.owner.data.bank.roots.len() != 7 {
            return Err(Error::ExpectedInputMismatch);
        }
        let mut negatives = reserve(ROLES.len(), 3)?;
        for (i, role) in ROLES.into_iter().enumerate() {
            let probe = frame.declaration.probes.get(i).ok_or(Error::InvalidProbe)?;
            if probe.role != role || !std::ptr::eq(case.owner.data.bank.roots[4+i], probe.formula) {
                return Err(Error::InvalidProbe);
            }
            // 日本語: 形・深さ・展開後の総node数は事前登録で検査済み。
            // English: Shape, depth and total expanded nodes were admitted at preregistration.
            negatives.push(BoolExpr::Not(Box::new(probe.formula.clone())));
        }
        let capacity = rows.len().checked_mul(negatives.len()).ok_or(Error::PayloadBudget)?;
        let mut chain = if kernel == f::Kernel::PersistentChain2V1 {
            let old = case.owner.data.native.as_ref().ok_or(Error::ExpectedInputMismatch)?;
            let scoped = f::identity::scope(root.digest, frame.declaration.root_goal_revision, kernel);
            let mut h = f::native::NativeHistory::new(capacity, old.context, scoped, view).map_err(framework)?;
            h.lowered = reserve(3, 3)?;
            h.models = reserve(rows.len(), 27)?;
            for expr in &negatives {
                h.lowered.push(f::lowering::lower(expr, atoms, map).map_err(framework)?);
            }
            Some(h)
        } else { None };
        let mut point_atoms = reserve(atoms.len(), 3)?;
        for a in atoms {
            let mapped = map.iter().find(|m| m.meaning == a.meaning).ok_or(Error::ExpectedInputMismatch)?;
            point_atoms.push(n::AtomMap { local: a.local, meaning: mapped.point_slot, grounding: a.grounding });
        }
        let mut by_role = [reserve(rows.len(), 27)?, reserve(rows.len(), 27)?, reserve(rows.len(), 27)?];
        let mut actual_dispatches = 0usize;
        for (ordinal, row) in rows.iter().enumerate() {
            let model = if kernel == f::Kernel::ClassicalPointV1 {
                f::native::point_assignment(row.valuation_states(), map).map_err(framework)?
            } else {
                let h = chain.as_mut().ok_or(Error::ExpectedInputMismatch)?;
                let model = f::native::chain_model(row.valuation_states(), map).map_err(framework)?;
                if case.owner.data.native.as_ref().and_then(|old| old.models.get(ordinal)) != Some(&model) {
                    return Err(Error::ExpectedInputMismatch);
                }
                h.models.push(model);
                0
            };
            for (i, expr) in negatives.iter().enumerate() {
                let bits = if let Some(h) = &mut chain {
                    h.evaluate(frame.native.state.source.text(), ordinal, i, view).map_err(framework)?
                } else {
                    u64::from(n::canonical::eval(expr, model, &point_atoms, work).map_err(native)?)
                };
                actual_dispatches += 1;
                by_role[i].push(QueryRow::checked(row, row.query_bits()[i], bits, kernel)?);
            }
        }
        let mut queries = reserve(3, 3)?;
        for (role, values) in ROLES.into_iter().zip(by_role) {
            queries.push(Query::from_rows(role, values)?);
        }
        let result = Self { queries, chain, actual_dispatches };
        result.check(case, view, work)?;
        Ok(result)
    }
    fn check(&self, case: m::CaseRef<'_, '_, '_>, view: &ResourceReservationView<'_>,
        work: &mut n::Work<'_>) -> Result<(), Error> {
        let rows = &case.owner.data.rows;
        work.charge((rows.len() as u64 + 1) * 256).map_err(native)?;
        if rows.len() * 3 != self.actual_dispatches || self.queries.len() != ROLES.len() {
            return Err(Error::ExpectedInputMismatch);
        }
        if let Some(chain) = &self.chain {
            chain.check(view).map_err(framework)?;
            if chain.records.len() != self.actual_dispatches { return Err(Error::ExpectedInputMismatch); }
        }
        for (role, q) in self.queries.iter().enumerate() {
            if q.role != ROLES[role] || q.rows().len() != rows.len() { return Err(Error::ExpectedInputMismatch); }
            for (i, (held, original)) in q.rows().iter().zip(rows).enumerate() {
                if held.valuation_states() != original.valuation_states() || held.admitted() != original.admitted()
                    || held.query_bits() != original.query_bits()[role] { return Err(Error::ExpectedInputMismatch); }
                if let Some(h) = &self.chain {
                    let r = h.records.get(i*3+role).ok_or(Error::ExpectedInputMismatch)?;
                    if r.model != i || r.formula != role || r.bits != held.negation_bits() {
                        return Err(Error::ExpectedInputMismatch);
                    }
                } else if held.negation_bits() != (held.query_bits() ^ 1) {
                    return Err(Error::ExpectedInputMismatch);
                }
            }
        }
        Ok(())
    }
}
