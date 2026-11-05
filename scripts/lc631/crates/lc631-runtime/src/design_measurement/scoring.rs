//! 日本語: AcceptedInputRangeを先に検査し、登録Minimize mapだけ飽和する。
//! English: Check the accepted raw domain FIRST; saturate only the registered map.
use super::*;
use std::cmp::Ordering;
pub(super) fn normalize(w: &OwnedWorkload, ticks: ModelTicks) -> Result<ScoreObservation, Error> {
    let raw = Rational::from_integer(ticks.0.into());
    if numeric::checked_compare(&raw, &w.accepted_low).map_err(Error::Numeric)? == Ordering::Less
        || numeric::checked_compare(&raw, &w.accepted_high).map_err(Error::Numeric)?
            == Ordering::Greater
    {
        return Err(Error::OutsideAcceptedRange);
    }
    let one = Rational::from_integer(1.into());
    let zero = Rational::from_integer(0.into());
    let (score, saturation) = if numeric::checked_compare(&raw, &w.m).map_err(Error::Numeric)?
        == Ordering::Less
    {
        (one, Saturation::SaturatedLow)
    } else if numeric::checked_compare(&raw, &w.upper).map_err(Error::Numeric)? == Ordering::Greater
    {
        (zero, Saturation::SaturatedHigh)
    } else {
        let numerator = numeric::checked_subtract(&raw, &w.m).map_err(Error::Numeric)?;
        let width = numeric::checked_subtract(&w.upper, &w.m).map_err(Error::Numeric)?;
        let ratio = numeric::checked_divide(&numerator, &width).map_err(Error::Numeric)?;
        (
            numeric::checked_subtract(&one, &ratio).map_err(Error::Numeric)?,
            Saturation::None,
        )
    };
    Ok(ScoreObservation {
        raw: ActualMakespan {
            ticks,
            unit: w.unit,
        },
        score: RationalInterval::new(score.clone(), score).map_err(Error::Numeric)?,
        saturation,
    })
}
pub(super) fn hash_workload(h: &mut Sha256, w: &OwnedWorkload) {
    field(h, &w.key);
    h.update([w.workers]);
    h.update(w.unit.0.as_bytes());
    h.update((w.jobs.len() as u16).to_le_bytes());
    for j in &w.jobs {
        field(h, &j.payload);
        h.update(j.duration.to_le_bytes());
        h.update([j.dependencies]);
    }
    for &j in &w.baseline {
        h.update((j as u16).to_le_bytes());
    }
    for r in [&w.accepted_low, &w.accepted_high, &w.m, &w.upper, &w.weight] {
        hash_rational(h, r);
    }
    h.update(w.map.as_bytes());
    h.update(b"Minimize/AcceptedRawRangeFIRST/StrictOutsideNormalizationFlags.v1");
}
pub(super) enum ScorePhase<'a> {
    Baseline(Digest32),
    Candidate(Digest32, &'a CandidateBinding),
}
/// Acyclic input-only lineage: no execution result, baseline node or final Suite
/// digest feeds this profile. All fixed owned programs/maps/workloads are bound.
pub(super) fn input_profile(
    b: DesignBinding,
    limits: DesignLimits,
    workloads: &[OwnedWorkload],
) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"CR-DESIGN.ValueNode.scoped-input-profile.v1");
    h.update(PRODUCER);
    h.update((ComponentMetric::Design as u16).to_le_bytes());
    for d in [
        b.selection.source.digest(),
        b.context.interpretation_digest(),
        b.selection.scope.comparison(),
        b.selection.scope.goal(),
        b.selection.scope.probes(),
        b.selection.scope.semantics(),
        b.selection.baseline,
        b.selection.hard_invariants,
        b.selection.policy,
        b.selection.evidence_revision,
        b.suite,
        b.oracle,
        b.definition,
        b.probe_set,
        b.dependency_group,
    ] {
        h.update(d.as_bytes());
    }
    match b.selection.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    };
    h.update(b"DesignValidation/Minimize/NonPreemptiveReadyListV1");
    h.update(limits.max_events_per_run.to_le_bytes());
    h.update(limits.max_replay_ticks_per_run.to_le_bytes());
    for n in [
        MAX_WORKLOADS as u64,
        MAX_JOBS as u64,
        MAX_WORKERS as u64,
        MAX_TICKS as u64,
        numeric::MAX_RATIONAL_BITS,
        SUITE_RAM,
        CANDIDATE_RAM,
        RESULT_RAM,
        SCRATCH,
    ] {
        h.update(n.to_le_bytes());
    }
    for w in workloads {
        hash_workload(&mut h, w);
    }
    Digest32::from_bytes(h.finalize().into())
}
pub(super) fn node(
    w: &OwnedWorkload,
    priority: &[usize],
    schedule: &ReportedSchedule,
    o: &ScoreObservation,
    phase: ScorePhase<'_>,
) -> Digest32 {
    let mut h = Sha256::new();
    h.update(PRODUCER);
    h.update((ComponentMetric::Design as u16).to_le_bytes());
    match phase {
        ScorePhase::Baseline(input) => {
            h.update([0]);
            h.update(input.as_bytes());
        }
        ScorePhase::Candidate(input, b) => {
            h.update([1]);
            h.update(input.as_bytes());
            for d in [
                b.candidate_digest,
                b.context.interpretation_digest(),
                b.target,
                b.transform,
            ] {
                h.update(d.as_bytes());
            }
        }
    }
    hash_workload(&mut h, w);
    for &j in priority {
        h.update((j as u16).to_le_bytes());
    }
    h.update(schedule.makespan.0.to_le_bytes());
    // Oracle output is canonical-job ordered; original JobIds are metadata only.
    for timing in &schedule.jobs {
        h.update([timing.worker]);
        h.update(timing.start.0.to_le_bytes());
        h.update(timing.end.0.to_le_bytes());
    }
    h.update(o.raw.ticks.0.to_le_bytes());
    h.update(o.raw.unit.0.as_bytes());
    hash_rational(&mut h, o.score.lower());
    hash_rational(&mut h, o.score.upper());
    h.update([match o.saturation {
        Saturation::None => 0,
        Saturation::SaturatedLow => 1,
        Saturation::SaturatedHigh => 2,
    }]);
    Digest32::from_bytes(h.finalize().into())
}
