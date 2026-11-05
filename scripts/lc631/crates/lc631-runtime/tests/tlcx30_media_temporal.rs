//! Real process owners are required; synthetic output is not model validation.
mod tlcx30_media_support;
use tlcx30_media_support::*;
use lc631_runtime::{observation_bridge::media::*, resource::Cancellation};
use lc631_receipt_kernel::ReplayGuard;
use lc631_analysis::creativity_measurement::RationalInterval;

fn probes(missing: bool) -> Vec<MediaTemporalProbe> {
    vec![MediaTemporalProbe { id: d(40), baseline: ["first".into(), "second".into()],
        candidate: ["first".into(), if missing { "absent" } else { "second" }.into()],
        relation: TemporalRelation::Meets, weight: 1 }]
}
fn all_relations() -> Vec<MediaTemporalProbe> {
    [TemporalRelation::Before, TemporalRelation::Meets, TemporalRelation::EqualDuration,
        TemporalRelation::MappedAbsoluteStart].into_iter().enumerate().map(|(i, relation)|
        MediaTemporalProbe { id: d(40 + i as u8), baseline: ["first".into(), "second".into()],
            candidate: ["first".into(), "second".into()], relation, weight: i as u32 + 1 }).collect()
}

#[test]
fn tlcx30_actual_media_temporal_measurement_changes_hypothesis_and_cut_is_unresolved() {
    let baseline = Fixture::new(1000, false);
    let candidate = Fixture::new(1001, false);
    let mut contexts = consumer();
    baseline.funded(Cancellation::default(), |lv| candidate.funded(Cancellation::default(), |rv| {
    let left = baseline.execute(lv, &mut contexts).unwrap();
    let right = candidate.execute(rv, &mut contexts).unwrap();
    for case in 0..3 {
        let mapping = if case == 2 { None } else { Some(DeclaredMediaClockMap {
            baseline_clock: d(60), candidate_clock: d(60), offset_milliseconds: 0 }) };
        let plan = MediaTemporalProbePlan::declared(contexts.original().state().scope(), &left, &right,
            mapping, if case == 0 { all_relations() } else { probes(case == 1) }).unwrap();
        let result = compare_media_temporal(&plan,
                CurrentMedia { owner: &left, authority: &baseline.authority, verifier: baseline.context.verifier(), resources: lv },
                CurrentMedia { owner: &right, authority: &candidate.authority, verifier: candidate.context.verifier(), resources: rv },
                &mut ReplayGuard::default(), epoch()).unwrap();
        assert_eq!(result.denominator(), if case == 0 { 10 } else { 1 });
        match case {
            0 => { assert_eq!(result.loss(), Some(&RationalInterval::exact_ratio(1, 1).unwrap()));
                assert_eq!(result.hypothesis(), TemporalHypothesis::RelationChanged); },
            1 => { assert!(result.loss().is_none()); assert_eq!(result.hypothesis(), TemporalHypothesis::Unresolved); },
            _ => { assert!(result.loss().is_none()); assert_eq!(result.hypothesis(), TemporalHypothesis::Unresolved); },
        }
    }
    }));
}

#[test]
fn tlcx30_temporal_restoration_and_current_source_invalidation() {
    let baseline = Fixture::new(1000, false);
    let candidate = Fixture::new(1000, false);
    let mut contexts = consumer();
    baseline.funded(Cancellation::default(), |lv| candidate.funded(Cancellation::default(), |rv| {
    let left = baseline.execute(lv, &mut contexts).unwrap();
    let right = candidate.execute(rv, &mut contexts).unwrap();
    let plan = MediaTemporalProbePlan::declared(contexts.original().state().scope(), &left, &right,
        Some(DeclaredMediaClockMap { baseline_clock: d(60), candidate_clock: d(60), offset_milliseconds: 0 }), probes(false)).unwrap();
    for changed in [false, true] {
        if changed { std::fs::write(&candidate.source, b"change").unwrap(); }
        let result = compare_media_temporal(&plan,
                CurrentMedia { owner: &left, authority: &baseline.authority, verifier: baseline.context.verifier(), resources: lv },
                CurrentMedia { owner: &right, authority: &candidate.authority, verifier: candidate.context.verifier(), resources: rv },
                &mut ReplayGuard::default(), epoch());
        if changed { assert!(result.is_err()); } else {
            let report = result.unwrap();
            assert_eq!(report.loss(), Some(&RationalInterval::exact_ratio(0, 1).unwrap()));
            assert_eq!(report.hypothesis(), TemporalHypothesis::RelationsPreserved);
        }
    }
    }));
}

#[test]
fn tlcx30_same_receipt_does_not_skip_foreign_authority_and_cancelled_owner() {
    let baseline = Fixture::new(1000, false);
    let foreign = Fixture::new(1000, false);
    let mut contexts = consumer();
    let cancel = Cancellation::default();
    baseline.funded(cancel.clone(), |view| {
        let owner = baseline.execute(view, &mut contexts).unwrap();
        let plan = MediaTemporalProbePlan::declared(contexts.original().state().scope(), &owner, &owner,
            Some(DeclaredMediaClockMap { baseline_clock: d(60), candidate_clock: d(60), offset_milliseconds: 0 }), probes(false)).unwrap();
        assert!(compare_media_temporal(&plan,
            CurrentMedia { owner: &owner, authority: &baseline.authority, verifier: baseline.context.verifier(), resources: view },
            CurrentMedia { owner: &owner, authority: &foreign.authority, verifier: foreign.context.verifier(), resources: view },
            &mut ReplayGuard::default(), epoch()).is_err());
        cancel.cancel();
        assert!(owner.check_live().is_err());
        assert!(compare_media_temporal(&plan,
            CurrentMedia { owner: &owner, authority: &baseline.authority, verifier: baseline.context.verifier(), resources: view },
            CurrentMedia { owner: &owner, authority: &baseline.authority, verifier: baseline.context.verifier(), resources: view },
            &mut ReplayGuard::default(), epoch()).is_err());
    });
}
