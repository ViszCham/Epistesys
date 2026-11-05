//! 日本語: R1互換の小さいadmission。native ownerや原式評価を代行しない。
//! English: Small R1 admission compatibility; it supplies neither native owners nor formula evaluation.
use super::{declarations::PHYSICAL, required_metrics, resources, *};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    source::SourceStore,
    Digest32,
};

pub struct ConditionalRootPlan<'i, 'v> {
    pub(super) spec: RootSpec<'i>,
    pub(super) source: &'i SourceStore,
    retained: ResourceClaim<'v>,
}
pub struct ConditionalCandidate<'r, 'i, 'v> {
    pub(super) root: &'r ConditionalRootPlan<'i, 'v>,
    spec: CandidateSpec<'i>,
    retained: ResourceClaim<'v>,
}
impl<'i, 'v> ConditionalRootPlan<'i, 'v> {
    pub fn capture(
        spec: RootSpec<'i>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        view.check().map_err(Error::Resource)?;
        let source = spec.source.ok_or(Error::SourceMissing)?;
        let question = spec.question.ok_or(Error::InvalidDeclaration)?;
        if source.text().len() > 4096
            || source
                .slice(question)
                .map_err(|_| Error::InvalidDeclaration)?
                .is_empty()
            || source.id().digest() != view.source_revision()
        {
            return Err(Error::InvalidDeclaration);
        }
        let retained = resources::claim(view, 16 * 1024, 0).map_err(Error::Resource)?;
        let _scratch = resources::claim(view, 16 * 1024, 0).map_err(Error::Resource)?;
        validate_root(spec)?;
        let root = Self {
            spec,
            source,
            retained,
        };
        root.check(view)?;
        Ok(root)
    }
    pub fn provenance(&self) -> ConditionalProvenance {
        ConditionalProvenance::GivenUnvalidated
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !resources::current(&self.retained, view).map_err(Error::Resource)? {
            return Err(Error::ForeignReservation);
        }
        if self.source.id().digest() != view.source_revision() {
            return Err(Error::InvalidDeclaration);
        }
        Ok(())
    }
}
impl<'r, 'i, 'v> ConditionalCandidate<'r, 'i, 'v> {
    pub fn admit(
        root: &'r ConditionalRootPlan<'i, 'v>,
        spec: CandidateSpec<'i>,
        view: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        root.check(view)?;
        if spec.baseline_profile != root.spec.baseline_profile {
            return Err(Error::BaselineProfileMismatch);
        }
        let retained = resources::claim(view, 16 * 1024, 0).map_err(Error::Resource)?;
        let _scratch = resources::claim(view, 16 * 1024, 0).map_err(Error::Resource)?;
        validate_candidate(root.spec, spec)?;
        let candidate = Self {
            root,
            spec,
            retained,
        };
        candidate.check(view)?;
        Ok(candidate)
    }
    pub(super) fn check(&self, view: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.root.check(view)?;
        if !resources::current(&self.retained, view).map_err(Error::Resource)? {
            return Err(Error::ForeignReservation);
        }
        if self.spec.baseline_profile != self.root.spec.baseline_profile {
            return Err(Error::BaselineProfileMismatch);
        }
        Ok(())
    }
}
fn nonzero(digest: Digest32) -> bool {
    digest.as_bytes() != &[0; 32]
}
fn validate_root(spec: RootSpec<'_>) -> Result<(), Error> {
    if ![
        spec.comparison,
        spec.root_goal,
        spec.baseline,
        spec.full_cost_profile,
        spec.total_tl_policy,
    ]
    .into_iter()
    .all(nonzero)
        || spec.coordinates.len() != 12
        || spec.applicability.len() > 21
    {
        return Err(Error::InvalidDeclaration);
    }
    for (i, row) in spec.coordinates.iter().enumerate() {
        if !PHYSICAL.contains(&row.coordinate) || !nonzero(row.schema) {
            return Err(Error::InvalidDeclaration);
        }
        if spec.coordinates[..i]
            .iter()
            .any(|r| r.coordinate == row.coordinate)
        {
            return Err(Error::DuplicateCoordinate);
        }
    }
    for coordinate in PHYSICAL {
        if !spec.coordinates.iter().any(|r| r.coordinate == coordinate) {
            return Err(Error::MissingCoordinate(coordinate));
        }
    }
    validate_applicability(spec)
}
fn validate_applicability(spec: RootSpec<'_>) -> Result<(), Error> {
    let f1 = required_metrics(OriginalFormula::F1V1);
    let f2 = required_metrics(OriginalFormula::F2V1);
    let mut bytes = std::mem::size_of_val(spec.coordinates)
        .checked_add(std::mem::size_of_val(spec.applicability))
        .ok_or(Error::InvalidDeclaration)?;
    for (i, row) in spec.applicability.iter().enumerate() {
        if !f1.contains(&row.metric) && !f2.contains(&row.metric) {
            return Err(Error::InvalidDeclaration);
        }
        if spec.applicability[..i]
            .iter()
            .any(|r| r.metric == row.metric)
        {
            return Err(Error::DuplicateApplicability);
        }
        if row.root_goal != spec.root_goal {
            return Err(Error::ApplicabilityGoalMismatch(row.metric));
        }
        if !nonzero(row.regime)
            || (row.coordinate != Coordinate::WholeComposite && !PHYSICAL.contains(&row.coordinate))
        {
            return Err(Error::InvalidDeclaration);
        }
        if let ApplicabilityClaim::GivenConditional(text) = row.claim {
            if text.is_empty() {
                return Err(Error::InvalidDeclaration);
            }
            bytes = bytes
                .checked_add(text.len())
                .ok_or(Error::InvalidDeclaration)?;
        }
    }
    if bytes > 8192 {
        return Err(Error::InvalidDeclaration);
    }
    for &metric in f2.iter().chain(f1) {
        if !spec.applicability.iter().any(|r| r.metric == metric) {
            return Err(Error::MissingApplicability(metric));
        }
    }
    Ok(())
}
fn validate_candidate(root: RootSpec<'_>, spec: CandidateSpec<'_>) -> Result<(), Error> {
    if spec.coordinates.len() > 12 {
        return Err(Error::InvalidDeclaration);
    }
    let mut bytes = 0usize;
    for (i, row) in spec.coordinates.iter().enumerate() {
        if !PHYSICAL.contains(&row.coordinate) || row.payload.is_empty() || row.payload.len() > 256
        {
            return Err(Error::InvalidDeclaration);
        }
        if spec.coordinates[..i]
            .iter()
            .any(|r| r.coordinate == row.coordinate)
        {
            return Err(Error::DuplicateCoordinate);
        }
        bytes = bytes
            .checked_add(row.payload.len())
            .ok_or(Error::InvalidDeclaration)?;
    }
    if bytes > 4096 {
        return Err(Error::InvalidDeclaration);
    }
    for declared in root.coordinates {
        let row = spec
            .coordinates
            .iter()
            .find(|r| r.coordinate == declared.coordinate)
            .ok_or(Error::MissingCoordinate(declared.coordinate))?;
        if row.schema != declared.schema {
            return Err(Error::CoordinateSchemaMismatch(declared.coordinate));
        }
    }
    Ok(())
}
