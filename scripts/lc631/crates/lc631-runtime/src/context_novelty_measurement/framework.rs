//! 日本語: 閉じた二有限体系の実capability/map/current。Given解釈、権限なし。
//! English: Actual closed two-kernel capabilities and current maps, not source truth.
//! P1 does not issue Dpar/Nknown/Dsem/A/Tnc scalars or a selection recommendation.
#![forbid(unsafe_code)]
mod admission;
mod bounds;
mod capability;
mod current;
mod execution;
mod identity;
mod lowering;
mod native;
pub mod novelty;
mod plan;
mod types;
use super::{canonical, resources, RootPlan, Work};
use crate::creative_candidate_bridge::{Operator, ValidatedFormalCandidate};
use crate::finite_modal_observation::{
    ModalError, ModalObservationId, ModalObservationStore, ModalRequest,
};
pub use current::{CurrentCase, OwnedFrameworkCase};
pub use execution::{Counts, MapBackRow, ModelRow};
use lc631_analysis::creativity_measurement::{
    ComponentMetric, IndependenceStatus, InputBasis, Rational, SemanticValidation,
};
use lc631_core::{
    context::{ContextRevision, ContextScope},
    AuthorityRevision,
};
use lc631_solvers::{
    finite_modal::{Formula, Frame, Model, Profile},
    typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery},
};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    source::SourceStore,
    Digest32,
};
pub use plan::FrameworkRoot;
use std::cell::Cell;
pub use types::*;
fn context(e: super::Error) -> Error {
    match e {
        super::Error::Resource(e) => Error::Resource(e),
        super::Error::ForeignReservation => Error::ForeignReservation,
        super::Error::WorkBudget => Error::WorkBudget,
        other => Error::Context(other),
    }
}
