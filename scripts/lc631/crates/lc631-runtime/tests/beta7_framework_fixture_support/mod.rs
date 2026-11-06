//! Capture the same Given root as the P1 fixture, exposing its typed admission result and limits.
use super::*;

pub(super) fn capture_native<'a, 'v>(
    h: &Inputs<'a>,
    limits: n::Limits,
    view: &'v ResourceReservationView<'_>,
) -> Result<n::RootPlan<'a, 'v>, n::Error> {
    n::RootPlan::capture_current(
        n::RootInput {
            source: h.source,
            original: original(h.source),
            source_query: h.theory,
            premises: h.premises,
            target: d(5),
            goal: GoalPolicy {
                revision: d(2),
                query: h.goal,
            },
            bridge_limits: bridge_limits(),
        },
        n::Declaration {
            atoms: h.atoms,
            facts: h.facts,
            constructions: h.constructions,
            archive: n::ArchiveInput::Unread,
            corpus_revision: d(70),
            oracle_revision: d(71),
            policy_revision: d(72),
            limits,
        },
        view,
    )
}
