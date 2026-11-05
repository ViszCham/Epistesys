//! Explicit borrowed grammar fields; no Serialize and no wildcard field omission.
use super::{number, Field, Node, Object, Sequence};

pub(super) fn parsereport(
    value: &crate::InstructionParseReport,
    index: usize,
) -> Option<Field<'_>> {
    let crate::InstructionParseReport {
        schema_version,
        source_revision,
        grammar_revision,
        state,
        selected,
        alternatives,
        forest,
        chart_item_count,
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
            key: "state",
            value: Node::Str(super::tags_grammar::parsestate(state)),
        }),
        4 => Some(Field {
            key: "selected",
            value: selected
                .as_ref()
                .map(|v| Node::Object(Object::Ast(v)))
                .unwrap_or(Node::Null),
        }),
        5 => Some(Field {
            key: "alternatives",
            value: Node::Seq(Sequence::Ast(alternatives)),
        }),
        6 => Some(Field {
            key: "forest",
            value: Node::Object(Object::Forest(forest)),
        }),
        7 => Some(Field {
            key: "chart_item_count",
            value: number(chart_item_count),
        }),
        8 => Some(Field {
            key: "claim_boundary",
            value: Node::Str(claim_boundary),
        }),
        _ => None,
    }
}

pub(super) fn forest(value: &crate::PackedInstructionForest, index: usize) -> Option<Field<'_>> {
    let crate::PackedInstructionForest { roots, nodes } = value;
    match index {
        0 => Some(Field {
            key: "roots",
            value: Node::Seq(Sequence::U32(roots)),
        }),
        1 => Some(Field {
            key: "nodes",
            value: Node::Seq(Sequence::ForestNode(nodes)),
        }),
        _ => None,
    }
}

pub(super) fn forestnode(value: &crate::PackedInstructionNode, index: usize) -> Option<Field<'_>> {
    let crate::PackedInstructionNode {
        id,
        symbol,
        source_span,
        token_span,
        production_ids,
        children,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "symbol",
            value: Node::Str(symbol),
        }),
        2 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        3 => Some(Field {
            key: "token_span",
            value: Node::Object(Object::Range(token_span)),
        }),
        4 => Some(Field {
            key: "production_ids",
            value: Node::Seq(Sequence::U32(production_ids)),
        }),
        5 => Some(Field {
            key: "children",
            value: Node::Seq(Sequence::U32(children)),
        }),
        _ => None,
    }
}

pub(super) fn ast(value: &crate::InstructionAst, index: usize) -> Option<Field<'_>> {
    let crate::InstructionAst {
        production_id,
        language,
        marker,
        source_span,
        marker_span,
        body_span,
        body,
        actions,
        coordination,
        coordination_state,
        polarity_scopes,
        conditions,
        scopes,
        references,
        constraint_graph,
        shared_argument_state,
    } = value;
    match index {
        0 => Some(Field {
            key: "production_id",
            value: number(production_id),
        }),
        1 => Some(Field {
            key: "language",
            value: Node::Str(super::tags_grammar::instructionlanguage(language)),
        }),
        2 => Some(Field {
            key: "marker",
            value: Node::Str(super::tags_grammar::marker(marker)),
        }),
        3 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        4 => Some(Field {
            key: "marker_span",
            value: Node::Object(Object::Range(marker_span)),
        }),
        5 => Some(Field {
            key: "body_span",
            value: Node::Object(Object::Range(body_span)),
        }),
        6 => Some(Field {
            key: "body",
            value: Node::Str(body),
        }),
        7 => Some(Field {
            key: "actions",
            value: Node::Seq(Sequence::Action(actions)),
        }),
        8 => Some(Field {
            key: "coordination",
            value: Node::Seq(Sequence::Coordination(coordination)),
        }),
        9 => Some(Field {
            key: "coordination_state",
            value: Node::Str(super::tags_grammar::coordinationstate(coordination_state)),
        }),
        10 => Some(Field {
            key: "polarity_scopes",
            value: Node::Seq(Sequence::PolarityScope(polarity_scopes)),
        }),
        11 => Some(Field {
            key: "conditions",
            value: Node::Seq(Sequence::Condition(conditions)),
        }),
        12 => Some(Field {
            key: "scopes",
            value: Node::Seq(Sequence::ScopeCandidate(scopes)),
        }),
        13 => Some(Field {
            key: "references",
            value: Node::Seq(Sequence::ReferenceCandidate(references)),
        }),
        14 => Some(Field {
            key: "constraint_graph",
            value: Node::Object(Object::ConstraintGraph(constraint_graph)),
        }),
        15 => Some(Field {
            key: "shared_argument_state",
            value: Node::Str(super::tags_grammar::sharedargument(shared_argument_state)),
        }),
        _ => None,
    }
}

pub(super) fn action(value: &crate::InstructionAction, index: usize) -> Option<Field<'_>> {
    let crate::InstructionAction {
        source_span,
        text,
        marker_span,
        modality,
        polarity,
        connector_before,
    } = value;
    match index {
        0 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        1 => Some(Field {
            key: "text",
            value: Node::Str(text),
        }),
        2 => Some(Field {
            key: "marker_span",
            value: marker_span
                .as_ref()
                .map(|v| Node::Object(Object::Range(v)))
                .unwrap_or(Node::Null),
        }),
        3 => Some(Field {
            key: "modality",
            value: Node::Str(super::tags_grammar::modality(modality)),
        }),
        4 => Some(Field {
            key: "polarity",
            value: Node::Str(super::tags_grammar::polarity(polarity)),
        }),
        5 => Some(Field {
            key: "connector_before",
            value: connector_before
                .as_ref()
                .map(|v| Node::Str(super::tags_grammar::conjunction(v)))
                .unwrap_or(Node::Null),
        }),
        _ => None,
    }
}

pub(super) fn coordination(
    value: &crate::InstructionCoordination,
    index: usize,
) -> Option<Field<'_>> {
    let crate::InstructionCoordination {
        left_action_index,
        right_action_index,
        kind,
        source_span,
    } = value;
    match index {
        0 => Some(Field {
            key: "left_action_index",
            value: number(left_action_index),
        }),
        1 => Some(Field {
            key: "right_action_index",
            value: number(right_action_index),
        }),
        2 => Some(Field {
            key: "kind",
            value: Node::Str(super::tags_grammar::conjunction(kind)),
        }),
        3 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        _ => None,
    }
}

pub(super) fn polarityscope(
    value: &crate::InstructionPolarityScope,
    index: usize,
) -> Option<Field<'_>> {
    let crate::InstructionPolarityScope {
        action_indices,
        polarity,
        marker_span,
        state,
    } = value;
    match index {
        0 => Some(Field {
            key: "action_indices",
            value: Node::Object(Object::Range(action_indices)),
        }),
        1 => Some(Field {
            key: "polarity",
            value: Node::Str(super::tags_grammar::polarity(polarity)),
        }),
        2 => Some(Field {
            key: "marker_span",
            value: Node::Object(Object::Range(marker_span)),
        }),
        3 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_grammar::polarityscope(state)),
        }),
        _ => None,
    }
}

pub(super) fn condition(value: &crate::InstructionCondition, index: usize) -> Option<Field<'_>> {
    let crate::InstructionCondition {
        source_span,
        marker_span,
        relation,
        temporal_relation,
        expression,
        applies_to_action_indices,
        scope_state,
        evidence_state,
        exception_precedence,
    } = value;
    match index {
        0 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        1 => Some(Field {
            key: "marker_span",
            value: Node::Object(Object::Range(marker_span)),
        }),
        2 => Some(Field {
            key: "relation",
            value: Node::Str(super::tags_grammar::conditionrelation(relation)),
        }),
        3 => Some(Field {
            key: "temporal_relation",
            value: temporal_relation
                .as_ref()
                .map(|v| Node::Str(super::tags_grammar::temporalrelation(v)))
                .unwrap_or(Node::Null),
        }),
        4 => Some(Field {
            key: "expression",
            value: Node::Object(Object::Expression(expression)),
        }),
        5 => Some(Field {
            key: "applies_to_action_indices",
            value: Node::Object(Object::Range(applies_to_action_indices)),
        }),
        6 => Some(Field {
            key: "scope_state",
            value: Node::Str(super::tags_grammar::conditionscope(scope_state)),
        }),
        7 => Some(Field {
            key: "evidence_state",
            value: Node::Str(super::tags_grammar::conditionevidence(evidence_state)),
        }),
        8 => Some(Field {
            key: "exception_precedence",
            value: exception_precedence
                .as_ref()
                .map(number)
                .unwrap_or(Node::Null),
        }),
        _ => None,
    }
}

pub(super) fn predicate(value: &crate::ConditionPredicate, index: usize) -> Option<Field<'_>> {
    let crate::ConditionPredicate { source_span, text } = value;
    match index {
        0 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        1 => Some(Field {
            key: "text",
            value: Node::Str(text),
        }),
        _ => None,
    }
}

pub(super) fn scopecandidate(
    value: &crate::InstructionScopeCandidate,
    index: usize,
) -> Option<Field<'_>> {
    let crate::InstructionScopeCandidate {
        kind,
        value,
        source_span,
        action_indices,
        state,
    } = value;
    match index {
        0 => Some(Field {
            key: "kind",
            value: Node::Str(super::tags_grammar::scopekind(kind)),
        }),
        1 => Some(Field {
            key: "value",
            value: Node::Str(value),
        }),
        2 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        3 => Some(Field {
            key: "action_indices",
            value: Node::Object(Object::Range(action_indices)),
        }),
        4 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_grammar::scopestate(state)),
        }),
        _ => None,
    }
}

pub(super) fn referencecandidate(
    value: &crate::InstructionReferenceCandidate,
    index: usize,
) -> Option<Field<'_>> {
    let crate::InstructionReferenceCandidate {
        text,
        source_span,
        candidate_action_indices,
        state,
    } = value;
    match index {
        0 => Some(Field {
            key: "text",
            value: Node::Str(text),
        }),
        1 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        2 => Some(Field {
            key: "candidate_action_indices",
            value: Node::Seq(Sequence::Usize(candidate_action_indices)),
        }),
        3 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_grammar::referencestate(state)),
        }),
        _ => None,
    }
}

pub(super) fn constraintgraph(
    value: &crate::InstructionConstraintGraph,
    index: usize,
) -> Option<Field<'_>> {
    let crate::InstructionConstraintGraph {
        nodes,
        edges,
        cycles,
    } = value;
    match index {
        0 => Some(Field {
            key: "nodes",
            value: Node::Seq(Sequence::ConstraintNode(nodes)),
        }),
        1 => Some(Field {
            key: "edges",
            value: Node::Seq(Sequence::ConstraintEdge(edges)),
        }),
        2 => Some(Field {
            key: "cycles",
            value: Node::Seq(Sequence::Cycle(cycles)),
        }),
        _ => None,
    }
}

pub(super) fn constraintnode(
    value: &crate::InstructionConstraintNode,
    index: usize,
) -> Option<Field<'_>> {
    let crate::InstructionConstraintNode {
        action_index,
        source_span,
        surface,
    } = value;
    match index {
        0 => Some(Field {
            key: "action_index",
            value: number(action_index),
        }),
        1 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        2 => Some(Field {
            key: "surface",
            value: Node::Str(surface),
        }),
        _ => None,
    }
}

pub(super) fn constraintedge(
    value: &crate::InstructionConstraintEdge,
    index: usize,
) -> Option<Field<'_>> {
    let crate::InstructionConstraintEdge {
        from_action_indices,
        to_action_indices,
        kind,
        source_span,
        target_span,
        state,
    } = value;
    match index {
        0 => Some(Field {
            key: "from_action_indices",
            value: Node::Object(Object::Range(from_action_indices)),
        }),
        1 => Some(Field {
            key: "to_action_indices",
            value: Node::Object(Object::Range(to_action_indices)),
        }),
        2 => Some(Field {
            key: "kind",
            value: Node::Str(super::tags_grammar::constraintkind(kind)),
        }),
        3 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        4 => Some(Field {
            key: "target_span",
            value: target_span
                .as_ref()
                .map(|v| Node::Object(Object::Range(v)))
                .unwrap_or(Node::Null),
        }),
        5 => Some(Field {
            key: "state",
            value: Node::Str(super::tags_grammar::edgestate(state)),
        }),
        _ => None,
    }
}

pub(super) fn cycle(value: &crate::InstructionDependencyCycle, index: usize) -> Option<Field<'_>> {
    let crate::InstructionDependencyCycle {
        action_indices,
        edge_indices,
        reason,
    } = value;
    match index {
        0 => Some(Field {
            key: "action_indices",
            value: Node::Seq(Sequence::Usize(action_indices)),
        }),
        1 => Some(Field {
            key: "edge_indices",
            value: Node::Seq(Sequence::Usize(edge_indices)),
        }),
        2 => Some(Field {
            key: "reason",
            value: Node::Str(reason),
        }),
        _ => None,
    }
}
