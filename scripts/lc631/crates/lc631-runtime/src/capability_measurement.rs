//! Actual finite four-valued capability probes and advisory scheduling input.
//! Registrations and hashes are declarations/observations, never authority.
#![forbid(unsafe_code)]
use lc631_analysis::creativity_measurement::{
    weighted_assessment, Rational, RationalInterval, WeightedProbe,
};
use lc631_core::context::ContextRevision;
use lc631_solvers::{
    four_valued::{self as horn, Atom, Evaluation, Literal, Program, Rule, Truth},
    supervisor_adapter::hash_registered_executable,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};
use lc631_world::exploration_schedule::priority::{
    MissingPriorityReason, PriorityInput, ScheduleError, UnitInterval,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

mod schedule;
pub use schedule::{
    schedule_current, CapabilityScheduler, CapabilityWork, OwnedSchedule, SchedulingFreshness,
};

const HELD: u64 = 256 * 1024;
const SCRATCH: u64 = 1024 * 1024;
const MAX_PROBES: usize = 3;
const MAX_STEPS: u64 = 10_000;
const PROFILE: &[u8] = b"epistesys.tl-modal.four-valued-capabilities.v1";
const BACKEND_SOURCE: &[u8] = include_bytes!("../../lc631-solvers/src/four_valued.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    Both,
    NoExplosion,
    Derived,
    Unsupported,
}
impl Capability {
    fn index(self) -> Option<usize> {
        match self {
            Self::Both => Some(0),
            Self::NoExplosion => Some(1),
            Self::Derived => Some(2),
            Self::Unsupported => None,
        }
    }
    fn key(self) -> &'static [u8] {
        match self {
            Self::Both => b"both",
            Self::NoExplosion => b"no-explosion",
            Self::Derived => b"derived",
            Self::Unsupported => b"unsupported",
        }
    }
}
#[derive(Clone, Debug)]
pub struct ProbeSpec {
    pub capability: Capability,
    pub weight: Rational,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeStatus {
    Matched,
    Mismatch,
    Unregistered,
    Unknown,
    Unsupported,
}
#[derive(Debug)]
pub enum Error {
    Resource(ResourceError),
    InvalidPlan,
    Binding,
    Allocation,
    Numeric,
    Executable(lc631_solvers::runner::ProcessFailure),
    ExecutableChanged,
    Schedule(ScheduleError),
}

/// In-process profile declaration. Executable content hash is only a path
/// observation: no running-image attestation, version proof or TOCTOU closure.
pub struct Registry<'view> {
    context: ContextRevision,
    scope: [u8; 32],
    available: [bool; 3],
    executable: PathBuf,
    executable_digest: [u8; 32],
    backend_source_digest: [u8; 32],
    held: ResourceClaim<'view>,
}
impl<'view> Registry<'view> {
    pub fn capture(
        context: ContextRevision,
        scope: [u8; 32],
        available: &[Capability],
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        check_source(view, context)?;
        if scope == [0; 32] || available.len() > MAX_PROBES {
            return Err(Error::InvalidPlan);
        }
        let held = claim(view, HELD, true)?;
        let _scratch = claim(view, SCRATCH, false)?;
        let mut flags = [false; 3];
        for capability in available {
            let index = capability.index().ok_or(Error::InvalidPlan)?;
            if flags[index] {
                return Err(Error::InvalidPlan);
            }
            flags[index] = true;
        }
        let executable = std::env::current_exe().map_err(|_| Error::Binding)?;
        let executable_digest =
            hash_registered_executable(view, &executable).map_err(Error::Executable)?;
        Ok(Self {
            context,
            scope,
            available: flags,
            executable,
            executable_digest,
            backend_source_digest: hash(BACKEND_SOURCE),
            held,
        })
    }
    pub fn scope(&self) -> [u8; 32] {
        self.scope
    }
    /// Current observation identity only; this issues no permission or evidence.
    pub fn context_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<ContextRevision, Error> {
        self.check(view)?;
        Ok(self.context)
    }
    pub fn executable_digest(&self) -> [u8; 32] {
        self.executable_digest
    }
    pub fn backend_source_digest(&self) -> [u8; 32] {
        self.backend_source_digest
    }
    fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        check_source(view, self.context)?;
        check_claim(&self.held, view)?;
        if hash(BACKEND_SOURCE) != self.backend_source_digest {
            return Err(Error::Binding);
        }
        if hash_registered_executable(view, &self.executable).map_err(Error::Executable)?
            != self.executable_digest
        {
            return Err(Error::ExecutableChanged);
        }
        Ok(())
    }
}

/// Immutable required set and denominator. Registry presence is not a result.
pub struct Plan<'reg, 'view> {
    registry: &'reg Registry<'view>,
    probes: Vec<ProbeSpec>,
    max_steps: u64,
    unobserved: Option<RationalInterval>,
    held: ResourceClaim<'view>,
}
impl<'reg, 'view> Plan<'reg, 'view> {
    pub fn capture(
        registry: &'reg Registry<'view>,
        probes: &[ProbeSpec],
        max_steps: u64,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        registry.check(view)?;
        if probes.is_empty() || probes.len() > MAX_PROBES || max_steps == 0 || max_steps > MAX_STEPS
        {
            return Err(Error::InvalidPlan);
        }
        for (i, p) in probes.iter().enumerate() {
            if p.weight.numer().bits() > 1024
                || p.weight.denom().bits() > 1024
                || lc631_substrate::measurement_numeric::check(&p.weight).is_err()
                || p.weight <= Rational::from_integer(0.into())
                || probes[..i].iter().any(|q| q.capability == p.capability)
            {
                return Err(Error::InvalidPlan);
            }
        }
        let held = claim(view, HELD, true)?;
        let _scratch = claim(view, SCRATCH, false)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(probes.len())
            .map_err(|_| Error::Allocation)?;
        owned.extend_from_slice(probes);
        let unobserved = if probes
            .iter()
            .any(|p| p.capability == Capability::Unsupported)
        {
            None
        } else {
            Some(interval(0, 1)?)
        };
        Ok(Self {
            registry,
            probes: owned,
            max_steps,
            unobserved,
            held,
        })
    }
    pub fn unobserved_bounds(&self) -> Option<&RationalInterval> {
        self.unobserved.as_ref()
    }
    /// Compare fixed typed requirements after checking both current owners.
    /// Availability may differ, but the required set/denominator cannot drift.
    pub fn same_profile_current(
        &self,
        registry: &Registry<'_>,
        other: &Plan<'_, '_>,
        other_registry: &Registry<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<bool, Error> {
        self.check(registry, view)?;
        other.check(other_registry, view)?;
        Ok(registry.scope == other_registry.scope
            && registry.context == other_registry.context
            && registry.backend_source_digest == other_registry.backend_source_digest
            && registry.executable_digest == other_registry.executable_digest
            && self.max_steps == other.max_steps
            && self.probes.len() == other.probes.len()
            && self
                .probes
                .iter()
                .zip(&other.probes)
                .all(|(a, b)| a.capability == b.capability && a.weight == b.weight))
    }
    fn check(
        &self,
        registry: &Registry<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), Error> {
        if !std::ptr::eq(self.registry, registry) {
            return Err(Error::Binding);
        }
        registry.check(view)?;
        check_claim(&self.held, view)
    }
}

struct ActualProbe {
    state: ProbeStatus,
    evaluation: Option<Evaluation>,
}
/// Actual finite evaluations remain owned; no caller report/status constructor.
pub struct Assessment {
    statuses: Vec<ProbeStatus>,
    bounds: Option<RationalInterval>,
    actual_count: usize,
}
impl Assessment {
    pub fn statuses(&self) -> &[ProbeStatus] {
        &self.statuses
    }
    pub fn bounds(&self) -> Option<&RationalInterval> {
        self.bounds.as_ref()
    }
    pub fn actual_evaluation_count(&self) -> usize {
        self.actual_count
    }
}
pub struct OwnedAssessment<'plan, 'reg, 'view> {
    actual: Vec<ActualProbe>,
    report: Assessment,
    plan: &'plan Plan<'reg, 'view>,
    held: ResourceClaim<'view>,
}
impl<'plan, 'reg, 'view> OwnedAssessment<'plan, 'reg, 'view> {
    pub fn current(
        &self,
        plan: &Plan<'_, '_>,
        registry: &Registry<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&Assessment, Error> {
        if !std::ptr::eq(self.plan, plan) {
            return Err(Error::Binding);
        }
        plan.check(registry, view)?;
        check_claim(&self.held, view)?;
        for actual in &self.actual {
            if let Some(evaluation) = &actual.evaluation {
                if evaluation.binding().context() != registry.context {
                    return Err(Error::Binding);
                }
            }
        }
        Ok(&self.report)
    }
    /// Explicit profile: capability is the SOLE active scalar for u; novelty is
    /// separately caller-declared and never inferred from capability score.
    pub fn priority_current<'a>(
        &'a self,
        plan: &Plan<'_, '_>,
        registry: &Registry<'_>,
        view: &'view ResourceReservationView<'_>,
        novelty: UnitInterval,
    ) -> Result<OwnedPriority<'a, 'plan, 'reg, 'view>, Error> {
        let bounds = self.current(plan, registry, view)?.bounds();
        let held = claim(view, HELD, true)?;
        let priority = match bounds {
            None => PriorityInput::Missing(MissingPriorityReason::Unsupported),
            Some(bounds) => {
                let width = bounds.upper() - bounds.lower();
                PriorityInput::Discrete {
                    uncertainty: UnitInterval::checked(width.clone(), width)
                        .map_err(Error::Schedule)?,
                    novelty,
                }
            }
        };
        Ok(OwnedPriority {
            owner: self,
            priority,
            held,
        })
    }
}
pub struct OwnedPriority<'a, 'plan, 'reg, 'view> {
    owner: &'a OwnedAssessment<'plan, 'reg, 'view>,
    priority: PriorityInput,
    held: ResourceClaim<'view>,
}
impl OwnedPriority<'_, '_, '_, '_> {
    pub fn current(
        &self,
        plan: &Plan<'_, '_>,
        registry: &Registry<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<&PriorityInput, Error> {
        self.owner.current(plan, registry, view)?;
        check_claim(&self.held, view)?;
        Ok(&self.priority)
    }

    /// Canonical content of this fixed in-process observation profile, not
    /// owner addresses or numeric score equality. PROFILE, the capability
    /// definitions/expected outcomes, horn::PROFILE_REVISION, Given program
    /// basis and unassessed shared-checker status are fixed by this producer.
    /// A future producer/basis/oracle must introduce its own comparison domain.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn same_evidence_current(
        &self,
        plan: &Plan<'_, '_>,
        registry: &Registry<'_>,
        other: &OwnedPriority<'_, '_, '_, '_>,
        other_plan: &Plan<'_, '_>,
        other_registry: &Registry<'_>,
        view: &ResourceReservationView<'_>,
    ) -> Result<bool, Error> {
        self.current(plan, registry, view)?;
        other.current(other_plan, other_registry, view)?;
        if !plan.same_profile_current(registry, other_plan, other_registry, view)?
            || registry.available != other_registry.available
            || self.priority != other.priority
        {
            return Ok(false);
        }
        same_actual_evidence(self.owner, other.owner)
    }
}

/// No serialization, Debug text, opaque observation ID or timestamp is used.
/// Binding digests cross-pin the fixed actual query/program; the complete
/// finite result/dependency payload is compared separately, not hash-only.
fn same_actual_evidence(
    a: &OwnedAssessment<'_, '_, '_>,
    b: &OwnedAssessment<'_, '_, '_>,
) -> Result<bool, Error> {
    if a.actual.len() != b.actual.len()
        || a.report.statuses != b.report.statuses
        || a.report.bounds != b.report.bounds
        || a.report.actual_count != b.report.actual_count
    {
        return Ok(false);
    }
    for (a, b) in a.actual.iter().zip(&b.actual) {
        if a.state != b.state {
            return Ok(false);
        }
        match (&a.evaluation, &b.evaluation) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                if a.binding() != b.binding()
                    || a.atom_count() != b.atom_count()
                    || a.steps() != b.steps()
                {
                    return Ok(false);
                }
                for index in 0..a.atom_count() {
                    let index = u8::try_from(index).map_err(|_| Error::Binding)?;
                    let atom = Atom::new(index).map_err(|_| Error::Binding)?;
                    if a.truth(atom).map_err(|_| Error::Binding)?
                        != b.truth(atom).map_err(|_| Error::Binding)?
                    {
                        return Ok(false);
                    }
                    for literal in [Literal::positive(atom), Literal::negative(atom)] {
                        if a.dependencies(literal).map_err(|_| Error::Binding)?
                            != b.dependencies(literal).map_err(|_| Error::Binding)?
                        {
                            return Ok(false);
                        }
                    }
                }
            }
            (None, Some(_)) | (Some(_), None) => return Ok(false),
        }
    }
    Ok(true)
}

pub fn evaluate<'plan, 'reg, 'view>(
    plan: &'plan Plan<'reg, 'view>,
    view: &'view ResourceReservationView<'_>,
) -> Result<OwnedAssessment<'plan, 'reg, 'view>, Error> {
    plan.check(plan.registry, view)?;
    let held = claim(view, HELD, true)?;
    let scratch = claim(view, SCRATCH, false)?;
    let mut actual = Vec::new();
    actual
        .try_reserve_exact(plan.probes.len())
        .map_err(|_| Error::Allocation)?;
    let mut statuses = Vec::new();
    statuses
        .try_reserve_exact(plan.probes.len())
        .map_err(|_| Error::Allocation)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(plan.probes.len())
        .map_err(|_| Error::Allocation)?;
    let mut actual_count = 0;
    for p in &plan.probes {
        let item = match p.capability.index() {
            None => ActualProbe {
                state: ProbeStatus::Unsupported,
                evaluation: None,
            },
            Some(i) if !plan.registry.available[i] => ActualProbe {
                state: ProbeStatus::Unregistered,
                evaluation: None,
            },
            Some(_) => {
                actual_count += 1;
                execute(p.capability, plan.registry, plan.max_steps)?
            }
        };
        statuses.push(item.state);
        values.push(match item.state {
            ProbeStatus::Matched => interval(1, 1)?,
            ProbeStatus::Mismatch => interval(0, 0)?,
            ProbeStatus::Unregistered | ProbeStatus::Unknown | ProbeStatus::Unsupported => {
                interval(0, 1)?
            }
        });
        actual.push(item);
    }
    let bounds = if statuses.contains(&ProbeStatus::Unsupported) {
        None
    } else {
        let mut weighted = Vec::new();
        weighted
            .try_reserve_exact(plan.probes.len())
            .map_err(|_| Error::Allocation)?;
        for (p, v) in plan.probes.iter().zip(&values) {
            weighted.push(WeightedProbe {
                canonical_payload: p.capability.key(),
                weight: &p.weight,
                value: v,
            });
        }
        Some(weighted_assessment(&weighted).map_err(|_| Error::Numeric)?)
    };
    scratch.check().map_err(Error::Resource)?;
    held.check().map_err(Error::Resource)?;
    Ok(OwnedAssessment {
        actual,
        report: Assessment {
            statuses,
            bounds,
            actual_count,
        },
        plan,
        held,
    })
}
fn execute(
    capability: Capability,
    registry: &Registry<'_>,
    max_steps: u64,
) -> Result<ActualProbe, Error> {
    let p = Atom::new(0).map_err(|_| Error::InvalidPlan)?;
    let q = Atom::new(1).map_err(|_| Error::InvalidPlan)?;
    let p_pos = Literal::positive(p);
    let p_neg = Literal::negative(p);
    let q_pos = Literal::positive(q);
    let contradictory = [p_pos, p_neg];
    let one = [p_pos];
    let body = [p_pos];
    let rule = [Rule {
        body: &body,
        head: q_pos,
    }];
    let (facts, rules, query, expected): (&[Literal], &[Rule<'_>], Atom, Truth) = match capability {
        Capability::Both => (&contradictory, &[], p, Truth::Both),
        Capability::NoExplosion => (&contradictory, &[], q, Truth::Neither),
        Capability::Derived => (&one, &rule, q, Truth::Supported),
        Capability::Unsupported => return Err(Error::InvalidPlan),
    };
    let mut h = Sha256::new();
    h.update(PROFILE);
    h.update(registry.scope);
    h.update(capability.key());
    let program = Program::new(2, facts, rules, registry.context, h.finalize().into())
        .map_err(|_| Error::InvalidPlan)?;
    match program.evaluate(max_steps) {
        Ok(value) => {
            let actual = value.truth(query).map_err(|_| Error::Binding)?;
            Ok(ActualProbe {
                state: if actual == expected {
                    ProbeStatus::Matched
                } else {
                    ProbeStatus::Mismatch
                },
                evaluation: Some(value),
            })
        }
        Err(horn::Error::ResourceExhausted) => Ok(ActualProbe {
            state: ProbeStatus::Unknown,
            evaluation: None,
        }),
        Err(_) => Err(Error::InvalidPlan),
    }
}
fn interval(lo: i64, hi: i64) -> Result<RationalInterval, Error> {
    RationalInterval::new(
        Rational::from_integer(lo.into()),
        Rational::from_integer(hi.into()),
    )
    .map_err(|_| Error::Numeric)
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn check_source(view: &ResourceReservationView<'_>, context: ContextRevision) -> Result<(), Error> {
    view.check().map_err(Error::Resource)?;
    if view.source_revision() != context.source().digest() {
        return Err(Error::Binding);
    }
    Ok(())
}
fn check_claim(claim: &ResourceClaim<'_>, view: &ResourceReservationView<'_>) -> Result<(), Error> {
    claim.check().map_err(Error::Resource)?;
    if !claim.same_reservation(view) {
        return Err(Error::Binding);
    }
    Ok(())
}
fn claim<'a>(
    view: &'a ResourceReservationView<'_>,
    ram: u64,
    output: bool,
) -> Result<ResourceClaim<'a>, Error> {
    let amount = ResourceAmount::checked(ram).map_err(Error::Resource)?;
    let request = ResourceRequest::new(ResourceKind::Ram, amount);
    if output {
        view.claim_resources(&[request, ResourceRequest::new(ResourceKind::Output, amount)])
            .map_err(Error::Resource)
    } else {
        view.claim_resources(&[request]).map_err(Error::Resource)
    }
}
