//! 日本語: 登録compilerの実3stageと宣言sourceの対応だけを観測する。
//! English: Observe three registered compiler stages and their declared source correspondence.
//! Semantic acceptance, source-goal eligibility and host authority remain separate.
#![forbid(unsafe_code)]
mod admission;
mod compiler_image;
mod current;
mod error;
mod grammar;
mod plan;
mod producer;
mod types;

pub use error::BuildError;
pub use plan::{compiler_registration_payload, compiler_registration_scope, BuildPlan};
pub use producer::observe as observe_registered_source_build;
use types::{check_claim, AuthorityStamp};
pub use types::{
    BuildAuthorityView, BuildEvent, BuildEventObserver, BuildLimits, BuildPlanInput, BuildServices,
    BuildStage, BuildView, CompilerChildWitness, CompilerSpec, CurrentBuildAuthority, Emission,
    EnvironmentEntry, HermeticClosure, Independence, OriginKind, OutputState, OwnedBuildFailure,
    OwnedSourceBuildOrigin, ProcessFrame, SemanticAcceptance, SourceC1, StandaloneProfile,
};
