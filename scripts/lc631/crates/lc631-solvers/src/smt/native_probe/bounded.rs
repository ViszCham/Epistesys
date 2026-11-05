//! One model-rechecked improvement with a finite arithmetic gap upper bound.
//! The gap is not an attained competing solution, independent UNSAT evidence,
//! a global optimum certificate, or a statistical estimate.
use super::{NativeProbeOutcome, NativeProbeStatus};
use crate::protocol::sexpr::SExprLimits;
use crate::registry::SolverRegistry;
use crate::runner::ExecutionBudget;
use crate::smt::{
    native_model::{execute_native_model, NativeCheckedModel, NativeModelError},
    ObjectiveDirection,
};
use crate::supervisor_adapter::{OwnedPayloadStore, OwnedPayloadStoreInitError};
use crate::typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtStatusQuery};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundsError {
    UndeclaredObjective,
    MissingLowerBound,
    MissingUpperBound,
    InconsistentBounds,
    IntegerBoundaryOverflow,
    ValueOutsideBounds,
}

/// Bounds are extracted only from asserted conjuncts, never disjuncts or
/// guessed algebraic rewrites. Other constraints may tighten this enclosure.
#[derive(Debug)]
pub struct FiniteObjectiveBounds<'query> {
    query: &'query SmtStatusQuery,
    objective: IntSymbolId,
    lower: i64,
    upper: i64,
}
impl<'query> FiniteObjectiveBounds<'query> {
    pub fn from_query(
        query: &'query SmtStatusQuery,
        objective: IntSymbolId,
    ) -> Result<Self, BoundsError> {
        if query.integer_symbols().binary_search(&objective).is_err() {
            return Err(BoundsError::UndeclaredObjective);
        }
        let (mut lower, mut upper) = (None, None);
        for assertion in query.assertions() {
            collect(assertion, objective, &mut lower, &mut upper)?;
        }
        let lower = lower.ok_or(BoundsError::MissingLowerBound)?;
        let upper = upper.ok_or(BoundsError::MissingUpperBound)?;
        if lower > upper {
            return Err(BoundsError::InconsistentBounds);
        }
        Ok(Self {
            query,
            objective,
            lower,
            upper,
        })
    }
    pub fn query(&self) -> &SmtStatusQuery {
        self.query
    }
    pub const fn objective(&self) -> IntSymbolId {
        self.objective
    }
    pub const fn lower(&self) -> i64 {
        self.lower
    }
    pub const fn upper(&self) -> i64 {
        self.upper
    }
    /// Conditional arithmetic: caller-supplied values alone are not witnesses.
    pub fn gap_upper_bound(
        &self,
        value: i64,
        direction: ObjectiveDirection,
    ) -> Result<u128, BoundsError> {
        if value < self.lower || value > self.upper {
            return Err(BoundsError::ValueOutsideBounds);
        }
        let gap = match direction {
            ObjectiveDirection::Minimize => i128::from(value) - i128::from(self.lower),
            ObjectiveDirection::Maximize => i128::from(self.upper) - i128::from(value),
        };
        u128::try_from(gap).map_err(|_| BoundsError::ValueOutsideBounds)
    }
}

fn collect(
    expr: &BoolExpr,
    objective: IntSymbolId,
    lo: &mut Option<i64>,
    hi: &mut Option<i64>,
) -> Result<(), BoundsError> {
    match expr {
        BoolExpr::And(items) => {
            for item in items {
                collect(item, objective, lo, hi)?;
            }
        }
        BoolExpr::LessEqual(a, b) | BoolExpr::LessThan(a, b) | BoolExpr::Equal(a, b) => {
            let strict = matches!(expr, BoolExpr::LessThan(..));
            let equal = matches!(expr, BoolExpr::Equal(..));
            let (value, is_lower) = match (a, b) {
                (IntExpr::Constant(c), IntExpr::Variable(v)) if *v == objective => (*c, true),
                (IntExpr::Variable(v), IntExpr::Constant(c)) if *v == objective => (*c, false),
                _ => return Ok(()),
            };
            let value = if strict {
                if is_lower {
                    value.checked_add(1)
                } else {
                    value.checked_sub(1)
                }
                .ok_or(BoundsError::IntegerBoundaryOverflow)?
            } else {
                value
            };
            if is_lower || equal {
                *lo = Some(lo.map_or(value, |old| old.max(value)));
            }
            if !is_lower || equal {
                *hi = Some(hi.map_or(value, |old| old.min(value)));
            }
        }
        BoolExpr::Constant(_) | BoolExpr::Variable(_) | BoolExpr::Not(_) | BoolExpr::Or(_) => {}
    }
    Ok(())
}

/// Owns the actual improved native witness; the strict query and base bounds
/// stay borrowed from the actual probe. No self-referential query/model owner.
pub struct NativeBoundedCandidate<'step, 'view> {
    model: NativeCheckedModel<'step, 'view>,
    store: OwnedPayloadStore<'view>,
    bounds: FiniteObjectiveBounds<'step>,
    value: i64,
    gap_upper_bound: u128,
    _claim: ResourceClaim<'view>,
}
impl NativeBoundedCandidate<'_, '_> {
    pub fn model(&self) -> &NativeCheckedModel<'_, '_> {
        &self.model
    }
    pub fn bounds(&self) -> &FiniteObjectiveBounds<'_> {
        &self.bounds
    }
    pub const fn value(&self) -> i64 {
        self.value
    }
    pub const fn gap_upper_bound(&self) -> u128 {
        self.gap_upper_bound
    }
    pub fn stored_reports(&self) -> usize {
        self.store.len()
    }
}

#[allow(clippy::large_enum_variant)]
pub enum BoundedCandidateError<'step, 'view> {
    Resource(ResourceError),
    Bounds(BoundsError),
    NotReportedImprovement,
    Store(OwnedPayloadStoreInitError),
    Model(NativeModelError<'view>),
    /// Preserve the actual witness even if a post-execution contract fails.
    RejectedCandidate {
        model: NativeCheckedModel<'step, 'view>,
        store: OwnedPayloadStore<'view>,
    },
}

/// One concrete step only. Repeated composition needs caller-owned scoped
/// query/model lifetimes and a run-total budget; neither is claimed here.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub fn consume_bounded_improvement<'step, 'view>(
    outcome: &'step NativeProbeOutcome<'_, '_, '_>,
    registry: &SolverRegistry,
    budget: &ExecutionBudget,
    reservation: &'view ResourceReservationView<'_>,
    working_directory: &Path,
    parser_limits: SExprLimits,
) -> Result<NativeBoundedCandidate<'step, 'view>, BoundedCandidateError<'step, 'view>> {
    outcome
        ._clone_claim
        .check()
        .map_err(BoundedCandidateError::Resource)?;
    if !outcome._clone_claim.same_reservation(reservation) {
        return Err(BoundedCandidateError::Resource(ResourceError::ForeignOwner));
    }
    reservation
        .check()
        .map_err(BoundedCandidateError::Resource)?;
    if outcome.status() != NativeProbeStatus::SolverReportedImprovementExists {
        return Err(BoundedCandidateError::NotReportedImprovement);
    }
    let probe = outcome.probe();
    let bounds =
        FiniteObjectiveBounds::from_query(probe.incumbent_model().query(), probe.objective())
            .map_err(BoundedCandidateError::Bounds)?;
    let bytes = u64::try_from(std::mem::size_of::<NativeBoundedCandidate<'_, '_>>())
        .map_err(|_| BoundedCandidateError::Resource(ResourceError::ArithmeticOverflow))?;
    let amount = ResourceAmount::checked(bytes).map_err(BoundedCandidateError::Resource)?;
    let claim = reservation
        .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(BoundedCandidateError::Resource)?;
    let mut store =
        OwnedPayloadStore::with_resources(1, reservation).map_err(BoundedCandidateError::Store)?;
    let model = execute_native_model(
        registry,
        probe.better_query(),
        outcome.context(),
        budget,
        reservation,
        working_directory,
        &mut store,
        parser_limits,
    )
    .map_err(BoundedCandidateError::Model)?;
    let Some(value) = model
        .model()
        .integer(probe.objective())
        .and_then(|n| i64::try_from(n).ok())
    else {
        return Err(BoundedCandidateError::RejectedCandidate { model, store });
    };
    let improves = match probe.direction() {
        ObjectiveDirection::Minimize => value < probe.incumbent(),
        ObjectiveDirection::Maximize => value > probe.incumbent(),
    };
    let Ok(gap_upper_bound) = bounds.gap_upper_bound(value, probe.direction()) else {
        return Err(BoundedCandidateError::RejectedCandidate { model, store });
    };
    if !improves || model.context() != outcome.context() {
        return Err(BoundedCandidateError::RejectedCandidate { model, store });
    }
    Ok(NativeBoundedCandidate {
        model,
        store,
        bounds,
        value,
        gap_upper_bound,
        _claim: claim,
    })
}
