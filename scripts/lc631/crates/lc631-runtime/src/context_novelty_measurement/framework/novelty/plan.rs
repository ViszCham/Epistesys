//! Own P1 and all fixed declarations;requests do not spend checked-payload quota.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComparisonRole {
    Primary,
    DiagnosticControl,
}

pub(super) fn case_key(c: CaseRef<'_, '_, '_>) -> f::CellKey {
    f::CellKey {
        kernel: c.expected.kernel,
        subject: match c.expected.subject {
            f::Subject::Original => f::SubjectKind::Original,
            f::Subject::Generated(_) => f::SubjectKind::Generated,
        },
    }
}

pub struct Root<'a, 'v> {
    pub(super) framework: f::FrameworkRoot<'a, 'v>,
    pub(super) registration: &'a Registration<'a>,
    pub(super) constructions: n::Universe,
    pub(super) signature: signature::Signature,
    pub(super) digest: Digest32,
    pub(super) producer: Digest32,
    pub(super) retained: ResourceClaim<'v>,
    pub(super) comparisons: Cell<usize>,
    pub(super) dispatches: Cell<usize>,
    pub(super) attempts: Cell<usize>,
}
impl<'a, 'v> Root<'a, 'v> {
    pub fn preregister(
        framework_owner: f::FrameworkRoot<'a, 'v>,
        registration: &'a Registration<'a>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let (retained, constructions, signature, digest, producer) =
            budget::with_call(&framework_owner, || -> Result<_, Error> {
                framework_owner.check(view).map_err(framework)?;
                budget::limits(registration.limits)?;
                budget::debit(&framework_owner, 4096)?;
                let retained = budget::claim(view, budget::PLAN, None)?;
                let scratch = budget::claim(view, budget::SCRATCH, None)?;
                let check = || framework_owner.native.check(view);
                let mut work = Work::new(
                    &framework_owner.native.meter,
                    framework_owner
                        .native
                        .state
                        .declaration
                        .limits
                        .max_call_work,
                    &check,
                );
                admission::inputs(&framework_owner, registration, &mut work)?;
                let constructions = construction::universe(&framework_owner, registration, &mut work)?;
                let signature = signature::build(&framework_owner, registration.limits, &mut work)?;
                let (digest, producer) = identity::root(
                    &framework_owner,
                    registration,
                    &constructions,
                    &signature,
                    &mut work,
                )?;
                scratch.check().map_err(Error::Resource)?;
                Self::check_parts(&framework_owner, &retained, view)?;
                Ok((retained, constructions, signature, digest, producer))
            })?;
        Ok(Self {
            framework: framework_owner,
            registration,
            constructions,
            signature,
            digest,
            producer,
            retained,
            comparisons: Cell::new(registration.limits.max_comparisons),
            dispatches: Cell::new(registration.limits.max_archive_dispatches),
            attempts: Cell::new(0),
        })
    }
    fn check_parts(
        framework_root: &f::FrameworkRoot<'_, '_>,
        retained: &ResourceClaim<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        framework_root.check(view).map_err(framework)?;
        budget::current(retained, view, framework_root.native.state.source.id().digest())
    }

    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        Self::check_parts(&self.framework, &self.retained, view)
    }
    pub fn generate_current(
        &mut self,
        op: Operator,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<ValidatedFormalCandidate<'a, 'v>, Error> {
        self.check(view)?;
        self.framework.generate_current(op, view).map_err(framework)
    }
    pub fn expected_case_current<'s>(
        &'s self,
        subject: f::Subject<'s, 'a, 'v>,
        kernel: f::Kernel,
        view: &ResourceReservationView<'_>,
    ) -> Result<f::ExpectedInputs<'s, 'a, 'v>, Error> {
        budget::with_call(&self.framework, || {
            self.check(view)?;
            self.framework
                .expected_inputs_current(subject, kernel, view)
                .map_err(framework)
        })
    }
    pub fn execute_case_current<'s>(
        &'s self,
        e: f::ExpectedInputs<'s, 'a, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<f::OwnedFrameworkCase<'s, 'a, 'v>, Error> {
        budget::with_call(&self.framework, || {
            self.check(view)?;
            self.framework
                .execute_case_current(e, view)
                .map_err(framework)
        })
    }
    pub fn expected_pair_current<'s>(
        &'s self,
        source: CaseRef<'s, 'a, 'v>,
        target: CaseRef<'s, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<ExpectedPair<'s, 'a, 'v>, Error> {
        budget::with_call(&self.framework, || {
            self.check(view)?;
            self.case_binding(source, view)?;
            self.case_binding(target, view)?;
            Ok(ExpectedPair {
                root: self,
                registration: self.registration,
                source,
                target,
                archive: self.registration.archive,
                corpus_revision: self.registration.corpus_revision,
            })
        })
    }
    pub(super) fn case_binding(
        &self,
        c: CaseRef<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        if !std::ptr::eq(c.owner.root, &self.framework)
            || !std::ptr::eq(c.expected.root, &self.framework)
        {
            return Err(Error::ForeignCase);
        }
        // Same actual private backing;NO consumption of P1 checked-payload quota.
        c.owner.check_binding(c.expected, view).map_err(framework)?;
        if !c.owner.data.hard_ok {
            return Err(Error::Framework(f::Error::HardInvariant));
        }
        Ok(())
    }
    pub(super) fn expected(&self, e: ExpectedPair<'_, 'a, 'v>) -> Result<(), Error> {
        if !std::ptr::eq(self, e.root) {
            return Err(Error::ForeignRoot);
        }
        if !std::ptr::eq(self.registration, e.registration)
            || e.corpus_revision != self.registration.corpus_revision
            || !same_archive(e.archive, self.registration.archive)
        {
            return Err(Error::ExpectedInputMismatch);
        }
        Ok(())
    }

    pub(super) fn comparison_role(
        &self,
        e: ExpectedPair<'_, 'a, 'v>,
    ) -> Result<ComparisonRole, Error> {
        self.expected(e)?;
        let primary = self.framework.declaration.comparison;
        Ok(if case_key(e.source) == primary.source && case_key(e.target) == primary.target {
            ComparisonRole::Primary
        } else {
            ComparisonRole::DiagnosticControl
        })
    }

    pub(super) fn payload_guard(
        &self,
        e: ExpectedPair<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.check(view)?;
        self.expected(e)?;
        // Each actual P1 owner pays ONCE per guard. 4measures+4callbacks x2=16.
        for c in [e.source, e.target] {
            if !std::ptr::eq(c.owner.root, &self.framework) {
                return Err(Error::ForeignCase);
            }
            c.owner.check_current(c.expected, view).map_err(framework)?;
        }
        Ok(())
    }
    pub fn work_remaining_current(&self, view: &ResourceReservationView<'_>) -> Result<u64, Error> {
        budget::with_call(&self.framework, || {
            self.check(view)?;
            Ok(self.framework.native.meter.remaining())
        })
    }
    pub fn historical_digest(&self) -> Digest32 {
        self.digest
    }
}
pub(super) fn same_archive(a: ArchiveInput<'_>, b: ArchiveInput<'_>) -> bool {
    match (a, b) {
        (ArchiveInput::Read(a), ArchiveInput::Read(b)) => std::ptr::eq(a, b),
        (ArchiveInput::Unread, ArchiveInput::Unread)
        | (ArchiveInput::Unavailable, ArchiveInput::Unavailable) => true,
        _ => false,
    }
}
pub(super) fn same_case(a: CaseRef<'_, '_, '_>, b: CaseRef<'_, '_, '_>) -> bool {
    let x = a.expected;
    let y = b.expected;
    let subject = match (x.subject, y.subject) {
        (f::Subject::Original, f::Subject::Original) => true,
        (f::Subject::Generated(x), f::Subject::Generated(y)) => std::ptr::eq(x, y),
        _ => false,
    };
    std::ptr::eq(a.owner, b.owner)
        && std::ptr::eq(x.root, y.root)
        && std::ptr::eq(x.declaration, y.declaration)
        && std::ptr::eq(x.source, y.source)
        && std::ptr::eq(x.source_query, y.source_query)
        && std::ptr::eq(x.subject_query, y.subject_query)
        && x.kernel == y.kernel
        && subject
}
