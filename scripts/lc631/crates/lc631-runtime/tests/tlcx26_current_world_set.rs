//! Current finite-world witness selection, not source-meaning validation.
use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::{finite_results::FiniteResultStore, materialized_world_bridge::{*, current_set::*}};
use lc631_solvers::temporal::*;
use lc631_substrate::{resource::*, Digest32};
use lc631_world::{context_graph::ContextGraph, materialized_worlds::*};
use std::{sync::Arc, time::Duration};

fn exercise(truth: Truth) {
    let source = SourceRevision::of("current world set");
    let pin = source.digest();
    let scope = ContextScope::declared(pin, pin, pin, pin);
    let graph = ContextGraph::new(OriginalContext::checked(source, scope, AuthorityRevision(1),
        vec![ContextPremise::reported(pin, Arc::from("Given trace"), PremiseBasis::Given, false)],
        ContextLimits { max_contexts: 2, max_premises: 2, max_edits: 2, max_statement_bytes: 128 }).unwrap(), 2).unwrap();
    let context = graph.states()[0].revision();
    let q = TemporalQuery::checked(context, vec![TemporalNode::Atom(0)], 0,
        TemporalProfile::ClosedFiniteKleeneV1).unwrap();
    let clock_pin = Digest32::from_bytes([4; 32]);
    let clock = ClockSpec::exact(clock_pin, TimeScale::Monotonic, clock_pin, clock_pin);
    let trace = FiniteTrace::checked(vec![TraceStep::new(
        Instant::absolute(clock, 0, TimeUnit::Second).unwrap(), vec![truth])], TraceClosure::Closed).unwrap();
    let cancel = Cancellation::default();
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let lease = issuer.issue_admitted(10, ReportedResourceLease::checked(7, pin,
        &[ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(1024 * 1024).unwrap())],
        Deadline::after(Duration::from_secs(30)).unwrap(), cancel.clone()).unwrap()).unwrap();
    let view = lease.view();
    let prepared = PreparedTemporalWorld::with_resources(&q, &trace, &view).unwrap();
    let mut worlds = MaterializedWorlds::new(&graph, MaterializedLimits { max_live: 2, max_attempts: 8,
        max_payload_bytes: 8192, max_referenced_bytes: 32768 }).unwrap();
    let key = worlds.admit(prepared.world_query()).unwrap().key();
    let mut store = FiniteResultStore::with_resources(1, &view).unwrap();
    let id = store.evaluate_temporal(&q, &trace, context, scope, EvaluationBudget::default(), &view).unwrap();
    let observed = observe_temporal_world_current(&worlds, key, &prepared, &store, id, scope, &view).unwrap();
    let rows = [&observed, &observed];
    let set = CurrentWorldSet::capture(&rows, scope, &view).unwrap();
    assert_eq!(set.coverage_current(&view).unwrap().executed_live, 1);
    let yes = TruthWitnessPlan::checked(scope, Truth::True, 1).unwrap();
    let no = TruthWitnessPlan::checked(scope, Truth::False, 1).unwrap();
    let expected_yes = match truth {
        Truth::True => TruthWitness::Unique { row: 0 },
        Truth::False => TruthWitness::Absent,
        Truth::Unknown => TruthWitness::NeedsEvidence { unknown_inputs: 1 },
    };
    let expected_no = match truth {
        Truth::False => TruthWitness::Unique { row: 0 },
        Truth::True => TruthWitness::Absent,
        Truth::Unknown => TruthWitness::NeedsEvidence { unknown_inputs: 1 },
    };
    assert_eq!(set.consume_truth_current(yes, &view).unwrap(), expected_yes);
    assert_eq!(set.consume_truth_current(no, &view).unwrap(), expected_no);
    let two = TruthWitnessPlan::checked(scope, Truth::True, 2).unwrap();
    assert!(matches!(set.consume_truth_current(two, &view), Err(CurrentSetError::InsufficientCoverage)));
    assert!(TruthWitnessPlan::checked(scope, Truth::Unknown, 1).is_err());
    let foreign = ContextScope::declared(Digest32::from_bytes([9; 32]), pin, pin, pin);
    assert!(CurrentWorldSet::capture(&rows, foreign, &view).is_err());
    cancel.cancel();
    assert!(set.coverage_current(&view).is_err());
    assert!(set.consume_truth_current(yes, &view).is_err());
    assert_eq!(observed.historical_report().truth(), truth);
}

#[test]
fn tlcx26_current_set_consumes_true_deduplicates_and_rejects_cancel() { exercise(Truth::True); }
#[test]
fn tlcx26_current_set_consumes_false_without_promoting_it_to_unknown() { exercise(Truth::False); }
#[test]
fn tlcx26_current_set_unknown_never_becomes_absent_or_unique() { exercise(Truth::Unknown); }
