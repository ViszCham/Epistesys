//! Explicit borrowed language fields; no Serialize and no wildcard field omission.
use super::{boolean, number, Field, Node, Object, Sequence};

pub(super) fn languageregion(value: &crate::DgclLanguageRegion, index: usize) -> Option<Field<'_>> {
    let crate::DgclLanguageRegion {
        region_id,
        language_span_id,
        source_span,
        root_source_revision,
        region_source_revision,
        language,
        backend_revision,
        model_manifest_digest,
        runtime_manifest_digest,
        requirements_lock_digest,
        python_version,
        payload_binding_revision,
        payload_digest,
        sentences,
        semantic_truth_claim,
    } = value;
    match index {
        0 => Some(Field {
            key: "region_id",
            value: number(region_id),
        }),
        1 => Some(Field {
            key: "language_span_id",
            value: number(language_span_id),
        }),
        2 => Some(Field {
            key: "source_span",
            value: Node::Object(Object::Range(source_span)),
        }),
        3 => Some(Field {
            key: "root_source_revision",
            value: Node::Str(root_source_revision),
        }),
        4 => Some(Field {
            key: "region_source_revision",
            value: Node::Str(region_source_revision),
        }),
        5 => Some(Field {
            key: "language",
            value: Node::Str(super::tags_pipeline::dg1language(language)),
        }),
        6 => Some(Field {
            key: "backend_revision",
            value: Node::Str(backend_revision),
        }),
        7 => Some(Field {
            key: "model_manifest_digest",
            value: Node::Str(model_manifest_digest),
        }),
        8 => Some(Field {
            key: "runtime_manifest_digest",
            value: Node::Str(runtime_manifest_digest),
        }),
        9 => Some(Field {
            key: "requirements_lock_digest",
            value: Node::Str(requirements_lock_digest),
        }),
        10 => Some(Field {
            key: "python_version",
            value: Node::Str(python_version),
        }),
        11 => Some(Field {
            key: "payload_binding_revision",
            value: Node::Str(payload_binding_revision),
        }),
        12 => Some(Field {
            key: "payload_digest",
            value: Node::Str(payload_digest),
        }),
        13 => Some(Field {
            key: "sentences",
            value: Node::Seq(Sequence::Sentence(sentences)),
        }),
        14 => Some(Field {
            key: "semantic_truth_claim",
            value: boolean(semantic_truth_claim),
        }),
        _ => None,
    }
}

pub(super) fn sentence(value: &crate::DependencySentence, index: usize) -> Option<Field<'_>> {
    let crate::DependencySentence { tokens } = value;
    match index {
        0 => Some(Field {
            key: "tokens",
            value: Node::Seq(Sequence::Token(tokens)),
        }),
        _ => None,
    }
}

pub(super) fn token(value: &crate::DependencyToken, index: usize) -> Option<Field<'_>> {
    let crate::DependencyToken {
        text,
        start_byte,
        end_byte,
        words,
    } = value;
    match index {
        0 => Some(Field {
            key: "text",
            value: Node::Str(text),
        }),
        1 => Some(Field {
            key: "start_byte",
            value: number(start_byte),
        }),
        2 => Some(Field {
            key: "end_byte",
            value: number(end_byte),
        }),
        3 => Some(Field {
            key: "words",
            value: Node::Seq(Sequence::Word(words)),
        }),
        _ => None,
    }
}

pub(super) fn word(value: &crate::DependencyWord, index: usize) -> Option<Field<'_>> {
    let crate::DependencyWord {
        id,
        text,
        head,
        deprel,
        upos,
        lemma,
        feats,
    } = value;
    match index {
        0 => Some(Field {
            key: "id",
            value: number(id),
        }),
        1 => Some(Field {
            key: "text",
            value: Node::Str(text),
        }),
        2 => Some(Field {
            key: "head",
            value: number(head),
        }),
        3 => Some(Field {
            key: "deprel",
            value: Node::Str(deprel),
        }),
        4 => Some(Field {
            key: "upos",
            value: upos.as_deref().map(Node::Str).unwrap_or(Node::Null),
        }),
        5 => Some(Field {
            key: "lemma",
            value: lemma.as_deref().map(Node::Str).unwrap_or(Node::Null),
        }),
        6 => Some(Field {
            key: "feats",
            value: feats.as_deref().map(Node::Str).unwrap_or(Node::Null),
        }),
        _ => None,
    }
}
