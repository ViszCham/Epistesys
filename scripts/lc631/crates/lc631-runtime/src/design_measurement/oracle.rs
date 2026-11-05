//! 日本語: event-jump producerと独立tick replay。物理workerの実行ではない。
//! English: Event-jump producer and independent tick replay, never physical workers.
use super::*;
fn output(
    w: &OwnedWorkload,
    records: &[Option<JobTiming>; MAX_JOBS],
    makespan: u16,
) -> Result<ReportedSchedule, Error> {
    let mut jobs = Vec::new();
    jobs.try_reserve_exact(w.jobs.len())
        .map_err(|_| Error::AllocationFailed)?;
    for r in &records[..w.jobs.len()] {
        jobs.push(r.ok_or(Error::ReplayMismatch)?);
    }
    Ok(ReportedSchedule {
        jobs,
        makespan: ModelTicks(makespan),
    })
}
pub(super) fn event_run(
    w: &OwnedWorkload,
    priority: &[usize],
    limits: DesignLimits,
) -> Result<Option<ReportedSchedule>, Error> {
    let mut active = [None; MAX_WORKERS];
    let mut end = [0u16; MAX_WORKERS];
    let mut dispatched = 0u8;
    let mut done = 0u8;
    let mut now = 0u16;
    let mut records = [None; MAX_JOBS];
    for _ in 0..limits.max_events_per_run {
        // Complete ALL simultaneous jobs before ANY dispatch.
        for k in 0..usize::from(w.workers) {
            if active[k].is_some() && end[k] == now {
                done |= 1_u8 << active[k].take().ok_or(Error::ReplayMismatch)?;
            }
        }
        if done.count_ones() as usize == w.jobs.len() {
            return Ok(Some(output(w, &records, now)?));
        }
        for k in 0..usize::from(w.workers) {
            if active[k].is_some() {
                continue;
            }
            if let Some(&j) = priority.iter().find(|&&j| {
                dispatched & (1 << j) == 0
                    && w.jobs[j].dependencies & done == w.jobs[j].dependencies
            }) {
                let finish = now
                    .checked_add(w.jobs[j].duration)
                    .ok_or(Error::ReplayMismatch)?;
                if finish > MAX_TICKS {
                    return Err(Error::ReplayMismatch);
                }
                active[k] = Some(j);
                end[k] = finish;
                dispatched |= 1 << j;
                records[j] = Some(JobTiming {
                    job: w.jobs[j].id,
                    worker: k as u8,
                    start: ModelTicks(now),
                    end: ModelTicks(finish),
                });
            }
        }
        now = (0..usize::from(w.workers))
            .filter_map(|k| active[k].map(|_| end[k]))
            .min()
            .ok_or(Error::ReplayMismatch)?;
    }
    Ok(None)
}
/// Separate algorithm: advance exactly one integer model tick, update remaining
/// service counters, release ALL completed jobs, then independently scan the
/// WHOLE priority permutation for each free worker. No producer-ready helper,
/// event queue, min-completion jump or reported status is reused.
pub(super) fn tick_replay(
    w: &OwnedWorkload,
    priority: &[usize],
    reported: &ReportedSchedule,
    limits: DesignLimits,
) -> Result<bool, Error> {
    if reported.jobs.len() != w.jobs.len() || reported.makespan.0 > MAX_TICKS {
        return Err(Error::ReplayMismatch);
    }
    let mut running = [None; MAX_WORKERS];
    let mut remaining = [0u16; MAX_WORKERS];
    let mut started = [false; MAX_JOBS];
    let mut completed = [false; MAX_JOBS];
    let mut expected = [None; MAX_JOBS];
    for tick in 0..=limits.max_replay_ticks_per_run {
        for k in 0..usize::from(w.workers) {
            if remaining[k] == 0 {
                if let Some(j) = running[k].take() {
                    completed[j] = true;
                }
            }
        }
        if completed[..w.jobs.len()].iter().all(|b| *b) {
            let expected = output(w, &expected, tick)?;
            if &expected != reported {
                return Err(Error::ReplayMismatch);
            }
            return Ok(true);
        }
        if tick == limits.max_replay_ticks_per_run {
            return Ok(false);
        }
        for worker in 0..usize::from(w.workers) {
            if running[worker].is_some() {
                continue;
            }
            let mut chosen = None;
            for &j in priority {
                if started[j] {
                    continue;
                }
                let mut ready = true;
                for (before, done) in completed.iter().enumerate().take(w.jobs.len()) {
                    if w.jobs[j].dependencies & (1 << before) != 0 && !done {
                        ready = false;
                    }
                }
                if ready {
                    chosen = Some(j);
                    break;
                }
            }
            if let Some(j) = chosen {
                running[worker] = Some(j);
                remaining[worker] = w.jobs[j].duration;
                started[j] = true;
                expected[j] = Some(JobTiming {
                    job: w.jobs[j].id,
                    worker: worker as u8,
                    start: ModelTicks(tick),
                    end: ModelTicks(
                        tick.checked_add(w.jobs[j].duration)
                            .ok_or(Error::ReplayMismatch)?,
                    ),
                });
            }
        }
        for k in 0..usize::from(w.workers) {
            if running[k].is_some() {
                remaining[k] = remaining[k].checked_sub(1).ok_or(Error::ReplayMismatch)?;
            }
        }
    }
    Ok(false)
}
