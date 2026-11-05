//! Explicit bounded query feedback. Weight ties never rank opaque identities.
use super::*;
use lc631_substrate::measurement_numeric::checked_compare;
use std::cmp::Ordering;

enum Choice {
    None,
    Probe(usize, ContextSide),
    RootGoal,
}
impl CurrentConfiguration<'_> {
    pub fn execute_follow_up_current<R>(
        &mut self,
        max_work_per_query: u64,
        resources: &ResourceReservationView<'_>,
        consume: impl for<'fresh> FnOnce(FollowUpResult<'fresh>) -> R,
    ) -> Result<R, Error> {
        (self.check)(resources)?;
        let (predicate, side) = match choose(self.probes)? {
            Choice::None => {
                let result = consume(FollowUpResult::NoQuery {
                    query_count: self.work.queries,
                });
                (self.check)(resources)?;
                return Ok(result);
            }
            Choice::Probe(i, side) => (self.probes[i].fixed.predicate, side),
            Choice::RootGoal => (self.goal, ContextSide::Candidate),
        };
        let fresh_claim = util::claim(resources, 64 * 1024, 8192)?;
        // Charge the SAME mutable root meter. Checked subtraction leaves it
        // unchanged on a rejected allowance; no fresh callback or f call runs.
        self.work.pair(max_work_per_query)?;
        let before = self.work.queries;
        let index = if side == ContextSide::Baseline { 0 } else { 1 };
        let spec = [support::PairSpec {
            query: self.queries[index],
            predicate,
            context: self.contexts[index],
            allowance: max_work_per_query,
            kind: support::PairKind::Probe(0, side),
        }];
        let check = self.check;
        let fresh_check = |view: &ResourceReservationView<'_>| {
            check(view)?;
            fresh_claim.check().map_err(Error::Resource)?;
            if !fresh_claim.same_reservation(view) {
                return Err(Error::ForeignPlan);
            }
            Ok(())
        };
        support::with_batch(
            &spec,
            self.map,
            self.probe_set,
            self.work,
            |actual, work| {
                fresh_check(resources)?;
                let pair = actual.first().ok_or(Error::InvalidPlan)?;
                let value = FreshSupport {
                    outcome: pair.outcome,
                    evaluations: pair.evaluations,
                    before,
                    after: work.queries,
                    check: &fresh_check,
                };
                let result = consume(FollowUpResult::Executed(value));
                fresh_check(resources)?;
                Ok(result)
            },
        )
    }
}
fn choose(rows: &[ProbeOutcome<'_>]) -> Result<Choice, Error> {
    let mut selected: Option<usize> = None;
    let mut tied = false;
    let mut changed = false;
    for (i, row) in rows.iter().enumerate() {
        let unknown = row
            .outcomes
            .iter()
            .any(|v| matches!(v, CanonicalOutcome::Unknown(_)));
        changed |= !unknown && row.outcomes[0] != row.outcomes[1];
        if !unknown {
            continue;
        }
        if let Some(previous) = selected {
            match checked_compare(row.fixed.weight, rows[previous].fixed.weight)
                .map_err(|e| Error::Arithmetic(e.into()))?
            {
                Ordering::Greater => {
                    selected = Some(i);
                    tied = false;
                }
                Ordering::Equal => tied = true,
                Ordering::Less => {}
            }
        } else {
            selected = Some(i);
        }
    }
    if tied {
        return Err(Error::AmbiguousFollowUp);
    }
    if let Some(index) = selected {
        let side = if matches!(rows[index].outcomes[0], CanonicalOutcome::Unknown(_)) {
            ContextSide::Baseline
        } else {
            ContextSide::Candidate
        };
        return Ok(Choice::Probe(index, side));
    }
    Ok(if changed {
        Choice::RootGoal
    } else {
        Choice::None
    })
}
