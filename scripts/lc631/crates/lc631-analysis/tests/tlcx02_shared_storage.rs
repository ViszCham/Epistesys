use lc631_analysis::{
    build_dgcl_pipeline, build_dgcl_projection, parse_controlled_instruction, DgclBackendSet,
    InstructionAst, InstructionParseBudget, InstructionParseState,
};

const SOURCE: &str = "Please test the parser.";

#[test]
fn tlcx02_selected_and_alternative_borrow_one_canonical_ast() {
    let report = parse_controlled_instruction(SOURCE, InstructionParseBudget::default()).unwrap();
    assert_eq!(report.state, InstructionParseState::Parsed);
    assert_eq!(report.alternatives.len(), 1);

    let selected: &InstructionAst = report.selected.as_ref().unwrap();
    let alternative: &InstructionAst = &report.alternatives[0];
    assert!(
        std::ptr::eq(selected, alternative),
        "selected AST must borrow the alternative's canonical allocation, not a deep clone"
    );
}

#[test]
fn tlcx02_program_and_contract_borrow_the_parser_ast_without_copying() {
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    assert_eq!(pipeline.program_ir.requirements.len(), 1);
    let requirement = &pipeline.program_ir.requirements[0];
    let parse = pipeline
        .instruction_parses
        .iter()
        .find(|parse| parse.region_id == requirement.region_id)
        .unwrap();
    let report = parse.report.as_ref().unwrap();
    assert_eq!(report.alternatives.len(), 1);
    assert_eq!(requirement.alternatives.len(), 1);

    let parser_ast: &InstructionAst = &report.alternatives[0];
    let program_ast: &InstructionAst = &requirement.alternatives[0].ast;
    let contract_ast: &InstructionAst =
        &requirement.contract.report.as_ref().unwrap().alternatives[0];
    assert!(
        std::ptr::eq(parser_ast, program_ast),
        "Program IR must reference the parser's canonical AST"
    );
    assert!(
        std::ptr::eq(parser_ast, contract_ast),
        "the contract view must reference the same canonical AST"
    );
    assert!(!pipeline.authority_created);
    assert!(!pipeline.program_ir.authority_created);
}

#[test]
fn tlcx02_legacy_pipeline_serialization_matches_the_installed_baseline_bytes() {
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let legacy = serde_json::to_string(&pipeline).unwrap();
    // Captured from the unchanged installed alpha.3 CLI's candidate_text for
    // this exact seed. This checks byte compatibility, not semantic correctness.
    assert_eq!(legacy.len(), 32_262);
    assert_eq!(
        lc631_core::stable_sha256(&legacy),
        "sha256:9e168f41ff0a3e8dc51109df288167d78970030ea2b7f35f662a6ee021990209"
    );
}

#[test]
fn tlcx02_canonical_source_and_parse_report_are_shared_downstream() {
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let requirement = &pipeline.program_ir.requirements[0];
    let parse = pipeline
        .instruction_parses
        .iter()
        .find(|parse| parse.region_id == requirement.region_id)
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(
        parse.report.as_ref().unwrap(),
        requirement.contract.report.as_ref().unwrap(),
    ));
    let ast = &requirement.alternatives[0].ast;
    let ast_source = ast.source_store().unwrap();
    assert_eq!(ast_source.id(), pipeline.source_store().id());
    assert_eq!(
        ast_source.text().as_ptr(),
        pipeline.source_store().text().as_ptr(),
    );
    assert_eq!(
        ast_source.slice(ast.root_source_span().unwrap()).unwrap(),
        SOURCE
    );
}

#[test]
fn tlcx02_derivation_ids_are_repeatable_and_distinct_without_runtime_handles() {
    let source = "Please testしてください";
    let first = parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
    let second = parse_controlled_instruction(source, InstructionParseBudget::default()).unwrap();
    assert_eq!(first.state, InstructionParseState::Ambiguous);
    assert_eq!(first.alternatives.len(), 2);
    let first_ids = first
        .alternatives
        .iter()
        .map(|ast| ast.canonical_id().unwrap())
        .collect::<Vec<_>>();
    let second_ids = second
        .alternatives
        .iter()
        .map(|ast| ast.canonical_id().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(first_ids, second_ids);
    assert_ne!(first_ids[0], first_ids[1]);
    let legacy = serde_json::to_string(&first).unwrap();
    assert!(!legacy.contains("canonical_id"));
    assert!(!legacy.contains("generation"));
    assert!(!legacy.contains("ArenaKey"));
}

#[test]
fn tlcx02_explicit_mutable_legacy_view_detaches_without_changing_the_canonical_ast() {
    let report = parse_controlled_instruction(SOURCE, InstructionParseBudget::default()).unwrap();
    let canonical = report.selected.as_ref().unwrap();
    let original = serde_json::to_string(canonical).unwrap();
    let mut legacy = canonical.clone();
    legacy.materialize_mut().actions[0]
        .text
        .push_str(" changed");
    assert_eq!(legacy.canonical_id(), None);
    assert_eq!(legacy.source_store().map(|store| store.id()), None);
    assert!(canonical.canonical_id().is_some());
    assert_eq!(serde_json::to_string(canonical).unwrap(), original);
    assert_ne!(serde_json::to_string(&legacy).unwrap(), original);
}

#[test]
fn tlcx02_identical_region_text_does_not_allow_transplanting_canonical_ast_handles() {
    let source = "Please test the parser.\n\nPlease test the parser.";
    let mut pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    assert_eq!(pipeline.instruction_parses.len(), 2);
    assert_eq!(pipeline.program_ir.requirements.len(), 2);
    assert_ne!(
        pipeline.instruction_parses[0].source_span,
        pipeline.instruction_parses[1].source_span,
    );
    let first = pipeline.instruction_parses[0].report.as_ref().unwrap();
    let first_revision = first.source_revision.clone();
    let selected = first.selected.clone();
    let alternatives = first.alternatives.clone();
    let second = std::sync::Arc::make_mut(pipeline.instruction_parses[1].report.as_mut().unwrap());
    assert_eq!(first_revision, second.source_revision);
    second.selected = selected;
    second.alternatives = alternatives;

    assert!(
        build_dgcl_projection(
            source,
            &pipeline.dg1,
            &pipeline.language_regions,
            &pipeline.instruction_parses,
        )
        .is_err(),
        "identical text must not authorize rebinding canonical AST handles to a different root span"
    );
}
