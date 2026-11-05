use lc631_solvers::asp::*;
use lc631_substrate::context_identity::{ContextRevision, SourceRevision};
use lc631_substrate::Digest32;

fn binding() -> AspBinding {
    AspBinding::declared(
        ContextRevision::original(SourceRevision::of("birds and exceptions")),
        Digest32::from_bytes([2; 32]),
        Digest32::from_bytes([3; 32]),
    )
}

fn atom(id: u16) -> AtomId {
    AtomId::new(id)
}

fn rule(head: Option<u16>, positive: &[u16], negative: &[u16]) -> GroundRule {
    GroundRule::new(
        head.map(atom),
        positive.iter().copied().map(atom).collect(),
        negative.iter().copied().map(atom).collect(),
    )
}

fn program(rules: Vec<GroundRule>) -> AspProgram {
    AspProgram::checked(
        binding(),
        AspSemantics::StableModelsClosedWorld,
        vec![atom(0), atom(1)],
        rules,
        vec![],
        AspLimits::default(),
    )
    .unwrap()
}

#[test]
fn tlcx12_rejects_undeclared_atom_and_implicit_closed_world() {
    assert_eq!(
        AspProgram::checked(
            binding(),
            AspSemantics::Unspecified,
            vec![],
            vec![],
            vec![],
            AspLimits::default()
        )
        .unwrap_err(),
        AspError::SemanticsRequired
    );
    assert_eq!(
        AspProgram::checked(
            binding(),
            AspSemantics::StableModelsClosedWorld,
            vec![atom(0)],
            vec![rule(Some(1), &[], &[])],
            vec![],
            AspLimits::default()
        )
        .unwrap_err(),
        AspError::UndeclaredAtom
    );
}

#[test]
fn tlcx12_brave_is_not_cautious_and_models_are_not_collapsed() {
    let p = program(vec![rule(Some(0), &[], &[1]), rule(Some(1), &[], &[0])]);
    let result = p.enumerate().unwrap();
    assert_eq!(result.models().len(), 2);
    assert_eq!(
        result.consequence(atom(0), ConsequenceMode::Brave).unwrap(),
        Consequence::Holds
    );
    assert_eq!(
        result
            .consequence(atom(0), ConsequenceMode::Cautious)
            .unwrap(),
        Consequence::DoesNotHold
    );
    assert_eq!(p.stratification(), Stratification::NonStratified);
}

#[test]
fn tlcx12_exception_retracts_default_in_separate_context() {
    let p = program(vec![rule(Some(0), &[], &[1])]);
    assert_eq!(
        p.enumerate()
            .unwrap()
            .consequence(atom(0), ConsequenceMode::Cautious)
            .unwrap(),
        Consequence::Holds
    );
    let old = binding();
    let new = AspBinding::declared(
        old.context().reconstructed([9; 32]),
        old.domain(),
        old.grounding_basis(),
    );
    let q = AspProgram::checked(
        new,
        AspSemantics::StableModelsClosedWorld,
        vec![atom(0), atom(1)],
        vec![rule(Some(0), &[], &[1]), rule(Some(1), &[], &[])],
        vec![],
        AspLimits::default(),
    )
    .unwrap();
    let result = q.enumerate().unwrap();
    assert_eq!(result.binding(), new);
    assert_eq!(
        result
            .consequence(atom(0), ConsequenceMode::Cautious)
            .unwrap(),
        Consequence::DoesNotHold
    );
    assert_ne!(
        p.prepare().unwrap().solver_query().unwrap().digest(),
        q.prepare().unwrap().solver_query().unwrap().digest()
    );
    assert_eq!(p.binding(), old);
}

#[test]
fn tlcx12_no_model_is_not_vacuous_cautious_truth() {
    let p = program(vec![rule(None, &[], &[])]);
    let result = p.enumerate().unwrap();
    assert!(result.models().is_empty());
    assert_eq!(
        result
            .consequence(atom(0), ConsequenceMode::Cautious)
            .unwrap(),
        Consequence::NoStableModels
    );
}

#[test]
fn tlcx12_cutoff_retains_unknown_and_no_native_optimization_claim() {
    let limits = AspLimits::checked(16, 32, 64, 1, 1_000_000, 64_000).unwrap();
    let p = AspProgram::checked(
        binding(),
        AspSemantics::StableModelsClosedWorld,
        vec![atom(0), atom(1)],
        vec![rule(Some(0), &[], &[1]), rule(Some(1), &[], &[0])],
        vec![],
        limits,
    )
    .unwrap();
    let result = p.enumerate().unwrap();
    assert_eq!(result.completeness(), AspCompleteness::ModelLimit);
    assert_eq!(
        result
            .consequence(atom(0), ConsequenceMode::Cautious)
            .unwrap(),
        Consequence::Unknown
    );
    assert!(!result.clingo_native_optimization());
}

#[test]
fn tlcx12_model_reconstruction_rejects_missing_or_wrong_answers() {
    let p = program(vec![rule(Some(0), &[], &[1]), rule(Some(1), &[], &[0])]);
    assert_eq!(
        p.reconstruct(b"Answer: 1\na0\nSATISFIABLE\n").unwrap_err(),
        AspError::IncompleteAnswerSet
    );
    assert_eq!(
        p.reconstruct(b"Answer: 1\na0 a1\nSATISFIABLE\n")
            .unwrap_err(),
        AspError::InvalidStableModel
    );
    assert_eq!(
        p.reconstruct(b"Answer: 1\na99\nSATISFIABLE\n").unwrap_err(),
        AspError::UndeclaredAtom
    );
    assert_eq!(
        p.reconstruct(b"UNKNOWN\n").unwrap().completeness(),
        AspCompleteness::SolverUnknown
    );
}

#[test]
fn tlcx12_cost_minimum_retains_ties_and_full_model_consequences() {
    let p = AspProgram::checked(
        binding(),
        AspSemantics::StableModelsClosedWorld,
        vec![atom(0), atom(1)],
        vec![rule(Some(0), &[], &[1]), rule(Some(1), &[], &[0])],
        vec![
            AtomCost {
                atom: atom(0),
                weight: 8,
            },
            AtomCost {
                atom: atom(1),
                weight: 3,
            },
        ],
        AspLimits::default(),
    )
    .unwrap();
    let result = p.enumerate().unwrap();
    assert_eq!(result.minimum_cost(), Some(3));
    assert_eq!(result.optimal_model_indices(), vec![1]);
    assert_eq!(result.models().len(), 2);
    assert_eq!(
        result
            .consequence(atom(1), ConsequenceMode::Cautious)
            .unwrap(),
        Consequence::DoesNotHold
    );
    let equal = AspProgram::checked(
        binding(),
        AspSemantics::StableModelsClosedWorld,
        vec![atom(0), atom(1)],
        vec![rule(Some(0), &[], &[1]), rule(Some(1), &[], &[0])],
        vec![],
        AspLimits::default(),
    )
    .unwrap()
    .enumerate()
    .unwrap();
    assert_eq!(equal.optimal_model_indices(), vec![0, 1]);
}

#[test]
fn tlcx12_step_grounding_output_and_cost_overflow_bounds() {
    assert_eq!(
        AspProgram::checked(
            binding(),
            AspSemantics::StableModelsClosedWorld,
            (0..17).map(atom).collect(),
            vec![],
            vec![],
            AspLimits::default()
        )
        .unwrap_err(),
        AspError::AtomLimit
    );
    assert_eq!(
        AspProgram::checked(
            binding(),
            AspSemantics::StableModelsClosedWorld,
            vec![atom(0), atom(1)],
            vec![],
            vec![
                AtomCost {
                    atom: atom(0),
                    weight: u64::MAX
                },
                AtomCost {
                    atom: atom(1),
                    weight: 1
                }
            ],
            AspLimits::default()
        )
        .unwrap_err(),
        AspError::ArithmeticOverflow
    );
    let limits = AspLimits::checked(16, 32, 64, 256, 1, 1).unwrap();
    let p = AspProgram::checked(
        binding(),
        AspSemantics::StableModelsClosedWorld,
        vec![atom(0)],
        vec![rule(Some(0), &[], &[])],
        vec![],
        limits,
    )
    .unwrap();
    let result = p.enumerate().unwrap();
    assert_eq!(result.completeness(), AspCompleteness::StepLimit);
    assert_eq!(result.minimum_cost(), None);
    assert_eq!(
        result.consequence(atom(0), ConsequenceMode::Brave).unwrap(),
        Consequence::Unknown
    );
    assert_eq!(p.prepare().unwrap_err(), AspError::OutputLimit);
}

#[test]
fn tlcx12_context_only_revision_changes_query_and_reconstruction_identity() {
    let p = program(vec![]);
    let q = AspProgram::checked(
        AspBinding::declared(
            binding().context().reconstructed([8; 32]),
            binding().domain(),
            binding().grounding_basis(),
        ),
        AspSemantics::StableModelsClosedWorld,
        vec![atom(0), atom(1)],
        vec![],
        vec![],
        AspLimits::default(),
    )
    .unwrap();
    let pp = p.prepare().unwrap();
    let qq = q.prepare().unwrap();
    assert_eq!(
        pp.solver_query().unwrap().source(),
        qq.solver_query().unwrap().source()
    );
    assert_ne!(
        pp.solver_query().unwrap().digest(),
        qq.solver_query().unwrap().digest()
    );
    let result = q.reconstruct(b"Answer: 1\n\nSATISFIABLE\n").unwrap();
    assert_eq!(result.binding(), q.binding());
    assert_eq!(result.model_atoms(0), Some(vec![]));
}

#[test]
fn tlcx12_duplicate_model_and_optimization_status_are_not_success() {
    let p = program(vec![]);
    assert_eq!(
        p.reconstruct(b"Answer: 1\n\nAnswer: 2\n\nSATISFIABLE\n")
            .unwrap_err(),
        AspError::DuplicateModel
    );
    assert_eq!(
        p.reconstruct(b"OPTIMUM FOUND\n").unwrap_err(),
        AspError::UnsupportedOutput
    );
    assert_eq!(
        p.reconstruct(b"UNSATISFIABLE\n").unwrap_err(),
        AspError::StatusMismatch
    );
    assert_eq!(
        p.reconstruct(b"Answer: 1\na0").unwrap_err(),
        AspError::OutputSyntax
    );
}

// Independent oracle: minimal models of the reduct, not the production
// least-fixed-point algorithm. Raw tuple masks intentionally avoid its IR helpers.
fn oracle(rules: &[(u8, u8, u8)]) -> Vec<u8> {
    let satisfies = |assignment: u8, candidate: u8| {
        rules.iter().all(|&(head, positive, negative)| {
            negative & candidate != 0 || positive & assignment != positive || head & assignment != 0
        })
    };
    (0..4u8)
        .filter(|&candidate| {
            satisfies(candidate, candidate)
                && !(0..4u8).any(|subset| {
                    subset != candidate
                        && subset & candidate == subset
                        && satisfies(subset, candidate)
                })
        })
        .collect()
}

#[test]
fn tlcx12_all_two_atom_two_rule_programs_match_independent_minimal_reduct_oracle() {
    let mut catalogue = Vec::new();
    for head in [0u8, 1, 2] {
        for positive in 0..4u8 {
            for negative in 0..4u8 {
                catalogue.push((head, positive, negative));
            }
        }
    }
    for first in &catalogue {
        for second in &catalogue {
            let raw = [*first, *second];
            let rules = raw
                .iter()
                .map(|&(head, positive, negative)| {
                    let atoms = |mask: u8| {
                        (0..2u16)
                            .filter(|i| mask & (1u8 << i) != 0)
                            .map(atom)
                            .collect()
                    };
                    GroundRule::new(
                        match head {
                            0 => None,
                            1 => Some(atom(0)),
                            2 => Some(atom(1)),
                            _ => unreachable!(),
                        },
                        atoms(positive),
                        atoms(negative),
                    )
                })
                .collect();
            let result = program(rules).enumerate().unwrap();
            let actual: Vec<u8> = (0..result.models().len())
                .map(|index| {
                    result
                        .model_atoms(index)
                        .unwrap()
                        .iter()
                        .fold(0u8, |mask, a| mask | (1u8 << a.get()))
                })
                .collect();
            assert_eq!(actual, oracle(&raw), "rules={raw:?}");
        }
    }
}
