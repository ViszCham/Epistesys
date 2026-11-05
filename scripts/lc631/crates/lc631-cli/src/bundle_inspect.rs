//! Explicit bounded read-only bundle command, not a resume executor.
use crate::effect_error::CliFailure;
use crate::io::{self, CommandIo};
use lc631_host::{JournalBundleLimits, JournalBundleOpenError};
use lc631_runtime::journal::{
    inspect_bundle_root, BundleInspectionError, BundleLiveError, JournalBundleInspection,
    JournalChainHeadState, JournalChainLimits,
};
use lc631_runtime::resource::ResourceReservationView;
use serde::Serialize;
use std::path::{Path, PathBuf};

const MAX_ROOT_BYTES: usize = 4096;

#[derive(Debug, Eq, PartialEq)]
pub struct BundleInspectArgs {
    ledger_root: PathBuf,
}
impl BundleInspectArgs {
    pub fn ledger_root(&self) -> &Path {
        &self.ledger_root
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleInspectArgsError {
    MissingLedgerRoot,
    MissingValue,
    EmptyValue,
    ValueTooLong,
    DuplicateOption,
    UnknownOption,
}

pub fn parse_args(args: &[String]) -> Result<BundleInspectArgs, BundleInspectArgsError> {
    let mut root = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] != "--ledger-root" {
            return Err(BundleInspectArgsError::UnknownOption);
        }
        if root.is_some() {
            return Err(BundleInspectArgsError::DuplicateOption);
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or(BundleInspectArgsError::MissingValue)?;
        if value.is_empty() {
            return Err(BundleInspectArgsError::EmptyValue);
        }
        if value.len() > MAX_ROOT_BYTES {
            return Err(BundleInspectArgsError::ValueTooLong);
        }
        root = Some(PathBuf::from(value));
        index += 2;
    }
    Ok(BundleInspectArgs {
        ledger_root: root.ok_or(BundleInspectArgsError::MissingLedgerRoot)?,
    })
}

#[derive(Debug, Serialize)]
pub struct BundleInspectionReport {
    schema_version: &'static str,
    status: &'static str,
    hold_code: Option<&'static str>,
    segment_count: Option<usize>,
    record_count: Option<usize>,
    segment_input_bytes: Option<usize>,
    latest_record_sequence: Option<u64>,
    local_head_frame_digest_bytes: Option<[u8; 32]>,
    head_state: Option<&'static str>,
    rollback_resistance: &'static str,
    fresh_revalidation_required: bool,
    external_anchor_observed: bool,
    authority_created: bool,
    completion_restored: bool,
    output_commit_allowed: bool,
    diagnostic_only: bool,
    claim_boundary: &'static str,
}

fn hold_code(error: &BundleInspectionError) -> &'static str {
    match error {
        BundleInspectionError::EmptyInventory => "empty_inventory",
        BundleInspectionError::Validation(error) => match error {
            BundleLiveError::Host(_) => "storage_read_hold",
            BundleLiveError::Resource(_) => "resource_hold",
            BundleLiveError::RebuildRequired => "bundle_rebuild_required",
            BundleLiveError::ChainLimit => "chain_limit",
            BundleLiveError::StaleCursor => "stale_cursor",
            BundleLiveError::InvalidCandidate(_) => "invalid_candidate",
            BundleLiveError::Index(_) => "index_hold",
            BundleLiveError::Codec(_) => "codec_hold",
            BundleLiveError::Poisoned => "poisoned_read",
            BundleLiveError::HostOpen(JournalBundleOpenError::BeforeEffect(_)) => {
                "unexpected_open_before_effect"
            }
            BundleLiveError::HostOpen(JournalBundleOpenError::MayHaveApplied(_)) => {
                "unexpected_open_may_have_applied"
            }
            BundleLiveError::StorageRejectedBeforeWrite(_) => {
                "unexpected_write_rejected_before_effect"
            }
            BundleLiveError::StorageMayHaveApplied(_) => "unexpected_write_may_have_applied",
        },
    }
}

pub fn render_report(
    result: Result<JournalBundleInspection, BundleInspectionError>,
) -> BundleInspectionReport {
    let mut report = BundleInspectionReport {
        schema_version: "lc631-journal-bundle-inspect.v1", status: "hold", hold_code: None,
        segment_count: None, record_count: None, segment_input_bytes: None,
        latest_record_sequence: None, local_head_frame_digest_bytes: None, head_state: None,
        rollback_resistance: "unavailable", fresh_revalidation_required: true,
        external_anchor_observed: false, authority_created: false, completion_restored: false,
        output_commit_allowed: false, diagnostic_only: true,
        claim_boundary: "read-only inspection profile; unexpected effect diagnostics are not evidence of no effects; non-atomic local byte consistency only; no semantic validation, authentication, external anchor, rollback resistance, authority, completion or host delivery",
    };
    match result {
        Ok(observed) => {
            report.status = "inspected_unanchored";
            report.segment_count = Some(observed.segment_count());
            report.record_count = Some(observed.record_count());
            report.segment_input_bytes = Some(observed.segment_input_bytes());
            report.latest_record_sequence = Some(observed.cursor().sequence());
            report.local_head_frame_digest_bytes = Some(*observed.head_frame_digest().as_bytes());
            report.head_state = Some(match observed.head_state() {
                JournalChainHeadState::Unanchored => "unanchored",
                JournalChainHeadState::LocalMatchUnanchored => "local_match_unanchored",
            });
        }
        Err(error) => report.hold_code = Some(hold_code(&error)),
    }
    report
}

pub fn inspect_existing_root<'a>(
    root: &Path,
    view: &'a ResourceReservationView<'a>,
) -> BundleInspectionReport {
    let result = JournalBundleLimits::checked(1000, 16 * 1024 * 1024)
        .map_err(|error| BundleInspectionError::Validation(BundleLiveError::Host(error)))
        .and_then(|limits| inspect_bundle_root(root, limits, view, JournalChainLimits::default()));
    render_report(result)
}

pub(crate) fn run(
    resources: &CommandIo,
    command: &str,
    args: BundleInspectArgs,
) -> Result<(), CliFailure> {
    let report = resources
        .with_view(32 * 1024 * 1024, |view| {
            Ok::<_, String>(inspect_existing_root(args.ledger_root(), view))
        })
        .map_err(CliFailure::from)?;
    let envelope = lc631_wire::WireEnvelope::shadow(command, report);
    resources
        .with_view(4 * 1024, |view| {
            io::write_pretty_json(&envelope, view, &mut std::io::stdout().lock())
                .map_err(|error| format!("bundle_inspect_output:{error:?}"))
        })
        .map_err(CliFailure::from)
}
