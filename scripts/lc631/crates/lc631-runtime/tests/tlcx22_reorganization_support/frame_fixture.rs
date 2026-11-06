//! Preserve the earlier oracle scenarios through the implemented exact v2 path.
//! No v1 root inference, local oracle, measurement stub or success certificate.
#![allow(dead_code)]
#[path = "../tlcx22_reframe_exact_support/scenario.rs"]
mod exact;
#[path = "../tlcx22_reframe_exact_support/mod.rs"]
mod support;

pub use lc631_analysis::creativity_measurement::ComponentMetric;
pub use support::{
    f, ratio, ComponentReport, ComponentState, ContextError, IndependenceStatus, InputBasis,
    SemanticValidation,
};

#[derive(Clone, Copy)]
pub enum Case {
    CheckedMacro,
    MissingPath,
    WrongEndpoint,
    TinyOracleBudget,
    MissingBaselineOwner,
    UnusedMacro,
    IdentityFrame,
    LabelOnlyDelta,
    UnregisteredMacroRule,
    OracleSwap,
    HardErase,
    AuthoritySwap,
    GoalScopeSwap,
}
pub struct Observed {
    pub report: ComponentReport,
    pub baseline: Vec<f::PerspectiveOutcome>,
    pub candidate: Vec<f::PerspectiveOutcome>,
    pub used_base_paths: Vec<Vec<u16>>,
    pub denominator: u64,
    pub same_oracle_and_budget: bool,
    pub root_reachability_before: bool,
    pub root_reachability_after: bool,
    pub has_gaps: bool,
}
pub fn scenario(case: Case) -> Result<Observed, f::Error> {
    use exact::Case as V2;
    let current = exact::scene(match case {
        Case::CheckedMacro => V2::Valid,
        Case::MissingPath => V2::MissingPath,
        Case::WrongEndpoint => V2::WrongPathEndpoint,
        Case::TinyOracleBudget => V2::TinyOracleBudget,
        Case::MissingBaselineOwner => V2::MissingBaselinePaid,
        Case::UnusedMacro => V2::UnusedMacro,
        Case::IdentityFrame => V2::IdentityFrame,
        Case::LabelOnlyDelta => V2::WrongDeltaDeclaration,
        Case::UnregisteredMacroRule => V2::UnregisteredFrameRule,
        // V2 exposes one fixed OracleId. A foreign plan is the remaining
        // safe substitution surface, binding the actual oracle/work owner.
        Case::OracleSwap => V2::ForeignRootPlan,
        Case::HardErase => V2::HardErase,
        Case::AuthoritySwap => V2::AuthoritySwap,
        Case::GoalScopeSwap => V2::ContextScopeSwap,
    })?;
    Ok(Observed {
        report: current.report,
        baseline: current.baseline,
        candidate: current.candidate,
        used_base_paths: current.candidate_path.into_iter().collect(),
        denominator: current.denominator,
        same_oracle_and_budget: current.same_oracle_and_budget,
        root_reachability_before: current.root_before,
        root_reachability_after: current.root_after,
        has_gaps: current.has_gaps,
    })
}
