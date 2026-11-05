//! Shared bounded part admission for live and read-only bundle access.
use super::*;

pub(super) fn read_snapshot_part<'a>(
    anchor: &DirectoryAnchor,
    entries: &[JournalBundleDescriptor],
    sequence: u64,
    part: BundlePart,
    resources: &'a ResourceReservationView<'a>,
) -> Result<OwnedJournalBundlePart<'a>, HostError> {
    resources.check().map_err(resource_error)?;
    anchor.validate_current().map_err(io_error)?;
    let found = entries
        .first()
        .and_then(|first| sequence.checked_sub(first.sequence()))
        .and_then(|offset| usize::try_from(offset).ok())
        .filter(|index| {
            entries
                .get(*index)
                .is_some_and(|entry| entry.sequence() == sequence)
        })
        .or_else(|| {
            entries
                .binary_search_by_key(&sequence, JournalBundleDescriptor::sequence)
                .ok()
        })
        .ok_or_else(|| store_error("bundle_part_missing"))?;
    let expected = entries[found]
        .part(part)
        .copied()
        .ok_or_else(|| store_error("bundle_part_missing"))?;
    let read_claim = u64::try_from(part.cap())
        .ok()
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| store_error("bundle_read_size_overflow"))?;
    let claim = resources
        .claim_resources(&[
            resource(ResourceKind::Ram, read_claim)?,
            resource(ResourceKind::Output, read_claim)?,
        ])
        .map_err(resource_error)?;
    let name = part.name(sequence);
    let mut file = anchor.open_read_only(Path::new(&name)).map_err(io_error)?;
    let bytes = file.read_bounded(part.cap()).map_err(io_error)?;
    file.validate_current().map_err(io_error)?;
    drop(file);
    anchor.validate_current().map_err(io_error)?;
    resources.check().map_err(resource_error)?;
    if bytes.is_empty() {
        return Err(store_error("bundle_part_empty"));
    }
    if descriptor(&bytes)? != expected {
        return Err(store_error("bundle_part_changed"));
    }
    Ok(OwnedJournalBundlePart {
        bytes,
        _claim: claim,
    })
}
