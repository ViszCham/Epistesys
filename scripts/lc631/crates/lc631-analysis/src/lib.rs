#![forbid(unsafe_code)]

pub const PROGRAM_ANALYSIS_SCHEMA: &str = "lc631-program-analysis.v1";

pub mod bounded_input;
pub mod breakcost_measurement;
pub mod context_lowering;
pub mod creative_candidates;
pub mod creativity_measurement;
pub mod declared_build_capture;
pub mod logical_lowering;
pub mod reconstruction_loss;
pub mod solver_coding_bridge;
pub mod spatiotemporal_loss;
pub use cargo_world::observe_cargo_world_with_resources;

mod assembly;
mod cargo_world;
mod closure;
mod closure_connection;
mod closure_resume;
mod completion_plan;
mod connection_graph;
mod corpus;
mod dgcl_artifact;
mod dgcl_consumer;
mod dgcl_distillation;
mod dgcl_final_output;
mod dgcl_gold;
mod dgcl_implementation_closure;
mod dgcl_pipeline;
mod dgcl_projection;
mod evidence_lifecycle;
mod file_repair;
mod instruction_grammar;
mod language_worker;
mod model;
mod pipeline;
mod process;
mod production_connection;
mod python;
mod python_script;
mod release;
mod repair_loop;
mod rust;
mod validation;
mod validation_producer;

pub use validation_producer::{
    capture_dgcl_cargo_snapshot, capture_dgcl_cargo_snapshot_with_runner, check_cargo_test_result,
    dgcl_cargo_registration_payload, dgcl_cargo_validation_scope,
    dgcl_implementation_binding_payload, dgcl_repository_root_digest, dgcl_validator_revision,
    issue_dgcl_cargo_evidence, issue_dgcl_implementation_binding, observe_dgcl_cargo_validation,
    observe_dgcl_cargo_validation_with_runner, preflight_dgcl_cargo_plan,
    preflight_dgcl_cargo_plan_with_runner, project_dgcl_cargo_target_snapshot,
    verify_dgcl_cargo_registration, CheckedDgclCargoObservation, DgclBindingIssuerContext,
    DgclCargoExecutionContext, DgclCargoValidationPlan, DgclValidationProducerError,
};

pub use production_connection::{
    dgcl_connection_registration_payload, dgcl_connection_registration_scope, DgclConnectionSpec,
    DIRECT_API_COVERAGE, DIRECT_API_OBSERVER,
};

pub use closure::{
    build_coding_closure, coding_evidence_payload_digest, coding_evidence_scope,
    CodingClosureError, CodingClosureReport, CodingClosureStatus, CodingEvidenceClaim,
    CodingEvidenceKind, CodingEvidenceProducer, CodingEvidenceRejection, CodingEvidenceState,
    CodingGap, CodingGapKind,
};
pub use closure_connection::{
    issue_connection_evidence, issue_observed_connection_evidence,
    observe_connection_plan_from_cli, observe_connection_plan_from_cli_with_consumer,
    observe_connection_plan_from_cli_with_resources, verify_connection_plan,
    verify_connection_plan_from_cli, verify_connection_plan_from_cli_with_resources,
    verify_connection_plan_with_resources, CodingConnectionPlan, CodingConnectionReport,
    ConnectionError, ConnectionStatus, VerifiedConnectionObservation,
};
pub use closure_resume::{
    append_checkpoint, append_checkpoint_with_resources, inspect_checkpoint_record_bytes,
    inspect_resume, inspect_resume_with_anchor, inspect_resume_with_anchor_with_resources,
    inspect_resume_with_historical_anchor, inspect_resume_with_historical_anchor_with_resources,
    inspect_resume_with_resources, issue_checkpoint_head_anchor, load_checkpoint_head,
    load_checkpoint_head_historical, load_checkpoint_head_historical_with_resources,
    load_checkpoint_head_with_resources, persist_checkpoint_head, persist_checkpoint_head_renewal,
    persist_checkpoint_head_renewal_with_resources, persist_checkpoint_head_with_resources,
    read_checkpoint_record_with_resources, CheckpointBody, CheckpointEventKind,
    CheckpointHeadAnchor, CheckpointHeadRequest, CheckpointIntegrityState, CheckpointReadIdentity,
    DeliveryState, ResumeError, ResumeObservation, DGCL_CHECKPOINT_V1_SCHEMA,
    DGCL_CHECKPOINT_V2_SCHEMA, MAX_CHECKPOINT_CHAIN_BYTES, MAX_CHECKPOINT_RECORDS,
    MAX_CHECKPOINT_RECORD_BYTES,
};
pub use completion_plan::{
    audit_completion_assignments, build_dgcl_completion_plan, verify_dgcl_completion_plan,
    CompletionAuditIssue, CompletionAuditIssueKind, CompletionAuditReport, CompletionAuditState,
    CompletionBlocker, CompletionBlockerKind, CompletionPlanError, CompletionPlanStatus,
    CompletionTaskAssignment, DgclCompletionPlan, EvidenceProducer, NotApplicableBasis,
    PlannedCodingRequirement, PlannedCondition, PlannedEvidenceTask, PlannedTargetCandidate,
    TaskApplicability, TaskAssignmentDisposition, DGCL_COMPLETION_PLAN_SCHEMA,
};
pub use connection_graph::{
    analyze_connection_graph_sources, ConnectionGraphEdge, ConnectionGraphEdgeKind,
    ConnectionGraphNode, ConnectionGraphNodeKind, StaticConnectionGraphError,
    StaticConnectionGraphReport, StaticConnectionState, STATIC_CONNECTION_GRAPH_SCHEMA,
};
pub use dgcl_artifact::{
    build_dgcl_artifact, ArtifactId, DgclArtifact, DgclArtifactError, DgclArtifactRequest,
    GrammarRevision, ProfileRevision, RequirementIdentity, SourceRevision, SourceSnapshot,
};
pub use dgcl_consumer::{
    project_dgcl_consumer_view, DgclConsumerVersion, LegacyDgclConsumerView, LegacyDgclViewSchema,
};
pub use dgcl_distillation::{
    distill_dgcl_constraints, DgclConstraintWitness, DgclStructuralDistillation,
};
pub use dgcl_final_output::{
    build_dgcl_standalone_candidate, build_verified_dgcl_completion_candidate,
    DgclHostObservationStages, DgclOutputRealizationState, DgclRequirementOutputRealization,
    DgclStandaloneCandidate, DgclStandaloneCandidateError, DgclStandaloneCandidateState,
    HostObservationState, DGCL_STANDALONE_CANDIDATE_SCHEMA,
};
pub use dgcl_gold::{
    evaluate_gold, evaluate_gold_decisions, gold_adjudication_payload_digest,
    gold_annotation_payload_digest, gold_case_content_digest, validate_gold_provenance,
    Dg1GoldCondition, Dg1GoldPolarity, DgclDecisionObservation, DgclObservedDecision, GoldCase,
    GoldCaseProvenance, GoldCaseProvenanceObservation, GoldCorpus, GoldDecisionEvaluationReport,
    GoldError, GoldEvaluationReport, GoldGateState, GoldLanguage, GoldPartition,
    GoldProvenanceBundle, GoldProvenanceReport, GoldRequirement, GoldSignerAttestation,
    LanguageDecisionMetrics, LanguageGoldMetrics, RiskCoveragePoint,
};
pub use dgcl_implementation_closure::{
    build_dgcl_implementation_closure, DgclEvidenceRejection, DgclImplementationClosureContext,
    DgclImplementationClosureError, DgclImplementationClosureReport, DgclImplementationGap,
    DgclImplementationStatus, DgclRequirementClosure, ImplementationGapState,
    DGCL_IMPLEMENTATION_CLOSURE_SCHEMA,
};
pub use dgcl_pipeline::{
    build_dgcl_pipeline, build_dgcl_pipeline_with_runner, build_dgcl_pipeline_with_source_observer,
    verify_dgcl_pipeline_identity, DgclBackendSet, DgclBindingState, DgclInstructionParse,
    DgclInstructionParseState, DgclLanguageRegion, DgclPipelineError, DgclPipelineReport,
    DgclRequirementBinding, DgclSourceObserver, DgclWorkerRunner,
};
pub use dgcl_projection::{
    build_dgcl_projection, verify_dgcl_projection, DgclProgramAlternative, DgclProgramDependency,
    DgclProgramIrReport, DgclProgramRequirement, DgclProgramResidual, DgclProjectionError,
    DgclTranslationProjection, ProjectionDefectDimension, TranslationLossEvent,
    TranslationLossKind, DGCL_PROGRAM_IR_SCHEMA, DGCL_TRANSLATION_PROJECTION_SCHEMA,
};
pub use evidence_lifecycle::{
    advance_dgcl_target_snapshot, build_dgcl_execution_snapshot, issue_dgcl_evidence_lease,
    lifecycle_binds_target, validate_dgcl_evidence_lease, validate_dgcl_target_lease,
    DgclEvidenceLease, DgclExecutionSnapshot, DgclLifecycleError, DgclLifecycleObservation,
    DgclLifecycleState, DgclTargetIdentity, DGCL_EVIDENCE_LIFECYCLE_SCHEMA,
};
pub use file_repair::{
    DgclFileEffectOwner, DgclFileEffectPlan, DgclFileEvidenceValidator, DgclFileRepairConfig,
    DgclFileRepairDriver,
};
pub use instruction_grammar::{
    parse_controlled_instruction, ConditionEvidenceState, ConditionExpression, ConditionPredicate,
    ConditionRelation, ConditionScopeState, InstructionAction, InstructionAst,
    InstructionCondition, InstructionConjunction, InstructionConstraintEdge,
    InstructionConstraintGraph, InstructionConstraintKind, InstructionConstraintNode,
    InstructionCoordination, InstructionCoordinationState, InstructionDependencyCycle,
    InstructionEdgeState, InstructionGrammarError, InstructionLanguage, InstructionMarker,
    InstructionModality, InstructionParseBudget, InstructionParseReport, InstructionParseState,
    InstructionPolarity, InstructionPolarityScope, InstructionPolarityScopeState,
    InstructionReferenceCandidate, InstructionReferenceState, InstructionScopeCandidate,
    InstructionScopeKind, InstructionScopeState, PackedInstructionForest, PackedInstructionNode,
    SharedArgumentState, SharedInstructionAst, TemporalRelation, INSTRUCTION_GRAMMAR_REVISION,
};
pub use language_worker::{
    load_dgcl_runtime_pins, load_dgcl_runtime_pins_with_resources, run_language_worker,
    run_language_worker_with_resources, DependencySentence, DependencyToken, DependencyWord,
    DgclRuntimePins, LanguageObservation, LanguageWorkerConfig, LanguageWorkerError,
};
pub use model::*;
pub use pipeline::{
    analyze, analyze_path, analyze_path_with_resources, analyze_with_receipts,
    analyze_with_receipts_and_resources, analyze_with_resources, AnalysisRequest,
};
pub(crate) use process::not_started_request;
pub use process::{
    run_bounded_tool, run_bounded_tool_with_resources, BoundedToolRequest, BoundedToolResult,
    ResourceToolResult, ToolProcessRunner, ToolRunState,
};
pub use release::{
    evaluation_payload_digest, expected_v630_baseline_digest, release_component_scope,
    stage_completion_payload_digest,
};
pub use repair_loop::{
    dgcl_repair_application_payload_digest, dgcl_repair_application_scope,
    dgcl_repair_request_digest, dgcl_repair_validation_payload_digest,
    dgcl_repair_validation_scope, repair_authorization_scope, repair_request_authorization_scope,
    run_dgcl_repair_loop, run_dgcl_repair_loop_with_total_budget, DgclRepairApplication,
    DgclRepairAttemptRecord, DgclRepairAuthorization, DgclRepairConfigError, DgclRepairDeltaUnit,
    DgclRepairDriver, DgclRepairDriverError, DgclRepairLoopInput, DgclRepairLoopReport,
    DgclRepairRequest, DgclRepairStatus, DgclRepairTotalBudgetObservation,
    DgclRepairTotalBudgetReport, DgclRepairValidation, DgclRepairValidationOutcome,
    DgclVerifiedRepairApplication, DgclVerifiedRepairValidation, RepairEffectState,
    MAX_DGCL_REPAIR_ATTEMPTS,
};
pub use validation::{validation_receipt_payload_digest, validation_receipt_scope};
