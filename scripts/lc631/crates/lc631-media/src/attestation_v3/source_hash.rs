//! Source-only anchored revalidation; model/executable path policies are separate.
use super::MediaAttestationV3Error;
use crate::source_ingress::{observe_regular_source, RegularSourceObservation};
use lc631_process_supervisor::ResourceReservationView;
use std::path::Path;

/// Return the observed occurrence and bytes for the private authority comparison.
pub(super) fn hash_source<'a>(
    path: &Path,
    expected_bytes: u64,
    expected_digest: &str,
    resources: Option<&'a ResourceReservationView<'a>>,
) -> Result<RegularSourceObservation, MediaAttestationV3Error> {
    observe_regular_source(path, Some((expected_digest, expected_bytes)), resources)
        .map_err(|_| MediaAttestationV3Error::InvalidBinding)
}
