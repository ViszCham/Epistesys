#![forbid(unsafe_code)]

pub const PROGRAM_ANALYSIS_SCHEMA: &str = "lc631-program-analysis.v1";

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
mod python;
mod python_script;
mod release;
mod repair_loop;
mod rust;
mod validation;
mod validation_producer;

pub use validation_producer::{
    capture_dgcl_cargo_snapshot, check_cargo_test_result, dgcl_cargo_registration_payload,
    dgcl_cargo_validation_scope, dgcl_implementation_binding_payload, dgcl_repository_root_digest,
    dgcl_validator_revision, issue_dgcl_cargo_evidence, issue_dgcl_implementation_binding,
    observe_dgcl_cargo_validation, preflight_dgcl_cargo_plan, project_dgcl_cargo_target_snapshot,
    verify_dgcl_cargo_registration, CheckedDgclCargoObservation, DgclBindingIssuerContext,
    DgclCargoExecutionContext, DgclCargoValidationPlan, DgclValidationProducerError,
};

pub use closure::{
    build_coding_closure, coding_evidence_payload_digest, coding_evidence_scope,
    CodingClosureError, CodingClosureReport, CodingClosureStatus, CodingEvidenceClaim,
    CodingEvidenceKind, CodingEvidenceProducer, CodingEvidenceRejection, CodingEvidenceState,
    CodingGap, CodingGapKind,
};
pub use closure_connection::{
    issue_connection_evidence, issue_observed_connection_evidence,
    observe_connection_plan_from_cli, verify_connection_plan, verify_connection_plan_from_cli,
    CodingConnectionPlan, CodingConnectionReport, ConnectionError, ConnectionStatus,
    VerifiedConnectionObservation,
};
pub use closure_resume::{
    append_checkpoint, inspect_resume, inspect_resume_with_anchor, issue_checkpoint_head_anchor,
    load_checkpoint_head, persist_checkpoint_head, CheckpointBody, CheckpointEventKind,
    CheckpointHeadAnchor, CheckpointHeadRequest, CheckpointIntegrityState, DeliveryState,
    ResumeError, ResumeObservation, DGCL_CHECKPOINT_V1_SCHEMA, DGCL_CHECKPOINT_V2_SCHEMA,
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
    build_dgcl_pipeline, verify_dgcl_pipeline_identity, DgclBackendSet, DgclBindingState,
    DgclInstructionParse, DgclInstructionParseState, DgclLanguageRegion, DgclPipelineError,
    DgclPipelineReport, DgclRequirementBinding,
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
pub use file_repair::{DgclFileEvidenceValidator, DgclFileRepairConfig, DgclFileRepairDriver};
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
    SharedArgumentState, TemporalRelation, INSTRUCTION_GRAMMAR_REVISION,
};
pub use language_worker::{
    load_dgcl_runtime_pins, run_language_worker, DependencySentence, DependencyToken,
    DependencyWord, DgclRuntimePins, LanguageObservation, LanguageWorkerConfig,
    LanguageWorkerError,
};
pub use model::*;
pub use pipeline::{analyze, analyze_path, analyze_with_receipts, AnalysisRequest};
pub(crate) use process::not_started_request;
pub use process::{run_bounded_tool, BoundedToolRequest, BoundedToolResult, ToolRunState};
pub use release::{
    evaluation_payload_digest, expected_v630_baseline_digest, release_component_scope,
    stage_completion_payload_digest,
};
pub use repair_loop::{
    dgcl_repair_application_payload_digest, dgcl_repair_application_scope,
    dgcl_repair_request_digest, dgcl_repair_validation_payload_digest,
    dgcl_repair_validation_scope, repair_authorization_scope, repair_request_authorization_scope,
    run_dgcl_repair_loop, DgclRepairApplication, DgclRepairAttemptRecord, DgclRepairAuthorization,
    DgclRepairConfigError, DgclRepairDriver, DgclRepairDriverError, DgclRepairLoopInput,
    DgclRepairLoopReport, DgclRepairRequest, DgclRepairStatus, DgclRepairValidation,
    DgclRepairValidationOutcome, RepairEffectState, MAX_DGCL_REPAIR_ATTEMPTS,
};
pub use validation::{validation_receipt_payload_digest, validation_receipt_scope};
