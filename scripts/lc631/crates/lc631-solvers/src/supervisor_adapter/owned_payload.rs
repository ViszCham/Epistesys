//! Resource-bound metadata and exact-byte owners for native payloads.
use super::SupervisorAdapterError;
use crate::registry::SolverRegistryError;
use crate::result_store::{
    ResultStore, ResultStoreError, ResultStoreInsert, ResultStoreLookupError, SolverResultBinding,
    StoredSolverResult,
};
use crate::runner::{InvocationDigest, QueryDigest, QuerySourceIdentity, SolverExecutionReport};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

mod execute;
pub use execute::execute_supervised_owned_payload_into_store;
#[cfg(test)]
pub(crate) use execute::{finish_owned_for_test, owned_test_hooks};

/// Fixed-capacity metadata store whose RAM claim lives until its backing drops.
/// It accepts no caller-supplied report; historic reads remain available.
pub struct OwnedPayloadStore<'view> {
    store: ResultStore,
    source: QuerySourceIdentity,
    // Drop backing store first, then refund its retained metadata claim.
    claim: ResourceClaim<'view>,
}
impl<'view> OwnedPayloadStore<'view> {
    pub fn with_resources(
        capacity: usize,
        reservation: &'view ResourceReservationView<'_>,
    ) -> Result<Self, OwnedPayloadStoreInitError> {
        let _ = ResultStore::with_capacity(capacity).map_err(OwnedPayloadStoreInitError::Store)?;
        let bytes = std::mem::size_of::<StoredSolverResult>()
            .checked_mul(capacity)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(OwnedPayloadStoreInitError::AllocationFailed)?;
        let amount =
            ResourceAmount::checked(bytes).map_err(OwnedPayloadStoreInitError::Resource)?;
        let claim = reservation
            .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
            .map_err(OwnedPayloadStoreInitError::Resource)?;
        let mut store =
            ResultStore::with_capacity(capacity).map_err(OwnedPayloadStoreInitError::Store)?;
        store
            .try_reserve_backing()
            .map_err(OwnedPayloadStoreInitError::Store)?;
        Ok(Self {
            store,
            source: QuerySourceIdentity::from_bytes(*reservation.source_revision().as_bytes()),
            claim,
        })
    }
    pub fn len(&self) -> usize {
        self.store.len()
    }
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
    pub fn lookup(
        &self,
        binding: SolverResultBinding,
    ) -> Result<&StoredSolverResult, ResultStoreLookupError> {
        self.store.lookup(binding)
    }
}

/// Store setup cannot have a process report: preserve that smaller phase type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedPayloadStoreInitError {
    Resource(ResourceError),
    Store(ResultStoreError),
    AllocationFailed,
}

/// Owns the exact stdout of one successful native V2 payload execution.
/// The retained bytes are not durable and do not validate a model or proof.
///
/// ```compile_fail
/// use lc631_solvers::supervisor_adapter::OwnedPayloadObservation;
/// fn copy_owner(owner: &OwnedPayloadObservation<'_>) { let _: OwnedPayloadObservation<'_> = owner.clone(); }
/// ```
///
/// ```compile_fail
/// use lc631_solvers::supervisor_adapter::OwnedPayloadObservation;
/// fn detach<'a>(owner: OwnedPayloadObservation<'a>) -> &'a [u8] { owner.stdout() }
/// ```
pub struct OwnedPayloadObservation<'view> {
    stdout: Vec<u8>,
    report: SolverExecutionReport,
    insertion: ResultStoreInsert,
    // Drop the bytes before refunding their live resource claim.
    _claim: ResourceClaim<'view>,
}
impl OwnedPayloadObservation<'_> {
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub fn payload(&self) -> Option<&[u8]> {
        let span = self.report.payload_wire_summary()?.payload_span();
        self.stdout
            .get(usize::try_from(span.start()).ok()?..usize::try_from(span.end()).ok()?)
    }
    pub const fn report(&self) -> SolverExecutionReport {
        self.report
    }
    pub const fn insertion(&self) -> ResultStoreInsert {
        self.insertion
    }
    pub const fn source(&self) -> QuerySourceIdentity {
        self.report.source()
    }
    pub const fn query(&self) -> QueryDigest {
        self.report.query()
    }
    pub const fn invocation(&self) -> Option<InvocationDigest> {
        self.report.invocation_digest()
    }
}

// No extra allocation on an error path after the child has run.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum OwnedPayloadError {
    Adapter(SupervisorAdapterError),
    Registry(SolverRegistryError),
    UnsupportedMapping,
    Resource(ResourceError),
    AllocationFailed,
    StoreBeforeExecution(ResultStoreError),
    /// Retains the actual report, including its process/cleanup/truncation state.
    Report(SolverExecutionReport),
    StoreAfterExecution {
        report: SolverExecutionReport,
        error: ResultStoreError,
    },
}
