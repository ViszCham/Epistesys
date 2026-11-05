//! Immutable captured roots and closed adapter configurations, before actual runs.
use super::*;
pub struct RootDefinition<'a, 'v> {
    pub(super) source: &'a SourceStore,
    pub(super) cases: &'a [LegacyCase<'a>],
    pub(super) rules: Vec<PropertyRule>,
    pub(super) order: Vec<usize>,
    pub(super) payloads: Vec<Vec<u8>>,
    pub(super) denominator: Rational,
    pub(super) baseline: AdapterKind,
    pub(super) budget: BreakCostBudget,
    pub(super) binding: BreakCostBinding,
    pub(super) digest: Digest32,
    retained: ResourceClaim<'v>,
}
impl RootDefinition<'_, '_> {
    pub fn historical_digest(&self) -> Digest32 {
        self.digest
    }
    pub fn binding(&self) -> BreakCostBinding {
        self.binding
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        resources::current(&self.retained, view)?;
        if view.source_revision() != self.binding.source.digest()
            || self.source.id().digest() != self.binding.source.digest()
        {
            return Err(Error::BindingMismatch);
        }
        resources::current(&self.retained, view)
    }
}
pub struct AdapterCandidate<'a, 'v> {
    pub(super) root: &'a RootDefinition<'a, 'v>,
    pub(super) binding: CandidateBinding,
    pub(super) kinds: Vec<AdapterKind>,
    retained: ResourceClaim<'v>,
}
impl AdapterCandidate<'_, '_> {
    pub fn binding(&self) -> CandidateBinding {
        self.binding
    }
    pub(super) fn check(
        &self,
        root: &RootDefinition<'_, '_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        self.root.check(view)?;
        resources::current(&self.retained, view)?;
        if !same(self.root, root) {
            return Err(Error::BindingMismatch);
        }
        resources::current(&self.retained, view)
    }
}
pub fn register<'a, 'v>(
    source: &'a SourceStore,
    binding: BreakCostBinding,
    cases: &'a [LegacyCase<'a>],
    rules: &[PropertyRule],
    baseline: AdapterKind,
    budget: BreakCostBudget,
    view: &'v ResourceReservationView<'_>,
) -> Result<RootDefinition<'a, 'v>, Error> {
    view.check().map_err(Error::Resource)?;
    admission::limits(budget)?;
    let retained = resources::claim(view, 0)?;
    if binding.source.digest() != source.id().digest()
        || binding.context.source() != binding.source
        || view.source_revision() != binding.source.digest()
    {
        return Err(Error::BindingMismatch);
    }
    let rules = admission::rules(rules, budget)?;
    let (payloads, order) = admission::cases(source, cases, budget)?;
    let denominator = admission::denominator(cases, &rules, budget)?;
    let digest = identity::root(binding, cases, &rules, &payloads, &order, baseline, budget);
    resources::current(&retained, view)?;
    Ok(RootDefinition {
        source,
        cases,
        rules,
        order,
        payloads,
        denominator,
        baseline,
        budget,
        binding,
        digest,
        retained,
    })
}
pub fn bind_candidate<'a, 'v>(
    root: &'a RootDefinition<'a, 'v>,
    binding: CandidateBinding,
    requests: &[AdapterRequest<'_>],
    view: &'v ResourceReservationView<'_>,
) -> Result<AdapterCandidate<'a, 'v>, Error> {
    root.check(view)?;
    if binding.context != root.binding.context {
        return Err(Error::BindingMismatch);
    }
    if requests.len() != root.cases.len() {
        return Err(Error::InvalidSuite);
    }
    let retained = resources::claim(view, 0)?;
    let mut kinds = resources::vector(root.cases.len())?;
    for case in root.cases {
        let mut matching = requests.iter().filter(|r| r.case == case.id);
        let request = matching.next().ok_or(Error::InvalidSuite)?;
        if matching.next().is_some() {
            return Err(Error::InvalidSuite);
        }
        if request.requested_schema != schema(request.kind) {
            return Err(Error::InvalidInput);
        }
        kinds.push(request.kind);
    }
    root.check(view)?;
    resources::current(&retained, view)?;
    Ok(AdapterCandidate {
        root,
        binding,
        kinds,
        retained,
    })
}
