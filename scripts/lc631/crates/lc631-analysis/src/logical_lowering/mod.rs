//! Bounded, source-bound task lowering for the declared instruction grammar.
//! Atoms are uninterpreted candidates; a syntax tree is never solver truth.
mod builder;
mod conditions;
mod model;

pub(crate) use model::LogicalDirective;
pub use model::*;

use crate::{context_lowering::LoweredContext, DgclPipelineReport};
use lc631_core::context::TaskKind;

/// No default TaskKind or inferred logic. The existing legacy JSON report is
/// borrowed, never expanded or modified by this typed internal route.
pub fn lower_dgcl_logical(
    report: &DgclPipelineReport,
    context: &LoweredContext,
    task: TaskKind,
    profile: LogicProfile,
    budget: LogicalLoweringBudget,
) -> Result<LogicalTask, LogicalLoweringError> {
    builder::lower(report, context, task, profile, budget)
}

fn formula_owner(
    context: lc631_core::context::ContextRevision,
    identity: lc631_substrate::Digest32,
    task: TaskKind,
    profile: LogicProfile,
    program: &str,
    grammar: &str,
) -> lc631_substrate::Digest32 {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"epistesys.logical-task-owner.v1\0");
    hash.update(context.source().digest().as_bytes());
    hash.update(identity.as_bytes());
    hash.update([
        match task {
            TaskKind::Mathematics => 0,
            TaskKind::Reasoning => 1,
            TaskKind::Explanation => 2,
            TaskKind::Coding => 3,
            TaskKind::Media => 4,
        },
        profile.code(),
    ]);
    for field in [program, grammar, profile.revision()] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    lc631_substrate::Digest32::from_bytes(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_dgcl_pipeline, context_lowering::lower_dgcl_context, DgclBackendSet};
    use lc631_core::{
        context::{ContextLimits, ContextScope, SourceRevision},
        AuthorityRevision,
    };

    #[test]
    fn tlcx09_formula_ref_rejects_program_revision_rebinding() {
        let report = build_dgcl_pipeline(
            "Please verify the result if ready.",
            DgclBackendSet {
                english: None,
                japanese: None,
            },
        )
        .unwrap();
        let pin = SourceRevision::of("scope").digest();
        let context = lower_dgcl_context(
            &report,
            ContextScope::declared(pin, pin, pin, pin),
            AuthorityRevision(1),
            ContextLimits {
                max_contexts: 4,
                max_premises: 1024,
                max_edits: 4,
                max_statement_bytes: 262_144,
            },
        )
        .unwrap();
        let mut task = lower_dgcl_logical(
            &report,
            &context,
            TaskKind::Coding,
            LogicProfile::ClassicalPropositionalV1,
            LogicalLoweringBudget::default(),
        )
        .unwrap();
        assert!(!task.formulas().is_empty());
        let id = task.formulas()[0].id();
        let original_digest = task.program.legacy_program_digest.clone();
        task.program.legacy_program_digest = "changed-program-revision".into();
        assert!(matches!(
            task.program.formula(id),
            Err(LogicalLoweringError::ForeignFormula)
        ));
        task.program.legacy_program_digest = original_digest;
        assert!(task.program.formula(id).is_ok());
        task.program.parser_grammar_revision = "changed-grammar-revision";
        assert!(matches!(
            task.program.formula(id),
            Err(LogicalLoweringError::ForeignFormula)
        ));
    }
}
