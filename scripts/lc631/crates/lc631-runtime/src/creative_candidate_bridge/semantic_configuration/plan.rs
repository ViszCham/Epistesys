//! Structural root capture and hard predicate registry, never Γ truth admission.
use super::*;
use lc631_substrate::measurement_numeric::{check, BigInt};
use sha2::{Digest, Sha256};

impl<'input, 'view> ConfigurationPlan<'input, 'view> {
    pub fn capture_current(
        bridge: &CreativeCandidateBridge<'input, '_>,
        source: &'input SmtStatusQuery,
        probes: &'input [ConfigurationProbe<'input>],
        declaration: ConfigurationDeclaration,
        budget: WorkBudget,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let mut work = util::Work::new(budget)?;
        bridge.check_current(resources).map_err(Error::Creative)?;
        let retained = util::claim(resources, 1024 * 1024, 0)?; // Plan RAM only; no Output.
        check_weights(probes)?; // Before any rational comparison, addition or hash.
        check_source(bridge, source)?;
        if declaration.hard_query_work == 0 || declaration.hard_query_work > 1_000_000 {
            return Err(Error::WorkBudget);
        }
        let context = bridge.contexts().original().state().revision();
        let scope = bridge.contexts().original().state().scope();
        let mut map = util::vector(source.boolean_symbols().len())?;
        map.extend(source.boolean_symbols().iter().map(|id| (*id, *id)));
        let hard = required_registry(bridge, &map, declaration.hard_query_work, &mut work)?;
        check_optional(
            probes,
            &hard,
            source,
            context,
            &map,
            declaration.hard_query_work,
            &mut work,
        )?;
        let identities = identities(bridge, source, probes, &hard, declaration, budget)?;
        work.charge(identities.nodes)?;
        let bindings = ConfigurationBindings {
            source: context.source(),
            context,
            scope,
            baseline: identities.baseline,
            hard_registry: identities.hard,
            probes: identities.probes,
            oracle: declaration.oracle,
            domain: declaration.domain,
            definition: util::digest(
                b"epistesys.dsem.definition.v1\0",
                &[b"GivenSameSignatureFiniteBooleanSupport-v1;K1;not-TL-SEM"],
            ),
        };
        let digest = util::digest(
            b"epistesys.dsem.plan.v1\0",
            &[
                identities.baseline.as_bytes(),
                identities.hard.as_bytes(),
                identities.probes.as_bytes(),
                declaration.oracle.as_bytes(),
                declaration.domain.as_bytes(),
                &declaration.hard_query_work.to_le_bytes(),
                scope.comparison().as_bytes(),
                scope.goal().as_bytes(),
                scope.probes().as_bytes(),
                scope.semantics().as_bytes(),
            ],
        );
        retained.check().map_err(Error::Resource)?;
        Ok(Self {
            source,
            goal: bridge.goal.query,
            probes,
            hard,
            map,
            declaration,
            bindings,
            digest,
            payload_bytes: identities.bytes,
            owner: Arc::clone(&bridge.owner),
            retained,
        })
    }
    pub fn digest(&self) -> Digest32 {
        self.digest
    }
    pub fn bindings(&self) -> ConfigurationBindings {
        self.bindings
    }
    pub(super) fn current(
        &self,
        bridge: &CreativeCandidateBridge<'_, '_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        bridge.check_current(resources).map_err(Error::Creative)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(resources) || !Arc::ptr_eq(&self.owner, &bridge.owner) {
            return Err(Error::ForeignPlan);
        }
        let root = bridge.contexts().original().state();
        if root.revision() != self.bindings.context
            || root.scope() != self.bindings.scope
            || !std::ptr::eq(self.goal, bridge.goal.query)
        {
            return Err(Error::ScopeMismatch);
        }
        check_source(bridge, self.source)
    }
}
fn check_weights(probes: &[ConfigurationProbe<'_>]) -> Result<(), Error> {
    if probes.is_empty() || probes.len() > 256 {
        return Err(Error::InvalidPlan);
    }
    for probe in probes {
        check(probe.weight).map_err(|e| Error::Arithmetic(e.into()))?;
        if probe.weight.numer() <= &BigInt::from(0) {
            return Err(Error::InvalidWeight);
        }
    }
    Ok(())
}
fn check_source(
    bridge: &CreativeCandidateBridge<'_, '_>,
    source: &SmtStatusQuery,
) -> Result<(), Error> {
    util::theory(source)?;
    if source.boolean_symbols() != bridge.goal.query.boolean_symbols()
        || source.assertions().len() != bridge.bindings.len()
    {
        return Err(Error::SourceQueryMismatch);
    }
    for (value, binding) in source.assertions().iter().zip(bridge.bindings) {
        if !std::ptr::eq(value, binding.formula) {
            return Err(Error::SourceQueryMismatch);
        }
    }
    Ok(())
}
fn required_registry<'i>(
    bridge: &CreativeCandidateBridge<'i, '_>,
    map: &[(BoolSymbolId, BoolSymbolId)],
    allowance: u64,
    work: &mut util::Work,
) -> Result<Vec<&'i BoolExpr>, Error> {
    let mut hard: Vec<&'i BoolExpr> =
        util::vector(bridge.bindings.len() + bridge.goal.query.assertions().len())?;
    let root = bridge.contexts().original().state();
    let candidates = bridge
        .bindings
        .iter()
        .filter(|binding| {
            root.premises()
                .iter()
                .any(|p| p.id() == binding.id && !p.changeable())
        })
        .map(|b| b.formula)
        .chain(bridge.goal.query.assertions());
    for value in candidates {
        let mut duplicate = false;
        for old in &hard {
            work.charge(allowance)?;
            duplicate |= support::equivalent(
                value,
                old,
                bridge.goal.query.boolean_symbols(),
                root.revision(),
                map,
                allowance,
            )?;
        }
        if !duplicate {
            hard.push(value);
        }
    }
    if hard.is_empty() {
        return Err(Error::InvalidPlan);
    }
    Ok(hard)
}
fn check_optional(
    probes: &[ConfigurationProbe<'_>],
    hard: &[&BoolExpr],
    source: &SmtStatusQuery,
    context: ContextRevision,
    map: &[(BoolSymbolId, BoolSymbolId)],
    allowance: u64,
    work: &mut util::Work,
) -> Result<(), Error> {
    for (i, probe) in probes.iter().enumerate() {
        if probe.identity.is_empty() || probe.identity.len() > 4096 {
            return Err(Error::InvalidPlan);
        }
        if probes[..i].iter().any(|p| p.identity == probe.identity) {
            return Err(Error::DuplicateProbe);
        }
        if probe.role == ProbeRole::Required {
            return Err(Error::RequiredProbe);
        }
        let mut hash = Sha256::new();
        let mut nodes = 0;
        util::expr(
            &mut hash,
            probe.predicate,
            source.boolean_symbols(),
            &mut nodes,
            1,
        )?;
        for required in hard {
            work.charge(allowance)?;
            if support::equivalent(
                probe.predicate,
                required,
                source.boolean_symbols(),
                context,
                map,
                allowance,
            )? {
                return Err(Error::RequiredProbe);
            }
        }
    }
    Ok(())
}
struct Identities {
    baseline: Digest32,
    hard: Digest32,
    probes: Digest32,
    bytes: u64,
    nodes: u64,
}
fn identities(
    bridge: &CreativeCandidateBridge<'_, '_>,
    source: &SmtStatusQuery,
    probes: &[ConfigurationProbe<'_>],
    hard: &[&BoolExpr],
    declaration: ConfigurationDeclaration,
    budget: WorkBudget,
) -> Result<Identities, Error> {
    let (source_theory, source_nodes) = util::theory(source)?;
    let (goal_theory, goal_nodes) = util::theory(bridge.goal.query)?;
    let root = bridge.contexts().original().state();
    let baseline = util::digest(
        b"epistesys.dsem.baseline.v1\0",
        &[
            bridge.source.id().digest().as_bytes(),
            root.revision().interpretation_digest().as_bytes(),
            source_theory.as_bytes(),
            goal_theory.as_bytes(),
        ],
    );
    let mut hh = Sha256::new();
    hh.update(b"epistesys.dsem.required.v1\0");
    let mut nodes = 0;
    for value in hard {
        util::expr(&mut hh, value, source.boolean_symbols(), &mut nodes, 1)?;
    }
    let mut hp = Sha256::new();
    hp.update(b"epistesys.dsem.probes.v1\0");
    let mut bytes = bridge.source.text().len() as u64 + 512;
    for probe in probes {
        util::field(&mut hp, probe.identity);
        util::expr(
            &mut hp,
            probe.predicate,
            source.boolean_symbols(),
            &mut nodes,
            1,
        )?;
        for number in [probe.weight.numer(), probe.weight.denom()] {
            let (_, magnitude) = number.to_bytes_le();
            hp.update([0]); // Both values are canonical and strictly positive before hashing.
            util::field(&mut hp, &magnitude);
            bytes = bytes
                .checked_add(magnitude.len() as u64)
                .ok_or(Error::PayloadBudget)?;
        }
        bytes = bytes
            .checked_add(probe.identity.len() as u64 + nodes * 5 + 64)
            .ok_or(Error::PayloadBudget)?;
        if bytes > budget.max_payload_bytes {
            return Err(Error::PayloadBudget);
        }
    }
    hp.update(declaration.domain.as_bytes());
    Ok(Identities {
        baseline,
        hard: util::finish(hh),
        probes: util::finish(hp),
        bytes,
        nodes: nodes + source_nodes + goal_nodes,
    })
}
