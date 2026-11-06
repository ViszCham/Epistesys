//! Own the existing actual RootPlan; capture before generating any candidate.
use super::*;
/// Closed Given profile owner, not Clone/serde/report constructible.
/// ```compile_fail
/// use lc631_runtime::context_novelty_measurement::framework::FrameworkRoot;
/// let _ = FrameworkRoot::from_scalar(0);
/// ```
pub struct FrameworkRoot<'a, 'v> {
    pub(super) native: RootPlan<'a, 'v>,
    pub(super) declaration: &'a Declaration<'a>,
    pub(super) digest: Digest32,
    pub(super) capability: capability::Capability<'v>,
    pub(super) attempts: Cell<usize>,
    retained: ResourceClaim<'v>,
}
impl<'a, 'v> FrameworkRoot<'a, 'v> {
    pub fn preregister(
        native: RootPlan<'a, 'v>,
        declaration: &'a Declaration<'a>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let (retained, digest, capability) = native.meter.with_call(
            native.state.declaration.limits.max_call_work,
            || -> Result<_, Error> {
                native.check(view).map_err(context)?;
                admission::declaration(&native, declaration)?;
                bounds::debit(&native, 4096)?; // before complete bounded source/probe scan
                let retained = bounds::claim(view, bounds::PLAN_RAM, None)?;
                let scratch = bounds::claim(view, bounds::SCRATCH_RAM, None)?;
                execution::roots(&native, declaration, native.state.source_query)?;
                let digest = identity::root(&native, declaration, view)?;
                let scope = identity::scope(
                    digest,
                    declaration.root_goal_revision,
                    Kernel::PersistentChain2V1,
                );
                let capability = capability::execute(&native, scope, view)?;
                scratch.check().map_err(Error::Resource)?;
                Self::check_parts(&native, &retained, &capability, view)?;
                Ok((retained, digest, capability))
            },
        )?;
        Ok(Self {
            native,
            declaration,
            digest,
            capability,
            attempts: Cell::new(0),
            retained,
        })
    }
    fn check_parts(
        native: &RootPlan<'a, 'v>,
        retained: &ResourceClaim<'v>,
        capability: &capability::Capability<'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        native.check(view).map_err(context)?;
        bounds::current(retained, view, native.state.source.id().digest())?;
        // Full native-store scan is bounded by the closed960 dispatch profile.
        bounds::debit(native, 65536)?;
        capability.check(view)
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        Self::check_parts(&self.native, &self.retained, &self.capability, view)
    }
    pub fn generate_current(
        &mut self,
        operator: Operator,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<ValidatedFormalCandidate<'a, 'v>, Error> {
        self.check(view)?;
        if operator != self.declaration.allowed_operator {
            return Err(Error::UnsupportedOperator);
        }
        self.native
            .generate_current(operator, view)
            .map_err(context)
    }
    pub fn expected_inputs_current<'s>(
        &'s self,
        subject: Subject<'s, 'a, 'v>,
        kernel: Kernel,
        view: &ResourceReservationView<'_>,
    ) -> Result<ExpectedInputs<'s, 'a, 'v>, Error> {
        self.check(view)?;
        let query = self.subject(subject, view)?;
        Ok(ExpectedInputs {
            root: self,
            declaration: self.declaration,
            source: self.native.state.source,
            source_query: self.native.state.source_query,
            subject_query: query,
            subject,
            kernel,
        })
    }
    pub(super) fn subject<'s>(
        &'s self,
        subject: Subject<'s, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&'s SmtStatusQuery, Error> {
        match subject {
            Subject::Original => Ok(self.native.state.source_query),
            Subject::Generated(c) => {
                if c.operator() != self.declaration.allowed_operator {
                    return Err(Error::ForeignCandidate);
                }
                self.native.project(c, view).map_err(|e| match e {
                    super::super::Error::ForeignOwner => Error::ForeignCandidate,
                    other => context(other),
                })
            }
        }
    }
    pub(super) fn expected(
        &self,
        e: ExpectedInputs<'_, 'a, 'v>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        if !std::ptr::eq(self, e.root) {
            return Err(Error::ForeignRoot);
        }
        if !std::ptr::eq(self.declaration, e.declaration)
            || !std::ptr::eq(self.native.state.source, e.source)
            || !std::ptr::eq(self.native.state.source_query, e.source_query)
            || !std::ptr::eq(self.subject(e.subject, view)?, e.subject_query)
        {
            return Err(Error::ExpectedInputMismatch);
        }
        Ok(())
    }
    pub fn execute_case_current<'s>(
        &'s self,
        expected: ExpectedInputs<'s, 'a, 'v>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<OwnedFrameworkCase<'s, 'a, 'v>, Error> {
        self.native
            .meter
            .with_call(self.native.state.declaration.limits.max_call_work, || {
                self.check(view)?;
                self.expected(expected, view)?;
                let attempts = self.attempts.get();
                if attempts >= self.declaration.limits.max_cases {
                    return Err(Error::CaseBudget);
                }
                self.attempts
                    .set(attempts.checked_add(1).ok_or(Error::CaseBudget)?);
                bounds::debit(&self.native, 4096)?; // failed valid attempts are not refunded
                execution::execute(self, expected, view)
            })
    }
    pub fn work_remaining_current(&self, view: &ResourceReservationView<'_>) -> Result<u64, Error> {
        self.check(view)?;
        Ok(self.native.meter.remaining())
    }
    pub fn historical_digest(&self) -> Digest32 {
        self.digest
    }
}
