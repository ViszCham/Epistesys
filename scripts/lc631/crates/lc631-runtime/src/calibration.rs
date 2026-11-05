//! 日本語: 有限形式oracle上の校正集計。一般意味精度・実host判断・独立性を認証しない。
//! English: Finite formal calibration counts, not general accuracy or host-decision evidence.
#![forbid(unsafe_code)]
use crate::formal_boolean as formal;
use lc631_substrate::measurement_numeric::{NumericError, RationalInterval};
use lc631_substrate::Digest32;
#[path = "calibration/corpus.rs"]
pub mod corpus;

pub const MAX_CASES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetRole {
    Training,
    Calibration,
    Holdout,
    Adversarial,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OracleLabel {
    Valid,
    Invalid,
    Unlabelled,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Novelty {
    NotNovel,
    Permitted,
    Unassessed,
}

/// Labels and family membership are Given; checking metadata does not verify gold provenance.
#[derive(Clone, Copy, Debug)]
pub struct CorpusCase<'a> {
    pub id: Digest32,
    pub family: Digest32,
    pub role: DatasetRole,
    pub oracle: OracleLabel,
    pub novelty: Novelty,
    pub request_binding: formal::Binding,
    pub request: Option<&'a formal::Request<'a>>,
}
#[derive(Debug)]
pub struct Corpus<'a> {
    revision: Digest32,
    cases: &'a [CorpusCase<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CalibrationError {
    EmptyCorpus,
    CaseLimit,
    DuplicateCase,
    FamilyAcrossRoles,
    UnknownCase,
    DuplicateObservation,
    ForeignCorpus,
    ForeignRole,
    EmptyRole,
    PolicyMismatch,
    RequestBindingMismatch,
    MissingPinnedRequest,
    Numeric(NumericError),
    Formal(formal::Failure),
}
impl<'a> Corpus<'a> {
    pub fn checked(
        revision: Digest32,
        cases: &'a [CorpusCase<'a>],
    ) -> Result<Self, CalibrationError> {
        if cases.is_empty() {
            return Err(CalibrationError::EmptyCorpus);
        }
        if cases.len() > MAX_CASES {
            return Err(CalibrationError::CaseLimit);
        }
        for (i, case) in cases.iter().enumerate() {
            for prior in &cases[..i] {
                if case.id == prior.id {
                    return Err(CalibrationError::DuplicateCase);
                }
                if case.family == prior.family && case.role != prior.role {
                    return Err(CalibrationError::FamilyAcrossRoles);
                }
            }
        }
        Ok(Self { revision, cases })
    }
    pub fn revision(&self) -> Digest32 {
        self.revision
    }
    pub fn cases(&self) -> &'a [CorpusCase<'a>] {
        self.cases
    }
    fn case(&self, id: Digest32) -> Result<&CorpusCase<'a>, CalibrationError> {
        self.cases
            .iter()
            .find(|c| c.id == id)
            .ok_or(CalibrationError::UnknownCase)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HoldReason {
    Unknown,
    NoComparable,
    Reported,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Accepted,
    Rejected,
    Held(HoldReason),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionBasis {
    ActualFiniteComputation,
    CallerReported,
}

/// No public constructor can label a caller-reported action as an actual evaluation.
#[derive(Clone, Copy, Debug)]
pub struct Observation<'a> {
    corpus: &'a Corpus<'a>,
    case: Digest32,
    policy: Digest32,
    action: Action,
    basis: DecisionBasis,
}
impl<'a> Observation<'a> {
    pub fn reported(
        corpus: &'a Corpus<'a>,
        case: Digest32,
        policy: Digest32,
        action: Action,
    ) -> Result<Self, CalibrationError> {
        corpus.case(case)?;
        Ok(Self {
            corpus,
            case,
            policy,
            action,
            basis: DecisionBasis::CallerReported,
        })
    }
    pub fn action(self) -> Action {
        self.action
    }
    pub fn basis(self) -> DecisionBasis {
        self.basis
    }
    pub fn case_id(self) -> Digest32 {
        self.case
    }
}

/// Actual finite Boolean evaluation; "Accepted" is this declared equivalence policy only.
/// Both directions must preserve. No authority, external solver receipt or host accept is created.
pub fn observe_finite<'a>(
    corpus: &'a Corpus<'a>,
    case: Digest32,
    policy: Digest32,
    request: &'a formal::Request<'a>,
    work_limit: u64,
) -> Result<Observation<'a>, CalibrationError> {
    if corpus.case(case)?.request_binding != request.binding {
        return Err(CalibrationError::RequestBindingMismatch);
    }
    let pinned = corpus
        .case(case)?
        .request
        .ok_or(CalibrationError::MissingPinnedRequest)?;
    if !std::ptr::eq(pinned, request) {
        return Err(CalibrationError::RequestBindingMismatch);
    }
    let action = match formal::evaluate(request, work_limit).map_err(CalibrationError::Formal)? {
        formal::Evaluation::Unknown { .. } => Action::Held(HoldReason::Unknown),
        formal::Evaluation::NoComparable { .. } => Action::Held(HoldReason::NoComparable),
        formal::Evaluation::Computed(report) => match (report.soundness(), report.completeness()) {
            (formal::DirectionResult::Preserved, formal::DirectionResult::Preserved) => {
                Action::Accepted
            }
            (formal::DirectionResult::Countermodel(_), _)
            | (_, formal::DirectionResult::Countermodel(_)) => Action::Rejected,
        },
    };
    Ok(Observation {
        corpus,
        case,
        policy,
        action,
        basis: DecisionBasis::ActualFiniteComputation,
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    pub population: u64,
    pub observed: u64,
    pub missing: u64,
    pub missing_possible_valid: u64,
    pub missing_possible_valid_novel: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub held: u64,
    pub actual_finite: u64,
    pub caller_reported: u64,
    pub oracle_valid: u64,
    pub oracle_invalid: u64,
    pub oracle_unlabelled: u64,
    pub silent_errors: u64,
    pub accepted_unlabelled: u64,
    pub false_holds: u64,
    pub held_unlabelled: u64,
    pub valid_novel_rejections: u64,
    pub possible_novel_rejections: u64,
}

/// Finite empirical fractions, not confidence intervals or probabilities outside this corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Rate {
    Bounded(RationalInterval),
    UndefinedNoAccepted,
}

#[derive(Debug)]
pub struct CalibrationReport {
    corpus_revision: Digest32,
    role: DatasetRole,
    policy: Digest32,
    counts: Counts,
    coverage: RationalInterval,
    selective_risk: Rate,
    false_hold_population: RationalInterval,
    novel_rejection_population: RationalInterval,
}
impl CalibrationReport {
    pub fn counts(&self) -> Counts {
        self.counts
    }
    pub fn corpus_revision(&self) -> Digest32 {
        self.corpus_revision
    }
    pub fn role(&self) -> DatasetRole {
        self.role
    }
    pub fn policy(&self) -> Digest32 {
        self.policy
    }
    pub fn coverage(&self) -> &RationalInterval {
        &self.coverage
    }
    pub fn selective_risk(&self) -> &Rate {
        &self.selective_risk
    }
    pub fn false_hold_population(&self) -> &RationalInterval {
        &self.false_hold_population
    }
    pub fn novel_rejection_population(&self) -> &RationalInterval {
        &self.novel_rejection_population
    }
    pub fn source_interpretation(&self) -> formal::SourceInterpretation {
        formal::SourceInterpretation::Unverified
    }
    pub fn checker_independence(&self) -> formal::CheckerIndependence {
        formal::CheckerIndependence::UnassessedSharedIr
    }
}

pub fn summarize(
    corpus: &Corpus<'_>,
    role: DatasetRole,
    policy: Digest32,
    observations: &[Observation<'_>],
) -> Result<CalibrationReport, CalibrationError> {
    if observations.len() > MAX_CASES {
        return Err(CalibrationError::CaseLimit);
    }
    validate_observations(corpus, role, policy, observations)?;
    let mut counts = Counts::default();
    for case in corpus.cases.iter().filter(|case| case.role == role) {
        counts.population += 1;
        match case.oracle {
            OracleLabel::Valid => counts.oracle_valid += 1,
            OracleLabel::Invalid => counts.oracle_invalid += 1,
            OracleLabel::Unlabelled => counts.oracle_unlabelled += 1,
        }
        let Some(observation) = observations.iter().find(|o| o.case == case.id) else {
            counts.missing += 1;
            if case.oracle != OracleLabel::Invalid {
                counts.missing_possible_valid += 1;
                if case.novelty != Novelty::NotNovel {
                    counts.missing_possible_valid_novel += 1;
                }
            }
            continue;
        };
        counts.observed += 1;
        match observation.basis {
            DecisionBasis::ActualFiniteComputation => counts.actual_finite += 1,
            DecisionBasis::CallerReported => counts.caller_reported += 1,
        }
        count_action(&mut counts, case, observation.action);
    }
    if counts.population == 0 {
        return Err(CalibrationError::EmptyRole);
    }
    let coverage = interval(
        counts.accepted,
        counts.accepted + counts.missing,
        counts.population,
    )?;
    let selective_risk = if counts.accepted == 0 {
        Rate::UndefinedNoAccepted
    } else {
        Rate::Bounded(interval(
            counts.silent_errors,
            counts.silent_errors + counts.accepted_unlabelled,
            counts.accepted,
        )?)
    };
    let false_hold_population = interval(
        counts.false_holds,
        counts.false_holds + counts.held_unlabelled + counts.missing_possible_valid,
        counts.population,
    )?;
    let novel_rejection_population = interval(
        counts.valid_novel_rejections,
        counts.valid_novel_rejections
            + counts.possible_novel_rejections
            + counts.missing_possible_valid_novel,
        counts.population,
    )?;
    Ok(CalibrationReport {
        corpus_revision: corpus.revision,
        role,
        policy,
        counts,
        coverage,
        selective_risk,
        false_hold_population,
        novel_rejection_population,
    })
}

fn validate_observations(
    corpus: &Corpus<'_>,
    role: DatasetRole,
    policy: Digest32,
    observations: &[Observation<'_>],
) -> Result<(), CalibrationError> {
    for (i, observation) in observations.iter().enumerate() {
        if !std::ptr::eq(corpus, observation.corpus) {
            return Err(CalibrationError::ForeignCorpus);
        }
        if observation.policy != policy {
            return Err(CalibrationError::PolicyMismatch);
        }
        if corpus.case(observation.case)?.role != role {
            return Err(CalibrationError::ForeignRole);
        }
        if observations[..i].iter().any(|o| o.case == observation.case) {
            return Err(CalibrationError::DuplicateObservation);
        }
    }
    Ok(())
}

fn count_action(c: &mut Counts, case: &CorpusCase<'_>, action: Action) {
    match action {
        Action::Accepted => {
            c.accepted += 1;
            match case.oracle {
                OracleLabel::Invalid => c.silent_errors += 1,
                OracleLabel::Unlabelled => c.accepted_unlabelled += 1,
                OracleLabel::Valid => {}
            }
        }
        Action::Held(_) => {
            c.held += 1;
            match case.oracle {
                OracleLabel::Valid => c.false_holds += 1,
                OracleLabel::Unlabelled => c.held_unlabelled += 1,
                OracleLabel::Invalid => {}
            }
        }
        Action::Rejected => {
            c.rejected += 1;
            match (case.oracle, case.novelty) {
                (OracleLabel::Valid, Novelty::Permitted) => c.valid_novel_rejections += 1,
                (OracleLabel::Valid, Novelty::Unassessed)
                | (OracleLabel::Unlabelled, Novelty::Permitted)
                | (OracleLabel::Unlabelled, Novelty::Unassessed) => {
                    c.possible_novel_rejections += 1
                }
                (OracleLabel::Invalid, _) | (_, Novelty::NotNovel) => {}
            }
        }
    }
}
fn interval(lo: u64, hi: u64, denominator: u64) -> Result<RationalInterval, CalibrationError> {
    // Admission caps population at 4096; all numerators count disjoint subsets.
    let lo = i64::try_from(lo).map_err(|_| CalibrationError::CaseLimit)?;
    let hi = i64::try_from(hi).map_err(|_| CalibrationError::CaseLimit)?;
    let den = i64::try_from(denominator).map_err(|_| CalibrationError::CaseLimit)?;
    RationalInterval::from_ratio(lo, den, hi, den).map_err(CalibrationError::Numeric)
}
