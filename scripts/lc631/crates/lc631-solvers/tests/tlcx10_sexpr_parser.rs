use lc631_solvers::protocol::sexpr::{parse, SExprError, SExprKind, SExprLimits};

fn limits(max_bytes: usize, max_nodes: usize, max_depth: usize) -> SExprLimits {
    SExprLimits::checked(max_bytes, max_nodes, max_depth).unwrap()
}

#[test]
fn tlcx10_sexpr_nested_solver_shaped_forms_are_syntax_only() {
    let source = b"; fixture\n(model (define-fun x () Int 1))\n(proof (asserted (= x 1)))\n(counterexample (x 2))";
    let document = parse(source, limits(4096, 64, 16)).unwrap();

    assert_eq!(document.source_bytes(), source);
    assert_eq!(document.roots().len(), 3);
    assert_eq!(document.node_count(), 21);

    let model = document.node(document.roots()[0]).unwrap();
    assert_eq!(model.kind(), SExprKind::List);
    assert_eq!(
        document.raw(model.id()),
        Some(b"(model (define-fun x () Int 1))".as_slice())
    );
    let model_children: Vec<_> = document.children(model.id()).unwrap().collect();
    assert_eq!(model_children.len(), 2);
    assert_eq!(document.raw(model_children[0]), Some(b"model".as_slice()));
    assert_eq!(
        document.node(model_children[1]).unwrap().kind(),
        SExprKind::List
    );

    for root in &document.roots()[1..] {
        assert_eq!(document.node(*root).unwrap().kind(), SExprKind::List);
    }
    // Form names and nested terms are not interpreted as proof/model truth.
}

#[test]
fn tlcx10_sexpr_views_preserve_raw_string_and_quoted_symbol_spans() {
    let source = b"(|symbol with space| \"a\"\"b\")";
    let document = parse(source, limits(1024, 8, 4)).unwrap();
    let root = document.roots()[0];
    let children: Vec<_> = document.children(root).unwrap().collect();

    assert_eq!(children.len(), 2);
    let quoted = document.node(children[0]).unwrap();
    assert_eq!(quoted.kind(), SExprKind::QuotedSymbol);
    assert_eq!(
        document.raw(quoted.id()),
        Some(b"|symbol with space|".as_slice())
    );
    assert_eq!(
        document.raw_span(quoted.content_span().unwrap()),
        Some(b"symbol with space".as_slice())
    );

    let string = document.node(children[1]).unwrap();
    assert_eq!(string.kind(), SExprKind::StringLiteral);
    assert_eq!(document.raw(string.id()), Some(b"\"a\"\"b\"".as_slice()));
    assert_eq!(
        document.raw_span(string.content_span().unwrap()),
        Some(b"a\"\"b".as_slice()),
        "doubled-quote escape bytes remain unnormalized"
    );
    assert_eq!(string.span().start(), 21);
    assert_eq!(string.span().end(), 27);
}

#[test]
fn tlcx10_sexpr_rejects_invalid_utf8_and_unbalanced_forms() {
    assert!(matches!(
        parse(b"(a \xff)", limits(64, 16, 4)),
        Err(SExprError::InvalidUtf8 { .. })
    ));
    assert!(matches!(
        parse(b")", limits(64, 16, 4)),
        Err(SExprError::UnexpectedClose { offset: 0 })
    ));
    assert!(matches!(
        parse(b"(model (define-fun x () Int 1)", limits(64, 16, 4)),
        Err(SExprError::UnclosedList { .. })
    ));
    assert!(matches!(
        parse(b"(x \"unterminated)", limits(64, 16, 4)),
        Err(SExprError::UnclosedString { .. })
    ));
    assert!(matches!(
        parse(b"(x |unterminated)", limits(64, 16, 4)),
        Err(SExprError::UnclosedQuotedSymbol { .. })
    ));
}

#[test]
fn tlcx10_sexpr_rejects_malformed_quoted_symbols_and_adjacent_tokens() {
    assert!(matches!(
        parse(b"(x |bad\\symbol|)", limits(64, 16, 4)),
        Err(SExprError::InvalidQuotedSymbol { .. })
    ));
    assert!(matches!(
        parse(b"(x |quoted|tail)", limits(64, 16, 4)),
        Err(SExprError::MissingSeparator { .. })
    ));
    assert!(matches!(
        parse(b"(x \"text\"tail)", limits(64, 16, 4)),
        Err(SExprError::MissingSeparator { .. })
    ));
}

#[test]
fn tlcx10_sexpr_enforces_byte_node_and_depth_caps() {
    assert!(matches!(
        parse(b"(a)", limits(2, 16, 4)),
        Err(SExprError::InputTooLarge {
            actual: 3,
            limit: 2
        })
    ));
    assert!(matches!(
        parse(b"(a b)", limits(64, 2, 4)),
        Err(SExprError::NodeLimitExceeded)
    ));
    assert!(matches!(
        parse(b"(((x)))", limits(64, 16, 2)),
        Err(SExprError::DepthLimitExceeded)
    ));
}
