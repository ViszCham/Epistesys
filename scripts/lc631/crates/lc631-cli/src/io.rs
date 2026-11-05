//! Command-owned I/O accounting. Payload quotas are not measured RSS or an OS sandbox.
mod analysis_runner;
mod canonical_services;
mod output;
mod path_services;
pub(crate) use analysis_runner::{AnalysisRunner, FileEffectOwner};
use lc631_process_supervisor::{read_bounded_bytes, BoundedBytes};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, Reservation, ResourceAmount, ResourceBroker, ResourceError,
    ResourceKind, ResourceLimits, ResourceRequest, ResourceReservationView, RunResources,
};
pub(crate) use output::encode_pretty_bounded;
pub use output::write_pretty_json;

const COMMAND_DISK_LIMIT: u64 = 256 * 1024 * 1024;
const HOST_KEY_DISK_BYTES: u64 = 65;
/// Media scratch and its persisted receipt key share one fixed command budget.
pub(crate) const MEDIA_TEMP_DISK_CAP: u64 = COMMAND_DISK_LIMIT - HOST_KEY_DISK_BYTES;
use std::cell::RefCell;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(any(feature = "gpu-cubecl-wgpu", test))]
pub(crate) const MAX_NVIDIA_SAMPLES: usize = 4_096;

fn next_run_id() -> Result<u64, String> {
    NEXT_RUN_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| "io_run_identity_exhausted".into())
}

pub fn read_text<'a, R: Read + ?Sized>(
    reader: &mut R,
    view: &'a ResourceReservationView<'a>,
    kind: ResourceKind,
) -> Result<BoundedBytes<'a>, ResourceError> {
    let bytes = read_bounded_bytes(reader, view, kind)?;
    if let Err(error) = bytes.as_utf8() {
        view.record_cleanup_unknown();
        return Err(error);
    }
    Ok(bytes)
}

/// All owned input Strings are consumed inside the command before finish().
/// Reservations deliberately outlive individual readers and derived DTOs.
pub(crate) struct CommandIo {
    cancellation: Cancellation,
    // Cloning shares the existing root and counters; it never creates a budget.
    owner: ResourceBroker,
    run: RunResources,
    retained: RefCell<Vec<Reservation>>,
}

impl CommandIo {
    pub(crate) fn analysis_runner(&self) -> AnalysisRunner<'_> {
        AnalysisRunner::new(self)
    }
    pub(crate) fn baseline_guard(
        &self,
        root: &Path,
    ) -> Result<lc631_compat::BaselineGuardReport, String> {
        analysis_runner::baseline_guard(self, root)
    }
    pub(crate) fn file_effect_owner(&self) -> FileEffectOwner<'_> {
        FileEffectOwner::new(self)
    }

    pub(crate) fn with_effect_resources<T>(
        &self,
        source: &str,
        ram: u64,
        disk: u64,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        let requests = [
            checked_request(ResourceKind::Ram, ram)?,
            checked_request(ResourceKind::Output, ram)?,
            checked_request(ResourceKind::Disk, disk)?,
        ];
        self.with_reserved_binding(
            lc631_core::context::SourceRevision::of(source).digest(),
            ram,
            ram,
            &requests,
            operation,
        )
    }

    pub(crate) fn with_journal_view<T>(
        &self,
        source: &str,
        ram_cap: u64,
        disk_cap: u64,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut requests = vec![
            checked_request(ResourceKind::Ram, ram_cap).map_err(resource_error)?,
            checked_request(ResourceKind::Output, ram_cap).map_err(resource_error)?,
        ];
        if disk_cap != 0 {
            // Owner admission and the writer's live claim coexist until durable
            // handoff. Reserved capacity is not observed physical file bytes.
            let simultaneous = disk_cap
                .checked_mul(2)
                .ok_or("journal_disk_budget_overflow")?;
            requests
                .push(checked_request(ResourceKind::Disk, simultaneous).map_err(resource_error)?);
        }
        let mut operation_error = None;
        let result = self.with_reserved_binding(
            lc631_core::context::SourceRevision::of(source).digest(),
            ram_cap,
            ram_cap,
            &requests,
            |view| {
                let result = operation(view);
                if let Err(error) = &result {
                    operation_error = Some(error.clone());
                }
                Ok(result)
            },
        );
        match result {
            Ok(result) => result,
            Err(cleanup) => match operation_error {
                Some(operation) => Err(format!("{operation}; io_resource_cleanup:{cleanup:?}")),
                None => Err(resource_error(cleanup)),
            },
        }
    }
    pub(crate) fn write_public<T: serde::Serialize>(
        &self,
        command: &str,
        payload: T,
    ) -> Result<(), String> {
        let envelope = lc631_wire::WireEnvelope::shadow(command, payload);
        // Explicit legacy-full-view budget; the compact solve view is a later
        // runtime contract. No public bytes precede complete bounded encoding.
        self.with_view(16 * 1024 * 1024, |view| {
            output::write_pretty_json_observed(&envelope, view, &mut std::io::stdout().lock())
                .map_err(|error| match error {
                    output::JsonWriteFailure::BeforeWrite(error) => resource_error(error),
                    output::JsonWriteFailure::SinkWrite(kind) => format!("stdout_write:{kind:?}"),
                })
        })
    }
    pub(crate) fn begin(owner: &ResourceBroker) -> Result<Self, String> {
        let limits = ResourceLimits::checked([
            128 * 1024 * 1024,
            COMMAND_DISK_LIMIT,
            64 * 1024 * 1024,
            16 * 1024 * 1024,
            2,
            2 * 1024 * 1024,
        ])
        .map_err(resource_error)?;
        // Ingress has not been read yet. This labels the accounting operation;
        // it is explicitly not a source-content or semantic-evidence digest.
        let deadline = Deadline::after(Duration::from_secs(1_800)).map_err(resource_error)?;
        let cancellation = Cancellation::default();
        let run = owner
            .begin_run(
                next_run_id()?,
                Digest32::from_bytes(*b"Epistesys CLI ingress budget v1!"),
                limits,
                deadline,
                cancellation.clone(),
            )
            .map_err(resource_error)?;
        Ok(Self {
            cancellation,
            owner: owner.clone(),
            run,
            retained: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn with_source_process_view<T>(
        &self,
        source: &str,
        output_limit: u64,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_process_resources(
            lc631_core::context::SourceRevision::of(source).digest(),
            source.len() as u64,
            output_limit,
            None,
            |view| Ok(operation(view)),
        )
        .map_err(resource_error)?
    }

    pub(crate) fn with_media_process_view<T>(
        &self,
        source_digest: Digest32,
        output_limit: u64,
        disk_limit: u64,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_process_resources(source_digest, 0, output_limit, Some(disk_limit), |view| {
            Ok(operation(view))
        })
        .map_err(resource_error)?
    }

    pub(crate) fn with_source_process_resources<T>(
        &self,
        source: &str,
        output_limit: u64,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        self.with_process_resources(
            lc631_core::context::SourceRevision::of(source).digest(),
            source.len() as u64,
            output_limit,
            None,
            operation,
        )
    }

    fn with_process_resources<T>(
        &self,
        source_digest: Digest32,
        input_bytes: u64,
        output_limit: u64,
        disk_limit: Option<u64>,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        let ram = output_limit
            .checked_mul(2)
            .and_then(|bytes| {
                bytes.checked_add(std::mem::size_of::<lc631_analysis::BoundedToolResult>() as u64)
            })
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let requests = process_requests(ram, input_bytes, output_limit, disk_limit)?;
        self.with_reserved_binding(source_digest, ram, output_limit, &requests, operation)
    }

    pub(crate) fn with_source_input_resources<T>(
        &self,
        source: &str,
        cap: usize,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        let cap = u64::try_from(cap).map_err(|_| ResourceError::ArithmeticOverflow)?;
        let requests = [
            checked_request(ResourceKind::Ram, cap)?,
            checked_request(ResourceKind::Output, cap)?,
        ];
        self.with_reserved_binding(
            lc631_core::context::SourceRevision::of(source).digest(),
            cap,
            cap,
            &requests,
            operation,
        )
    }

    /// Reuse this command's run and cancellation state for bounded preparation.
    /// The allowance accounts for transient legacy construction and retained
    /// artifacts; it does not measure allocator/RSS or create another broker.
    pub(crate) fn with_preparation_view<T>(
        &self,
        source: &str,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let output = crate::tlcx_commands::MAX_SUMMARY_BYTES as u64;
        let ram = lc631_runtime::solve_preparation::PREPARATION_RAM_FLOOR
            .checked_add(output)
            .ok_or("preparation_ram_overflow")?;
        let requests = [
            checked_request(ResourceKind::Ram, ram).map_err(resource_error)?,
            checked_request(ResourceKind::Output, output).map_err(resource_error)?,
        ];
        self.with_reserved_binding(
            lc631_core::context::SourceRevision::of(source).digest(),
            ram,
            output,
            &requests,
            |view| Ok(operation(view)),
        )
        .map_err(resource_error)?
    }

    fn with_reserved_binding<T>(
        &self,
        source_digest: Digest32,
        payload_ram: u64,
        output_limit: u64,
        requests: &[ResourceRequest],
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        self.cancellation.check()?;
        self.retained
            .borrow_mut()
            .try_reserve(2)
            .map_err(|_| ResourceError::AllocationFailed)?;
        let mut payload = self.run.reserve(&[
            checked_request(ResourceKind::Ram, payload_ram)?,
            checked_request(ResourceKind::Output, output_limit)?,
        ])?;
        let mut child = match self.run.reserve_for_source(source_digest, requests) {
            Ok(child) => child,
            Err(error) => {
                payload.release_confirmed()?;
                return Err(error);
            }
        };
        // Both peak buffers are charged before the callback. The child's live
        // process/pipe claim must be gone before its reservation can be closed.
        let result = operation(&child.lease());
        let cleanup = child.release_confirmed();
        self.retained.borrow_mut().push(payload);
        if let Err(error) = cleanup {
            self.retained.borrow_mut().push(child);
            return result.and_then(|_| Err(error));
        }
        result
    }

    pub(crate) fn with_view<T>(
        &self,
        limit: u64,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let ram = limit.checked_mul(2).ok_or("io_budget_overflow")?;
        let reservation = self
            .run
            .reserve(&[
                request(ResourceKind::Ram, ram)?,
                request(ResourceKind::Output, limit)?,
            ])
            .map_err(resource_error)?;
        let result = operation(&reservation.lease());
        // Never refund on reader drop. The command retains its charges until
        // inputs, derived DTOs and callback consumers have all finished.
        let mut retained = self.retained.borrow_mut();
        retained
            .try_reserve(1)
            .map_err(|_| "io_owner_allocation_failed")?;
        retained.push(reservation);
        result
    }

    pub(crate) fn with_storage_view<T>(
        &self,
        output_limit: u64,
        disk_limit: u64,
        operation: impl FnOnce(&ResourceReservationView<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let ram = output_limit
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(1024 * 1024))
            .ok_or("storage_ram_budget_overflow")?;
        self.retained
            .borrow_mut()
            .try_reserve(1)
            .map_err(|_| "storage_owner_allocation_failed")?;
        let reservation = self
            .run
            .reserve(&[
                request(ResourceKind::Ram, ram)?,
                request(ResourceKind::Output, output_limit)?,
                request(ResourceKind::Disk, disk_limit)?,
            ])
            .map_err(resource_error)?;
        let result = operation(&reservation.lease());
        if result.is_err() {
            reservation.lease().record_cleanup_unknown();
        }
        // Successful writer callbacks have synced and handed logical disk
        // ownership to the bounded per-root store. Returned DTO RAM stays here.
        self.retained.borrow_mut().push(reservation);
        result
    }

    pub(crate) fn read_from(&self, reader: &mut impl Read, limit: u64) -> Result<String, String> {
        self.with_view(limit, |view| {
            let bytes = read_text(reader, view, ResourceKind::Output).map_err(resource_error)?;
            let text = bytes.as_utf8().map_err(resource_error)?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(text.len())
                .map_err(|_| "io_text_allocation_failed")?;
            owned.push_str(text);
            Ok(owned)
        })
    }

    pub(crate) fn read_path(&self, path: &Path, limit: u64) -> Result<String, String> {
        let mut file = std::fs::File::open(path).map_err(|error| format!("io_open:{error}"))?;
        self.read_from(&mut file, limit)
    }

    pub(crate) fn host_context(
        &self,
        path: &Path,
        create: bool,
    ) -> Result<lc631_host::HostReceiptContext, String> {
        let operation = |view: &ResourceReservationView<'_>| {
            let result = if create {
                lc631_host::HostReceiptContext::load_or_create_with_resources(path, view)
            } else {
                lc631_host::HostReceiptContext::load_existing_with_resources(path, view)
            };
            result.map_err(|error| format!("host_receipt_root:{error:?}"))
        };
        if create {
            // The owner re-reads a bounded key plus one overflow sentinel.
            self.with_storage_view(257, HOST_KEY_DISK_BYTES, operation)
        } else {
            self.with_view(256, operation)
        }
    }

    #[cfg(any(feature = "gpu-cubecl-wgpu", test))]
    pub(crate) fn sample_storage(
        &self,
    ) -> Result<std::sync::Arc<std::sync::Mutex<Vec<[u64; 4]>>>, String> {
        let bytes = MAX_NVIDIA_SAMPLES
            .checked_mul(std::mem::size_of::<[u64; 4]>())
            .ok_or("sample_budget_overflow")?;
        self.retained
            .borrow_mut()
            .try_reserve(1)
            .map_err(|_| "sample_owner_allocation_failed")?;
        let mut reservation = self
            .run
            .reserve(&[request(ResourceKind::Ram, bytes as u64)?])
            .map_err(resource_error)?;
        let mut samples = Vec::new();
        if samples.try_reserve_exact(MAX_NVIDIA_SAMPLES).is_err()
            || samples.capacity() > MAX_NVIDIA_SAMPLES
        {
            drop(samples);
            reservation.release_confirmed().map_err(resource_error)?;
            return Err("sample_storage_allocation_exceeds_profile".into());
        }
        // The command—not the sampler child—owns these payload bytes through
        // summary consumption. Fixed Arc/Mutex overhead is not measured RSS.
        self.retained.borrow_mut().push(reservation);
        Ok(std::sync::Arc::new(std::sync::Mutex::new(samples)))
    }

    #[cfg(feature = "gpu-cubecl-wgpu")]
    pub(crate) fn reserve_sampler(&self) -> Result<Reservation, String> {
        self.run
            .reserve(&[
                request(ResourceKind::Ram, 1024 * 1024)?,
                request(ResourceKind::Output, 80 * 1024)?,
                request(ResourceKind::Child, 1)?,
                request(ResourceKind::Cleanup, 65_536)?,
            ])
            .map_err(resource_error)
    }

    pub(crate) fn finish(mut self) -> Result<(), String> {
        let mut first_error = None;
        for mut reservation in self.retained.into_inner() {
            if let Err(error) = reservation.release_confirmed() {
                first_error.get_or_insert_with(|| resource_error(error));
            }
        }
        if let Err(error) = self.run.close_confirmed() {
            first_error.get_or_insert_with(|| resource_error(error));
        }
        first_error.map_or(Ok(()), Err)
    }
}

fn request(kind: ResourceKind, amount: u64) -> Result<ResourceRequest, String> {
    checked_request(kind, amount).map_err(resource_error)
}

fn checked_request(kind: ResourceKind, amount: u64) -> Result<ResourceRequest, ResourceError> {
    Ok(ResourceRequest::new(kind, ResourceAmount::checked(amount)?))
}

fn resource_error(error: ResourceError) -> String {
    format!("io_resource:{error:?}")
}

pub(crate) fn parse_source_digest(encoded: &str) -> Result<Digest32, String> {
    let hex = encoded
        .strip_prefix("sha256:")
        .filter(|hex| hex.len() == 64)
        .ok_or("media_source_digest_invalid")?;
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("media_source_digest_invalid".into());
    }
    let mut bytes = [0; 32];
    for (index, value) in bytes.iter_mut().enumerate() {
        *value = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "media_source_digest_invalid")?;
    }
    Ok(Digest32::from_bytes(bytes))
}

fn process_requests(
    payload_ram: u64,
    input_bytes: u64,
    output_limit: u64,
    disk_limit: Option<u64>,
) -> Result<Vec<ResourceRequest>, ResourceError> {
    let ram = payload_ram
        .checked_add(input_bytes)
        .and_then(|bytes| bytes.checked_add(32_768))
        // Fixed small-launch profile allowance. The supervisor still computes
        // its actual checked launch plan and rejects excess; caps never grow.
        .and_then(|bytes| bytes.checked_add(32 * 1024 * 1024))
        .ok_or(ResourceError::ArithmeticOverflow)?;
    let mut requests = vec![
        checked_request(ResourceKind::Ram, ram)?,
        checked_request(ResourceKind::Output, output_limit)?,
        checked_request(ResourceKind::Child, 1)?,
        checked_request(ResourceKind::Cleanup, 65_536)?,
    ];
    if let Some(disk) = disk_limit {
        requests
            .try_reserve(1)
            .map_err(|_| ResourceError::AllocationFailed)?;
        requests.push(checked_request(ResourceKind::Disk, disk)?);
    }
    Ok(requests)
}

#[cfg(test)]
mod tests;
