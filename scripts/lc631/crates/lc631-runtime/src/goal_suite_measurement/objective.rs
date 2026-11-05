//! Closed finite native root, not model-presence scoring or optimizer completion.
use super::*;
use lc631_analysis::creativity_measurement::{ComponentReport, RationalInterval};
use lc631_solvers::{
    registry::{SolverRegistration, SolverRegistry},
    smt::native_model::NativeCheckedModel,
    typed_query::{IntSymbolId, SmtStatusQuery},
};
use lc631_substrate::source::SourceStore;
mod current;
mod measure;
pub use measure::{measure_native_pair, ObjectiveView, OwnedObjectivePair};

pub enum ObjectiveProfile {
    FiniteZeroToThreeHardAtMostTwoV1,
}
pub struct ObjectiveRootInput<'q> {
    pub source: &'q SourceStore,
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub root: &'q SmtStatusQuery,
    pub symbol: IntSymbolId,
    pub registry: &'q SolverRegistry,
    pub profile: ObjectiveProfile,
    pub limits: Limits,
}
pub struct ObjectiveRoot<'q, 'v> {
    source: &'q SourceStore,
    context: ContextRevision,
    scope: ContextScope,
    query: &'q SmtStatusQuery,
    symbol: IntSymbolId,
    registry: &'q SolverRegistry,
    identity: Digest32,
    invocation: lc631_solvers::runner::InvocationDigest,
    budget: Budget,
    retained: ResourceClaim<'v>,
}
#[derive(Clone, Copy)]
pub struct NativePair<'o, 'q, 'v> {
    pub baseline: &'o NativeCheckedModel<'q, 'v>,
    pub candidate: &'o NativeCheckedModel<'q, 'v>,
}
impl<'q, 'v> ObjectiveRoot<'q, 'v> {
    pub fn given(
        input: ObjectiveRootInput<'q>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        view.check()?;
        let budget = Budget::checked(input.limits)?;
        budget.root_funding()?;
        if input.source.text().len() > SOURCE_CAP {
            return Err(Error::InputLimit);
        }
        if input.source.id().digest() != view.source_revision()
            || input.context.source().digest() != view.source_revision()
            || !nonzero(input.scope.comparison())
            || !nonzero(input.scope.goal())
            || !nonzero(input.scope.probes())
            || !nonzero(input.scope.semantics())
        {
            return Err(Error::BindingMismatch);
        }
        match input.profile {
            ObjectiveProfile::FiniteZeroToThreeHardAtMostTwoV1 => {}
        }
        let retained = claim(view, ROOT_RAM)?;
        current::root_query(input.root, input.symbol)?;
        let registration = current::registration(input.registry)?;
        let invocation = current::invocation(registration);
        let identity = hash(&[
            b"FiniteNativeObjectiveZeroToThree-v1;baseline=1;root-hard-first",
            input.source.id().digest().as_bytes(),
            input.context.interpretation_digest().as_bytes(),
            input.scope.comparison().as_bytes(),
            input.scope.goal().as_bytes(),
            input.scope.probes().as_bytes(),
            input.scope.semantics().as_bytes(),
            &input.symbol.get().to_le_bytes(),
            invocation.as_bytes(),
        ]);
        let root = Self {
            source: input.source,
            context: input.context,
            scope: input.scope,
            query: input.root,
            symbol: input.symbol,
            registry: input.registry,
            identity,
            invocation,
            budget,
            retained,
        };
        root.check(view)?;
        Ok(root)
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        check_claim(&self.retained, view)?;
        if self.source.text().len() > SOURCE_CAP {
            return Err(Error::InputLimit);
        }
        if self.source.id().digest() != view.source_revision()
            || self.context.source().digest() != view.source_revision()
        {
            return Err(Error::BindingMismatch);
        }
        current::root_query(self.query, self.symbol)?;
        if current::invocation(current::registration(self.registry)?) != self.invocation {
            return Err(Error::BindingMismatch);
        }
        check_claim(&self.retained, view)
    }
}
