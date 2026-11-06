use super::*;

pub struct Root<'a, 'v> {
    pub(super) parent: m::Root<'a, 'v>,
    pub(super) registration: &'a Registration<'a>,
    pub(super) digest: Digest32,
    pub(super) producer: Digest32,
    pub(super) attempts: Cell<usize>,
    pub(super) dispatches_left: Cell<usize>,
    pub(super) retained: ResourceClaim<'v>,
}
#[derive(Clone, Copy)]
pub struct ExpectedPair<'s, 'a, 'v> {
    pub root: &'s Root<'a, 'v>,
    pub registration: &'a Registration<'a>,
    pub pair: m::ExpectedPair<'s, 'a, 'v>,
}
impl<'a, 'v> Root<'a, 'v> {
    pub fn preregister(
        parent: m::Root<'a, 'v>,
        registration: &'a Registration<'a>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let (retained, digest, producer) = m::budget::with_call(&parent.framework, || -> Result<_, Error> {
            parent.check(view)?;
            admission::limits(registration)?;
            if !parent.framework.native.bridge.proposals().is_empty() {
                return Err(Error::AlreadyGenerated);
            }
            let retained = m::budget::claim(view, registration.limits.plan_ram, None)?;
            let scratch = m::budget::claim(view, registration.limits.scratch_ram, None)?;
            let check = || parent.framework.native.check(view);
            let mut work = n::Work::new(&parent.framework.native.meter,
                parent.framework.native.state.declaration.limits.max_call_work, &check);
            admission::probes(&parent, registration, &mut work)?;
            let (digest, producer) = identity::root(&parent, registration, &mut work)?;
            scratch.check().map_err(Error::Resource)?;
            parent.check(view)?;
            m::budget::current(&retained, view, parent.framework.native.state.source.id().digest())?;
            Ok((retained, digest, producer))
        })?;
        Ok(Self { parent, registration, digest, producer, retained, attempts: Cell::new(0),
            dispatches_left: Cell::new(registration.limits.max_native_dispatches) })
    }
    pub(super) fn own_current(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        m::budget::current(&self.retained, view, self.parent.framework.native.state.source.id().digest())?;
        Ok(())
    }
    pub fn generate_current(&mut self, op: f::Operator, view: &'v ResourceReservationView<'_>)
        -> Result<f::ValidatedFormalCandidate<'a, 'v>, Error> {
        self.own_current(view)?;
        self.parent.generate_current(op, view).map_err(Into::into)
    }
    pub fn expected_case_current<'s>(&'s self, subject: f::Subject<'s, 'a, 'v>, kernel: f::Kernel,
        view: &ResourceReservationView<'_>) -> Result<f::ExpectedInputs<'s, 'a, 'v>, Error> {
        self.own_current(view)?;
        self.parent.expected_case_current(subject, kernel, view).map_err(Into::into)
    }
    pub fn execute_case_current<'s>(&'s self, e: f::ExpectedInputs<'s, 'a, 'v>,
        view: &'v ResourceReservationView<'_>) -> Result<f::OwnedFrameworkCase<'s, 'a, 'v>, Error> {
        self.own_current(view)?;
        self.parent.execute_case_current(e, view).map_err(Into::into)
    }
    pub fn expected_pair_current<'s>(&'s self, source: m::CaseRef<'s, 'a, 'v>, target: m::CaseRef<'s, 'a, 'v>,
        view: &ResourceReservationView<'_>) -> Result<ExpectedPair<'s, 'a, 'v>, Error> {
        self.own_current(view)?;
        let pair = self.parent.expected_pair_current(source, target, view)?;
        Ok(ExpectedPair { root: self, registration: self.registration, pair })
    }
    pub(super) fn expected(&self, e: ExpectedPair<'_, 'a, 'v>) -> Result<(), Error> {
        if !std::ptr::eq(self, e.root) { return Err(Error::ForeignRoot); }
        if !std::ptr::eq(self.registration, e.registration) { return Err(Error::ExpectedInputMismatch); }
        self.parent.expected(e.pair)?;
        Ok(())
    }
    pub fn historical_digest(&self) -> Digest32 { self.digest }
    pub fn work_remaining_current(&self, view: &ResourceReservationView<'_>) -> Result<u64, Error> {
        self.own_current(view)?;
        self.parent.work_remaining_current(view).map_err(Into::into)
    }
}
