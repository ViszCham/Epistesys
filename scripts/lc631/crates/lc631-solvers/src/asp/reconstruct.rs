use super::evaluate::{charge, is_stable};
use super::{AspCompleteness, AspError, AspEvaluation, AspProgram, AtomId};
use crate::protocol::clingo::{
    parse_clingo_output, ClingoOutputError, ClingoOutputLimits, ClingoOutputView,
    ClingoTerminalStatus, MAX_CLINGO_ANSWER_BLOCKS, MAX_CLINGO_ATOM_LINES,
};
use crate::result_store::StoredSolverResult;
use crate::runner::{
    output_witness_digest, ProcessCleanupState, ProcessRunState, SolverExecutionState,
};

impl AspProgram {
    /// Checks bytes against a stored runner observation before semantic reconstruction.
    /// This is identity/finite validation, not process authentication or a receipt issuer.
    /// First look up the store using the caller's current invocation binding;
    /// this method does not establish current version, expiry or revocation itself.
    pub fn reconstruct_stored(
        &self,
        stored: &StoredSolverResult,
        stdout: &[u8],
    ) -> Result<AspEvaluation, AspError> {
        let prepared = self.prepare()?;
        let query = prepared.solver_query()?;
        let report = stored.report();
        if stored.binding().source() != query.source() || stored.binding().query() != query.digest()
        {
            return Err(AspError::BindingMismatch);
        }
        if report.state() != SolverExecutionState::SolverReported
            || !report.stdout_fully_observed()
            || report.stdout_truncated()
            || report.stderr_truncated()
            || !report.stderr_fully_observed()
            || report.process_state() != Some(ProcessRunState::Completed)
            || report.cleanup_state() != ProcessCleanupState::RunnerReportedVerified
        {
            return Err(AspError::ObservationIncomplete);
        }
        let digest = output_witness_digest(b"stdout", stdout, stdout.len(), false);
        if report.stdout_bytes() != stdout.len() || report.stdout_digest() != digest {
            return Err(AspError::OutputMismatch);
        }
        self.reconstruct(stdout)
    }

    /// Pure reconstruction of supplied text. The caller supplies bytes, not evidence.
    /// A SAT answer is complete only when it matches independent finite enumeration.
    pub fn reconstruct(&self, stdout: &[u8]) -> Result<AspEvaluation, AspError> {
        let limits = ClingoOutputLimits::checked(
            self.limits.output,
            MAX_CLINGO_ANSWER_BLOCKS,
            MAX_CLINGO_ATOM_LINES,
        )
        .map_err(|_| AspError::OutputLimit)?;
        let view = parse_clingo_output(stdout, limits).map_err(|error| match error {
            ClingoOutputError::UnsupportedStatus => AspError::UnsupportedOutput,
            ClingoOutputError::OutputTooLarge
            | ClingoOutputError::TooManyAnswers
            | ClingoOutputError::TooManyAtomLines => AspError::OutputLimit,
            _ => AspError::OutputSyntax,
        })?;
        let masks = self.reported_masks(&view)?;
        self.recheck_reported(view.status(), &masks)
    }

    // Syntax and duplicate admission precede semantic cutoff, including UNKNOWN.
    fn reported_masks(&self, view: &ClingoOutputView<'_>) -> Result<Vec<(u32, u16)>, AspError> {
        let mut masks = Vec::new();
        masks
            .try_reserve_exact(view.answer_blocks().len())
            .map_err(|_| AspError::AllocationFailed)?;
        for answer in view.answer_blocks() {
            let lines = view.atom_lines_for(answer).ok_or(AspError::OutputSyntax)?;
            let mut mask = 0u16;
            for line in lines {
                let text = view.text(line.span()).ok_or(AspError::OutputSyntax)?;
                for token in text.split_ascii_whitespace() {
                    let atom = parse_atom(token)?;
                    let bit = self.mask(&[atom])?;
                    if mask & bit != 0 {
                        return Err(AspError::DuplicateModelAtom);
                    }
                    mask |= bit;
                }
            }
            if masks.iter().any(|&(_, prior)| prior == mask) {
                return Err(AspError::DuplicateModel);
            }
            masks.push((answer.number(), mask));
        }
        Ok(masks)
    }

    fn recheck_reported(
        &self,
        status: ClingoTerminalStatus,
        masks: &[(u32, u16)],
    ) -> Result<AspEvaluation, AspError> {
        let mut models = Vec::new();
        models
            .try_reserve_exact(masks.len().min(self.limits.models))
            .map_err(|_| AspError::AllocationFailed)?;
        let compiled = self.compiled_rules()?;
        let mut steps = 0;
        for &(answer, mask) in masks {
            if models.len() == self.limits.models {
                return self.evaluation(models, AspCompleteness::ModelLimit, steps, Some(status));
            }
            if !charge(&mut steps, self.limits.steps) {
                return self.evaluation(models, AspCompleteness::StepLimit, steps, Some(status));
            }
            match is_stable(&compiled, mask, &mut steps, self.limits.steps) {
                Some(true) => models.push(self.model(mask, Some(answer))?),
                Some(false) => return Err(AspError::InvalidStableModel),
                None => {
                    return self.evaluation(models, AspCompleteness::StepLimit, steps, Some(status))
                }
            }
        }
        drop(compiled);
        if status == ClingoTerminalStatus::Unknown {
            return self.evaluation(models, AspCompleteness::SolverUnknown, steps, Some(status));
        }
        let enumerated = self.enumerate_with_steps(&mut steps)?;
        if enumerated.completeness() != AspCompleteness::Exhaustive {
            return self.evaluation(models, enumerated.completeness(), steps, Some(status));
        }
        match status {
            ClingoTerminalStatus::Satisfiable => {
                if enumerated.models().is_empty() {
                    return Err(AspError::StatusMismatch);
                }
                if models.len() != enumerated.models().len() {
                    return Err(AspError::IncompleteAnswerSet);
                }
            }
            ClingoTerminalStatus::Unsatisfiable => {
                if !enumerated.models().is_empty() {
                    return Err(AspError::StatusMismatch);
                }
            }
            ClingoTerminalStatus::Unknown => return Err(AspError::StatusMismatch),
        }
        self.evaluation(models, AspCompleteness::Exhaustive, steps, Some(status))
    }
}

fn parse_atom(token: &str) -> Result<AtomId, AspError> {
    let digits = token.strip_prefix('a').ok_or(AspError::InvalidAtomSyntax)?;
    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err(AspError::InvalidAtomSyntax);
    }
    Ok(AtomId::new(
        digits.parse().map_err(|_| AspError::InvalidAtomSyntax)?,
    ))
}
