//! HRTB read-only consumption of actual held backing, never a detached report.
use super::*;
/// Private actual attempt owner, including hard-ineligible execution history.
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::OwnedFrameworkCase;
/// fn inject<'s,'a,'v>() { let _:OwnedFrameworkCase<'s,'a,'v>=serde_json::from_str("{}").unwrap(); }
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::OwnedFrameworkCase;
/// let _=OwnedFrameworkCase::from_report(());
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::OwnedFrameworkCase;
/// let _=OwnedFrameworkCase::from_scalar(0);
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::OwnedFrameworkCase;
/// fn clone_checked(owner:&OwnedFrameworkCase<'_,'_,'_>) { let _:OwnedFrameworkCase<'_,'_,'_>=Clone::clone(owner); }
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::OwnedFrameworkCase;
/// fn rewrite(owner:&mut OwnedFrameworkCase<'_,'_,'_>) {owner.data=();}
/// ```
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::{OwnedFrameworkCase,ExpectedInputs,CurrentCase};
/// use lc631_substrate::resource::ResourceReservationView;
/// fn escape<'s,'a,'v>(o:&OwnedFrameworkCase<'s,'a,'v>,e:ExpectedInputs<'s,'a,'v>,v:&ResourceReservationView<'_>)->CurrentCase<'static>{o.with_current(e,v,|c|c).unwrap()}
/// ```
pub struct OwnedFrameworkCase<'s, 'a, 'v> {
    pub(super) root: &'s FrameworkRoot<'a, 'v>,
    pub(super) expected: ExpectedInputs<'s, 'a, 'v>,
    pub(super) data: execution::Data<'s, 'v>,
    pub(super) checks: Cell<usize>,
    pub(super) retained: ResourceClaim<'v>,
}
pub struct CurrentCase<'scope> {
    rows: &'scope [ModelRow],
    mapbacks: &'scope [MapBackRow],
    query: &'scope SmtStatusQuery,
    roots: &'scope [&'scope BoolExpr],
    counts: Counts,
    roles: &'scope [SignatureRole],
    packages: &'scope [Package],
    capability: &'scope [usize],
}
impl<'s, 'a, 'v> OwnedFrameworkCase<'s, 'a, 'v> {
    pub(super) fn check_binding(
        &self,
        e: ExpectedInputs<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.root.check(view)?;
        bounds::current(
            &self.retained,
            view,
            self.root.native.state.source.id().digest(),
        )?;
        self.root.expected(e, view)?;
        let same_subject = match (self.expected.subject, e.subject) {
            (Subject::Original, Subject::Original) => true,
            (Subject::Generated(a), Subject::Generated(b)) => std::ptr::eq(a, b),
            _ => false,
        };
        if e.kernel != self.expected.kernel
            || !same_subject
            || !std::ptr::eq(e.subject_query, self.expected.subject_query)
        {
            return Err(Error::ExpectedInputMismatch);
        }
        bounds::debit(&self.root.native, 4096)?;
        if let Some(history) = &self.data.native {
            history.check(view)?;
        } else {
            self.check_point(view)?;
        }
        if self.data.counts.native_dispatches != self.data.rows.len() * self.data.bank.roots.len() {
            return Err(Error::ExpectedInputMismatch);
        }
        Ok(())
    }
    fn check_point(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        let mut atoms = bounds::reserve(self.root.native.state.declaration.atoms.len(), 3)?;
        for a in self.root.native.state.declaration.atoms {
            let slot = self
                .root
                .declaration
                .map
                .atoms
                .iter()
                .find(|m| m.meaning == a.meaning)
                .ok_or(Error::InvalidMap)?
                .point_slot;
            atoms.push(super::super::AtomMap {
                local: a.local,
                meaning: slot,
                grounding: a.grounding,
            });
        }
        let check = || self.root.native.check(view);
        let mut work = Work::new(
            &self.root.native.meter,
            self.root.native.state.declaration.limits.max_call_work,
            &check,
        );
        if self.data.point.len() != self.data.counts.native_dispatches {
            return Err(Error::ExpectedInputMismatch);
        }
        for t in &self.data.point {
            let row = self
                .data
                .rows
                .get(t.row)
                .ok_or(Error::ExpectedInputMismatch)?;
            let expr = self
                .data
                .bank
                .roots
                .get(t.formula)
                .ok_or(Error::ExpectedInputMismatch)?;
            let assignment =
                native::point_assignment(&row.states, self.root.declaration.map.atoms)?;
            if canonical::eval(expr, assignment, &atoms, &mut work).map_err(context)? != t.value {
                return Err(Error::ExpectedInputMismatch);
            }
        }
        Ok(())
    }
    pub(super) fn check_current(
        &self,
        e: ExpectedInputs<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        let next = self
            .checks
            .get()
            .checked_add(1)
            .ok_or(Error::CurrentBudget)?;
        if next > self.root.declaration.limits.max_current_checks {
            return Err(Error::CurrentBudget);
        }
        self.checks.set(next); // failures/callback cuts never reset this owner ledger
        self.check_binding(e, view)?;
        if !self.data.hard_ok {
            return Err(Error::HardInvariant);
        }
        Ok(())
    }
    pub fn with_current<R>(
        &self,
        expected: ExpectedInputs<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'scope> FnOnce(CurrentCase<'scope>) -> R,
    ) -> Result<R, Error> {
        self.check_current(expected, view)?;
        let (packages, capability): (&[Package], &[usize]) = match self.expected.kernel {
            Kernel::ClassicalPointV1 => (&[Package::C, Package::Kc], &self.root.capability.point),
            Kernel::PersistentChain2V1 => (&[Package::C, Package::Ki], &self.root.capability.chain),
        };
        let result = consume(CurrentCase {
            rows: &self.data.rows,
            mapbacks: &self.data.mapbacks,
            query: self.expected.subject_query,
            roots: &self.data.bank.roots,
            counts: self.data.counts,
            roles: &ROLES,
            packages,
            capability,
        });
        self.check_current(expected, view)?;
        Ok(result)
    }
}
impl<'scope> CurrentCase<'scope> {
    pub fn rows(&self) -> &'scope [ModelRow] {
        self.rows
    }
    pub fn mapbacks(&self) -> &'scope [MapBackRow] {
        self.mapbacks
    }
    pub fn subject_query(&self) -> &'scope SmtStatusQuery {
        self.query
    }
    pub fn lowering_source_roots(&self) -> &'scope [&'scope BoolExpr] {
        self.roots
    }
    pub fn counts(&self) -> Counts {
        self.counts
    }
    pub fn signature_roles(&self) -> &'scope [SignatureRole] {
        self.roles
    }
    pub fn signature_packages(&self) -> &'scope [Package] {
        self.packages
    }
    pub fn capability_model_counts(&self) -> &'scope [usize] {
        self.capability
    }
    pub fn basis(&self) -> InputBasis {
        InputBasis::Given
    }
    pub fn validation(&self) -> SemanticValidation {
        SemanticValidation::Unassessed
    }
    pub fn independence(&self) -> IndependenceStatus {
        IndependenceStatus::SharedDependency
    }
    pub fn metric_state(&self, _metric: ComponentMetric) -> MetricState {
        MetricState::PendingLaterPacket
    }
}
