use lc631_analysis::{build_dgcl_pipeline, context_lowering::*, DgclBackendSet};
use lc631_core::{context::*, AuthorityRevision};

fn limits() -> ContextLimits {
    ContextLimits {
        max_contexts: 16,
        max_premises: 1024,
        max_edits: 32,
        max_statement_bytes: 262_144,
    }
}

fn scope() -> ContextScope {
    let pin = SourceRevision::of("declared test comparison").digest();
    ContextScope::declared(pin, pin, pin, pin)
}

#[test]
fn tlcx08_lowering_preserves_every_byte_and_residual_without_truth() {
    let source = "Please test this.\n\n> quoted command\n\n```rust\nfn f() {}\n```\n\n背景情報。";
    let report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let lowered = lower_dgcl_context(&report, scope(), AuthorityRevision(7), limits()).unwrap();
    assert_eq!(
        lowered.source_store().text().as_ptr(),
        report.source_store().text().as_ptr()
    );
    let reconstructed: String = lowered
        .regions()
        .iter()
        .map(|r| lowered.source_store().slice(r.span()).unwrap())
        .collect();
    assert_eq!(reconstructed, source);
    assert_eq!(
        lowered.residuals().len(),
        report.dg1.instruction_residuals.len()
    );
    assert_eq!(
        lowered.original().state().premises().len(),
        lowered.regions().len()
    );
    assert!(lowered
        .original()
        .state()
        .premises()
        .iter()
        .all(|p| p.basis() == PremiseBasis::Proposed));
    assert!(lowered
        .regions()
        .iter()
        .any(|r| r.semantics() != LoweredSemantics::AdvisoryParsed));
    for residual in lowered.residuals() {
        assert_eq!(
            lowered.source_store().slice(residual.span()).unwrap(),
            residual.text()
        );
    }
}

#[test]
fn tlcx08_lowering_rejects_mutated_or_rebound_report() {
    let mut report = build_dgcl_pipeline(
        "Please test this.",
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    report.dg1.source_accounting[0].source_span.start = 1;
    assert!(matches!(
        lower_dgcl_context(&report, scope(), AuthorityRevision(7), limits()),
        Err(ContextLoweringError::PipelineIdentity)
    ));
}

#[test]
fn tlcx08_lowering_applies_count_and_byte_budgets_before_publication() {
    let report = build_dgcl_pipeline(
        "Please test this.\n\nMore context.",
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let mut small = limits();
    small.max_premises = 1;
    assert!(matches!(
        lower_dgcl_context(&report, scope(), AuthorityRevision(7), small),
        Err(ContextLoweringError::Context(ContextError::BudgetExceeded))
    ));
    small = limits();
    small.max_statement_bytes = 1;
    assert!(matches!(
        lower_dgcl_context(&report, scope(), AuthorityRevision(7), small),
        Err(ContextLoweringError::Context(ContextError::BudgetExceeded))
    ));
}
