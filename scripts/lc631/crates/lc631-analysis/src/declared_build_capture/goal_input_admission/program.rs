//! Explicit borrowed program fields; no Serialize and no wildcard field omission.
use super::{boolean, number, Field, Node, Object, Sequence};

pub(super) fn program(value: &crate::DgclProgramIrReport, index: usize) -> Option<Field<'_>> {
    let crate::DgclProgramIrReport {
        schema_version,
        source_revision,
        grammar_revision,
        program_digest,
        requirements,
        residuals,
        structural_distillation,
        semantic_truth_claim,
        authority_created,
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
            key: "grammar_revision",
            value: Node::Str(grammar_revision),
        }),
        3 => Some(Field {
            key: "program_digest",
            value: Node::Str(program_digest),
        }),
        4 => Some(Field {
            key: "requirements",
            value: Node::Seq(Sequence::ProgramRequirement(requirements)),
        }),
        5 => Some(Field {
            key: "residuals",
            value: Node::Seq(Sequence::ProgramResidual(residuals)),
        }),
        6 => Some(Field {
            key: "structural_distillation",
            value: Node::Object(Object::Distillation(structural_distillation)),
        }),
        7 => Some(Field {
            key: "semantic_truth_claim",
            value: boolean(semantic_truth_claim),
        }),
        8 => Some(Field {
            key: "authority_created",
            value: boolean(authority_created),
        }),
        9 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn programrequirement(
    value: &crate::DgclProgramRequirement,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclProgramRequirement {
        requirement_id,
        region_id,
        root_source_revision,
        source_span,
        source_digest,
        parse_state,
        legacy_candidate_references,
        contract_digest,
        contract,
        selected_alternative_id,
        alternatives,
        dependencies,
        semantic_truth_claim,
        authority_grant,
    } = value;
    match index {
        0 => Some(Field {
            key: "requirement_id",
            value: Node::Str(requirement_id),
        }),
        1 => Some(Field {
            key: "region_id",
            value: number(region_id),
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
            key: "source_digest",
            value: Node::Str(source_digest),
        }),
        5 => Some(Field {
            key: "parse_state",
            value: Node::Str(super::tags_pipeline::dgclparsestate(parse_state)),
        }),
        6 => Some(Field {
            key: "legacy_candidate_references",
            value: Node::Seq(Sequence::U32(legacy_candidate_references)),
        }),
        7 => Some(Field {
            key: "contract_digest",
            value: Node::Str(contract_digest),
        }),
        8 => Some(Field {
            key: "contract",
            value: Node::Object(Object::InstructionParse(contract)),
        }),
        9 => Some(Field {
            key: "selected_alternative_id",
            value: selected_alternative_id
                .as_deref()
                .map(Node::Str)
                .unwrap_or(Node::Null),
        }),
        10 => Some(Field {
            key: "alternatives",
            value: Node::Seq(Sequence::Alternative(alternatives)),
        }),
        11 => Some(Field {
            key: "dependencies",
            value: Node::Seq(Sequence::Dependency(dependencies)),
        }),
        12 => Some(Field {
            key: "semantic_truth_claim",
            value: boolean(semantic_truth_claim),
        }),
        13 => Some(Field {
            key: "authority_grant",
            value: boolean(authority_grant),
        }),
        _ => None,
    }
}

pub(super) fn alternative(
    value: &crate::DgclProgramAlternative,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclProgramAlternative {
        alternative_id,
        production_id,
        language,
        parser_ast_digest,
        region_offset,
        root_source_span,
        ast,
    } = value;
    match index {
        0 => Some(Field {
            key: "alternative_id",
            value: Node::Str(alternative_id),
        }),
        1 => Some(Field {
            key: "production_id",
            value: number(production_id),
        }),
        2 => Some(Field {
            key: "language",
            value: Node::Str(super::tags_grammar::instructionlanguage(language)),
        }),
        3 => Some(Field {
            key: "parser_ast_digest",
            value: Node::Str(parser_ast_digest),
        }),
        4 => Some(Field {
            key: "region_offset",
            value: number(region_offset),
        }),
        5 => Some(Field {
            key: "root_source_span",
            value: Node::Object(Object::Range(root_source_span)),
        }),
        6 => Some(Field {
            key: "ast",
            value: Node::Object(Object::Ast(ast)),
        }),
        _ => None,
    }
}

pub(super) fn dependency(value: &crate::DgclProgramDependency, index: usize) -> Option<Field<'_>> {
    let crate::DgclProgramDependency {
        language_span_id,
        source_span,
        backend_revision,
        model_manifest_digest,
        runtime_manifest_digest,
        requirements_lock_digest,
        payload_binding_revision,
        payload_digest,
        semantic_truth_claim,
    } = value;
    match index {
        0 => Some(Field {
            key: "language_span_id",
            value: number(language_span_id),
        }),
        1 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        2 => Some(Field {
            key: "backend_revision",
            value: Node::Str(backend_revision),
        }),
        3 => Some(Field {
            key: "model_manifest_digest",
            value: Node::Str(model_manifest_digest),
        }),
        4 => Some(Field {
            key: "runtime_manifest_digest",
            value: Node::Str(runtime_manifest_digest),
        }),
        5 => Some(Field {
            key: "requirements_lock_digest",
            value: Node::Str(requirements_lock_digest),
        }),
        6 => Some(Field {
            key: "payload_binding_revision",
            value: Node::Str(payload_binding_revision),
        }),
        7 => Some(Field {
            key: "payload_digest",
            value: Node::Str(payload_digest),
        }),
        8 => Some(Field {
            key: "semantic_truth_claim",
            value: boolean(semantic_truth_claim),
        }),
        _ => None,
    }
}

pub(super) fn programresidual(
    value: &crate::DgclProgramResidual,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclProgramResidual {
        residual_id,
        region_id,
        source_span,
        source_digest,
        state,
        legacy_candidate_references,
        reason,
        blocking_requirement,
    } = value;
    match index {
        0 => Some(Field {
            key: "residual_id",
            value: Node::Str(residual_id),
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
            key: "source_digest",
            value: Node::Str(source_digest),
        }),
        4 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_pipeline::dgclparsestate(state)),
        }),
        5 => Some(Field {
            key: "legacy_candidate_references",
            value: Node::Seq(Sequence::U32(legacy_candidate_references)),
        }),
        6 => Some(Field {
            key: "reason",
            value: Node::Str(reason),
        }),
        7 => Some(Field {
            key: "blocking_requirement",
            value: boolean(blocking_requirement),
        }),
        _ => None,
    }
}

pub(super) fn distillation(
    value: &crate::DgclStructuralDistillation,
    index: usize,
) -> Option<Field<'_>> {
    let crate::DgclStructuralDistillation {
        schema_version,
        input_constraints_digest,
        run,
        constraint_witnesses,
        claim_boundary,
    } = value;
    match index {
        0 => Some(Field {
            key: "schema_version",
            value: Node::Str(schema_version),
        }),
        1 => Some(Field {
            key: "input_constraints_digest",
            value: Node::Str(input_constraints_digest),
        }),
        2 => Some(Field {
            key: "run",
            value: run
                .as_ref()
                .map(|v| Node::Object(Object::Run(v)))
                .unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "constraint_witnesses",
            value: Node::Seq(Sequence::Witness(constraint_witnesses)),
        }),
        4 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn witness(value: &crate::DgclConstraintWitness, index: usize) -> Option<Field<'_>> {
    let crate::DgclConstraintWitness {
        requirement_id,
        ast_digest,
        constraint_edge_index,
        materialization_id,
        semantic_truth_claim,
        authority_created,
    } = value;
    match index {
        0 => Some(Field {
            key: "requirement_id",
            value: Node::Str(requirement_id),
        }),
        1 => Some(Field {
            key: "ast_digest",
            value: Node::Str(ast_digest),
        }),
        2 => Some(Field {
            key: "constraint_edge_index",
            value: number(constraint_edge_index),
        }),
        3 => Some(Field {
            key: "materialization_id",
            value: Node::Str(materialization_id),
        }),
        4 => Some(Field {
            key: "semantic_truth_claim",
            value: boolean(semantic_truth_claim),
        }),
        5 => Some(Field {
            key: "authority_created",
            value: boolean(authority_created),
        }),
        _ => None,
    }
}
