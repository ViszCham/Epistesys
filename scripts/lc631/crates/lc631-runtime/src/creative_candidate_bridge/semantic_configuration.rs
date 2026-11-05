//! 日本語: 実生成ownerの有限CR-DSEM。保存損失・source解釈・権限と分離する。
//! English: Finite CR-DSEM under actual generated owners, separate from TL and authority.
#![forbid(unsafe_code)]
mod evaluate;
mod follow_up;
mod plan;
mod support;
mod util;

use super::{CreativeCandidateBridge, CreativeError, ValidatedFormalCandidate};
use crate::{formal_boolean as f, formal_measurement::WorkBudget};
use lc631_analysis::creativity_measurement::{ComponentReport, MeasurementError, Rational};
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
use lc631_substrate::resource::{ResourceClaim, ResourceError, ResourceReservationView};
use lc631_substrate::Digest32;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeRole {
    Nonmandatory,
    Required,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextSide {
    Baseline,
    Candidate,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeGap {
    MissingDispatch,
    WorkExhausted,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalOutcome {
    Supported,
    Refuted,
    Undetermined,
    Unknown(OutcomeGap),
}
#[derive(Clone, Copy, Debug)]
pub struct ConfigurationDeclaration {
    pub oracle: Digest32,
    pub domain: Digest32,
    pub hard_query_work: u64,
}
pub struct ConfigurationProbe<'a> {
    pub identity: &'a [u8],
    pub predicate: &'a BoolExpr,
    pub weight: &'a Rational,
    pub role: ProbeRole,
}
pub struct SupportDispatch<'a> {
    pub plan: Digest32,
    pub identity: &'a [u8],
    pub side: ContextSide,
    pub max_work: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigurationBindings {
    pub source: SourceRevision,
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub baseline: Digest32,
    pub hard_registry: Digest32,
    pub probes: Digest32,
    pub oracle: Digest32,
    pub domain: Digest32,
    pub definition: Digest32,
}
#[derive(Debug)]
pub enum Error {
    RequiredProbe,
    RequiredViolation,
    RequiredPending(OutcomeGap),
    SourceQueryMismatch,
    ForeignPlan,
    ScopeMismatch,
    InvalidPlan,
    DuplicateProbe,
    DuplicateDispatch,
    ForeignProbe,
    InvalidWeight,
    AmbiguousFollowUp,
    WorkBudget,
    PayloadBudget,
    AllocationFailed,
    Arithmetic(MeasurementError),
    Formal(f::Failure),
    NoComparable(f::NoComparable),
    Creative(CreativeError),
    Resource(ResourceError),
    Identity(super::measurement::Error),
}

pub struct ConfigurationPlan<'input, 'view> {
    source: &'input SmtStatusQuery,
    goal: &'input SmtStatusQuery,
    probes: &'input [ConfigurationProbe<'input>],
    hard: Vec<&'input BoolExpr>,
    map: Vec<(BoolSymbolId, BoolSymbolId)>,
    declaration: ConfigurationDeclaration,
    bindings: ConfigurationBindings,
    digest: Digest32,
    payload_bytes: u64,
    owner: Arc<()>,
    retained: ResourceClaim<'view>,
}

pub struct ProbeOutcome<'scope> {
    fixed: &'scope ConfigurationProbe<'scope>,
    outcomes: [CanonicalOutcome; 2],
}
impl ProbeOutcome<'_> {
    pub fn identity(&self) -> &[u8] {
        self.fixed.identity
    }
    pub fn baseline(&self) -> CanonicalOutcome {
        self.outcomes[0]
    }
    pub fn candidate(&self) -> CanonicalOutcome {
        self.outcomes[1]
    }
}
type Evaluations<'a> = (&'a f::Evaluation<'a>, &'a f::Evaluation<'a>);
pub struct HardOutcome<'scope> {
    evaluations: [Evaluations<'scope>; 2],
    outcomes: [CanonicalOutcome; 2],
}
impl HardOutcome<'_> {
    pub fn baseline(&self) -> CanonicalOutcome {
        self.outcomes[0]
    }
    pub fn candidate(&self) -> CanonicalOutcome {
        self.outcomes[1]
    }
    pub fn baseline_evaluations(&self) -> Evaluations<'_> {
        self.evaluations[0]
    }
    pub fn candidate_evaluations(&self) -> Evaluations<'_> {
        self.evaluations[1]
    }
}

type CurrentCheck<'a> = dyn for<'v> Fn(&ResourceReservationView<'v>) -> Result<(), Error> + 'a;
/// Only scoped actual evaluation constructs this token. Report copies are
/// historical metadata and cannot reconstruct a current token.
/// ```compile_fail
/// use lc631_runtime::creative_candidate_bridge::semantic_configuration::CurrentConfiguration;
/// use lc631_analysis::creativity_measurement::ComponentReport;
/// fn inject(report: ComponentReport) { CurrentConfiguration::from_report(report); }
/// ```
/// ```compile_fail
/// use lc631_runtime::creative_candidate_bridge::semantic_configuration::CurrentConfiguration;
/// use lc631_analysis::creativity_measurement::ComponentReport;
/// fn escape(current: CurrentConfiguration<'_>) -> &'static ComponentReport { current.report() }
/// ```
pub struct CurrentConfiguration<'scope> {
    report: &'scope ComponentReport,
    probes: &'scope [ProbeOutcome<'scope>],
    hard: &'scope [HardOutcome<'scope>],
    queries: [&'scope SmtStatusQuery; 2],
    contexts: [ContextRevision; 2],
    map: &'scope [(BoolSymbolId, BoolSymbolId)],
    probe_set: Digest32,
    goal: &'scope BoolExpr,
    check: &'scope CurrentCheck<'scope>,
    work: &'scope mut util::Work,
}
impl CurrentConfiguration<'_> {
    pub fn report(&self) -> &ComponentReport {
        self.report
    }
    pub fn probe(&self, identity: &[u8]) -> Option<&ProbeOutcome<'_>> {
        self.probes.iter().find(|p| p.identity() == identity)
    }
    pub fn hard_outcomes(&self) -> &[HardOutcome<'_>] {
        self.hard
    }
    pub fn query_count(&self) -> usize {
        self.work.queries
    }
    pub fn remaining_work(&self) -> u64 {
        self.work.remaining
    }
    pub fn source_interpretation(&self) -> f::SourceInterpretation {
        f::SourceInterpretation::Unverified
    }
}
pub enum FollowUpResult<'fresh> {
    NoQuery { query_count: usize },
    Executed(FreshSupport<'fresh>),
}
pub struct FreshSupport<'fresh> {
    outcome: CanonicalOutcome,
    evaluations: Evaluations<'fresh>,
    before: usize,
    after: usize,
    check: &'fresh CurrentCheck<'fresh>,
}
impl FreshSupport<'_> {
    pub fn outcome(&self) -> CanonicalOutcome {
        self.outcome
    }
    pub fn current_outcome(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<CanonicalOutcome, Error> {
        (self.check)(resources)?;
        Ok(self.outcome)
    }
    pub fn evaluations(&self) -> Evaluations<'_> {
        self.evaluations
    }
    pub fn query_count(&self) -> usize {
        self.after - self.before
    }
    pub fn before_count(&self) -> usize {
        self.before
    }
    pub fn after_count(&self) -> usize {
        self.after
    }
}
