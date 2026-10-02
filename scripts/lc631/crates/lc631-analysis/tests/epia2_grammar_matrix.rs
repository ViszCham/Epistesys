use lc631_analysis::{
    build_dgcl_pipeline, parse_controlled_instruction, ConditionEvidenceState, DgclBackendSet,
    InstructionAst, InstructionParseBudget, InstructionParseState,
};

fn feature(family: &str, ast: &InstructionAst) -> bool {
    match family {
        "request" | "modality" | "japanese" => !ast.actions.is_empty(),
        "polarity" => ast
            .actions
            .iter()
            .any(|action| action.polarity == lc631_analysis::InstructionPolarity::Forbidden),
        "coordination" => ast.actions.len() >= 2 && !ast.coordination.is_empty(),
        "condition" | "temporal" | "exception" => {
            !ast.conditions.is_empty()
                && ast
                    .conditions
                    .iter()
                    .all(|condition| condition.evidence_state == ConditionEvidenceState::Unknown)
        }
        "scope" => !ast.scopes.is_empty(),
        "reference" => !ast.references.is_empty(),
        "dependency" => !ast.constraint_graph.edges.is_empty(),
        _ => false,
    }
}

#[test]
fn declared_families_each_have_three_positive_and_three_origin_boundary_negatives() {
    let families = [
        (
            "request",
            [
                "Please test the package.",
                "Please inspect the parser.",
                "Please validate the fixture.",
            ],
        ),
        (
            "modality",
            [
                "Must test the package.",
                "Should inspect the parser.",
                "May validate the fixture.",
            ],
        ),
        (
            "polarity",
            [
                "Do not publish.",
                "Must not publish.",
                "Should not publish.",
            ],
        ),
        (
            "japanese",
            ["テストしてください。", "公開しないでください。", "公開許可"],
        ),
        (
            "coordination",
            [
                "Please test and inspect.",
                "Please build and test.",
                "Please test but do not publish.",
            ],
        ),
        (
            "condition",
            [
                "Please publish if tests pass.",
                "Please publish only if approved.",
                "Please publish when approved.",
            ],
        ),
        (
            "exception",
            [
                "Please publish unless blocked.",
                "Please publish unless tests fail.",
                "Please publish unless approval is absent.",
            ],
        ),
        (
            "temporal",
            [
                "Please publish before tests expire.",
                "Please publish after tests pass.",
                "テストが終わるまで公開してください。",
            ],
        ),
        (
            "scope",
            [
                "Please deploy to staging.",
                "Please deploy to production.",
                "Please update file README.md.",
            ],
        ),
        (
            "reference",
            [
                "Please update it.",
                "Please inspect them.",
                "Please test the file and update it.",
            ],
        ),
        (
            "dependency",
            [
                "Please test then publish.",
                "Please publish only if tests pass.",
                "Please publish but do not publish.",
            ],
        ),
    ];
    let mut positive = 0;
    let mut negative = 0;
    for (family, examples) in families {
        for source in examples {
            let parsed =
                parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
            assert_eq!(
                parsed.state,
                InstructionParseState::Parsed,
                "{family}:{source}"
            );
            assert!(
                feature(family, parsed.selected.as_ref().unwrap()),
                "{family}:{source}"
            );
            positive += 1;
            // A positive instruction becomes three non-operative document
            // origins. These are development boundary cases, not gold labels.
            for quoted in [
                format!("> {source}"),
                format!("```text\n{source}\n```"),
                format!("`{source}`"),
            ] {
                let pipeline = build_dgcl_pipeline(
                    &quoted,
                    DgclBackendSet {
                        english: None,
                        japanese: None,
                    },
                )
                .unwrap();
                assert!(
                    pipeline.program_ir.requirements.is_empty(),
                    "{family}:{quoted}"
                );
                assert!(!pipeline.authority_created);
                assert!(
                    !pipeline
                        .implementation_closure
                        .implementation_complete_candidate
                );
                negative += 1;
            }
        }
    }
    assert_eq!(positive, 33);
    assert_eq!(negative, 99);
}

#[test]
fn malformed_ambiguous_and_cross_feature_contracts_remain_visible() {
    for source in ["Please test and", "Please build or", "Please inspect but"] {
        let parsed =
            parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
        assert_ne!(parsed.state, InstructionParseState::Parsed);
        assert!(parsed.selected.is_none());
    }
    for source in [
        "Please testしてください",
        "Please buildしてください",
        "Please inspectしてください",
    ] {
        let parsed =
            parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
        assert_eq!(parsed.state, InstructionParseState::Ambiguous);
        assert!(parsed.selected.is_none());
    }
    for source in [
        "Please publish if tests pass and approval exists.",
        "Please deploy to staging and do not publish.",
        "Please test the file and update it.",
    ] {
        let pipeline = build_dgcl_pipeline(
            source,
            DgclBackendSet {
                english: None,
                japanese: None,
            },
        )
        .unwrap();
        assert!(!pipeline.program_ir.requirements.is_empty());
        assert!(
            !pipeline
                .implementation_closure
                .implementation_complete_candidate
        );
        assert!(!pipeline.authority_created);
    }
}

#[test]
fn every_declared_family_has_three_semantic_or_malformed_negatives() {
    // Controlled-surface developer contracts, not an independent gold corpus.
    // Missing grammar operands must never select a normal instruction AST.
    let rows = [
        ("request", ["Please.", "Please!", "Please?"]),
        ("modality", ["Must.", "Should.", "May."]),
        ("polarity", ["Do not.", "Must not.", "Should not."]),
        ("japanese", ["してください。", "しないでください。", "許可"]),
        (
            "coordination",
            ["Please test and.", "Please test or.", "Please test but."],
        ),
        (
            "condition",
            [
                "Please publish if.",
                "Please publish only if.",
                "Please publish when.",
            ],
        ),
        (
            "exception",
            [
                "Please publish unless.",
                "Please publish unless not.",
                "Please publish unless ().",
            ],
        ),
        (
            "temporal",
            [
                "Please publish before.",
                "Please publish after.",
                "Please publish until.",
            ],
        ),
        (
            "scope",
            [
                "Please update file.",
                "Please deploy to.",
                "Please update branch.",
            ],
        ),
        (
            "reference",
            [
                "Please update it.",
                "Please inspect them.",
                "それを更新してください。",
            ],
        ),
        (
            "dependency",
            [
                "Please test then.",
                "Please publish only if.",
                "Please test then and.",
            ],
        ),
    ];
    for (family, examples) in rows {
        for source in examples {
            let parsed =
                parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
            if family == "reference" {
                // A syntactically valid anaphor is not a resolved target.
                let ast = parsed.selected.as_ref().unwrap();
                assert!(
                    ast.references.iter().any(|reference| reference.state
                        != lc631_analysis::InstructionReferenceState::Candidate),
                    "{source}"
                );
            } else {
                assert_ne!(
                    parsed.state,
                    InstructionParseState::Parsed,
                    "{family}:{source}"
                );
                assert!(parsed.selected.is_none(), "{family}:{source}");
            }
        }
    }
}
