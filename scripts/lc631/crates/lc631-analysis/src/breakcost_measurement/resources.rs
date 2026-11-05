//! Logical payload preclaims; not allocator headers, RSS, global OOM or Authority.
use super::*;
use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};
pub(super) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    output: u64,
) -> Result<ResourceClaim<'v>, Error> {
    let request = |kind, n| {
        ResourceAmount::checked(n)
            .map(|amount| ResourceRequest::new(kind, amount))
            .map_err(Error::Resource)
    };
    if output == 0 {
        view.claim_resources(&[request(ResourceKind::Ram, MIB)?])
    } else {
        view.claim_resources(&[
            request(ResourceKind::Ram, MIB)?,
            request(ResourceKind::Output, output)?,
        ])
    }
    .map_err(Error::Resource)
}
pub(super) fn current(
    claim: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    claim.check().map_err(Error::Resource)?;
    if !claim.same_reservation(view) {
        return Err(Error::ForeignReservation);
    }
    view.check().map_err(Error::Resource)
}
pub(super) fn vector<T>(n: usize) -> Result<Vec<T>, Error> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| Error::ProducerFailure)?;
    Ok(v)
}
