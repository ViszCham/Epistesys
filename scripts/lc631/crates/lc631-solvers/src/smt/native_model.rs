//! Concrete native SMT model execution followed by exact typed reevaluation.
//! No injected report/stdout constructor, source-truth or proof promotion.
use super::{validate_model, CheckedModel, ModelValidationError};
use crate::protocol::{
    payload_wire::PayloadWireStatus,
    sexpr::{self, SExprError, SExprLimits},
};
use crate::registry::SolverRegistry;
use crate::result_store::SolverResultBinding;
use crate::runner::{ExecutionBudget, SolverQuery};
use crate::supervisor_adapter::{
    execute_supervised_owned_payload_into_store, OwnedPayloadError, OwnedPayloadObservation,
    OwnedPayloadStore,
};
use crate::typed_query::{QueryEmitError, SmtStatusQuery};
use lc631_substrate::context_identity::ContextRevision;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use std::path::Path;

/// Owns actual stdout and borrows only the immutable original typed query.
/// Context identity is retained in memory, not inferred from model values.
pub struct NativeCheckedModel<'query, 'view> {
    context: ContextRevision,
    observation: OwnedPayloadObservation<'view>,
    model: CheckedModel<'query>,
    // Inline model/state accounting; backing observations drop before refund.
    _model_claim: ResourceClaim<'view>,
}
impl<'query> NativeCheckedModel<'query, '_> {
    /// Recheck this actual owner's resource reservation and lifetime only.
    /// Root constraints, source mapping, registry and executable freshness need
    /// their separate current checks; this neither dispatches nor grants authority.
    pub fn check_current(&self, view: &ResourceReservationView<'_>) -> Result<(), ResourceError> {
        self.check_reservation(view)
    }

    pub(super) fn check_reservation(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), ResourceError> {
        self._model_claim.check()?;
        if !self._model_claim.same_reservation(view) {
            return Err(ResourceError::ForeignOwner);
        }
        view.check()
    }

    pub const fn context(&self) -> ContextRevision {
        self.context
    }
    pub fn observation(&self) -> &OwnedPayloadObservation<'_> {
        &self.observation
    }
    pub fn model(&self) -> &CheckedModel<'query> {
        &self.model
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeModelRejection {
    BindingMismatch,
    MissingPayload,
    NotSat,
    Syntax(SExprError),
    Model(ModelValidationError),
    Resource(ResourceError),
}

/// A model rejection retains the sole actual observation, including raw bytes.
/// Existing native-execution failures preserve their explicit report/effect state.
#[allow(clippy::large_enum_variant)]
pub enum NativeModelError<'view> {
    SourceMismatch,
    Resource(ResourceError),
    Query(QueryEmitError),
    Execution(OwnedPayloadError),
    Rejected {
        reason: NativeModelRejection,
        observation: OwnedPayloadObservation<'view>,
    },
}

fn ram_claim<'view>(
    reservation: &'view ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'view>, ResourceError> {
    reservation.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(bytes)?,
    )])
}

/// SAT-model profile only: get-model after UNSAT may produce a backend error,
/// which remains an execution/report failure, never a checked model.
///
/// Parser arrays are preallocated under their exact published scratch charge.
/// Emission charges twice its logical byte cap before allocation; allocator
/// headers/over-allocation and process RSS are not measured or certified here.
/// No AST clone is made. Source/context interpretation is caller-supplied scope,
/// not an independently checked source-to-formula semantic correspondence.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub fn execute_native_model<'query, 'view>(
    registry: &SolverRegistry,
    query: &'query SmtStatusQuery,
    context: ContextRevision,
    budget: &ExecutionBudget,
    reservation: &'view ResourceReservationView<'_>,
    working_directory: &Path,
    store: &mut OwnedPayloadStore<'_>,
    parser_limits: SExprLimits,
) -> Result<NativeCheckedModel<'query, 'view>, NativeModelError<'view>> {
    reservation.check().map_err(NativeModelError::Resource)?;
    let source = context.source().digest();
    if source != reservation.source_revision() {
        return Err(NativeModelError::SourceMismatch);
    }
    let emitter_bytes = u64::try_from(query.limits().max_output_bytes())
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or(NativeModelError::Resource(
            ResourceError::ArithmeticOverflow,
        ))?;
    let _emitter_claim =
        ram_claim(reservation, emitter_bytes).map_err(NativeModelError::Resource)?;
    let parser_bytes = sexpr::scratch_bytes(parser_limits).ok_or(NativeModelError::Resource(
        ResourceError::ArithmeticOverflow,
    ))?;
    let _parser_claim = ram_claim(reservation, parser_bytes).map_err(NativeModelError::Resource)?;
    let model_bytes = u64::try_from(std::mem::size_of::<NativeCheckedModel<'_, '_>>())
        .map_err(|_| NativeModelError::Resource(ResourceError::ArithmeticOverflow))?;
    let model_claim = ram_claim(reservation, model_bytes).map_err(NativeModelError::Resource)?;
    let prepared = query
        .emit_model(crate::runner::QuerySourceIdentity::from_bytes(
            *source.as_bytes(),
        ))
        .map_err(NativeModelError::Query)?;
    let solver_query = prepared.solver_query().map_err(NativeModelError::Query)?;
    let observation = execute_supervised_owned_payload_into_store(
        registry,
        &solver_query,
        budget,
        reservation,
        working_directory,
        store,
    )
    .map_err(NativeModelError::Execution)?;
    let checked = check_observation(
        query,
        &solver_query,
        &observation,
        store,
        parser_limits,
        reservation,
    );
    match checked {
        Ok(model) => Ok(NativeCheckedModel {
            context,
            observation,
            model,
            _model_claim: model_claim,
        }),
        Err(reason) => Err(NativeModelError::Rejected {
            reason,
            observation,
        }),
    }
}

fn check_observation<'query>(
    original: &'query SmtStatusQuery,
    emitted: &SolverQuery<'_>,
    observation: &OwnedPayloadObservation<'_>,
    store: &OwnedPayloadStore<'_>,
    limits: SExprLimits,
    reservation: &ResourceReservationView<'_>,
) -> Result<CheckedModel<'query>, NativeModelRejection> {
    if observation.source() != emitted.source()
        || observation.query() != emitted.digest()
        || observation.invocation().is_none()
    {
        return Err(NativeModelRejection::BindingMismatch);
    }
    let stored = store
        .lookup(SolverResultBinding::for_query(
            emitted,
            observation.invocation(),
        ))
        .map_err(|_| NativeModelRejection::BindingMismatch)?;
    if stored.id() != observation.insertion().id()
        || stored.sequence() != observation.insertion().sequence()
        || stored.report() != observation.report()
    {
        return Err(NativeModelRejection::BindingMismatch);
    }
    if observation.report().payload_wire_status() != Some(PayloadWireStatus::Sat) {
        return Err(NativeModelRejection::NotSat);
    }
    reservation
        .check()
        .map_err(NativeModelRejection::Resource)?;
    let payload = observation
        .payload()
        .ok_or(NativeModelRejection::MissingPayload)?;
    let document =
        sexpr::parse_preallocated(payload, limits).map_err(NativeModelRejection::Syntax)?;
    let model = validate_model(original, &document).map_err(NativeModelRejection::Model)?;
    reservation
        .check()
        .map_err(NativeModelRejection::Resource)?;
    Ok(model)
}
