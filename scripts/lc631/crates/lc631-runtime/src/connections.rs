//! Required edge contracts. A declaration is explicitly not an observation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    NotConnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectionContract {
    pub id: &'static str,
    pub producer: &'static str,
    pub consumer: &'static str,
    pub state: ConnectionState,
}

const fn edge(
    id: &'static str,
    producer: &'static str,
    consumer: &'static str,
) -> ConnectionContract {
    ConnectionContract {
        id,
        producer,
        consumer,
        state: ConnectionState::NotConnected,
    }
}

static CONNECTIONS: [ConnectionContract; 20] = [
    edge("E01", "host_ingress", "source_store"),
    edge("E02", "source_store", "deep_grammar_context_ir"),
    edge("E03", "context_ir", "context_graph"),
    edge("E04", "context_graph", "solver_projection"),
    edge("E05", "solver_projection", "supervised_process"),
    edge("E06", "solver_result", "result_validator"),
    edge("E07", "validated_observation", "translation_ledger"),
    edge("E08", "candidate_provider", "creative_assessment"),
    edge("E09", "loss_and_gain", "frontier"),
    edge("E10", "frontier_and_geometry", "scheduler"),
    edge("E11", "scheduler", "resource_broker_backend"),
    edge(
        "E12",
        "continuous_proposal",
        "discrete_validation_next_epoch",
    ),
    edge("E13", "selection", "program_ir_task_plan"),
    edge("E14", "coding_candidate_permit", "validation_producers"),
    edge("E15", "repair_checkpoint", "resume"),
    edge("E16", "media_rpa_nlp", "candidate_premises"),
    edge("E17", "observed_numeric_batch", "gpu_cpu_final_check"),
    edge("E18", "decision_artifact", "exact_renderer"),
    edge("E19", "render", "host_stages_and_replay"),
    edge("E20", "artifact_store", "cache_journal_security_consumers"),
];

pub fn connection_contracts() -> &'static [ConnectionContract; 20] {
    &CONNECTIONS
}
