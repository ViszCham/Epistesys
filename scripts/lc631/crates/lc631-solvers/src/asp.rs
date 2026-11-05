//! Context-bound finite ground normal ASP. No truth/permission issuer lives here.
//! Enumeration and reconstruction validate the declared typed program only;
//! shared lowering, grounding authenticity and arbitrary prose remain unchecked.

mod emit;
mod evaluate;
pub mod native;
mod program;
mod reconstruct;

pub use crate::ConsequenceMode;
pub use emit::PreparedAspQuery;
pub use evaluate::{AspCompleteness, AspEvaluation, Consequence, StableModel};
pub use program::{
    AspBinding, AspError, AspLimits, AspProgram, AspSemantics, AtomCost, AtomId, GroundRule,
    Stratification,
};
