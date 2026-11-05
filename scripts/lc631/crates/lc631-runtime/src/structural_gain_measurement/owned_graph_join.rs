//! 日本語: 実graph/K6対応と両方の現在ownerを借用する限定join。
//! English: Bounded actual graph/K6 correspondence and borrowed current owners.
//! Given interpretation, unassessed semantics and original provenance remain
//! unchanged. This is neither full F1/F2 input nor a goal recommendation.
//! Safe borrowing/private construction and pure bounded checks are sufficient;
//! no unsafe, persistent ABI, parallel backend or performance claim is needed.
#![forbid(unsafe_code)]
mod correspondence;

use super::OwnedPreservation;
use crate::description_measurement::{self as description, OwnedDescription};
use crate::joint_selection::{compare_rows, AxisKind, PairRelation, SelectionError};
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_substrate::{
    resource::{ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView},
    semantic_term::TermError,
    Digest32,
};

#[derive(Debug)]
pub enum Error {
    Resource(ResourceError),
    Preservation(super::Error),
    Description(description::Error),
    Codec(TermError),
    Correspondence,
    MissingTarget,
    IncompleteMapping,
    ProfileMismatch,
    Budget,
    Selection(SelectionError),
}

/// Separate aggregate identity; never substitutes for an original result digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JoinBinding(Digest32);

/// Both producers and exact backing bytes must outlive every current read.
/// No report, scalar, replacement binding, Clone or Deserialize constructor.
/// ```compile_fail
/// use lc631_runtime::structural_gain_measurement::owned_graph_join::OwnedGraphJoin;
/// let _: OwnedGraphJoin<'_, '_, '_, '_, '_, '_> = serde_json::from_str("{}").unwrap();
/// ```
pub struct OwnedGraphJoin<'j, 'i, 'r, 'a, 'p, 'v> {
    preservation: &'j OwnedPreservation<'i, 'v>,
    description: &'j OwnedDescription<'r, 'a, 'p, 'v>,
    source: &'j [u8],
    target: &'j [u8],
    binding: JoinBinding,
    retained: ResourceClaim<'v>,
}

impl<'j, 'i, 'r, 'a, 'p, 'v> OwnedGraphJoin<'j, 'i, 'r, 'a, 'p, 'v> {
    pub fn capture(
        preservation: &'j OwnedPreservation<'i, 'v>,
        description: &'j OwnedDescription<'r, 'a, 'p, 'v>,
        source: &'j [u8],
        target: &'j [u8],
        resources: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let retained = claim(resources, 16 * 1024)?;
        let binding = correspondence::verify(preservation, description, source, target, resources)?;
        retained.check().map_err(Error::Resource)?;
        Ok(Self { preservation, description, source, target, binding, retained })
    }

    pub fn current(&self, resources: &ResourceReservationView<'_>) -> Result<Current<'_>, Error> {
        self.retained.check().map_err(Error::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(Error::ProfileMismatch);
        }
        let actual = correspondence::verify(
            self.preservation, self.description, self.source, self.target, resources,
        )?;
        if actual != self.binding {
            return Err(Error::Correspondence);
        }
        let preservation = self.preservation.report_current(resources).map_err(Error::Preservation)?;
        let description = self.description.report_current(resources).map_err(Error::Description)?;
        self.retained.check().map_err(Error::Resource)?;
        Ok(Current { preservation, description, binding: self.binding })
    }
}

/// Original reports are borrowed without equalizing their subjects or producers.
pub struct Current<'a> {
    preservation: &'a ComponentReport,
    description: &'a ComponentReport,
    binding: JoinBinding,
}
impl Current<'_> {
    pub fn preservation(&self) -> &ComponentReport { self.preservation }
    pub fn description(&self) -> &ComponentReport { self.description }
    pub const fn binding(&self) -> Digest32 { self.binding.0 }
}

/// Actual consumer: compare only measured Preserve under the same fixed plans.
/// CR-K is a required joined input, never an additional Pareto gain. No goal,
/// gate, TL total, original score or recommendation is manufactured here.
pub fn compare_current(
    left: &OwnedGraphJoin<'_, '_, '_, '_, '_, '_>,
    right: &OwnedGraphJoin<'_, '_, '_, '_, '_, '_>,
    resources: &ResourceReservationView<'_>,
) -> Result<PairRelation, Error> {
    left.current(resources)?;
    right.current(resources)?;
    if !std::ptr::eq(left.preservation.plan, right.preservation.plan)
        || left.preservation.binding.pins != right.preservation.binding.pins
    {
        return Err(Error::ProfileMismatch);
    }
    // Existing producer API additionally checks exact shared description-root
    // identity. Its numeric preference is not included in the Pareto comparison.
    description::compare_current(left.description, right.description, resources)
        .map_err(Error::Description)?;
    let a = left.preservation.axis_current(resources).map_err(Error::Preservation)?;
    let b = right.preservation.axis_current(resources).map_err(Error::Preservation)?;
    let result = compare_rows(&[AxisKind::Preserve], &[a.value], &[b.value])
        .map_err(Error::Selection)?;
    left.current(resources)?;
    right.current(resources)?;
    Ok(result)
}

fn claim<'v>(resources: &'v ResourceReservationView<'_>, bytes: u64) -> Result<ResourceClaim<'v>, Error> {
    resources.check().map_err(Error::Resource)?;
    resources.claim_resources(&[ResourceRequest::new(ResourceKind::Ram,
        ResourceAmount::checked(bytes).map_err(Error::Resource)?)])
        .map_err(Error::Resource)
}
