use super::{diagnostic_binding, CanonicalRendered};
use crate::canonical_original_pending::{ENGLISH_LINE, JAPANESE_LINE};
use crate::canonical_solve::types::*;
use crate::explicit_formal::FormalSymbol;
use std::fmt::{self, Write};

struct BoundedText {
    text: String,
    cap: usize,
}
impl BoundedText {
    fn new(cap: usize) -> Result<Self, PostprocessError> {
        let mut text = String::new();
        text.try_reserve_exact(cap)
            .map_err(|_| PostprocessError::AllocationFailed)?;
        Ok(Self { text, cap })
    }
}
impl Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self
            .text
            .len()
            .checked_add(value.len())
            .is_none_or(|n| n > self.cap)
        {
            return Err(fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

pub(super) fn model_explanation(
    evidence: &NativeEvidence<'_, '_>,
    measured: Option<&crate::canonical_solve::evaluation::OwnedBooleanMeasurement<'_>>,
    objective: Option<&crate::canonical_solve::optimization::OwnedObjectiveStep<'_>>,
) -> Result<String, PostprocessError> {
    let Some(NativeAttempt::Model(native)) = evidence.attempts.first() else {
        return Err(PostprocessError::SourceBinding);
    };
    let FormalInput::Problem(problem) = evidence.request.input else {
        return Err(PostprocessError::SourceBinding);
    };
    let mut out = BoundedText::new(8192)?;
    write!(out,"Conditional on the declared formal interpretation, the actual native model rechecked {} assertions; assignments:",objective.map_or_else(||native.model().assertions_checked(),|step|step.selected_assertions_checked()))
        .map_err(|_|PostprocessError::OutputBudget)?;
    // Canonical source names, not opaque symbol IDs or backend arrival order.
    let mut names = Vec::new();
    names
        .try_reserve_exact(problem.symbols().len())
        .map_err(|_| PostprocessError::AllocationFailed)?;
    for symbol in problem.symbols() {
        let name = problem
            .source()
            .slice(symbol.span())
            .map_err(|_| PostprocessError::SourceBinding)?;
        names.push((name, symbol.symbol()));
    }
    names.sort_unstable_by_key(|(name, _)| *name);
    for (name, symbol) in names {
        match symbol {
            FormalSymbol::Boolean(id) => write!(
                out,
                " {name}={};",
                objective
                    .map_or_else(
                        || native.model().boolean(id),
                        |step| step.selected_boolean(id)
                    )
                    .ok_or(PostprocessError::SourceBinding)?
            ),
            FormalSymbol::Integer(id) => write!(
                out,
                " {name}={};",
                objective
                    .map_or_else(
                        || native.model().integer(id),
                        |step| step.selected_integer(id)
                    )
                    .ok_or(PostprocessError::SourceBinding)?
            ),
        }
        .map_err(|_| PostprocessError::OutputBudget)?;
    }
    if let Some(goal) = problem.goal() {
        let binding = problem
            .symbols()
            .iter()
            .find(|s| s.symbol() == FormalSymbol::Integer(goal.symbol()))
            .ok_or(PostprocessError::SourceBinding)?;
        let name = problem
            .source()
            .slice(binding.span())
            .map_err(|_| PostprocessError::SourceBinding)?;
        if let Some(step) = objective {
            write!(out," Explicit single-v1 objective step: {:?}; parsed {:?} {name} bounds [{}..={}]; initial={:?}, selected={:?}, attempts={}, gap upper bound={:?}; overall search remains BudgetLimited/Deferred, never an optimality certificate.",step.state(),goal.direction(),goal.lower(),goal.upper(),step.initial_value(),step.selected_value(),step.attempts_requested(),step.gap_upper_bound()).map_err(|_|PostprocessError::OutputBudget)?;
            if let Some(quality) = step.goal_quality() {
                write!(
                    out,
                    " Declared finite goal quality=[{},{}].",
                    quality.lower(),
                    quality.upper()
                )
                .map_err(|_| PostprocessError::OutputBudget)?;
            } else {
                write!(
                    out,
                    " Goal normalization: {:?}; NoScalar/NeedsEvidence.",
                    step.normalization()
                )
                .map_err(|_| PostprocessError::OutputBudget)?;
            }
            write!(out," Strict-probe UNSAT is only SolverReported, not checked proof. Raw probe stdout/stderr are unowned; actual report/digests/counts remain retained.").map_err(|_|PostprocessError::OutputBudget)?;
            write!(out," The wall setting clamps dispatch allowances and native timeouts; preparation/cleanup are non-atomic under the separate shared real deadline.").map_err(|_|PostprocessError::OutputBudget)?;
        } else {
            write!(out," Parsed objective {:?} {name} with declared bounds [{}..={}] is retained and deferred; no optimization step or optimum is claimed.",goal.direction(),goal.lower(),goal.upper())
                .map_err(|_|PostprocessError::OutputBudget)?;
        }
    }
    if let Some(receipt) = measured {
        write!(out," Explicit finite-boolean-v1 / TheoryToWitnessModelSpace-v1: {:?}; original theory versus actual model valuation, all declared atoms and both polarities; narrowing measures lost model-space coverage, not incorrectness of a valid SAT witness.",receipt.state()).map_err(|_|PostprocessError::OutputBudget)?;
        if let (Some(sound), Some(complete), Some(semantic)) = (
            receipt.soundness(),
            receipt.completeness(),
            receipt.semantic(),
        ) {
            write!(out," Soundness loss=[{},{}]; completeness loss=[{},{}]; equal-direction model-space loss=[{},{}].",sound.lower(),sound.upper(),complete.lower(),complete.upper(),semantic.lower(),semantic.upper()).map_err(|_|PostprocessError::OutputBudget)?;
        }
    }
    if measured.is_some() {
        write!(out," Required reconstruction/total translation evaluators remain missing; total TL is NoScalar/MissingEvaluator, never measured zero. Source interpretation and shared-checker independence remain unverified.")
    } else {
        write!(out," Translation evaluators/probe plans are missing; TL is NoScalar/MissingEvaluator, never measured zero. Source interpretation and shared-checker independence remain unverified.")
    }.map_err(|_|PostprocessError::OutputBudget)?;
    write!(out, " {JAPANESE_LINE} {ENGLISH_LINE}").map_err(|_| PostprocessError::OutputBudget)?;
    Ok(out.text)
}

pub(super) fn incumbent_explanation(
    evidence: &NativeEvidence<'_, '_>,
    goal: &lc631_analysis::creativity_measurement::RationalInterval,
) -> Result<String, PostprocessError> {
    let native = evidence
        .native_model()
        .ok_or(PostprocessError::SourceBinding)?;
    let FormalInput::Problem(problem) = evidence.request.input else {
        return Err(PostprocessError::SourceBinding);
    };
    let mut out = BoundedText::new(8192)?;
    write!(out,"Retained initial native feasible incumbent, conditional on the declared formal interpretation; {} original assertions rechecked; assignments:",native.model().assertions_checked()).map_err(|_|PostprocessError::OutputBudget)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(problem.symbols().len())
        .map_err(|_| PostprocessError::AllocationFailed)?;
    for symbol in problem.symbols() {
        names.push((
            problem
                .source()
                .slice(symbol.span())
                .map_err(|_| PostprocessError::SourceBinding)?,
            symbol.symbol(),
        ));
    }
    names.sort_unstable_by_key(|(name, _)| *name);
    for (name, symbol) in names {
        match symbol {
            FormalSymbol::Boolean(id) => write!(
                out,
                " {name}={};",
                native
                    .model()
                    .boolean(id)
                    .ok_or(PostprocessError::SourceBinding)?
            ),
            FormalSymbol::Integer(id) => write!(
                out,
                " {name}={};",
                native
                    .model()
                    .integer(id)
                    .ok_or(PostprocessError::SourceBinding)?
            ),
        }
        .map_err(|_| PostprocessError::OutputBudget)?;
    }
    write!(out," Initial finite goal quality=[{},{}] under the SAME parsed bounds/direction. The separately rechecked improved witness does not establish certified dominance while required TL/map-back are missing. NoRecommendation; total TL is NoScalar/MissingEvaluator. Source interpretation/shared-checker independence remain unverified; no Delta/Gain or optimum is inferred.",goal.lower(),goal.upper()).map_err(|_|PostprocessError::OutputBudget)?;
    write!(out, " {JAPANESE_LINE} {ENGLISH_LINE}").map_err(|_| PostprocessError::OutputBudget)?;
    Ok(out.text)
}

pub(super) fn diagnostic(
    evidence: &NativeEvidence<'_, '_>,
    status: CanonicalStatus,
    reason: NoRecommendationReason,
    cap: usize,
) -> Result<CanonicalRendered, PostprocessError> {
    let mut out = BoundedText::new(cap)?;
    writeln!(out,"日本語・条件付き回答 / NoRecommendation: {reason:?}。実際の検証済みmodelをこの経路では提示できません。source解釈・TL・独立性・host配信は未確認です。")
        .map_err(|_|PostprocessError::OutputBudget)?;
    writeln!(out, "{JAPANESE_LINE}").map_err(|_| PostprocessError::OutputBudget)?;
    writeln!(out,"English: {status:?}; no recommendation ({reason:?}). No native-checked model is supplied by this outcome. Source interpretation, translation loss, independence and host delivery remain unverified.")
        .map_err(|_|PostprocessError::OutputBudget)?;
    writeln!(out, "{ENGLISH_LINE}").map_err(|_| PostprocessError::OutputBudget)?;
    match evidence.request.input {
        FormalInput::Malformed(error) => writeln!(
            out,
            "Formal parse clarification: {:?}; bytes {}..{}.",
            error.kind(),
            error.span().start(),
            error.span().end()
        ),
        FormalInput::Unsupported(report) => writeln!(
            out,
            "Explicit unsupported formal regions: {}; no executable problem or backend dispatch.",
            report.residuals().len()
        ),
        FormalInput::Problem(problem) => writeln!(
            out,
            "Declared source objective retained: {:?}.",
            problem.goal().map(|g| g.direction())
        ),
    }
    .map_err(|_| PostprocessError::OutputBudget)?;
    let source = evidence.request.source_revision();
    let binding = diagnostic_binding(source, out.text.as_bytes());
    Ok(CanonicalRendered::Diagnostic {
        source,
        text: out.text,
        binding,
    })
}
