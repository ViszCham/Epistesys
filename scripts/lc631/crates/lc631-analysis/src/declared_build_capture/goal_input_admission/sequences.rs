//! Known-length typed slice cursors; only one borrowed child is exposed per step.
use super::{Node, Object};
#[derive(Clone, Copy)]
pub(super) enum Sequence<'a> {
    LanguageRegion(&'a [crate::DgclLanguageRegion]),
    U32(&'a [u32]),
    RequirementBinding(&'a [crate::DgclRequirementBinding]),
    InstructionParse(&'a [crate::DgclInstructionParse]),
    Region(&'a [lc631_tldg::Dg1Region]),
    SyntaxTree(&'a [lc631_tldg::Dg1SyntaxTree]),
    RequirementCandidate(&'a [lc631_tldg::Dg1RequirementCandidate]),
    InstructionResidual(&'a [lc631_tldg::Dg1InstructionResidual]),
    Accounting(&'a [lc631_tldg::Dg1SourceAccountingSegment]),
    LanguageSpan(&'a [lc631_tldg::Dg1LanguageSpan]),
    SyntaxNode(&'a [lc631_tldg::Dg1SyntaxNode]),
    Sentence(&'a [crate::DependencySentence]),
    Token(&'a [crate::DependencyToken]),
    Word(&'a [crate::DependencyWord]),
    String(&'a [String]),
    Ast(&'a [crate::SharedInstructionAst]),
    ForestNode(&'a [crate::PackedInstructionNode]),
    Action(&'a [crate::InstructionAction]),
    Coordination(&'a [crate::InstructionCoordination]),
    PolarityScope(&'a [crate::InstructionPolarityScope]),
    Condition(&'a [crate::InstructionCondition]),
    ScopeCandidate(&'a [crate::InstructionScopeCandidate]),
    ReferenceCandidate(&'a [crate::InstructionReferenceCandidate]),
    Usize(&'a [usize]),
    ConstraintNode(&'a [crate::InstructionConstraintNode]),
    ConstraintEdge(&'a [crate::InstructionConstraintEdge]),
    Cycle(&'a [crate::InstructionDependencyCycle]),
    ProgramRequirement(&'a [crate::DgclProgramRequirement]),
    ProgramResidual(&'a [crate::DgclProgramResidual]),
    Alternative(&'a [crate::DgclProgramAlternative]),
    Dependency(&'a [crate::DgclProgramDependency]),
    Witness(&'a [crate::DgclConstraintWitness]),
    Loss(&'a [crate::TranslationLossEvent]),
    Target(&'a [lc631_tl::TargetClaim]),
    ProjectionEdge(&'a [lc631_tl::ProjectionEdgeV3]),
    Domain(&'a [lc631_tl::DomainTranslationPayload]),
    TargetId(&'a [lc631_tl::TargetClaimId]),
    PlannedRequirement(&'a [crate::PlannedCodingRequirement]),
    Task(&'a [crate::PlannedEvidenceTask]),
    Blocker(&'a [crate::CompletionBlocker]),
    PlannedTarget(&'a [crate::PlannedTargetCandidate]),
    PlannedCondition(&'a [crate::PlannedCondition]),
    Gap(&'a [crate::DgclImplementationGap]),
    Rejection(&'a [crate::DgclEvidenceRejection]),
    RequirementClosure(&'a [crate::DgclRequirementClosure]),
    Point(&'a [lc631_tldg::GeometryPoint]),
    Proposal(&'a [lc631_tldg::GeometryProposal]),
    Coupling(&'a [lc631_tldg::GeometryCoupling]),
    Epoch(&'a [lc631_tldg::DistillationEpochRecord]),
    Reprojection(&'a [lc631_tldg::ReprojectionRequest]),
    Decoded(&'a [lc631_tldg::DecodedProposal]),
    Accepted(&'a [lc631_tldg::AcceptedProposal]),
    Rejected(&'a [lc631_tldg::RejectedProposal]),
    Validation(&'a [lc631_tldg::ValidationReceipt]),
    Anchor(&'a [lc631_tldg::StructuralAnchor]),
    Materialized(&'a [lc631_tldg::MaterializedRelationEvidence]),
    SourceSpan(&'a [lc631_core::SourceSpan]),
    Expression(&'a [crate::ConditionExpression]),
}
impl<'a> Sequence<'a> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::LanguageRegion(v) => v.len(),
            Self::U32(v) => v.len(),
            Self::RequirementBinding(v) => v.len(),
            Self::InstructionParse(v) => v.len(),
            Self::Region(v) => v.len(),
            Self::SyntaxTree(v) => v.len(),
            Self::RequirementCandidate(v) => v.len(),
            Self::InstructionResidual(v) => v.len(),
            Self::Accounting(v) => v.len(),
            Self::LanguageSpan(v) => v.len(),
            Self::SyntaxNode(v) => v.len(),
            Self::Sentence(v) => v.len(),
            Self::Token(v) => v.len(),
            Self::Word(v) => v.len(),
            Self::String(v) => v.len(),
            Self::Ast(v) => v.len(),
            Self::ForestNode(v) => v.len(),
            Self::Action(v) => v.len(),
            Self::Coordination(v) => v.len(),
            Self::PolarityScope(v) => v.len(),
            Self::Condition(v) => v.len(),
            Self::ScopeCandidate(v) => v.len(),
            Self::ReferenceCandidate(v) => v.len(),
            Self::Usize(v) => v.len(),
            Self::ConstraintNode(v) => v.len(),
            Self::ConstraintEdge(v) => v.len(),
            Self::Cycle(v) => v.len(),
            Self::ProgramRequirement(v) => v.len(),
            Self::ProgramResidual(v) => v.len(),
            Self::Alternative(v) => v.len(),
            Self::Dependency(v) => v.len(),
            Self::Witness(v) => v.len(),
            Self::Loss(v) => v.len(),
            Self::Target(v) => v.len(),
            Self::ProjectionEdge(v) => v.len(),
            Self::Domain(v) => v.len(),
            Self::TargetId(v) => v.len(),
            Self::PlannedRequirement(v) => v.len(),
            Self::Task(v) => v.len(),
            Self::Blocker(v) => v.len(),
            Self::PlannedTarget(v) => v.len(),
            Self::PlannedCondition(v) => v.len(),
            Self::Gap(v) => v.len(),
            Self::Rejection(v) => v.len(),
            Self::RequirementClosure(v) => v.len(),
            Self::Point(v) => v.len(),
            Self::Proposal(v) => v.len(),
            Self::Coupling(v) => v.len(),
            Self::Epoch(v) => v.len(),
            Self::Reprojection(v) => v.len(),
            Self::Decoded(v) => v.len(),
            Self::Accepted(v) => v.len(),
            Self::Rejected(v) => v.len(),
            Self::Validation(v) => v.len(),
            Self::Anchor(v) => v.len(),
            Self::Materialized(v) => v.len(),
            Self::SourceSpan(v) => v.len(),
            Self::Expression(v) => v.len(),
        }
    }
    pub(super) fn item(self, index: usize) -> Option<Node<'a>> {
        match self {
            Self::LanguageRegion(v) => v
                .get(index)
                .map(|v| Node::Object(Object::LanguageRegion(v))),
            Self::U32(v) => v.get(index).map(|v| super::number(v)),
            Self::RequirementBinding(v) => v
                .get(index)
                .map(|v| Node::Object(Object::RequirementBinding(v))),
            Self::InstructionParse(v) => v
                .get(index)
                .map(|v| Node::Object(Object::InstructionParse(v))),
            Self::Region(v) => v.get(index).map(|v| Node::Object(Object::Region(v))),
            Self::SyntaxTree(v) => v.get(index).map(|v| Node::Object(Object::SyntaxTree(v))),
            Self::RequirementCandidate(v) => v
                .get(index)
                .map(|v| Node::Object(Object::RequirementCandidate(v))),
            Self::InstructionResidual(v) => v
                .get(index)
                .map(|v| Node::Object(Object::InstructionResidual(v))),
            Self::Accounting(v) => v.get(index).map(|v| Node::Object(Object::Accounting(v))),
            Self::LanguageSpan(v) => v.get(index).map(|v| Node::Object(Object::LanguageSpan(v))),
            Self::SyntaxNode(v) => v.get(index).map(|v| Node::Object(Object::SyntaxNode(v))),
            Self::Sentence(v) => v.get(index).map(|v| Node::Object(Object::Sentence(v))),
            Self::Token(v) => v.get(index).map(|v| Node::Object(Object::Token(v))),
            Self::Word(v) => v.get(index).map(|v| Node::Object(Object::Word(v))),
            Self::String(v) => v.get(index).map(|v| Node::Str(v)),
            Self::Ast(v) => v.get(index).map(|v| Node::Object(Object::Ast(v))),
            Self::ForestNode(v) => v.get(index).map(|v| Node::Object(Object::ForestNode(v))),
            Self::Action(v) => v.get(index).map(|v| Node::Object(Object::Action(v))),
            Self::Coordination(v) => v.get(index).map(|v| Node::Object(Object::Coordination(v))),
            Self::PolarityScope(v) => v.get(index).map(|v| Node::Object(Object::PolarityScope(v))),
            Self::Condition(v) => v.get(index).map(|v| Node::Object(Object::Condition(v))),
            Self::ScopeCandidate(v) => v
                .get(index)
                .map(|v| Node::Object(Object::ScopeCandidate(v))),
            Self::ReferenceCandidate(v) => v
                .get(index)
                .map(|v| Node::Object(Object::ReferenceCandidate(v))),
            Self::Usize(v) => v.get(index).map(|v| super::number(v)),
            Self::ConstraintNode(v) => v
                .get(index)
                .map(|v| Node::Object(Object::ConstraintNode(v))),
            Self::ConstraintEdge(v) => v
                .get(index)
                .map(|v| Node::Object(Object::ConstraintEdge(v))),
            Self::Cycle(v) => v.get(index).map(|v| Node::Object(Object::Cycle(v))),
            Self::ProgramRequirement(v) => v
                .get(index)
                .map(|v| Node::Object(Object::ProgramRequirement(v))),
            Self::ProgramResidual(v) => v
                .get(index)
                .map(|v| Node::Object(Object::ProgramResidual(v))),
            Self::Alternative(v) => v.get(index).map(|v| Node::Object(Object::Alternative(v))),
            Self::Dependency(v) => v.get(index).map(|v| Node::Object(Object::Dependency(v))),
            Self::Witness(v) => v.get(index).map(|v| Node::Object(Object::Witness(v))),
            Self::Loss(v) => v.get(index).map(|v| Node::Object(Object::Loss(v))),
            Self::Target(v) => v.get(index).map(|v| Node::Object(Object::Target(v))),
            Self::ProjectionEdge(v) => v
                .get(index)
                .map(|v| Node::Object(Object::ProjectionEdge(v))),
            Self::Domain(v) => v.get(index).map(|v| Node::Object(Object::Domain(v))),
            Self::TargetId(v) => v.get(index).map(|v| super::number(v)),
            Self::PlannedRequirement(v) => v
                .get(index)
                .map(|v| Node::Object(Object::PlannedRequirement(v))),
            Self::Task(v) => v.get(index).map(|v| Node::Object(Object::Task(v))),
            Self::Blocker(v) => v.get(index).map(|v| Node::Object(Object::Blocker(v))),
            Self::PlannedTarget(v) => v.get(index).map(|v| Node::Object(Object::PlannedTarget(v))),
            Self::PlannedCondition(v) => v
                .get(index)
                .map(|v| Node::Object(Object::PlannedCondition(v))),
            Self::Gap(v) => v.get(index).map(|v| Node::Object(Object::Gap(v))),
            Self::Rejection(v) => v.get(index).map(|v| Node::Object(Object::Rejection(v))),
            Self::RequirementClosure(v) => v
                .get(index)
                .map(|v| Node::Object(Object::RequirementClosure(v))),
            Self::Point(v) => v.get(index).map(|v| Node::Object(Object::Point(v))),
            Self::Proposal(v) => v.get(index).map(|v| Node::Object(Object::Proposal(v))),
            Self::Coupling(v) => v.get(index).map(|v| Node::Object(Object::Coupling(v))),
            Self::Epoch(v) => v.get(index).map(|v| Node::Object(Object::Epoch(v))),
            Self::Reprojection(v) => v.get(index).map(|v| Node::Object(Object::Reprojection(v))),
            Self::Decoded(v) => v.get(index).map(|v| Node::Object(Object::Decoded(v))),
            Self::Accepted(v) => v.get(index).map(|v| Node::Object(Object::Accepted(v))),
            Self::Rejected(v) => v.get(index).map(|v| Node::Object(Object::Rejected(v))),
            Self::Validation(v) => v.get(index).map(|v| Node::Object(Object::Validation(v))),
            Self::Anchor(v) => v.get(index).map(|v| Node::Object(Object::Anchor(v))),
            Self::Materialized(v) => v.get(index).map(|v| Node::Object(Object::Materialized(v))),
            Self::SourceSpan(v) => v.get(index).map(|v| Node::Object(Object::SourceSpan(v))),
            Self::Expression(v) => v.get(index).map(|v| Node::Object(Object::Expression(v))),
        }
    }
}
