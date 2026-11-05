use lc631_core::{context::*, AuthorityRevision};
use lc631_world::{context_graph::ContextGraph, materialized_worlds::*};
use std::sync::Arc;

fn graph() -> ContextGraph {
    let source = SourceRevision::of("source");
    let pin = source.digest();
    ContextGraph::new(
        OriginalContext::checked(
            source,
            ContextScope::declared(pin, pin, pin, pin),
            AuthorityRevision(1),
            vec![ContextPremise::reported(
                pin,
                Arc::from("premise"),
                PremiseBasis::Given,
                false,
            )],
            ContextLimits {
                max_contexts: 4,
                max_premises: 4,
                max_edits: 4,
                max_statement_bytes: 1024,
            },
        )
        .unwrap(),
        4,
    )
    .unwrap()
}

fn limits() -> MaterializedLimits {
    MaterializedLimits {
        max_live: 2,
        max_attempts: 20,
        max_payload_bytes: 1024,
        max_referenced_bytes: 4096,
    }
}

#[test]
fn tlcx26_aliases_do_not_pad_coverage_or_imply_execution() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let query = BoundWorldQuery::reported(context, b"bool-v1", b"p");
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let first = worlds.admit(query).unwrap();
    let second = worlds.admit(query).unwrap();
    assert_eq!(first.key(), second.key());
    assert!(second.is_duplicate());
    let coverage = worlds.coverage();
    assert_eq!(coverage.attempted, 2);
    assert_eq!(coverage.structural_live, 1);
    assert_eq!(coverage.semantic_distinct_unconfirmed, 1);
    assert_eq!(
        coverage.execution,
        ExecutionCoverage::Unobserved {
            reported_completed: 0
        }
    );
    assert_eq!(
        coverage.base,
        BaseCoverage::Insufficient {
            required: 256,
            actual: 1
        }
    );
}

#[test]
fn tlcx26_stale_and_foreign_keys_are_rejected() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let key = worlds
        .admit(BoundWorldQuery::reported(context, b"bool", b"p"))
        .unwrap()
        .key();
    worlds.remove(key).unwrap();
    let next = worlds
        .admit(BoundWorldQuery::reported(context, b"bool", b"q"))
        .unwrap()
        .key();
    assert_ne!(key, next);
    assert!(matches!(worlds.get(key), Err(MaterializedError::StaleKey)));
    let other = MaterializedWorlds::new(&graph, limits()).unwrap();
    assert!(matches!(
        other.get(next),
        Err(MaterializedError::ForeignArena)
    ));
}

#[test]
fn tlcx26_foreign_context_and_empty_queries_are_not_worlds() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    assert!(matches!(
        worlds.admit(BoundWorldQuery::reported(
            ContextRevision::original(SourceRevision::of("foreign")),
            b"bool",
            b"p",
        )),
        Err(MaterializedError::UnknownContext)
    ));
    assert!(matches!(
        worlds.admit(BoundWorldQuery::reported(context, b"bool", b"")),
        Err(MaterializedError::InvalidPayload)
    ));
    assert_eq!(worlds.coverage().structural_live, 0);
}

#[test]
fn tlcx26_result_binding_and_execution_claims_stay_separate() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let query = BoundWorldQuery::reported(context, b"bool", b"p");
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let key = worlds.admit(query).unwrap().key();
    let wrong = ReportedWorldResult::reported(
        BoundWorldQuery::reported(context, b"bool", b"q"),
        ReportedRunState::Completed,
        Some(b"model"),
        b"sat",
    );
    assert_eq!(
        worlds.attach_reported(key, wrong),
        Err(MaterializedError::QueryMismatch)
    );
    let result =
        ReportedWorldResult::reported(query, ReportedRunState::Completed, Some(b"model"), b"sat");
    worlds.attach_reported(key, result).unwrap();
    assert_eq!(
        worlds
            .get(key)
            .unwrap()
            .reported_result()
            .unwrap()
            .result_bytes(),
        b"sat"
    );
    assert_eq!(
        worlds.coverage().execution,
        ExecutionCoverage::Unobserved {
            reported_completed: 1
        }
    );
    assert_eq!(
        worlds.attach_reported(key, result),
        Err(MaterializedError::ResultAlreadyPresent)
    );
}

#[test]
fn tlcx26_limits_refuse_silent_padding_and_history_growth() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let mut small = limits();
    small.max_live = 1;
    small.max_attempts = 2;
    let mut worlds = MaterializedWorlds::new(&graph, small).unwrap();
    worlds
        .admit(BoundWorldQuery::reported(context, b"bool", b"p"))
        .unwrap();
    assert!(matches!(
        worlds.admit(BoundWorldQuery::reported(context, b"bool", b"q")),
        Err(MaterializedError::LiveCapacity)
    ));
    assert!(matches!(
        worlds.admit(BoundWorldQuery::reported(context, b"bool", b"p")),
        Err(MaterializedError::AttemptBudget)
    ));
    assert_eq!(worlds.coverage().attempted, 2);
}

#[test]
fn tlcx26_exact_profile_payload_changes_are_structural_only() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
    let p = worlds
        .admit(BoundWorldQuery::reported(context, b"classical", b"p"))
        .unwrap();
    let q = worlds
        .admit(BoundWorldQuery::reported(context, b"four-valued", b"p"))
        .unwrap();
    assert_ne!(p.key(), q.key());
    assert_eq!(worlds.coverage().semantic_distinct_unconfirmed, 2);
}

#[test]
fn tlcx26_payload_budget_failure_does_not_publish_partial_result() {
    let graph = graph();
    let context = graph.states()[0].revision();
    let query = BoundWorldQuery::reported(context, b"b", b"p");
    let mut small = limits();
    small.max_referenced_bytes = 3;
    let mut worlds = MaterializedWorlds::new(&graph, small).unwrap();
    let key = worlds.admit(query).unwrap().key();
    let result = ReportedWorldResult::reported(query, ReportedRunState::Timeout, None, b"timeout");
    assert_eq!(
        worlds.attach_reported(key, result),
        Err(MaterializedError::PayloadBudget)
    );
    assert!(worlds.get(key).unwrap().reported_result().is_none());
    assert_eq!(worlds.coverage().referenced_bytes, 2);
}

#[test]
fn tlcx26_all_four_step_small_traces_match_independent_set_model() {
    use std::collections::{BTreeMap, BTreeSet};
    let graph = graph();
    let context = graph.states()[0].revision();
    let payloads: [&[u8]; 2] = [b"p", b"q"];
    for trace in 0u16..256 {
        let mut worlds = MaterializedWorlds::new(&graph, limits()).unwrap();
        let mut model = BTreeSet::new();
        let mut handles = BTreeMap::new();
        let mut attempts = 0;
        for step in 0..4 {
            let operation = (trace >> (2 * step)) & 3;
            let label = usize::from(operation & 1);
            if operation < 2 {
                attempts += 1;
                let was_present = model.contains(&label);
                let admission = worlds
                    .admit(BoundWorldQuery::reported(
                        context,
                        b"finite-bool",
                        payloads[label],
                    ))
                    .unwrap();
                assert_eq!(admission.is_duplicate(), was_present);
                model.insert(label);
                handles.insert(label, admission.key());
            } else if let Some(key) = handles.remove(&label) {
                worlds.remove(key).unwrap();
                model.remove(&label);
                assert!(worlds.get(key).is_err());
            }
            assert_eq!(worlds.coverage().structural_live, model.len());
            assert_eq!(worlds.coverage().semantic_distinct_unconfirmed, model.len());
            assert_eq!(worlds.coverage().attempted, attempts);
        }
    }
}
