//! Separate recursive per-world oracle, sharing only the public formula syntax.
use lc631_solvers::finite_modal::{
    self, evaluate, projection_obligations, Error, Formula as F, Frame, Model, Profile,
};

fn related(rows: &[u64], w: usize, v: usize) -> bool {
    rows[w] & (1 << v) != 0
}

// Independently expressed frame predicate, not Frame::new or its implementation.
fn admissible(p: Profile, rows: &[u64]) -> bool {
    let n = rows.len();
    match p {
        Profile::ModalK => true,
        Profile::DeonticSerialKd => (0..n).all(|w| (0..n).any(|v| related(rows, w, v))),
        Profile::EpistemicS5SingleAgent | Profile::IntuitionisticPreorder => {
            (0..n).all(|w| related(rows, w, w))
                && (0..n).all(|w| {
                    (0..n).all(|v| {
                        (!related(rows, w, v)
                            || p != Profile::EpistemicS5SingleAgent
                            || related(rows, v, w))
                            && (0..n).all(|u| {
                                !related(rows, w, v) || !related(rows, v, u) || related(rows, w, u)
                            })
                    })
                })
        }
    }
}

// Direct recursive satisfaction over world membership, no bitset truth closure.
fn oracle(p: Profile, rows: &[u64], atom: u64, nodes: &[F], id: usize, w: usize) -> bool {
    let at = |i: u16, v| oracle(p, rows, atom, nodes, usize::from(i), v);
    match nodes[id] {
        F::Bottom => false,
        F::Top => true,
        F::Atom(0) => atom & (1 << w) != 0,
        F::Atom(_) => panic!("oracle corpus contains one declared atom"),
        F::And(a, b) => at(a, w) && at(b, w),
        F::Or(a, b) => at(a, w) || at(b, w),
        F::Not(a) if p == Profile::IntuitionisticPreorder => {
            (0..rows.len()).all(|v| !related(rows, w, v) || !at(a, v))
        }
        F::Not(a) => !at(a, w),
        F::Implies(a, b) if p == Profile::IntuitionisticPreorder => {
            (0..rows.len()).all(|v| !related(rows, w, v) || !at(a, v) || at(b, v))
        }
        F::Implies(a, b) => !at(a, w) || at(b, w),
        F::Box(a) | F::Know(a) | F::Ought(a) => {
            (0..rows.len()).all(|v| !related(rows, w, v) || at(a, v))
        }
        F::Diamond(a) => (0..rows.len()).any(|v| related(rows, w, v) && at(a, v)),
    }
}

#[test]
fn tlcx15_exhaustive_frames_and_valuations_through_three_worlds_match_recursive_oracle() {
    let mut evaluated = [0_usize; 4];
    for (pi, profile) in [
        Profile::ModalK,
        Profile::EpistemicS5SingleAgent,
        Profile::IntuitionisticPreorder,
        Profile::DeonticSerialKd,
    ]
    .into_iter()
    .enumerate()
    {
        let mut nodes = vec![
            F::Atom(0),
            F::Bottom,
            F::Top,
            F::Not(0),
            F::Or(0, 3),
            F::Not(3),
            F::Implies(5, 0),
            F::And(0, 3),
            F::Implies(1, 0),
            F::Implies(0, 1),
        ];
        match profile {
            Profile::ModalK => {
                nodes.push(F::Box(0));
                nodes.push(F::Diamond(0));
            }
            Profile::EpistemicS5SingleAgent => nodes.push(F::Know(0)),
            Profile::DeonticSerialKd => nodes.push(F::Ought(0)),
            Profile::IntuitionisticPreorder => {}
        }
        for n in 1..=3 {
            for bits in 0..(1_u64 << (n * n)) {
                let rows: Vec<u64> = (0..n).map(|w| (bits >> (w * n)) & ((1 << n) - 1)).collect();
                let frame = Frame::new(profile, &rows);
                assert_eq!(
                    frame.is_ok(),
                    admissible(profile, &rows),
                    "{profile:?}/{rows:?}"
                );
                let Ok(frame) = frame else {
                    continue;
                };
                assert_eq!(frame.profile(), profile);
                assert_eq!(frame.world_count(), n);
                for atom in 0..(1_u64 << n) {
                    let persistent = (0..n).all(|w| {
                        (0..n).all(|v| {
                            atom & (1 << w) == 0 || !related(&rows, w, v) || atom & (1 << v) != 0
                        })
                    });
                    let atoms = [atom];
                    let model = Model::new(frame, &atoms);
                    assert_eq!(
                        model.is_ok(),
                        profile != Profile::IntuitionisticPreorder || persistent
                    );
                    let Ok(model) = model else {
                        continue;
                    };
                    for root in 0..nodes.len() {
                        let actual = evaluate(model, &nodes, root as u16).unwrap();
                        assert_eq!(actual.profile(), profile);
                        assert_eq!(actual.root(), root as u16);
                        assert_eq!(actual.true_worlds() & actual.false_worlds(), 0);
                        assert_eq!(actual.true_worlds() | actual.false_worlds(), (1 << n) - 1);
                        for w in 0..n {
                            assert_eq!(
                                actual.at_world(w as u8).unwrap(),
                                oracle(profile, &rows, atom, &nodes, root, w),
                                "{profile:?}, rows={rows:?}, atom={atom}, root={root}, world={w}"
                            );
                        }
                        assert_eq!(actual.at_world(n as u8), Err(Error::InvalidWorldBits));
                        evaluated[pi] += 1;
                    }
                }
            }
        }
        assert!(!projection_obligations(profile).is_empty());
    }
    assert!(evaluated.iter().all(|count| *count > 100));
}

fn truth(profile: Profile, rows: &[u64], atoms: &[u64], nodes: &[F]) -> u64 {
    evaluate(
        Model::new(Frame::new(profile, rows).unwrap(), atoms).unwrap(),
        nodes,
        (nodes.len() - 1) as u16,
    )
    .unwrap()
    .true_worlds()
}

#[test]
fn tlcx15_hand_authored_countermodels_distinguish_profiles() {
    // K has no reflexivity requirement: box p can hold where p is false.
    assert_eq!(
        truth(Profile::ModalK, &[2, 0], &[2], &[F::Atom(0), F::Box(0)]),
        3
    );
    // Knowledge is factive on checked S5 frames; ignorance is not falsity.
    assert_eq!(
        truth(
            Profile::EpistemicS5SingleAgent,
            &[3, 3],
            &[2],
            &[F::Atom(0), F::Know(0)]
        ),
        0
    );
    assert_eq!(
        truth(
            Profile::EpistemicS5SingleAgent,
            &[3, 3],
            &[3],
            &[F::Atom(0), F::Know(0)]
        ),
        3
    );
    // Excluded middle and double-negation elimination fail at the root.
    let nodes = [F::Atom(0), F::Not(0), F::Or(0, 1)];
    assert_eq!(
        truth(Profile::IntuitionisticPreorder, &[3, 2], &[2], &nodes),
        2
    );
    assert_eq!(truth(Profile::ModalK, &[3, 2], &[2], &nodes), 3);
    assert_eq!(
        truth(
            Profile::IntuitionisticPreorder,
            &[3, 2],
            &[2],
            &[F::Atom(0), F::Not(0), F::Not(1), F::Implies(2, 0)]
        ),
        2
    );
    // Ideal-world obligation can hold while actual p is false. No grant API exists.
    assert_eq!(
        truth(
            Profile::DeonticSerialKd,
            &[2, 2],
            &[2],
            &[F::Atom(0), F::Ought(0)]
        ),
        3
    );
    assert_eq!(
        truth(
            Profile::DeonticSerialKd,
            &[2, 2],
            &[0],
            &[F::Atom(0), F::Ought(0)]
        ),
        0
    );
}

#[test]
fn tlcx15_rejects_invalid_frames_fragments_and_resource_bounds() {
    assert!(matches!(
        Frame::new(Profile::ModalK, &[]),
        Err(Error::EmptyFrame)
    ));
    assert!(matches!(
        Frame::new(Profile::ModalK, &[0; 33]),
        Err(Error::LimitExceeded)
    ));
    assert!(matches!(
        Frame::new(Profile::ModalK, &[2]),
        Err(Error::InvalidWorldBits)
    ));
    assert!(matches!(
        Frame::new(Profile::DeonticSerialKd, &[0]),
        Err(Error::NotSerial)
    ));
    assert!(matches!(
        Frame::new(Profile::EpistemicS5SingleAgent, &[0]),
        Err(Error::NotReflexive)
    ));
    assert!(matches!(
        Frame::new(Profile::EpistemicS5SingleAgent, &[3, 2]),
        Err(Error::NotSymmetric)
    ));
    assert!(matches!(
        Frame::new(Profile::IntuitionisticPreorder, &[3, 6, 4]),
        Err(Error::NotTransitive)
    ));
    let frame = Frame::new(Profile::ModalK, &[0]).unwrap();
    assert!(matches!(
        Model::new(frame, &[2]),
        Err(Error::InvalidWorldBits)
    ));
    assert!(matches!(
        Model::new(frame, &[0; 65]),
        Err(Error::LimitExceeded)
    ));
    let i = Frame::new(Profile::IntuitionisticPreorder, &[3, 2]).unwrap();
    assert!(matches!(Model::new(i, &[1]), Err(Error::NonPersistentAtom)));
    let model = Model::new(frame, &[1]).unwrap();
    assert_eq!(evaluate(model, &[], 0), Err(Error::EmptyFormula));
    assert_eq!(
        evaluate(model, &[F::Top; 257], 0),
        Err(Error::LimitExceeded)
    );
    assert_eq!(evaluate(model, &[F::Top], 1), Err(Error::InvalidRoot));
    assert_eq!(evaluate(model, &[F::Atom(1)], 0), Err(Error::InvalidAtom));
    assert_eq!(
        evaluate(model, &[F::Not(0)], 0),
        Err(Error::ForwardOrCyclicReference)
    );
    // Malformed unused nodes are rejected too.
    assert_eq!(
        evaluate(model, &[F::Top, F::Not(2)], 0),
        Err(Error::ForwardOrCyclicReference)
    );
    for op in [F::Know(0), F::Ought(0)] {
        assert_eq!(
            evaluate(model, &[F::Atom(0), op], 1),
            Err(Error::UnsupportedOperator)
        );
    }
    for profile in [
        Profile::EpistemicS5SingleAgent,
        Profile::IntuitionisticPreorder,
        Profile::DeonticSerialKd,
    ] {
        let f = Frame::new(profile, &[1]).unwrap();
        for op in [F::Box(0), F::Diamond(0)] {
            assert_eq!(
                evaluate(Model::new(f, &[1]).unwrap(), &[F::Atom(0), op], 1),
                Err(Error::UnsupportedOperator)
            );
        }
    }
}

#[test]
fn tlcx15_relabeling_worlds_preserves_satisfaction() {
    // Swap worlds 0 and 1 in the two-world K countermodel.
    let formula = [F::Atom(0), F::Box(0), F::Diamond(0), F::Implies(1, 2)];
    let a = truth(Profile::ModalK, &[2, 0], &[2], &formula);
    let b = truth(Profile::ModalK, &[0, 1], &[1], &formula);
    assert_eq!(b, ((a & 1) << 1) | ((a & 2) >> 1));
}

#[test]
fn tlcx15_native_lineage_is_retained_without_validation_promotion() {
    use finite_modal::{evaluate_with_lineage, CallerLineage, Termination, ValidationBasis};
    use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
    let context =
        ContextRevision::original(SourceRevision::of("finite model source")).reconstructed([2; 32]);
    let lineage = CallerLineage {
        context,
        query_revision: [3; 32],
    };
    let model = Model::new(Frame::new(Profile::ModalK, &[0]).unwrap(), &[1]).unwrap();
    let observation = evaluate_with_lineage(lineage, model, &[F::Atom(0)], 0).unwrap();
    assert_eq!(observation.caller_lineage(), lineage);
    assert_eq!(observation.context(), context);
    assert_eq!(
        observation.source_digest(),
        *context.source().digest().as_bytes()
    );
    assert_eq!(observation.interpretation_digest(), [2; 32]);
    assert_eq!(observation.query_digest(), [3; 32]);
    assert_eq!(observation.finite_world_count(), 1);
    assert_eq!(
        observation.termination(),
        Termination::CompletedFiniteEvaluation
    );
    assert_eq!(
        observation.validation_basis(),
        ValidationBasis::EvaluatorReported
    );
    assert_eq!(observation.evaluation().true_worlds(), 1);
    assert_eq!(
        evaluate_with_lineage(lineage, model, &[F::Atom(1)], 0),
        Err(Error::InvalidAtom)
    );
}

#[test]
fn tlcx15_internal_identity_changes_for_model_formula_context_query_and_root() {
    use finite_modal::{evaluate_with_lineage, CallerLineage};
    use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
    let context = ContextRevision::original(SourceRevision::of("declared source"));
    let lineage = CallerLineage {
        context,
        query_revision: [4; 32],
    };
    let nodes = [F::Atom(0), F::Not(0)];
    let frame = Frame::new(Profile::ModalK, &[1]).unwrap();
    let original =
        evaluate_with_lineage(lineage, Model::new(frame, &[1]).unwrap(), &nodes, 0).unwrap();
    let changed_model =
        evaluate_with_lineage(lineage, Model::new(frame, &[0]).unwrap(), &nodes, 0).unwrap();
    assert_ne!(original.model_digest(), changed_model.model_digest());
    assert_ne!(original.input_digest(), changed_model.input_digest());
    let root = evaluate_with_lineage(lineage, Model::new(frame, &[1]).unwrap(), &nodes, 1).unwrap();
    assert_ne!(original.formula_digest(), root.formula_digest());
    let formula =
        evaluate_with_lineage(lineage, Model::new(frame, &[1]).unwrap(), &[F::Top], 0).unwrap();
    assert_ne!(original.formula_digest(), formula.formula_digest());
    let other_context = CallerLineage {
        context: context.reconstructed([7; 32]),
        ..lineage
    };
    let other_query = CallerLineage {
        query_revision: [8; 32],
        ..lineage
    };
    for altered in [other_context, other_query] {
        let other =
            evaluate_with_lineage(altered, Model::new(frame, &[1]).unwrap(), &nodes, 0).unwrap();
        assert_eq!(original.formula_digest(), other.formula_digest());
        assert_eq!(original.model_digest(), other.model_digest());
        assert_ne!(original.input_digest(), other.input_digest());
    }
}
