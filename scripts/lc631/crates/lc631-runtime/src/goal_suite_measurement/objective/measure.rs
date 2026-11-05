//! Actual native values against one immutable hard root, with distinct absolute/delta views.
use super::*;
use lc631_analysis::creativity_measurement::ComponentMetric;

pub struct OwnedObjectivePair<'r, 'o, 'q, 'v> {
    root: &'r ObjectiveRoot<'q, 'v>,
    pair: NativePair<'o, 'q, 'v>,
    facts: [current::Fact; 2],
    baseline_goal: RationalInterval,
    candidate_goal: RationalInterval,
    solve: ComponentReport,
    dependency: Digest32,
    retained: ResourceClaim<'v>,
}
pub struct ObjectiveView<'current> {
    baseline_goal: &'current RationalInterval,
    candidate_goal: &'current RationalInterval,
    solve: &'current ComponentReport,
    dependency: Digest32,
}
pub fn measure_native_pair<'r, 'o, 'q, 'v>(
    root: &'r ObjectiveRoot<'q, 'v>,
    pair: NativePair<'o, 'q, 'v>,
    view: &'v ResourceReservationView<'_>,
) -> Result<OwnedObjectivePair<'r, 'o, 'q, 'v>, Error> {
    root.check(view)?;
    root.budget.pair_funding()?;
    let retained = claim(view, PAIR_RAM)?;
    let scratch = claim(view, VIEW_RAM)?;
    root.budget.pair()?;
    root.budget.read()?;
    let facts = current::check(root, pair, view)?;
    let dependency = hash(&[
        b"same-native-goal-observations.v1",
        root.identity.as_bytes(),
        facts[0].binding.as_bytes(),
        facts[1].binding.as_bytes(),
    ]);
    let baseline_goal = RationalInterval::exact_ratio(facts[0].value, 3)?;
    let candidate_goal = RationalInterval::exact_ratio(facts[1].value, 3)?;
    let solve = numeric::objective(
        numeric::ReportInput {
            metric: ComponentMetric::Solve,
            scope: root.scope,
            context: root.context,
            source: view.source_revision(),
            baseline: facts[0].target,
            target: facts[1].target,
            candidate: facts[1].binding,
            baseline_observation: facts[0].binding,
            candidate_observation: facts[1].binding,
            dependency,
            oracle: hash(&[b"Given-finite-root-constraints-with-actual-CheckedModel.v1"]),
        },
        facts[0].value,
        facts[1].value,
    )?;
    let after = current::check(root, pair, view)?;
    if facts
        .iter()
        .zip(after)
        .any(|(a, b)| a.binding != b.binding || a.target != b.target || a.value != b.value)
    {
        return Err(Error::BindingMismatch);
    }
    scratch.check()?;
    check_claim(&retained, view)?;
    Ok(OwnedObjectivePair {
        root,
        pair,
        facts,
        baseline_goal,
        candidate_goal,
        solve,
        dependency,
        retained,
    })
}
impl<'r, 'o, 'q, 'v> OwnedObjectivePair<'r, 'o, 'q, 'v> {
    /// Only cross-row identity accounting; it grants no current model/binary or
    /// root eligibility. Adoption additionally uses with_current on every row.
    pub fn require_same_baseline(
        &self,
        other: &OwnedObjectivePair<'_, '_, '_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.root.check(view)?;
        other.root.check(view)?;
        check_claim(&self.retained, view)?;
        check_claim(&other.retained, view)?;
        if !same_owner(self.root, other.root)
            || !same_owner(self.pair.baseline, other.pair.baseline)
            || self.facts[0].binding != other.facts[0].binding
            || self.facts[0].target != other.facts[0].target
        {
            return Err(Error::BaselineMismatch);
        }
        Ok(())
    }
    pub fn with_current<R>(
        &self,
        root: &ObjectiveRoot<'q, 'v>,
        pair: NativePair<'o, 'q, 'v>,
        view: &ResourceReservationView<'_>,
        consume: impl for<'current> FnOnce(ObjectiveView<'current>) -> R,
    ) -> Result<R, Error> {
        root.check(view)?;
        if !std::ptr::eq(root, self.root) {
            return Err(Error::BindingMismatch);
        }
        if !std::ptr::eq(pair.baseline, self.pair.baseline)
            || !std::ptr::eq(pair.candidate, self.pair.candidate)
        {
            return Err(Error::RoleOrOriginMismatch);
        }
        check_claim(&self.retained, view)?;
        let scratch = claim(view, VIEW_RAM)?;
        root.budget.read()?;
        let before = current::check(root, pair, view)?;
        self.same_facts(before)?;
        let result = consume(ObjectiveView {
            baseline_goal: &self.baseline_goal,
            candidate_goal: &self.candidate_goal,
            solve: &self.solve,
            dependency: self.dependency,
        });
        self.same_facts(current::check(root, pair, view)?)?;
        scratch.check()?;
        check_claim(&self.retained, view)?;
        Ok(result)
    }
    fn same_facts(&self, actual: [current::Fact; 2]) -> Result<(), Error> {
        if self
            .facts
            .iter()
            .zip(actual)
            .any(|(a, b)| a.binding != b.binding || a.target != b.target || a.value != b.value)
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}
impl<'current> ObjectiveView<'current> {
    pub fn baseline_goal(&self) -> &'current RationalInterval {
        self.baseline_goal
    }
    pub fn candidate_goal(&self) -> &'current RationalInterval {
        self.candidate_goal
    }
    pub fn solve(&self) -> &'current ComponentReport {
        self.solve
    }
    pub fn goal_dependency(&self) -> Digest32 {
        self.dependency
    }
    pub fn optimality(&self) -> GoalOptimality {
        GoalOptimality::BestAmongEvaluated
    }
    pub fn require_overlap(&self, declared: DeclaredOverlap) -> Result<(), Error> {
        overlap(declared)
    }
}
