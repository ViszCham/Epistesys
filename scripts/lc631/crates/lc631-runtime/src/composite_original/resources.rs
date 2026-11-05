//! 日本語: allocation前の論理claim。同leaseの検査は権限やOS上限ではない。
//! English: Logical preclaims and exact lease checks, not authority or OS memory enforcement.
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

pub(super) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: u64,
) -> Result<ResourceClaim<'v>, ResourceError> {
    view.check()?;
    let request = |kind, bytes| {
        ResourceAmount::checked(bytes).map(|amount| ResourceRequest::new(kind, amount))
    };
    if output == 0 {
        view.claim_resources(&[request(ResourceKind::Ram, ram)?])
    } else {
        view.claim_resources(&[
            request(ResourceKind::Ram, ram)?,
            request(ResourceKind::Output, output)?,
        ])
    }
}

pub(super) fn current(
    retained: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<bool, ResourceError> {
    view.check()?;
    retained.check()?;
    Ok(retained.same_reservation(view))
}
