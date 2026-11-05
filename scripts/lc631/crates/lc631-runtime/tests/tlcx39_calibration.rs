use lc631_runtime::calibration::{self, *};
use lc631_runtime::formal_boolean;
use lc631_substrate::measurement_numeric::RationalInterval;
use lc631_substrate::Digest32;

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn rat(n: i64, den: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, den).unwrap()
}
fn entry(n: u8, label: OracleLabel, novelty: Novelty) -> CorpusCase<'static> {
    CorpusCase {
        id: d(n),
        family: d(n),
        role: DatasetRole::Calibration,
        oracle: label,
        novelty,
        request: None,
        request_binding: calibration::corpus::fixture(corpus::ReferenceCase::Identity)
            .unwrap()
            .request(100_000)
            .unwrap()
            .binding,
    }
}
fn policy() -> Digest32 {
    d(90)
}

#[test]
fn tlcx39_negative_missing_unknown_and_zero_accepted_are_not_zero_risk() {
    let cases = [
        entry(1, OracleLabel::Valid, Novelty::NotNovel),
        entry(2, OracleLabel::Unlabelled, Novelty::Unassessed),
    ];
    let corpus = Corpus::checked(d(80), &cases).unwrap();
    let report = summarize(&corpus, DatasetRole::Calibration, policy(), &[]).unwrap();
    assert_eq!(report.counts().population, 2);
    assert_eq!(report.counts().missing, 2);
    assert_eq!(
        report.coverage(),
        &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
    );
    assert_eq!(
        report.false_hold_population(),
        &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
    );
    assert_eq!(
        report.novel_rejection_population(),
        &RationalInterval::from_ratio(0, 1, 1, 2).unwrap()
    );
    assert_eq!(report.selective_risk(), &Rate::UndefinedNoAccepted);
    assert_eq!(report.counts().oracle_unlabelled, 1);
}

#[test]
fn tlcx39_exact_reported_population_bounds_do_not_drop_unlabelled_cases() {
    let cases = [
        entry(1, OracleLabel::Invalid, Novelty::NotNovel),
        entry(2, OracleLabel::Unlabelled, Novelty::Unassessed),
        entry(3, OracleLabel::Valid, Novelty::Permitted),
        entry(4, OracleLabel::Valid, Novelty::NotNovel),
    ];
    let corpus = Corpus::checked(d(80), &cases).unwrap();
    let observations = [
        Observation::reported(&corpus, d(1), policy(), Action::Accepted).unwrap(),
        Observation::reported(&corpus, d(2), policy(), Action::Accepted).unwrap(),
        Observation::reported(&corpus, d(3), policy(), Action::Rejected).unwrap(),
        Observation::reported(&corpus, d(4), policy(), Action::Held(HoldReason::Reported)).unwrap(),
    ];
    let report = summarize(&corpus, DatasetRole::Calibration, policy(), &observations).unwrap();
    let counts = report.counts();
    assert_eq!(
        (
            counts.accepted,
            counts.rejected,
            counts.held,
            counts.missing
        ),
        (2, 1, 1, 0)
    );
    assert_eq!((counts.silent_errors, counts.accepted_unlabelled), (1, 1));
    assert_eq!((counts.false_holds, counts.valid_novel_rejections), (1, 1));
    assert_eq!(counts.caller_reported, 4);
    assert_eq!(counts.actual_finite, 0);
    assert_eq!(report.coverage(), &rat(1, 2));
    assert_eq!(
        report.selective_risk(),
        &Rate::Bounded(RationalInterval::from_ratio(1, 2, 1, 1).unwrap())
    );
    assert_eq!(report.false_hold_population(), &rat(1, 4));
    assert_eq!(report.novel_rejection_population(), &rat(1, 4));
}

#[test]
fn tlcx39_negative_duplicate_foreign_policy_and_split_leakage() {
    let a = entry(1, OracleLabel::Valid, Novelty::NotNovel);
    let duplicate = [a, a];
    assert!(matches!(
        Corpus::checked(d(80), &duplicate),
        Err(CalibrationError::DuplicateCase)
    ));
    let mut leaked = entry(2, OracleLabel::Invalid, Novelty::NotNovel);
    leaked.family = a.family;
    leaked.role = DatasetRole::Holdout;
    let cases = [a, leaked];
    assert!(matches!(
        Corpus::checked(d(80), &cases),
        Err(CalibrationError::FamilyAcrossRoles)
    ));
    let one = [a];
    let corpus = Corpus::checked(d(80), &one).unwrap();
    assert!(matches!(
        Observation::reported(&corpus, d(99), policy(), Action::Accepted),
        Err(CalibrationError::UnknownCase)
    ));
    let obs = [Observation::reported(&corpus, a.id, d(99), Action::Accepted).unwrap()];
    assert!(matches!(
        summarize(&corpus, a.role, policy(), &obs),
        Err(CalibrationError::PolicyMismatch)
    ));
    let same = Observation::reported(&corpus, a.id, policy(), Action::Accepted).unwrap();
    assert!(matches!(
        summarize(&corpus, a.role, policy(), &[same, same]),
        Err(CalibrationError::DuplicateObservation)
    ));
    assert!(matches!(
        summarize(&corpus, DatasetRole::Holdout, policy(), &[]),
        Err(CalibrationError::EmptyRole)
    ));
}

#[test]
fn tlcx39_positive_actual_finite_api_matches_manual_reference_labels() {
    for reference in corpus::REFERENCE_CASES {
        let fixture = corpus::fixture(reference).unwrap();
        let request = fixture.request(100_000).unwrap();
        let cases = [CorpusCase {
            id: d(1),
            family: d(2),
            role: DatasetRole::Calibration,
            oracle: reference.oracle(),
            novelty: reference.novelty(),
            request_binding: request.binding,
            request: Some(&request),
        }];
        let dataset = Corpus::checked(d(80), &cases).unwrap();
        let obs = observe_finite(&dataset, d(1), policy(), &request, 1_000_000).unwrap();
        // Expected labels are handwritten in the catalogue, never generated by evaluate().
        let expected = match reference.oracle() {
            OracleLabel::Valid => Action::Accepted,
            OracleLabel::Invalid => Action::Rejected,
            OracleLabel::Unlabelled => Action::Held(HoldReason::NoComparable),
        };
        assert_eq!(obs.action(), expected);
        assert_eq!(obs.basis(), DecisionBasis::ActualFiniteComputation);
        let report = summarize(&dataset, DatasetRole::Calibration, policy(), &[obs]).unwrap();
        assert_eq!(report.counts().actual_finite, 1);
        assert_eq!(report.counts().caller_reported, 0);
        assert_eq!(report.counts().silent_errors, 0);
        assert_eq!(
            report.source_interpretation(),
            formal_boolean::SourceInterpretation::Unverified
        );
        assert_eq!(
            report.checker_independence(),
            formal_boolean::CheckerIndependence::UnassessedSharedIr
        );
    }
}

#[test]
fn tlcx39_negative_budget_cut_is_actual_hold_and_changes_coverage() {
    let fixture = corpus::fixture(corpus::ReferenceCase::Identity).unwrap();
    let request = fixture.request(100_000).unwrap();
    let cases = [CorpusCase {
        id: d(1),
        family: d(2),
        role: DatasetRole::Calibration,
        oracle: OracleLabel::Valid,
        novelty: Novelty::NotNovel,
        request_binding: request.binding,
        request: Some(&request),
    }];
    let dataset = Corpus::checked(d(80), &cases).unwrap();
    let full = observe_finite(&dataset, d(1), policy(), &request, 1_000_000).unwrap();
    let cut = observe_finite(&dataset, d(1), policy(), &request, 1).unwrap();
    assert_eq!(full.action(), Action::Accepted);
    assert_eq!(cut.action(), Action::Held(HoldReason::Unknown));
    let accepted = summarize(&dataset, DatasetRole::Calibration, policy(), &[full]).unwrap();
    let held = summarize(&dataset, DatasetRole::Calibration, policy(), &[cut]).unwrap();
    assert_eq!(accepted.coverage(), &rat(1, 1));
    assert_eq!(held.coverage(), &rat(0, 1));
    assert_eq!(held.counts().false_holds, 1);
    assert_eq!(held.selective_risk(), &Rate::UndefinedNoAccepted);
    assert!(matches!(
        observe_finite(&dataset, d(1), policy(), &request, 0),
        Err(CalibrationError::Formal(
            formal_boolean::Failure::InvalidBudget
        ))
    ));
}

#[test]
fn tlcx39_negative_binding_or_foreign_corpus_cannot_supply_actual_observation() {
    let fixture = corpus::fixture(corpus::ReferenceCase::Identity).unwrap();
    let request = fixture.request(100_000).unwrap();
    let mut bad = entry(1, OracleLabel::Valid, Novelty::NotNovel);
    bad.request_binding.probe_set = d(33);
    let cases = [bad];
    let dataset = Corpus::checked(d(80), &cases).unwrap();
    assert!(matches!(
        observe_finite(&dataset, d(1), policy(), &request, 1_000_000),
        Err(CalibrationError::RequestBindingMismatch)
    ));
    let other = Corpus::checked(d(80), &cases).unwrap();
    let obs = [Observation::reported(&other, d(1), policy(), Action::Accepted).unwrap()];
    assert!(matches!(
        summarize(&dataset, DatasetRole::Calibration, policy(), &obs),
        Err(CalibrationError::ForeignCorpus)
    ));
}

#[test]
fn tlcx39_unknown_held_and_rejected_labels_enclose_population_events() {
    let cases = [
        entry(1, OracleLabel::Unlabelled, Novelty::Unassessed),
        entry(2, OracleLabel::Valid, Novelty::Unassessed),
        entry(3, OracleLabel::Invalid, Novelty::Permitted),
    ];
    let dataset = Corpus::checked(d(80), &cases).unwrap();
    let obs = [
        Observation::reported(&dataset, d(1), policy(), Action::Held(HoldReason::Reported))
            .unwrap(),
        Observation::reported(&dataset, d(2), policy(), Action::Rejected).unwrap(),
        Observation::reported(&dataset, d(3), policy(), Action::Rejected).unwrap(),
    ];
    let report = summarize(&dataset, DatasetRole::Calibration, policy(), &obs).unwrap();
    assert_eq!(
        report.false_hold_population(),
        &RationalInterval::from_ratio(0, 1, 1, 3).unwrap()
    );
    assert_eq!(
        report.novel_rejection_population(),
        &RationalInterval::from_ratio(0, 1, 1, 3).unwrap()
    );
    assert_eq!(report.counts().valid_novel_rejections, 0);
}

#[test]
fn tlcx39_negative_empty_and_oversized_corpus_rejected_before_iteration() {
    assert!(matches!(
        Corpus::checked(d(80), &[]),
        Err(CalibrationError::EmptyCorpus)
    ));
    let cases = vec![entry(1, OracleLabel::Valid, Novelty::NotNovel); MAX_CASES + 1];
    assert!(matches!(
        Corpus::checked(d(80), &cases),
        Err(CalibrationError::CaseLimit)
    ));
}
