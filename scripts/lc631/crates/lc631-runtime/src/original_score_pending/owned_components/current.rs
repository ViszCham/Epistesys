//! Current bounded classification and real borrowed ComponentAssessment consumer.
use super::{CollectionPlan, ComponentOwnerRef, Error};
use lc631_analysis::creativity_measurement::{
    AssessmentPolicy, CandidateSubject, ComponentAssessment, ComponentMetric as M, ComponentReport,
    RationalInterval,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceKind, ResourceRequest, ResourceReservationView,
};

const RETAINED_RAM: u64 = 16 * 1024;
const SCRATCH_RAM: u64 = 16 * 1024;

/// No report/scalar/provider constructor, mutable report view or Deserialize.
/// Actual owners/inputs remain alive and are checked before AND after callbacks.
/// Callbacks consume diagnostic data only; checks are not an atomic cancellation
/// fence, rollback facility, semantic validator or execution authority.
pub struct OwnedComponentCollection<'o, 'a, 'v> {
    plan: &'o CollectionPlan,
    subject: CandidateSubject,
    owners: &'o [ComponentOwnerRef<'a, 'v>],
    retained: ResourceClaim<'v>,
}
impl<'o, 'a, 'v> OwnedComponentCollection<'o, 'a, 'v> {
    pub fn capture(
        plan: &'o CollectionPlan,
        subject: CandidateSubject,
        owners: &'o [ComponentOwnerRef<'a, 'v>],
        resources: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        if owners.is_empty() || owners.len() > 6 {
            return Err(Error::InvalidOwners);
        }
        if subject.scope != plan.pins.scope.comparison() || subject.baseline != plan.pins.baseline {
            return Err(Error::ScopeMismatch);
        }
        let retained = claim(resources, RETAINED_RAM)?;
        let value = Self {
            plan,
            subject,
            owners,
            retained,
        };
        value.check_current(resources)?;
        Ok(value)
    }

    /// HRTB prevents returning a report/current wrapper borrowed from the local
    /// classification/assessment buffers. R may retain only separately owned data.
    pub fn with_current<R>(
        &self,
        resources: &ResourceReservationView<'_>,
        consume: impl for<'current> FnOnce(CurrentComponents<'current>) -> R,
    ) -> Result<R, Error> {
        self.check_current(resources)?;
        let scratch = claim(resources, SCRATCH_RAM)?;
        let snapshot = self.snapshot(resources)?;
        let policy = AssessmentPolicy {
            scope: self.plan.pins.scope.comparison(),
            baseline: self.plan.pins.baseline,
            required: self.plan.required_subset(),
        };
        let ready = policy
            .required
            .iter()
            .all(|metric| snapshot.exact.iter().any(|report| report.metric == *metric));
        let assessment = if ready {
            Some(
                ComponentAssessment::from_reports(&policy, self.subject, &snapshot.exact)
                    .map_err(Error::Assessment)?,
            )
        } else {
            None
        };
        let result = consume(CurrentComponents {
            native: &snapshot.native,
            exact: &snapshot.exact,
            correspondence: &snapshot.correspondence,
            missing: &snapshot.missing,
            assessment,
        });
        scratch.check().map_err(Error::Resource)?;
        self.check_current(resources)?;
        Ok(result)
    }

    fn check_current(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        resources.check().map_err(Error::Resource)?;
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(Error::ForeignReservation);
        }
        if resources.source_revision() != self.plan.pins.source.digest() {
            return Err(Error::ScopeMismatch);
        }
        let mut metrics = [None; 6];
        for (index, owner) in self.owners.iter().enumerate() {
            let report = owner.report_current(self.plan.pins, resources)?;
            if report.binding.scope != self.subject.scope
                || report.binding.baseline != self.subject.baseline
            {
                return Err(Error::ScopeMismatch);
            }
            if metrics[..index].contains(&Some(report.metric)) {
                return Err(Error::DuplicateMetric);
            }
            metrics[index] = Some(report.metric);
        }
        self.retained.check().map_err(Error::Resource)
    }

    fn snapshot(&self, resources: &ResourceReservationView<'_>) -> Result<Snapshot<'a>, Error> {
        let mut native = reserved(6)?;
        let mut exact = reserved(6)?;
        let mut correspondence = reserved(6)?;
        let mut missing = reserved(19)?;
        for owner in self.owners {
            let report = owner.report_current(self.plan.pins, resources)?;
            native.push(report);
            if CandidateSubject::from_binding(report.binding) == self.subject {
                exact.push(report);
            } else {
                correspondence.push(report.metric);
            }
        }
        for metric in self.plan.original_required() {
            if !exact.iter().any(|report| report.metric == *metric) {
                missing.push(*metric);
            }
        }
        Ok(Snapshot {
            native,
            exact,
            correspondence,
            missing,
        })
    }
}
struct Snapshot<'a> {
    native: Vec<&'a ComponentReport>,
    exact: Vec<&'a ComponentReport>,
    correspondence: Vec<M>,
    missing: Vec<M>,
}

/// Exact here means matching six reported subject fields only. Every original
/// state/basis/validation/independence field remains in its original report.
pub struct CurrentComponents<'a> {
    native: &'a [&'a ComponentReport],
    exact: &'a [&'a ComponentReport],
    correspondence: &'a [M],
    missing: &'a [M],
    assessment: Option<ComponentAssessment<'a>>,
}
impl<'a> CurrentComponents<'a> {
    pub fn native_reports(&self) -> &'a [&'a ComponentReport] {
        self.native
    }
    pub fn exact_reports(&self) -> &'a [&'a ComponentReport] {
        self.exact
    }
    pub fn correspondence_required(&self) -> &'a [M] {
        self.correspondence
    }
    pub fn missing_leaves(&self) -> &'a [M] {
        self.missing
    }
    pub fn assessment(&self) -> Option<&ComponentAssessment<'a>> {
        self.assessment.as_ref()
    }
    pub fn scalar(&self) -> Option<&RationalInterval> {
        None
    }
    pub const fn formula_evaluated(&self) -> bool {
        false
    }
    pub const fn tl_correspondence_pending(&self) -> bool {
        true
    }
}
fn reserved<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    if values.capacity() > count {
        return Err(Error::AllocationFailed);
    }
    Ok(values)
}
fn claim<'v>(
    resources: &'v ResourceReservationView<'_>,
    bytes: u64,
) -> Result<ResourceClaim<'v>, Error> {
    resources.check().map_err(Error::Resource)?;
    resources
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(bytes).map_err(Error::Resource)?,
        )])
        .map_err(Error::Resource)
}
