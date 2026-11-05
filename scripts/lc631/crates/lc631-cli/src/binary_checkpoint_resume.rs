//! Bounded, read-only bridge from the host's persisted segment inventory to
//! runtime checkpoint-chain inspection. It never restores completion or grants
//! Authority, and it does not treat a local chain as an external anchor.

use lc631_analysis::{CheckpointReadIdentity, ResumeObservation};
use lc631_host::persistent_owner::{HostPersistentStoreReader, PersistentStoreLimits};
use lc631_host::{HostError, OwnedBinaryCheckpointSegment};
use lc631_runtime::journal::{
    inspect_checkpoint_chain_resume, CheckpointChainResumeHold, JournalChainHeadState,
    JournalChainLimits, JournalChainSummary, JournalHead, MAX_JOURNAL_SEGMENT_BYTES,
};
use lc631_runtime::resource::ResourceReservationView;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Hard upper bound for bytes submitted to the runtime analyzer in one call.
pub const MAX_BINARY_RESUME_BYTES: usize = 16 * 1024 * 1024;
const MAX_LEDGER_ROOT_ARG_BYTES: usize = 4 * 1024;
const MAX_IDENTITY_ARG_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryCheckpointResumeInspectArgs {
    pub ledger_root: PathBuf,
    pub source_revision: String,
    pub plan_digest: String,
    pub authority_revision: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryCheckpointResumeInspectArgsError {
    MissingLedgerRoot,
    MissingSourceRevision,
    MissingPlanDigest,
    MissingAuthorityRevision,
    MissingValue,
    EmptyValue,
    ValueTooLong,
    DuplicateOption,
    UnknownOption,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BinaryCheckpointResumeHold {
    Storage(HostError),
    EmptyInventory,
    DescriptorSequence {
        expected: u64,
        actual: u64,
    },
    DescriptorSequenceOverflow,
    SegmentBytesExceeded {
        sequence: u64,
        limit: usize,
        actual: usize,
    },
    DescriptorBytesMismatch {
        sequence: u64,
        expected: usize,
        actual: usize,
    },
    AggregateBytesExceeded {
        limit: usize,
        actual: usize,
    },
    AggregateBytesOverflow,
    AllocationFailed,
    Runtime(CheckpointChainResumeHold),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BinaryCheckpointResumeDecision {
    Candidate {
        chain: JournalChainSummary,
        observation: ResumeObservation,
    },
    Hold(BinaryCheckpointResumeHold),
}

impl BinaryCheckpointResumeDecision {
    pub const fn claim_boundary(&self) -> &'static str {
        "bounded persisted binary checkpoint inspection only; no execution, Authority, completion restoration, or external-anchor claim"
    }
}

/// Minimal CLI-facing report. It intentionally omits checkpoint payloads,
/// paths, descriptor digests, and untrusted storage error strings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BinaryCheckpointResumeInspectionReport {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub hold_code: Option<&'static str>,
    pub segment_count: Option<usize>,
    pub record_count: Option<usize>,
    pub input_bytes: Option<usize>,
    pub head_state: Option<&'static str>,
    pub authenticated_head_provider_state: &'static str,
    pub fresh_revalidation_required: Option<bool>,
    pub authority_created: bool,
    pub completion_restored: bool,
    pub external_anchor_observed: bool,
    pub rollback_resistance_claimed: bool,
    pub output_commit_allowed: bool,
    pub diagnostic_only: bool,
    pub claim_boundary: &'static str,
}

/// Parse the exact read-only command surface. Unknown flags (including
/// `--execute`, head/commit options, and unrelated legacy CLI switches) fail
/// closed rather than being consumed by the general command parser.
pub fn parse_binary_checkpoint_resume_inspect_args(
    args: &[String],
) -> Result<BinaryCheckpointResumeInspectArgs, BinaryCheckpointResumeInspectArgsError> {
    let mut ledger_root = None;
    let mut source_revision = None;
    let mut plan_digest = None;
    let mut authority_revision = None;
    let mut index = 0;

    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--ledger-root" | "--source-revision" | "--plan-digest" | "--authority-revision"
        ) {
            return Err(BinaryCheckpointResumeInspectArgsError::UnknownOption);
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or(BinaryCheckpointResumeInspectArgsError::MissingValue)?;
        match option {
            "--ledger-root" => set_arg(&mut ledger_root, value, MAX_LEDGER_ROOT_ARG_BYTES)?,
            "--source-revision" => set_arg(&mut source_revision, value, MAX_IDENTITY_ARG_BYTES)?,
            "--plan-digest" => set_arg(&mut plan_digest, value, MAX_IDENTITY_ARG_BYTES)?,
            "--authority-revision" => {
                set_arg(&mut authority_revision, value, MAX_IDENTITY_ARG_BYTES)?
            }
            _ => return Err(BinaryCheckpointResumeInspectArgsError::UnknownOption),
        }
        index += 2;
    }

    let ledger_root =
        ledger_root.ok_or(BinaryCheckpointResumeInspectArgsError::MissingLedgerRoot)?;
    let source_revision =
        source_revision.ok_or(BinaryCheckpointResumeInspectArgsError::MissingSourceRevision)?;
    let plan_digest =
        plan_digest.ok_or(BinaryCheckpointResumeInspectArgsError::MissingPlanDigest)?;
    let authority_revision = authority_revision
        .ok_or(BinaryCheckpointResumeInspectArgsError::MissingAuthorityRevision)?;

    Ok(BinaryCheckpointResumeInspectArgs {
        ledger_root: PathBuf::from(ledger_root),
        source_revision,
        plan_digest,
        authority_revision,
    })
}

fn set_arg(
    slot: &mut Option<String>,
    value: &str,
    byte_limit: usize,
) -> Result<(), BinaryCheckpointResumeInspectArgsError> {
    if slot.is_some() {
        return Err(BinaryCheckpointResumeInspectArgsError::DuplicateOption);
    }
    if value.is_empty() {
        return Err(BinaryCheckpointResumeInspectArgsError::EmptyValue);
    }
    if value.len() > byte_limit {
        return Err(BinaryCheckpointResumeInspectArgsError::ValueTooLong);
    }
    *slot = Some(value.to_owned());
    Ok(())
}

/// Open only an already-existing binary checkpoint root and inspect it with no
/// expected head. This command surface therefore remains Unanchored by design.
pub fn inspect_existing_binary_checkpoint_root<'a>(
    root: &Path,
    resources: &'a ResourceReservationView<'a>,
    limits: JournalChainLimits,
    identity: CheckpointReadIdentity<'_>,
) -> BinaryCheckpointResumeDecision {
    let reader = match HostPersistentStoreReader::open_binary_checkpoint_root(
        root,
        PersistentStoreLimits::default(),
        resources,
    ) {
        Ok(reader) => reader,
        Err(error) => return hold(BinaryCheckpointResumeHold::Storage(error)),
    };
    inspect_persisted_binary_checkpoint_resume(&reader, None, limits, identity)
}

pub fn report_binary_checkpoint_resume_inspection(
    decision: &BinaryCheckpointResumeDecision,
) -> BinaryCheckpointResumeInspectionReport {
    let mut report = BinaryCheckpointResumeInspectionReport {
        schema_version: "lc631-binary-checkpoint-inspect.v1",
        status: "hold",
        hold_code: None,
        segment_count: None,
        record_count: None,
        input_bytes: None,
        head_state: None,
        authenticated_head_provider_state: "not_wired",
        fresh_revalidation_required: None,
        authority_created: false,
        completion_restored: false,
        external_anchor_observed: false,
        rollback_resistance_claimed: false,
        output_commit_allowed: false,
        diagnostic_only: true,
        claim_boundary: "read-only diagnostic candidate; local bytes only; no completion/Authority/output commitment, external anchor, or rollback-resistance claim",
    };
    match decision {
        BinaryCheckpointResumeDecision::Candidate { chain, observation } => {
            report.status = "candidate_unanchored";
            report.segment_count = Some(chain.segment_count());
            report.record_count = Some(chain.record_count());
            report.input_bytes = Some(chain.total_input_bytes());
            report.head_state = Some(match chain.head_state() {
                JournalChainHeadState::Unanchored => "unanchored",
                JournalChainHeadState::LocalMatchUnanchored => "local_match_unanchored",
            });
            report.fresh_revalidation_required = Some(observation.fresh_revalidation_required);
        }
        BinaryCheckpointResumeDecision::Hold(reason) => {
            report.hold_code = Some(hold_code(reason));
        }
    }
    report
}

fn hold_code(reason: &BinaryCheckpointResumeHold) -> &'static str {
    match reason {
        BinaryCheckpointResumeHold::Storage(_) => "storage_read_hold",
        BinaryCheckpointResumeHold::EmptyInventory => "empty_inventory",
        BinaryCheckpointResumeHold::DescriptorSequence { .. } => "descriptor_sequence_gap",
        BinaryCheckpointResumeHold::DescriptorSequenceOverflow => "descriptor_sequence_overflow",
        BinaryCheckpointResumeHold::SegmentBytesExceeded { .. } => "segment_bytes_exceeded",
        BinaryCheckpointResumeHold::DescriptorBytesMismatch { .. } => "descriptor_bytes_mismatch",
        BinaryCheckpointResumeHold::AggregateBytesExceeded { .. } => "aggregate_bytes_exceeded",
        BinaryCheckpointResumeHold::AggregateBytesOverflow => "aggregate_bytes_overflow",
        BinaryCheckpointResumeHold::AllocationFailed => "allocation_hold",
        BinaryCheckpointResumeHold::Runtime(_) => "runtime_chain_hold",
    }
}

/// Enumerate one read-only inventory snapshot and pass its ordered segment
/// bytes to the runtime's cross-segment source/plan/authority revalidator.
///
/// The returned candidate remains Unanchored and requires fresh validation.
/// Every host storage, order, or size failure is an explicit Hold. Owned segment
/// buffers stay alive through the analysis call, retaining their RAM/output
/// resource claims for the full borrowed-slice lifetime.
pub fn inspect_persisted_binary_checkpoint_resume(
    reader: &HostPersistentStoreReader<'_>,
    expected_local_head: Option<&JournalHead>,
    limits: JournalChainLimits,
    identity: CheckpointReadIdentity<'_>,
) -> BinaryCheckpointResumeDecision {
    // This is the adapter's only inventory enumeration. Reader reads below are
    // bound to these exact descriptors and do not rescan the directory.
    let inventory = match reader.inventory() {
        Ok(inventory) => inventory,
        Err(error) => return hold(BinaryCheckpointResumeHold::Storage(error)),
    };
    if inventory.is_empty() {
        return hold(BinaryCheckpointResumeHold::EmptyInventory);
    }

    let segment_limit = limits.max_segments();
    if inventory.len() > segment_limit {
        return hold(BinaryCheckpointResumeHold::Runtime(
            CheckpointChainResumeHold::Chain(
                lc631_runtime::journal::JournalChainHoldReason::SegmentCountExceeded {
                    limit: segment_limit,
                },
            ),
        ));
    }

    let byte_limit = limits.max_input_bytes().min(MAX_BINARY_RESUME_BYTES);
    let mut aggregate_bytes = 0_usize;
    let mut previous_sequence: Option<u64> = None;
    for descriptor in inventory {
        let expected_sequence = match previous_sequence {
            Some(previous) => match previous.checked_add(1) {
                Some(expected) => expected,
                None => return hold(BinaryCheckpointResumeHold::DescriptorSequenceOverflow),
            },
            None => 1,
        };
        if descriptor.sequence() != expected_sequence {
            return hold(BinaryCheckpointResumeHold::DescriptorSequence {
                expected: expected_sequence,
                actual: descriptor.sequence(),
            });
        }
        previous_sequence = Some(descriptor.sequence());

        let actual = match usize::try_from(descriptor.byte_len()) {
            Ok(actual) => actual,
            Err(_) => return hold(BinaryCheckpointResumeHold::AggregateBytesOverflow),
        };
        if actual == 0 || actual > MAX_JOURNAL_SEGMENT_BYTES {
            return hold(BinaryCheckpointResumeHold::SegmentBytesExceeded {
                sequence: descriptor.sequence(),
                limit: MAX_JOURNAL_SEGMENT_BYTES,
                actual,
            });
        }
        aggregate_bytes = match aggregate_bytes.checked_add(actual) {
            Some(total) => total,
            None => return hold(BinaryCheckpointResumeHold::AggregateBytesOverflow),
        };
        if aggregate_bytes > byte_limit {
            return hold(BinaryCheckpointResumeHold::AggregateBytesExceeded {
                limit: byte_limit,
                actual: aggregate_bytes,
            });
        }
    }

    // Claim both collection backings before allocation. Raw segment payloads
    // carry separate host claims; analysis/parser scratch is a separate scope.
    // Declaring the claim first drops both later collections before refunding.
    let metadata_bytes = std::mem::size_of::<OwnedBinaryCheckpointSegment<'_>>()
        .checked_add(std::mem::size_of::<&[u8]>())
        .and_then(|per_segment| per_segment.checked_mul(inventory.len()));
    let Some(metadata_bytes) = metadata_bytes else {
        return hold(BinaryCheckpointResumeHold::AggregateBytesOverflow);
    };
    let _metadata_claim = match reader.claim_inspection_metadata(metadata_bytes) {
        Ok(claim) => claim,
        Err(error) => return hold(BinaryCheckpointResumeHold::Storage(error)),
    };
    let mut owned_segments = Vec::new();
    if owned_segments.try_reserve_exact(inventory.len()).is_err() {
        return hold(BinaryCheckpointResumeHold::AllocationFailed);
    }
    for descriptor in inventory {
        let segment = match reader.read_binary_checkpoint_segment_descriptor(descriptor) {
            Ok(segment) => segment,
            Err(error) => return hold(BinaryCheckpointResumeHold::Storage(error)),
        };
        let expected_bytes = match usize::try_from(descriptor.byte_len()) {
            Ok(expected) => expected,
            Err(_) => return hold(BinaryCheckpointResumeHold::AggregateBytesOverflow),
        };
        if segment.len() != expected_bytes || segment.is_empty() {
            return hold(BinaryCheckpointResumeHold::DescriptorBytesMismatch {
                sequence: descriptor.sequence(),
                expected: expected_bytes,
                actual: segment.len(),
            });
        }
        owned_segments.push(segment);
    }

    let mut borrowed_segments = Vec::new();
    if borrowed_segments
        .try_reserve_exact(owned_segments.len())
        .is_err()
    {
        return hold(BinaryCheckpointResumeHold::AllocationFailed);
    }
    for segment in &owned_segments {
        borrowed_segments.push(segment.as_bytes());
    }

    // Do not move/drop owned_segments before the analyzer has returned: each
    // owner holds the host's RAM/output claim for its borrowed byte slice.
    match inspect_checkpoint_chain_resume(&borrowed_segments, expected_local_head, limits, identity)
    {
        lc631_runtime::journal::CheckpointChainResumeDecision::Candidate { chain, observation } => {
            BinaryCheckpointResumeDecision::Candidate { chain, observation }
        }
        lc631_runtime::journal::CheckpointChainResumeDecision::Hold(reason) => {
            hold(BinaryCheckpointResumeHold::Runtime(reason))
        }
    }
}

fn hold(reason: BinaryCheckpointResumeHold) -> BinaryCheckpointResumeDecision {
    BinaryCheckpointResumeDecision::Hold(reason)
}
