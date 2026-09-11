#![forbid(unsafe_code)]

mod accelerated;
mod adapters;
mod calibration;
mod distillation;
mod execution;
mod geometry;
mod grammar;
mod integration;
mod kernel;
mod parser;
mod profile;
mod semantics;
mod source;

pub use accelerated::{
    run_tldg_geometry_accelerator, TldgGeometryAcceleratorError, TldgGeometryAcceleratorReport,
};
pub use adapters::{
    backend_execution_payload_digest, backend_execution_scope, default_backend_registry,
    execute_builtin_backend_receipts, BackendDescriptor, BackendExecutionReceipt, BackendFamily,
    BackendRegistry, BackendState, UNIFIED_SYNTAX_SCHEMA,
};
pub use calibration::{
    CalibrationError, CalibrationPartition, EmpiricalRiskOverlay, EmpiricalRiskSource,
};
pub use distillation::{
    run_mutual_distillation, AcceptedProposal, Anchored, BackflowReport, Decoded, DecodedProposal,
    DistillationEpoch, DistillationEpochRecord, DistillationError, EpochPayload,
    MutualDistillationRun, ParseBudget, Proposed, RejectedProposal, ReprojectionRequest, Validated,
    ValidationReceipt, ValidationState,
};
pub use execution::{
    evaluate_geometry_execution, run_adversarial_evaluation, AdversarialEvaluationReport,
    GeometryExecutionReceipt, GeometryFault, GeometryGpuObservation, GeometryParityState,
};
pub use geometry::{
    build_geometry_shadow, CpuGeometryBackend, GeometryCoupling, GeometryError, GeometryPoint,
    GeometryProposal, GeometryShadow, GeometrySignature, ProposalAuthority,
};
pub use grammar::{
    ConstraintEdge, ConstraintKind, ConstraintState, DeepGrammarArtifact, GrammarProduction,
    PackedDerivation, ParseDefect, RegionCandidate, RegionLattice, RelationKind, RelationState,
    SyntaxNode, SyntaxNodeId, SyntaxNodeKind, SyntaxRelation, Token, TokenKind, TokenLattice,
    TypedConstraintGraph, UnifiedGrammarIr, UnifiedSyntaxHypergraph,
};
pub use integration::{
    build_tldg_pipeline, build_tldg_pipeline_with_receipts, build_tldg_release_gate,
    build_tldg_release_gate_verified, build_tldg_release_gate_with_receipts,
    tldg_gpu_release_payload_digest, DistillationSummary, OutputClosure, PipelineError,
    TldgPipelineReport, TldgReleaseEvidence, TldgReleaseGate,
};
pub use kernel::{
    build_structural_kernel, AnchorState, StructuralAnchor, StructuralKernel, StructuralKernelError,
};
pub use parser::{analyze, analyze_document, TldgError, UnifiedParseReport};
pub use profile::{
    default_profile_registry, AmbiguityPolicy, GrammarFeature, GrammarProfile, GrammarProfileId,
    GrammarProfileRegistry, LexiconPolicy,
};
pub use semantics::{build_semantic_views, SemanticView, SemanticViewKind};
pub use source::{BoundaryCell, BoundaryClass, BoundaryLedger, SourceRevision};

pub const TLDG_SCHEMA: &str = "lc631-unified-geometric-deepgrammar.v1";
