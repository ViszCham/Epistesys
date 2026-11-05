//! 日本語: 同じ固定workloadのbounded admissionとPROGRAM構造検査。
//! English: Bounded admission of the fixed workload and complete priority programs.
use super::*;
use std::cmp::Ordering;
pub(super) fn preflight(inputs: &[Workload<'_>]) -> Result<(), Error> {
    // Length-only admission BEFORE equality, lookup, sorting or copies.
    let mut bytes = 0usize;
    for w in inputs {
        if w.case.is_empty()
            || w.case.len() > 4096
            || w.jobs.is_empty()
            || w.jobs.len() > MAX_JOBS
            || w.dependencies.len() > 56
            || w.baseline_priority.len() != w.jobs.len()
            || w.workers == 0
            || usize::from(w.workers) > MAX_WORKERS
        {
            return Err(Error::InvalidWorkload);
        }
        bytes = bytes
            .checked_add(w.case.len())
            .ok_or(Error::InvalidWorkload)?;
        for j in w.jobs {
            bytes = bytes
                .checked_add(j.canonical_payload.len())
                .ok_or(Error::InvalidWorkload)?;
            if j.canonical_payload.is_empty() || bytes > 65536 {
                return Err(Error::InvalidWorkload);
            }
        }
        for r in [
            w.weight,
            w.accepted.lower,
            w.accepted.upper,
            w.normalization.lower,
            w.normalization.upper,
        ] {
            numeric::check(r).map_err(Error::Numeric)?;
        }
    }
    let zero = Rational::from_integer(0.into());
    for (i, w) in inputs.iter().enumerate() {
        if inputs[..i].iter().any(|p| p.case == w.case) {
            return Err(Error::DuplicateCase);
        }
        if w.unit != w.accepted.unit || w.unit != w.normalization.unit {
            return Err(Error::UnitMismatch);
        }
        if w.normalization.direction != Direction::Minimize {
            return Err(Error::UnsupportedDirection);
        }
        if numeric::checked_compare(w.weight, &zero).map_err(Error::Numeric)? != Ordering::Greater {
            return Err(Error::InvalidWeight);
        }
        if numeric::checked_compare(w.accepted.lower, w.accepted.upper).map_err(Error::Numeric)?
            == Ordering::Greater
            || numeric::checked_compare(w.normalization.lower, w.normalization.upper)
                .map_err(Error::Numeric)?
                != Ordering::Less
        {
            return Err(Error::InvalidMap);
        }
        let mut total = 0u16;
        for (n, j) in w.jobs.iter().enumerate() {
            if j.duration_ticks == 0 || j.duration_ticks > 32 {
                return Err(Error::InvalidWorkload);
            }
            total = total
                .checked_add(j.duration_ticks)
                .ok_or(Error::InvalidWorkload)?;
            if w.jobs[..n]
                .iter()
                .any(|p| j.id == p.id || j.canonical_payload == p.canonical_payload)
            {
                return Err(Error::DuplicateJob);
            }
        }
        if total > MAX_TICKS {
            return Err(Error::InvalidWorkload);
        }
        for (n, (from, to)) in w.dependencies.iter().enumerate() {
            if from == to
                || !w.jobs.iter().any(|j| j.id == *from)
                || !w.jobs.iter().any(|j| j.id == *to)
                || w.dependencies[..n].contains(&(*from, *to))
            {
                return Err(Error::InvalidDependency);
            }
        }
        let mut seen = 0u8;
        for id in w.baseline_priority {
            let at = w
                .jobs
                .iter()
                .position(|j| j.id == *id)
                .ok_or(Error::InvalidProgram)?;
            if seen & (1 << at) != 0 {
                return Err(Error::InvalidProgram);
            }
            seen |= 1 << at;
        }
        // Independent of the priority: prove the entire mandatory graph is a DAG.
        let mut done = 0u8;
        for _ in 0..w.jobs.len() {
            let mut progress = false;
            for (n, j) in w.jobs.iter().enumerate() {
                if done & (1 << n) == 0
                    && w.dependencies
                        .iter()
                        .filter(|(_, to)| *to == j.id)
                        .all(|(from, _)| {
                            w.jobs
                                .iter()
                                .position(|p| p.id == *from)
                                .is_some_and(|k| done & (1 << k) != 0)
                        })
                {
                    done |= 1 << n;
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        if done.count_ones() as usize != w.jobs.len() {
            return Err(Error::Cycle);
        }
    }
    Ok(())
}
fn copy(b: &[u8]) -> Result<Vec<u8>, Error> {
    let mut v = Vec::new();
    v.try_reserve_exact(b.len())
        .map_err(|_| Error::AllocationFailed)?;
    v.extend_from_slice(b);
    Ok(v)
}
pub(super) fn own(w: &Workload<'_>) -> Result<OwnedWorkload, Error> {
    let mut jobs = Vec::new();
    jobs.try_reserve_exact(w.jobs.len())
        .map_err(|_| Error::AllocationFailed)?;
    for j in w.jobs {
        jobs.push(OwnedJob {
            id: j.id,
            payload: copy(j.canonical_payload)?,
            duration: j.duration_ticks,
            dependencies: 0,
        });
    }
    jobs.sort_by(|a, b| a.payload.cmp(&b.payload));
    for (from, to) in w.dependencies {
        let a = jobs
            .iter()
            .position(|j| j.id == *from)
            .ok_or(Error::InvalidDependency)?;
        let b = jobs
            .iter()
            .position(|j| j.id == *to)
            .ok_or(Error::InvalidDependency)?;
        jobs[b].dependencies |= 1 << a;
    }
    let mut out = OwnedWorkload {
        key: copy(w.case)?,
        jobs,
        workers: w.workers,
        unit: w.unit,
        baseline: vec![],
        accepted_low: w.accepted.lower.clone(),
        accepted_high: w.accepted.upper.clone(),
        m: w.normalization.lower.clone(),
        upper: w.normalization.upper.clone(),
        map: w.normalization.revision,
        weight: w.weight.clone(),
    };
    out.baseline = program(&out, w.baseline_priority)?;
    Ok(out)
}
pub(super) fn program(w: &OwnedWorkload, order: &[JobId]) -> Result<Vec<usize>, Error> {
    if order.len() != w.jobs.len() {
        return Err(Error::InvalidProgram);
    }
    let mut seen = 0u8;
    let mut out = Vec::new();
    out.try_reserve_exact(order.len())
        .map_err(|_| Error::AllocationFailed)?;
    for id in order {
        let i = w
            .jobs
            .iter()
            .position(|j| j.id == *id)
            .ok_or(Error::InvalidProgram)?;
        if seen & (1 << i) != 0 {
            return Err(Error::InvalidProgram);
        }
        seen |= 1 << i;
        out.push(i);
    }
    Ok(out)
}
