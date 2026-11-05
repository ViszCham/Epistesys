use lc631_solvers::four_valued::{Atom, Error, Literal, Program, Rule, Truth};
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};

fn atom(index: u8) -> Atom {
    Atom::new(index).unwrap()
}
fn literal(slot: u8) -> Literal {
    if slot.is_multiple_of(2) {
        Literal::positive(atom(slot / 2))
    } else {
        Literal::negative(atom(slot / 2))
    }
}
fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of("four-valued fixture"))
}

#[test]
fn tlcx14_negative_no_explosion_neither_and_both_are_distinct() {
    let facts = [literal(0), literal(1)];
    let result = Program::new(2, &facts, &[], context(), [3; 32])
        .unwrap()
        .evaluate(100)
        .unwrap();
    assert_eq!(result.truth(atom(0)), Ok(Truth::Both));
    assert_eq!(result.truth(atom(1)), Ok(Truth::Neither));
    assert_eq!(result.contradiction_scope(atom(0)), Ok(Some(1)));
    assert_eq!(result.contradiction_scope(atom(1)), Ok(None));
    assert_eq!(result.dependencies(literal(2)), Ok(None));
}

#[test]
fn tlcx14_truth_tables_match_independent_designated_sets() {
    let values = [
        Truth::Neither,
        Truth::Supported,
        Truth::Refuted,
        Truth::Both,
    ];
    // Hand-written FDE order: Neither, true-only, false-only, Both.
    let and = [[0, 0, 2, 2], [0, 1, 2, 3], [2, 2, 2, 2], [2, 3, 2, 3]];
    let or = [[0, 1, 0, 1], [1, 1, 1, 1], [0, 1, 2, 3], [1, 1, 3, 3]];
    for (i, left) in values.iter().enumerate() {
        assert_eq!(left.negated(), values[[0, 2, 1, 3][i]]);
        for (j, right) in values.iter().enumerate() {
            assert_eq!(left.conjunction(*right), values[and[i][j]]);
            assert_eq!(left.disjunction(*right), values[or[i][j]]);
        }
    }
}

/// Independent model oracle: enumerate all subsets of six signed atoms,
/// retain models satisfying facts/rules, intersect them. No fixed-point code.
fn model_intersection(facts: u8, rules: &[(u8, u8)]) -> u8 {
    let mut intersection = 63;
    let mut models = 0;
    for model in 0u8..64 {
        if model & facts != facts {
            continue;
        }
        if rules
            .iter()
            .any(|(body, head)| model & body == *body && model & (1 << head) == 0)
        {
            continue;
        }
        models += 1;
        intersection &= model;
    }
    assert!(models > 0); // Full signed set is always a model of this fragment.
    intersection
}

#[test]
fn tlcx14_small_models_match_exhaustive_independent_oracle() {
    let vocabulary = [(1u8, 2u8), (4, 3), (3, 4), (16, 0), (0, 5)];
    let bodies: Vec<Vec<Literal>> = vocabulary
        .iter()
        .map(|(mask, _)| {
            (0..6)
                .filter(|slot| mask & (1 << slot) != 0)
                .map(literal)
                .collect()
        })
        .collect();
    for rules_mask in 0u8..32 {
        let selected: Vec<_> = vocabulary
            .iter()
            .enumerate()
            .filter(|(i, _)| rules_mask & (1 << i) != 0)
            .map(|(_, rule)| *rule)
            .collect();
        let rules: Vec<_> = vocabulary
            .iter()
            .enumerate()
            .filter(|(i, _)| rules_mask & (1 << i) != 0)
            .map(|(i, (_, head))| Rule {
                body: &bodies[i],
                head: literal(*head),
            })
            .collect();
        for facts_mask in 0u8..64 {
            let facts: Vec<_> = (0..6)
                .filter(|slot| facts_mask & (1 << slot) != 0)
                .map(literal)
                .collect();
            let expected = model_intersection(facts_mask, &selected);
            let got = Program::new(3, &facts, &rules, context(), [3; 32])
                .unwrap()
                .evaluate(100_000)
                .unwrap();
            for index in 0..3 {
                let state = got.truth(atom(index)).unwrap();
                assert_eq!(state.support(), expected & (1 << (index * 2)) != 0);
                assert_eq!(state.refutation(), expected & (1 << (index * 2 + 1)) != 0);
            }
        }
    }
}

#[test]
fn tlcx14_contradiction_provenance_and_cut_change_consumed_answer() {
    let facts = [literal(0), literal(1)];
    let body = [literal(0)];
    let rule = [Rule {
        body: &body,
        head: literal(2),
    }];
    let result = Program::new(3, &facts, &rule, context(), [3; 32])
        .unwrap()
        .evaluate(100)
        .unwrap();
    assert_eq!(result.truth(atom(1)), Ok(Truth::Supported));
    assert_eq!(result.dependencies(literal(2)), Ok(Some(1)));
    assert_eq!(result.binding().context(), context());
    assert_eq!(
        result.binding().source_digest(),
        *context().source().digest().as_bytes()
    );
    assert_eq!(result.binding().query_digest(), [3; 32]);
    assert_eq!(result.atom_count(), 3);
    assert!(result.steps() > 0);
    let cut = Program::new(3, &facts, &[], context(), [3; 32])
        .unwrap()
        .evaluate(100)
        .unwrap();
    assert_ne!(
        result.binding().formula_digest(),
        cut.binding().formula_digest()
    );
    assert_eq!(cut.truth(atom(1)), Ok(Truth::Neither));
    assert_eq!(cut.truth(atom(2)), Ok(Truth::Neither));
}

#[test]
fn tlcx14_duplicate_invalid_atom_scope_and_budget_rejected() {
    assert_eq!(Atom::new(64), Err(Error::InvalidAtom));
    assert!(matches!(
        Program::new(0, &[], &[], context(), [3; 32]),
        Err(Error::InvalidAtomCount)
    ));
    assert!(matches!(
        Program::new(1, &[literal(2)], &[], context(), [3; 32]),
        Err(Error::UndeclaredAtom)
    ));
    assert!(matches!(
        Program::new(1, &[literal(0), literal(0)], &[], context(), [3; 32]),
        Err(Error::DuplicateFact)
    ));
    let facts = [literal(0)];
    let program = Program::new(1, &facts, &[], context(), [3; 32]).unwrap();
    assert!(matches!(program.evaluate(0), Err(Error::ResourceExhausted)));
    let result = program.evaluate(1).unwrap();
    assert_eq!(result.truth(atom(1)), Err(Error::UndeclaredAtom));
    assert_eq!(literal(1).atom().index(), 0);
    assert!(literal(1).is_negative());
}

#[test]
fn tlcx14_rule_order_and_atom_relabel_preserve_semantics() {
    let facts = [literal(0)];
    let first_body = [literal(0)];
    let second_body = [literal(2)];
    let first = Rule {
        body: &first_body,
        head: literal(2),
    };
    let second = Rule {
        body: &second_body,
        head: literal(5),
    };
    for rules in [[first, second], [second, first]] {
        let got = Program::new(3, &facts, &rules, context(), [3; 32])
            .unwrap()
            .evaluate(100)
            .unwrap();
        assert_eq!(got.truth(atom(2)), Ok(Truth::Refuted));
    }
    let moved_facts = [literal(4)];
    let moved_body = [literal(4)];
    let moved_next = [literal(0)];
    let moved_rules = [
        Rule {
            body: &moved_body,
            head: literal(0),
        },
        Rule {
            body: &moved_next,
            head: literal(3),
        },
    ];
    let moved = Program::new(3, &moved_facts, &moved_rules, context(), [3; 32])
        .unwrap()
        .evaluate(100)
        .unwrap();
    assert_eq!(moved.truth(atom(1)), Ok(Truth::Refuted));
    assert_eq!(moved.dependencies(literal(3)), Ok(Some(4)));
}

#[test]
fn tlcx14_binding_derives_source_and_retains_reconstruction_and_typed_content() {
    let facts = [literal(0)];
    let base = Program::new(1, &facts, &[], context(), [3; 32])
        .unwrap()
        .evaluate(10)
        .unwrap();
    let reconstructed = context().reconstructed([7; 32]);
    let next = Program::new(1, &facts, &[], reconstructed, [3; 32])
        .unwrap()
        .evaluate(10)
        .unwrap();
    assert_eq!(
        base.binding().source_digest(),
        next.binding().source_digest()
    );
    assert_ne!(
        base.binding().interpretation_digest(),
        next.binding().interpretation_digest()
    );
    assert_eq!(
        base.binding().formula_digest(),
        next.binding().formula_digest()
    );
    assert_ne!(base.binding().input_digest(), next.binding().input_digest());
    assert_eq!(
        lc631_solvers::four_valued::PROFILE_REVISION,
        "four-valued-horn-v1"
    );
}
