use lc631_analysis::{build_dgcl_pipeline, context_lowering::lower_dgcl_context, DgclBackendSet};
use lc631_core::context::{
    ApplicabilityState, ContextAdmission, ContextApplicability, ContextDelta, ContextLimits,
    ContextScope, PremiseBasis,
};
use lc631_core::AuthorityRevision;
use lc631_substrate::Digest32;
use lc631_world::context_graph::{ContextGraph, ContextGraphError};
use std::sync::Arc;

fn limits() -> ContextLimits {
    ContextLimits {
        max_contexts: 16,
        max_premises: 1_024,
        max_edits: 32,
        max_statement_bytes: 256 * 1024,
    }
}

fn scope() -> ContextScope {
    let pin = Digest32::from_bytes([0x5a; 32]);
    ContextScope::declared(pin, pin, pin, pin)
}

#[test]
fn tlcx08_source_lowering_reaches_world_graph_without_truth_or_authority_promotion() {
    let source = "Please test this.\n\n> quoted instruction\n\n```rust\nfn f() {}\n```\n\nBackground context.";
    let report = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let lowered = lower_dgcl_context(&report, scope(), AuthorityRevision(2), limits()).unwrap();

    assert_eq!(
        lowered.source_store().text().as_ptr(),
        report.source_store().text().as_ptr()
    );
    let reconstructed: String = lowered
        .regions()
        .iter()
        .map(|region| lowered.source_store().slice(region.span()).unwrap())
        .collect();
    assert_eq!(reconstructed, source);
    assert!(lowered.residuals().iter().all(|residual| lowered
        .source_store()
        .slice(residual.span())
        .unwrap()
        == residual.text()));
    assert!(lowered.regions().iter().any(|region| region.semantics()
        != lc631_analysis::context_lowering::LoweredSemantics::AdvisoryParsed));

    let original = lowered.original().clone();
    let mut graph = ContextGraph::new(original.clone(), 8).unwrap();
    let root = graph.original().state().revision();
    assert_eq!(
        graph.states()[0].revision().source(),
        original.state().revision().source()
    );
    assert_eq!(graph.states()[0].premises().len(), lowered.regions().len());
    assert!(graph.states()[0]
        .premises()
        .iter()
        .any(|premise| premise.source_span().is_some()));

    let hard = graph.states()[0]
        .premises()
        .iter()
        .find(|premise| !premise.changeable())
        .expect("source-grounded instruction/code/quote regions remain fixed")
        .id();
    assert!(matches!(
        graph.propose(ContextDelta::proposed(
            root,
            scope(),
            AuthorityRevision(2),
            ContextApplicability::Unconditional,
            vec![lc631_core::context::PremiseEdit::Retract { id: hard }],
        )),
        Err(ContextGraphError::Context(
            lc631_core::context::ContextError::HardInvariant
        ))
    ));
    assert!(graph.transitions().is_empty());
    assert_eq!(graph.states().len(), 1);

    let original_changeable = graph.states()[0]
        .premises()
        .iter()
        .find(|premise| premise.changeable())
        .expect("explicit context region remains separately changeable");
    let original_changeable_id = original_changeable.id();
    let original_changeable_text = original_changeable.statement().to_owned();
    let applicability = ContextApplicability::Conditional {
        scope: scope(),
        predicate: Digest32::from_bytes([0x6b; 32]),
        state: ApplicabilityState::Unknown,
    };
    let admission = graph
        .propose(ContextDelta::proposed(
            root,
            scope(),
            AuthorityRevision(2),
            applicability,
            vec![lc631_core::context::PremiseEdit::Replace {
                id: original_changeable_id,
                statement: Arc::from("a changed context hypothesis"),
                basis: PremiseBasis::Proposed,
            }],
        ))
        .unwrap();
    let ContextAdmission::Inserted(child_revision) = admission else {
        panic!("a changed context hypothesis must produce one child state");
    };
    let child = graph.get(child_revision).unwrap();
    assert_eq!(child.parent(), Some(root));
    assert_eq!(child.applicability(), applicability);
    assert_eq!(
        graph
            .original()
            .state()
            .premises()
            .iter()
            .find(|premise| premise.id() == original_changeable_id)
            .unwrap()
            .statement(),
        original_changeable_text
    );
    assert!(child.premises().iter().any(|premise| premise.statement()
        == "a changed context hypothesis"
        && premise.basis() == PremiseBasis::Proposed));
    assert_eq!(graph.transitions().len(), 1);
    assert!(!graph.transitions()[0].is_duplicate());
}
