//! Read-only consumer scratch claims against the reader's existing lease.
use super::{resource, resource_error, store_error, HostError, HostPersistentStoreReader};
use lc631_process_supervisor::{ResourceClaim, ResourceKind};

impl<'a> HostPersistentStoreReader<'a> {
    /// Reserve logical metadata bytes before creating inspection collections.
    /// Hold this claim until every corresponding collection has dropped.
    /// This grants no file access, authority or allocator/RSS guarantee.
    pub fn claim_inspection_metadata(&self, bytes: usize) -> Result<ResourceClaim<'a>, HostError> {
        self.inventory()?;
        let bytes =
            u64::try_from(bytes).map_err(|_| store_error("inspection_metadata_size_overflow"))?;
        self.resources
            .claim_resources(&[
                resource(ResourceKind::Ram, bytes)?,
                resource(ResourceKind::Output, bytes)?,
            ])
            .map_err(resource_error)
    }
}
