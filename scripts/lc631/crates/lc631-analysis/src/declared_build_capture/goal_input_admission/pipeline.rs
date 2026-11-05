//! Explicit borrowed pipeline fields; no Serialize and no wildcard field omission.
use super::{boolean, number, Field, Node, Object, Sequence};

pub(super) fn pipeline(value: &crate::DgclPipelineReport, index: usize) -> Option<Field<'_>> {
    let crate::DgclPipelineReport {
        schema_version,
        source_revision,
        dg1,
        language_regions,
        unsupported_operative_region_ids,
        requirement_bindings,
        instruction_parses,
        program_ir,
        translation_projection,
        completion_plan,
        implementation_closure,
        authority_created,
        complete_instruction_grammar_claim,
        claim_boundary,
        immutable_core_digest: _,
        source_store: _,
    } = value;
    match index {
        0 => Some(Field {
            key: "schema_version",
            value: Node::Str(schema_version),
        }),
        1 => Some(Field {
            key: "source_revision",
            value: Node::Str(source_revision),
        }),
        2 => Some(Field {
            key: "dg1",
            value: Node::Object(Object::Dg1(dg1)),
        }),
        3 => Some(Field {
            key: "language_regions",
            value: Node::Seq(Sequence::LanguageRegion(language_regions)),
        }),
        4 => Some(Field {
            key: "unsupported_operative_region_ids",
            value: Node::Seq(Sequence::U32(unsupported_operative_region_ids)),
        }),
        5 => Some(Field {
            key: "requirement_bindings",
            value: Node::Seq(Sequence::RequirementBinding(requirement_bindings)),
        }),
        6 => Some(Field {
            key: "instruction_parses",
            value: Node::Seq(Sequence::InstructionParse(instruction_parses)),
        }),
        7 => Some(Field {
            key: "program_ir",
            value: Node::Object(Object::Program(program_ir)),
        }),
        8 => Some(Field {
            key: "translation_projection",
            value: Node::Object(Object::Translation(translation_projection)),
        }),
        9 => Some(Field {
            key: "completion_plan",
            value: Node::Object(Object::Completion(completion_plan)),
        }),
        10 => Some(Field {
            key: "implementation_closure",
            value: Node::Object(Object::Closure(implementation_closure)),
        }),
        11 => Some(Field {
            key: "authority_created",
            value: boolean(authority_created),
        }),
        12 => Some(Field {
            key: "complete_instruction_grammar_claim",
            value: boolean(complete_instruction_grammar_claim),
        }),
        13 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn dg1(value: &lc631_tldg::Dg1Report, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::Dg1Report {
        schema_version,
        source_revision,
        source_bytes,
        exact_source_roundtrip,
        regions,
        rust_trees,
        requirement_candidates,
        instruction_residuals,
        source_accounting,
        language_span_lattice,
        status,
        semantic_backend_state,
        claim_boundary,
    } = value;
    match index {
        0 => Some(Field {
            key: "schema_version",
            value: Node::Str(schema_version),
        }),
        1 => Some(Field {
            key: "source_revision",
            value: Node::Str(source_revision),
        }),
        2 => Some(Field {
            key: "source_bytes",
            value: number(source_bytes),
        }),
        3 => Some(Field {
            key: "exact_source_roundtrip",
            value: boolean(exact_source_roundtrip),
        }),
        4 => Some(Field {
            key: "regions",
            value: Node::Seq(Sequence::Region(regions)),
        }),
        5 => Some(Field {
            key: "rust_trees",
            value: Node::Seq(Sequence::SyntaxTree(rust_trees)),
        }),
        6 => Some(Field {
            key: "requirement_candidates",
            value: Node::Seq(Sequence::RequirementCandidate(requirement_candidates)),
        }),
        7 => Some(Field {
            key: "instruction_residuals",
            value: Node::Seq(Sequence::InstructionResidual(instruction_residuals)),
        }),
        8 => Some(Field {
            key: "source_accounting",
            value: Node::Seq(Sequence::Accounting(source_accounting)),
        }),
        9 => Some(Field {
            key: "language_span_lattice",
            value: Node::Seq(Sequence::LanguageSpan(language_span_lattice)),
        }),
        10 => Some(Field {
            key: "status",
            value: Node::Str(super::tags_pipeline::dg1status(status)),
        }),
        11 => Some(Field {
            key: "semantic_backend_state",
            value: Node::Str(semantic_backend_state),
        }),
        12 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn region(value: &lc631_tldg::Dg1Region, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::Dg1Region {
        id,
        parent,
        kind,
        content_role,
        span,
        language,
        dialect,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "parent",
            value: parent.as_ref().map(number).unwrap_or(Node::Null),
        }),
        2 => Some(Field {
            key: "kind",
            value: Node::Str(super::tags_pipeline::regionkind(kind)),
        }),
        3 => Some(Field {
            key: "content_role",
            value: Node::Str(super::tags_pipeline::contentrole(content_role)),
        }),
        4 => Some(Field {
            key: "span",
            value: Node::Object(Object::Range(span)),
        }),
        5 => Some(Field {
            key: "language",
            value: Node::Str(super::tags_pipeline::dg1language(language)),
        }),
        6 => Some(Field {
            key: "dialect",
            value: dialect.as_deref().map(Node::Str).unwrap_or(Node::Null),
        }),
        _ => None,
    }
}

pub(super) fn syntaxtree(value: &lc631_tldg::Dg1SyntaxTree, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::Dg1SyntaxTree {
        region_id,
        grammar_revision,
        root_has_error,
        nodes,
    } = value;
    match index {
        0 => Some(Field {
            key: "region_id",
            value: number(region_id),
        }),
        1 => Some(Field {
            key: "grammar_revision",
            value: Node::Str(grammar_revision),
        }),
        2 => Some(Field {
            key: "root_has_error",
            value: boolean(root_has_error),
        }),
        3 => Some(Field {
            key: "nodes",
            value: Node::Seq(Sequence::SyntaxNode(nodes)),
        }),
        _ => None,
    }
}

pub(super) fn syntaxnode(value: &lc631_tldg::Dg1SyntaxNode, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::Dg1SyntaxNode {
        id,
        parent_id,
        kind,
        named,
        span,
        has_error,
        is_error,
        missing,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "parent_id",
            value: parent_id.as_ref().map(number).unwrap_or(Node::Null),
        }),
        2 => Some(Field {
            key: "kind",
            value: Node::Str(kind),
        }),
        3 => Some(Field {
            key: "named",
            value: boolean(named),
        }),
        4 => Some(Field {
            key: "span",
            value: Node::Object(Object::Range(span)),
        }),
        5 => Some(Field {
            key: "has_error",
            value: boolean(has_error),
        }),
        6 => Some(Field {
            key: "is_error",
            value: boolean(is_error),
        }),
        7 => Some(Field {
            key: "missing",
            value: boolean(missing),
        }),
        _ => None,
    }
}

pub(super) fn requirementcandidate(
    value: &lc631_tldg::Dg1RequirementCandidate,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tldg::Dg1RequirementCandidate {
        id,
        source_span,
        source_text,
        language,
        strength,
        polarity,
        conditional,
        exception_present,
        condition_kind,
        scope,
        unresolved_reference,
        authority_grant,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        2 => Some(Field {
            key: "source_text",
            value: Node::Str(source_text),
        }),
        3 => Some(Field {
            key: "language",
            value: Node::Str(super::tags_pipeline::dg1language(language)),
        }),
        4 => Some(Field {
            key: "strength",
            value: Node::Str(super::tags_pipeline::strength(strength)),
        }),
        5 => Some(Field {
            key: "polarity",
            value: Node::Str(super::tags_pipeline::dg1polarity(polarity)),
        }),
        6 => Some(Field {
            key: "conditional",
            value: boolean(conditional),
        }),
        7 => Some(Field {
            key: "exception_present",
            value: boolean(exception_present),
        }),
        8 => Some(Field {
            key: "condition_kind",
            value: condition_kind
                .as_ref()
                .map(|v| Node::Str(super::tags_pipeline::conditionkind(v)))
                .unwrap_or(Node::Null),
        }),
        9 => Some(Field {
            key: "scope",
            value: scope.as_deref().map(Node::Str).unwrap_or(Node::Null),
        }),
        10 => Some(Field {
            key: "unresolved_reference",
            value: boolean(unresolved_reference),
        }),
        11 => Some(Field {
            key: "authority_grant",
            value: boolean(authority_grant),
        }),
        _ => None,
    }
}

pub(super) fn instructionresidual(
    value: &lc631_tldg::Dg1InstructionResidual,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tldg::Dg1InstructionResidual {
        source_span,
        source_text,
        language,
        reason,
    } = value;
    match index {
        0 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        1 => Some(Field {
            key: "source_text",
            value: Node::Str(source_text),
        }),
        2 => Some(Field {
            key: "language",
            value: Node::Str(super::tags_pipeline::dg1language(language)),
        }),
        3 => Some(Field {
            key: "reason",
            value: Node::Str(super::tags_pipeline::residualreason(reason)),
        }),
        _ => None,
    }
}

pub(super) fn accounting(
    value: &lc631_tldg::Dg1SourceAccountingSegment,
    index: usize,
) -> Option<Field<'_>> {
    let lc631_tldg::Dg1SourceAccountingSegment {
        source_span,
        role,
        region_id,
        residual_reason,
    } = value;
    match index {
        0 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        1 => Some(Field {
            key: "role",
            value: Node::Str(super::tags_pipeline::sourcerole(role)),
        }),
        2 => Some(Field {
            key: "region_id",
            value: region_id.as_ref().map(number).unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "residual_reason",
            value: residual_reason
                .as_ref()
                .map(|v| Node::Str(super::tags_pipeline::residualreason(v)))
                .unwrap_or(Node::Null),
        }),
        _ => None,
    }
}

pub(super) fn languagespan(value: &lc631_tldg::Dg1LanguageSpan, index: usize) -> Option<Field<'_>> {
    let lc631_tldg::Dg1LanguageSpan {
        id,
        region_id,
        source_span,
        language,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "region_id",
            value: number(region_id),
        }),
        2 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        3 => Some(Field {
            key: "language",
            value: Node::Str(super::tags_pipeline::dg1language(language)),
        }),
        _ => None,
    }
}

pub(super) fn requirementbinding(
    value: &crate::DgclRequirementBinding,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclRequirementBinding {
        requirement_id,
        region_id,
        dependency_payload_digest,
        dependency_language_span_ids,
        dependency_payload_digests,
        polarity_cue_observed,
        conditional_cue_observed,
        state,
    } = value;
    match index {
        0 => Some(Field {
            key: "requirement_id",
            value: number(requirement_id),
        }),
        1 => Some(Field {
            key: "region_id",
            value: region_id.as_ref().map(number).unwrap_or(Node::Null),
        }),
        2 => Some(Field {
            key: "dependency_payload_digest",
            value: dependency_payload_digest
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "dependency_language_span_ids",
            value: Node::Seq(Sequence::U32(dependency_language_span_ids)),
        }),
        4 => Some(Field {
            key: "dependency_payload_digests",
            value: Node::Seq(Sequence::String(dependency_payload_digests)),
        }),
        5 => Some(Field {
            key: "polarity_cue_observed",
            value: boolean(polarity_cue_observed),
        }),
        6 => Some(Field {
            key: "conditional_cue_observed",
            value: boolean(conditional_cue_observed),
        }),
        7 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_pipeline::bindingstate(state)),
        }),
        _ => None,
    }
}

pub(super) fn instructionparse(
    value: &crate::DgclInstructionParse,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclInstructionParse {
        region_id,
        legacy_requirement_ids,
        root_source_revision,
        source_span,
        state,
        report,
        error,
    } = value;
    match index {
        0 => Some(Field {
            key: "region_id",
            value: number(region_id),
        }),
        1 => Some(Field {
            key: "legacy_requirement_ids",
            value: Node::Seq(Sequence::U32(legacy_requirement_ids)),
        }),
        2 => Some(Field {
            key: "root_source_revision",
            value: Node::Str(root_source_revision),
        }),
        3 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        4 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_pipeline::dgclparsestate(state)),
        }),
        5 => Some(Field {
            key: "report",
            value: report
                .as_ref()
                .map(|v| Node::Object(Object::ParseReport(v)))
                .unwrap_or(Node::Null),
        }),
        6 => Some(Field {
            key: "error",
            value: error
                .as_ref()
                .map(|v| Node::Str(super::tags_grammar::grammarerror(v)))
                .unwrap_or(Node::Null),
        }),
        _ => None,
    }
}
