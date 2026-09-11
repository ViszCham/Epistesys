#![forbid(unsafe_code)]

use lc631_core::{stable_sha256, ClosureLevel, ObligationId, SourceSpan};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision, UntrustedReceipt,
};
use lc631_tl::{
    CanonicalTranslationEnvelope, DomainTranslationPayload, Obligation, ObligationKind,
    ObligationPolarity, ObligationStrength, PreservationState, ProjectionDefectGraphV3,
    ProjectionEdgeId, ProjectionEdgeV3, ProjectionStage, SourceLedgerId, TargetClaim,
    TargetClaimId, VerificationReceipt, VerificationStatus, VerifierKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const MEDIA_SCHEMA: &str = "lc631-media-evidence.v1";
pub const MEDIA_BACKEND_SCHEMA: &str = "lc631-media-backend.v1";
pub const MAX_MEDIA_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_MEDIA_OBSERVATIONS: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaSourceKind {
    LocalUserProvided,
    UserOwnedExport,
    AuthorizedCaptionTrack,
    YoutubeMetadataOnly,
    YoutubeUrlUnacquired,
    ExternalUserManagedAcquisition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaSourceRequest {
    pub kind: MediaSourceKind,
    pub locator: String,
    pub user_asserts_rights: bool,
    pub media_bytes_requested: bool,
    pub network_requested: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaSourceAuthority {
    pub authorized: bool,
    pub metadata_only: bool,
    pub local_path: Option<PathBuf>,
    pub blockers: Vec<String>,
    pub source_digest: String,
    pub source_snapshot_sha256: Option<String>,
    pub source_bytes: Option<u64>,
    pub attestation_receipt_digest: Option<String>,
    eligible_for_attestation: bool,
    source_attestation: Option<UntrustedReceipt>,
}

impl MediaSourceAuthority {
    pub fn attestation_verified(&self) -> bool {
        self.authorized
            && self.attestation_receipt_digest.is_some()
            && self.source_attestation.is_some()
    }
}

pub fn authorize_media_source(request: &MediaSourceRequest) -> MediaSourceAuthority {
    let source_digest = stable_sha256(&format!("{:?}:{}", request.kind, request.locator));
    let local = Path::new(&request.locator);
    let local_exists = local.is_file();
    let mut blockers = Vec::new();
    let (eligible, metadata_only, local_path) = match request.kind {
        MediaSourceKind::YoutubeMetadataOnly => (true, true, None),
        MediaSourceKind::AuthorizedCaptionTrack => {
            if request.user_asserts_rights {
                (true, false, local_exists.then(|| local.to_path_buf()))
            } else {
                blockers.push("caption_permission_not_asserted".into());
                (false, false, None)
            }
        }
        MediaSourceKind::LocalUserProvided | MediaSourceKind::UserOwnedExport => {
            if request.user_asserts_rights && local_exists && !request.network_requested {
                (true, false, Some(local.to_path_buf()))
            } else {
                if !request.user_asserts_rights {
                    blockers.push("media_rights_not_asserted".into());
                }
                if !local_exists {
                    blockers.push("local_media_not_found".into());
                }
                if request.network_requested {
                    blockers.push("local_route_cannot_request_network".into());
                }
                (false, false, None)
            }
        }
        MediaSourceKind::ExternalUserManagedAcquisition => {
            if request.user_asserts_rights && local_exists && !request.network_requested {
                (true, false, Some(local.to_path_buf()))
            } else {
                blockers.push("external_acquisition_must_arrive_as_authorized_local_file".into());
                (false, false, None)
            }
        }
        MediaSourceKind::YoutubeUrlUnacquired => {
            if request.media_bytes_requested {
                blockers.push("youtube_url_alone_does_not_authorize_media_download".into());
            }
            if request.network_requested {
                blockers.push("automated_youtube_media_acquisition_disabled".into());
            }
            (false, true, None)
        }
    };
    let source_snapshot_sha256 = local_path
        .as_deref()
        .and_then(|path| sha256_file(path).ok());
    let source_bytes = local_path
        .as_deref()
        .and_then(|path| path.metadata().ok())
        .map(|metadata| metadata.len());
    let authorized = eligible && metadata_only;
    if eligible && !metadata_only {
        blockers.push("authenticated_media_source_receipt_required".into());
    }
    MediaSourceAuthority {
        authorized,
        metadata_only,
        local_path,
        blockers,
        source_digest,
        source_snapshot_sha256,
        source_bytes,
        attestation_receipt_digest: None,
        eligible_for_attestation: eligible,
        source_attestation: None,
    }
}

pub fn authorize_media_source_attested(
    request: &MediaSourceRequest,
    attestation: UntrustedReceipt,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<MediaSourceAuthority, MediaError> {
    let mut authority = authorize_media_source(request);
    if !authority.eligible_for_attestation || authority.metadata_only {
        return Err(MediaError::SourceAuthorityDenied);
    }
    let snapshot = authority
        .source_snapshot_sha256
        .clone()
        .ok_or(MediaError::SourceSnapshotUnavailable)?;
    let subject =
        SubjectRevision::checked(snapshot).map_err(|_| MediaError::SourceReceiptRejected)?;
    let scope = ReceiptScope::checked(media_source_scope(request.kind))
        .map_err(|_| MediaError::SourceReceiptRejected)?;
    let verified = verifier
        .verify(
            attestation,
            &ReceiptPolicy::exact(
                ReceiptClass::MediaSource,
                subject,
                scope,
                media_source_payload_digest(request, &authority)?,
                now_epoch,
            ),
            replay,
        )
        .map_err(|_| MediaError::SourceReceiptRejected)?;
    authority.authorized = true;
    authority
        .blockers
        .retain(|blocker| blocker != "authenticated_media_source_receipt_required");
    authority.attestation_receipt_digest = Some(verified.receipt_digest());
    authority.source_attestation = Some(verified.into_untrusted());
    Ok(authority)
}

pub fn media_source_payload_digest(
    request: &MediaSourceRequest,
    authority: &MediaSourceAuthority,
) -> Result<String, MediaError> {
    let snapshot = authority
        .source_snapshot_sha256
        .as_deref()
        .ok_or(MediaError::SourceSnapshotUnavailable)?;
    Ok(stable_sha256(&format!(
        "{:?}\0{}\0{}\0{}\0{}\0{}\0{}",
        request.kind,
        snapshot,
        authority.source_bytes.unwrap_or_default(),
        request.user_asserts_rights,
        request.media_bytes_requested,
        request.network_requested,
        authority.source_digest
    )))
}

pub fn media_source_scope(kind: MediaSourceKind) -> String {
    format!("media/source/{}", media_source_kind_name(kind))
}

fn media_source_kind_name(kind: MediaSourceKind) -> &'static str {
    match kind {
        MediaSourceKind::LocalUserProvided => "local_user_provided",
        MediaSourceKind::UserOwnedExport => "user_owned_export",
        MediaSourceKind::AuthorizedCaptionTrack => "authorized_caption_track",
        MediaSourceKind::YoutubeMetadataOnly => "youtube_metadata_only",
        MediaSourceKind::YoutubeUrlUnacquired => "youtube_url_unacquired",
        MediaSourceKind::ExternalUserManagedAcquisition => "external_user_managed_acquisition",
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendFamily {
    Decode,
    ShotBoundary,
    ObjectDetection,
    SegmentationTracking,
    Pose,
    FaceActionUnit,
    ActionRecognition,
    Ocr,
    DepthMotion,
    Asr,
    Diarization,
    AudioEvent,
    CrossModalAlignment,
    FrameDifference,
    AcousticClustering,
    TemporalCoPresence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MethodCandidate {
    pub family: BackendFamily,
    pub method_id: &'static str,
    pub expected_output: &'static str,
    pub backend_required: bool,
    pub bundled: bool,
    pub claim_boundary: &'static str,
}

pub fn default_method_catalog() -> Vec<MethodCandidate> {
    vec![
        method(
            BackendFamily::Decode,
            "ffmpeg-or-nvdec",
            "decoded frame/audio stream",
        ),
        method(
            BackendFamily::ShotBoundary,
            "transnet-v2",
            "shot-boundary candidates",
        ),
        method(
            BackendFamily::ObjectDetection,
            "grounding-dino",
            "open-vocabulary box candidates",
        ),
        method(
            BackendFamily::SegmentationTracking,
            "sam2-or-sam3-plus-bytetrack",
            "anonymous mask tracks",
        ),
        method(
            BackendFamily::Pose,
            "mediapipe-pose",
            "pose landmark candidates",
        ),
        method(
            BackendFamily::FaceActionUnit,
            "libreface-or-openface",
            "facial action-unit candidates",
        ),
        method(
            BackendFamily::ActionRecognition,
            "slowfast-or-videomae",
            "multi-label action candidates",
        ),
        method(
            BackendFamily::Ocr,
            "configured-ocr",
            "timestamped text-region candidates",
        ),
        method(
            BackendFamily::DepthMotion,
            "depth-anything-and-motion",
            "relative depth/motion candidates",
        ),
        method(
            BackendFamily::Asr,
            "whisper-or-whisperx",
            "timestamped transcript candidates",
        ),
        method(
            BackendFamily::Diarization,
            "pyannote-audio",
            "anonymous speaker segments",
        ),
        method(
            BackendFamily::AudioEvent,
            "panns-audioset",
            "audio-event candidates",
        ),
        method(
            BackendFamily::CrossModalAlignment,
            "typed-event-graph",
            "cross-modal relation candidates",
        ),
        method(
            BackendFamily::FrameDifference,
            "opencv-mean-absolute-frame-difference",
            "frame-difference motion candidates; not action recognition",
        ),
        method(
            BackendFamily::AcousticClustering,
            "pcm-feature-two-cluster",
            "anonymous acoustic-cluster candidates; not speaker diarization",
        ),
        method(
            BackendFamily::TemporalCoPresence,
            "timestamp-overlap-only",
            "audio/video temporal co-presence; not semantic alignment",
        ),
    ]
}

fn method(
    family: BackendFamily,
    method_id: &'static str,
    expected_output: &'static str,
) -> MethodCandidate {
    MethodCandidate {
        family,
        method_id,
        expected_output,
        backend_required: true,
        bundled: false,
        claim_boundary:
            "method candidate only; model weights, license, execution, calibration, and source authority require separate receipts",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendState {
    Unavailable,
    ConfiguredNotExecuted,
    ExecutedCandidate,
    ValidatedObservation,
    FailedWithDiagnostics,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BackendDescriptor {
    pub family: BackendFamily,
    pub backend_id: String,
    pub executable_or_model: Option<PathBuf>,
    pub model_sha256: Option<String>,
    pub license_receipt: Option<String>,
    pub state: BackendState,
    pub closure_level: ClosureLevel,
    pub blockers: Vec<String>,
}

impl BackendDescriptor {
    pub fn configured(
        family: BackendFamily,
        backend_id: impl Into<String>,
        executable_or_model: Option<PathBuf>,
        model_sha256: Option<String>,
        license_receipt: Option<String>,
    ) -> Self {
        let exists = executable_or_model
            .as_ref()
            .is_some_and(|path| path.is_file());
        let model_ready = model_sha256.is_some() && license_receipt.is_some();
        let state = if exists && model_ready {
            BackendState::ConfiguredNotExecuted
        } else {
            BackendState::Unavailable
        };
        let mut blockers = Vec::new();
        if !exists {
            blockers.push("backend_executable_or_model_missing".into());
        }
        if model_sha256.is_none() {
            blockers.push("model_hash_missing".into());
        }
        if license_receipt.is_none() {
            blockers.push("model_license_receipt_missing".into());
        }
        Self {
            family,
            backend_id: backend_id.into(),
            executable_or_model,
            model_sha256,
            license_receipt,
            state,
            closure_level: if exists && model_ready {
                ClosureLevel::Reachable
            } else {
                ClosureLevel::Typed
            },
            blockers,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BackendRegistry {
    entries: BTreeMap<BackendFamily, BackendDescriptor>,
}

impl BackendRegistry {
    pub fn register(&mut self, descriptor: BackendDescriptor) -> Result<(), MediaError> {
        if self.entries.insert(descriptor.family, descriptor).is_some() {
            return Err(MediaError::DuplicateBackendFamily);
        }
        Ok(())
    }

    pub fn entries(&self) -> impl Iterator<Item = &BackendDescriptor> {
        self.entries.values()
    }

    pub fn family_state(&self, family: BackendFamily) -> BackendState {
        self.entries
            .get(&family)
            .map(|entry| entry.state)
            .unwrap_or(BackendState::Unavailable)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RationalTime {
    pub ticks: i64,
    pub timescale: u32,
}

impl RationalTime {
    pub fn checked(ticks: i64, timescale: u32) -> Result<Self, MediaError> {
        if timescale == 0 {
            return Err(MediaError::ZeroTimescale);
        }
        Ok(Self { ticks, timescale })
    }

    pub fn compare(&self, other: &Self) -> std::cmp::Ordering {
        let left = i128::from(self.ticks) * i128::from(other.timescale);
        let right = i128::from(other.ticks) * i128::from(self.timescale);
        left.cmp(&right)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TimeSpan {
    pub start: RationalTime,
    pub end: RationalTime,
}

impl TimeSpan {
    pub fn checked(start: RationalTime, end: RationalTime) -> Result<Self, MediaError> {
        if start.compare(&end).is_gt() {
            return Err(MediaError::NegativeTimeSpan);
        }
        Ok(Self { start, end })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    ShotBoundary,
    ObjectBox,
    ObjectMask,
    AnonymousPersonTrack,
    PoseLandmarks,
    FaceLandmarks,
    FacialActionUnit,
    ActionCandidate,
    OcrTextCandidate,
    RelativeDepthCandidate,
    MotionCandidate,
    TranscriptCandidate,
    AnonymousSpeakerSegment,
    AudioEventCandidate,
    CrossModalRelationCandidate,
    FaceRegionCandidate,
    AcousticClusterCandidate,
    TemporalCoPresenceCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaObservation {
    pub observation_id: String,
    pub kind: ObservationKind,
    pub span: TimeSpan,
    pub track_id: Option<u64>,
    pub label: String,
    pub backend_id: String,
    pub model_revision: String,
    pub evidence_digest: String,
    pub source_fragment_digest: String,
    pub provenance_digest: String,
    pub configuration_digest: String,
    pub candidate_only: bool,
    pub identity_committed: bool,
    pub internal_state_committed: bool,
}

impl MediaObservation {
    pub fn validate(&self) -> Result<(), MediaError> {
        if self.identity_committed {
            return Err(MediaError::IdentityPromotionForbidden);
        }
        if self.internal_state_committed {
            return Err(MediaError::InternalStatePromotionForbidden);
        }
        if !is_sha256(&self.evidence_digest)
            || !is_sha256(&self.source_fragment_digest)
            || !is_sha256(&self.provenance_digest)
            || !is_sha256(&self.configuration_digest)
        {
            return Err(MediaError::EvidenceDigestMissing);
        }
        Ok(())
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MultimodalEventGraph {
    observations: Vec<MediaObservation>,
    ids: BTreeSet<String>,
}

impl MultimodalEventGraph {
    pub fn add(&mut self, observation: MediaObservation) -> Result<(), MediaError> {
        observation.validate()?;
        if !self.ids.insert(observation.observation_id.clone()) {
            return Err(MediaError::DuplicateObservationId);
        }
        self.observations.push(observation);
        self.observations.sort_by(|left, right| {
            left.span
                .start
                .compare(&right.span.start)
                .then_with(|| left.observation_id.cmp(&right.observation_id))
        });
        Ok(())
    }

    pub fn observations(&self) -> &[MediaObservation] {
        &self.observations
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaAnalysisGate {
    pub source_authorized: bool,
    pub backend_states: BTreeMap<BackendFamily, BackendState>,
    pub highest_closure: ClosureLevel,
    pub output_allowed: bool,
    pub blockers: Vec<String>,
}

pub fn build_media_gate(
    authority: &MediaSourceAuthority,
    registry: &BackendRegistry,
) -> MediaAnalysisGate {
    let backend_states = registry
        .entries()
        .map(|entry| (entry.family, entry.state))
        .collect::<BTreeMap<_, _>>();
    let any_executed = backend_states.values().any(|state| {
        matches!(
            state,
            BackendState::ExecutedCandidate | BackendState::ValidatedObservation
        )
    });
    let mut blockers = authority.blockers.clone();
    if !any_executed && !authority.metadata_only {
        blockers.push("no_media_backend_executed".into());
    }
    let output_allowed = authority.authorized && (authority.metadata_only || any_executed);
    MediaAnalysisGate {
        source_authorized: authority.authorized,
        backend_states,
        highest_closure: if output_allowed && any_executed {
            ClosureLevel::Executed
        } else if authority.authorized {
            ClosureLevel::Reachable
        } else {
            ClosureLevel::Typed
        },
        output_allowed,
        blockers,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaBackendConfig {
    pub python_executable: PathBuf,
    pub backend_script: PathBuf,
    pub ffmpeg_executable: PathBuf,
    pub model_cache: PathBuf,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaResourceBudget {
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_temporary_bytes: u64,
    pub max_decoded_frames: usize,
    pub max_pcm_samples: u64,
    pub max_observations: usize,
}

impl Default for MediaResourceBudget {
    fn default() -> Self {
        Self {
            max_stdout_bytes: 16 * 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            max_temporary_bytes: 8 * 1024 * 1024 * 1024,
            max_decoded_frames: 120,
            max_pcm_samples: 16_000 * 60 * 60,
            max_observations: MAX_MEDIA_OBSERVATIONS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaExecutionPlan {
    pub source_snapshot_sha256: String,
    pub source_bytes: u64,
    pub python_sha256: String,
    pub backend_script_sha256: String,
    pub ffmpeg_sha256: String,
    pub model_cache_manifest_sha256: String,
    pub configuration_digest: String,
    pub resource_budget: MediaResourceBudget,
    pub network_policy: String,
    pub network_isolation_os_enforced: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaProcessReceipt {
    pub exit_code: Option<i32>,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub process_tree_kill_attempted: bool,
    pub process_tree_kill_verified: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct RawBackendReceipt {
    schema_version: String,
    source_sha256: String,
    backend_id: String,
    backend_version: String,
    model_revision: String,
    model_cache_manifest_sha256: String,
    model_card_license_hint: String,
    executed_families: Vec<String>,
    observations: Vec<RawMediaObservation>,
    diagnostics: Vec<String>,
    #[serde(default)]
    executed_methods: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
struct RawMediaObservation {
    observation_id: String,
    kind: String,
    start_ms: i64,
    end_ms: i64,
    track_id: Option<u64>,
    label: String,
    evidence_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaBackendExecutionReceipt {
    pub schema_version: &'static str,
    pub source_sha256: String,
    pub backend_id: String,
    pub backend_version: String,
    pub model_revision: String,
    pub model_cache_manifest_sha256: String,
    pub model_card_license_hint: String,
    pub backend_receipt_sha256: String,
    pub execution_plan: MediaExecutionPlan,
    pub process: MediaProcessReceipt,
    pub executed_family_states: BTreeMap<BackendFamily, BackendState>,
    pub graph: MultimodalEventGraph,
    pub diagnostics: Vec<String>,
    pub source_authority_preserved: bool,
    pub identity_inference_committed: bool,
    pub internal_state_committed: bool,
    pub family_validation_count: usize,
    pub authenticity_receipt_digest: Option<String>,
    pub authenticity_attestation: Option<UntrustedReceipt>,
    pub translation_envelope: CanonicalTranslationEnvelope,
    pub claim_boundary: &'static str,
}

pub fn execute_local_media_backend(
    authority: &MediaSourceAuthority,
    config: &MediaBackendConfig,
) -> Result<MediaBackendExecutionReceipt, MediaError> {
    if !authority.authorized || authority.metadata_only {
        return Err(MediaError::SourceAuthorityDenied);
    }
    let _ = config;
    Err(MediaError::LegacyBackendPathRejected)
}

#[allow(clippy::too_many_arguments)]
pub fn execute_local_media_backend_attested(
    authority: &MediaSourceAuthority,
    descriptor: &BackendDescriptor,
    config: &MediaBackendConfig,
    budget: &MediaResourceBudget,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<MediaBackendExecutionReceipt, MediaError> {
    if !authority.authorized || authority.metadata_only {
        return Err(MediaError::SourceAuthorityDenied);
    }
    if !authority.attestation_verified() {
        return Err(MediaError::SourceReceiptRejected);
    }
    let source = authority
        .local_path
        .as_ref()
        .ok_or(MediaError::SourceAuthorityDenied)?;
    let metadata = source
        .metadata()
        .map_err(|error| MediaError::Io(error.to_string()))?;
    if metadata.len() == 0 || metadata.len() > MAX_MEDIA_BYTES {
        return Err(MediaError::MediaBudgetExceeded);
    }
    for required in [
        &config.python_executable,
        &config.backend_script,
        &config.ffmpeg_executable,
    ] {
        if !required.is_file() {
            return Err(MediaError::BackendUnavailable(
                required.display().to_string(),
            ));
        }
    }
    if config.timeout_seconds == 0 || config.timeout_seconds > 1_800 {
        return Err(MediaError::InvalidTimeout);
    }
    let source_sha256 = sha256_file(source)?;
    if authority.source_snapshot_sha256.as_deref() != Some(source_sha256.as_str())
        || authority.source_bytes != Some(metadata.len())
    {
        return Err(MediaError::SourceSnapshotChanged);
    }
    if !config.model_cache.is_dir() {
        return Err(MediaError::BackendUnavailable(
            config.model_cache.display().to_string(),
        ));
    }
    if budget.max_observations == 0
        || budget.max_observations > MAX_MEDIA_OBSERVATIONS
        || budget.max_decoded_frames == 0
        || budget.max_pcm_samples == 0
        || budget.max_stdout_bytes == 0
        || budget.max_stderr_bytes == 0
    {
        return Err(MediaError::MediaBudgetExceeded);
    }
    let python_sha256 = sha256_file(&config.python_executable)?;
    let backend_script_sha256 = sha256_file(&config.backend_script)?;
    let ffmpeg_sha256 = sha256_file(&config.ffmpeg_executable)?;
    let model_cache_manifest_sha256 = sha256_directory(&config.model_cache)?;
    if descriptor.state != BackendState::ConfiguredNotExecuted
        || descriptor.backend_id.trim().is_empty()
        || descriptor.model_sha256.as_deref() != Some(model_cache_manifest_sha256.as_str())
        || descriptor
            .license_receipt
            .as_deref()
            .is_none_or(str::is_empty)
    {
        return Err(MediaError::BackendDescriptorMismatch);
    }
    let configuration_digest = stable_sha256(&format!(
        "{}\0{}\0{}\0{}\0{}\0{:?}",
        python_sha256,
        backend_script_sha256,
        ffmpeg_sha256,
        model_cache_manifest_sha256,
        config.timeout_seconds,
        budget
    ));
    let execution_plan = MediaExecutionPlan {
        source_snapshot_sha256: source_sha256.clone(),
        source_bytes: metadata.len(),
        python_sha256,
        backend_script_sha256,
        ffmpeg_sha256,
        model_cache_manifest_sha256: model_cache_manifest_sha256.clone(),
        configuration_digest: configuration_digest.clone(),
        resource_budget: budget.clone(),
        network_policy: "offline_environment_and_local_model_path; os_network_isolation_unobserved"
            .into(),
        network_isolation_os_enforced: false,
    };
    let mut command = Command::new(&config.python_executable);
    command
        .arg(&config.backend_script)
        .arg("--media")
        .arg(source)
        .arg("--ffmpeg")
        .arg(&config.ffmpeg_executable)
        .arg("--model-cache")
        .arg(&config.model_cache)
        .arg("--mode")
        .arg("all")
        .arg("--max-temp-bytes")
        .arg(budget.max_temporary_bytes.to_string())
        .arg("--max-frames")
        .arg(budget.max_decoded_frames.to_string())
        .arg("--max-pcm-samples")
        .arg(budget.max_pcm_samples.to_string())
        .arg("--max-observations")
        .arg(budget.max_observations.to_string())
        .env("PYTHONUTF8", "1")
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .env("NO_PROXY", "*")
        .env("no_proxy", "*")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_media_process(
        command,
        Duration::from_secs(config.timeout_seconds),
        budget.max_stdout_bytes,
        budget.max_stderr_bytes,
    )?;
    if !output.success {
        return Err(MediaError::BackendFailed(truncate_diagnostic(
            &String::from_utf8_lossy(&output.stderr),
        )));
    }
    if output.stdout_truncated || output.stderr_truncated {
        return Err(MediaError::MediaBudgetExceeded);
    }
    let raw: RawBackendReceipt = serde_json::from_slice(&output.stdout)
        .map_err(|error| MediaError::InvalidBackendJson(error.to_string()))?;
    if raw.schema_version != MEDIA_BACKEND_SCHEMA || raw.source_sha256 != source_sha256 {
        return Err(MediaError::BackendReceiptBindingMismatch);
    }
    if raw.model_cache_manifest_sha256 != model_cache_manifest_sha256
        || sha256_directory(&config.model_cache)? != model_cache_manifest_sha256
        || raw.backend_id != descriptor.backend_id
    {
        return Err(MediaError::BackendReceiptBindingMismatch);
    }
    if raw.observations.len() > budget.max_observations {
        return Err(MediaError::MediaBudgetExceeded);
    }
    let mut graph = MultimodalEventGraph::default();
    for observation in raw.observations {
        if observation.label.len() > 4_096
            || observation.start_ms < 0
            || observation.end_ms < observation.start_ms
        {
            return Err(MediaError::InvalidBackendObservation);
        }
        let start = RationalTime::checked(observation.start_ms, 1_000)?;
        let end = RationalTime::checked(observation.end_ms, 1_000)?;
        let kind = parse_observation_kind(&observation.kind)?;
        let source_fragment_digest = stable_sha256(&format!(
            "{}:{}:{}",
            source_sha256, observation.start_ms, observation.end_ms
        ));
        let provenance_digest = stable_sha256(&format!(
            "{}\0{}\0{:?}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            source_fragment_digest,
            observation.observation_id,
            kind,
            observation.label,
            raw.backend_id,
            raw.backend_version,
            raw.model_revision,
            model_cache_manifest_sha256,
            configuration_digest,
            observation.evidence_digest,
        ));
        graph.add(MediaObservation {
            observation_id: observation.observation_id,
            kind,
            span: TimeSpan::checked(start, end)?,
            track_id: observation.track_id,
            label: observation.label,
            backend_id: raw.backend_id.clone(),
            model_revision: raw.model_revision.clone(),
            evidence_digest: provenance_digest.clone(),
            source_fragment_digest,
            provenance_digest,
            configuration_digest: configuration_digest.clone(),
            candidate_only: true,
            identity_committed: false,
            internal_state_committed: false,
        })?;
    }
    let executed = raw
        .executed_families
        .iter()
        .map(|family| parse_backend_family(family))
        .collect::<Result<BTreeSet<_>, _>>()?;
    for family in &executed {
        let key = backend_family_wire_name(*family);
        let actual = raw
            .executed_methods
            .get(key)
            .ok_or(MediaError::BackendMethodMismatch)?;
        if expected_executed_method(*family) != Some(actual.as_str()) {
            return Err(MediaError::BackendMethodMismatch);
        }
    }
    let executed_family_states = BackendFamily::all()
        .into_iter()
        .map(|family| {
            (
                family,
                if executed.contains(&family) {
                    BackendState::ExecutedCandidate
                } else {
                    BackendState::Unavailable
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let translation_envelope =
        build_media_translation_envelope(&source_sha256, &graph, &execution_plan)?;
    let mut receipt = MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: source_sha256.clone(),
        backend_id: raw.backend_id,
        backend_version: raw.backend_version,
        model_revision: raw.model_revision,
        model_cache_manifest_sha256: raw.model_cache_manifest_sha256,
        model_card_license_hint: raw.model_card_license_hint,
        backend_receipt_sha256: sha256_bytes(&output.stdout),
        execution_plan,
        process: MediaProcessReceipt {
            exit_code: output.exit_code,
            stdout_digest: sha256_bytes(&output.stdout),
            stderr_digest: sha256_bytes(&output.stderr),
            stdout_truncated: output.stdout_truncated,
            stderr_truncated: output.stderr_truncated,
            process_tree_kill_attempted: output.process_tree_kill_attempted,
            process_tree_kill_verified: output.process_tree_kill_verified,
        },
        executed_family_states,
        graph,
        diagnostics: raw.diagnostics,
        source_authority_preserved: true,
        identity_inference_committed: false,
        internal_state_committed: false,
        family_validation_count: 0,
        authenticity_receipt_digest: None,
        authenticity_attestation: None,
        translation_envelope,
        claim_boundary: "authenticated backend execution yields provenance-bound candidates only; family semantics remain unvalidated until independent method receipts exist, and identity, emotion, intent, transcript perfection, or diarization accuracy are not established",
    };
    let payload = media_backend_payload_digest(&receipt);
    let subject = SubjectRevision::checked(source_sha256)
        .map_err(|_| MediaError::BackendReceiptBindingMismatch)?;
    let scope = ReceiptScope::checked(format!("media/backend/{}", receipt.backend_id))
        .map_err(|_| MediaError::BackendReceiptBindingMismatch)?;
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            now_epoch,
            None,
            authority.attestation_receipt_digest.clone(),
        )
        .map_err(|_| MediaError::BackendReceiptBindingMismatch)?;
    let verified = verifier
        .verify(
            wire,
            &ReceiptPolicy::exact(
                ReceiptClass::MediaBackend,
                subject,
                scope,
                payload,
                now_epoch,
            )
            .with_parent(
                authority
                    .attestation_receipt_digest
                    .clone()
                    .ok_or(MediaError::SourceReceiptRejected)?,
            ),
            replay,
        )
        .map_err(|_| MediaError::BackendReceiptBindingMismatch)?;
    receipt.authenticity_receipt_digest = Some(verified.receipt_digest());
    receipt.authenticity_attestation = Some(verified.into_untrusted());
    Ok(receipt)
}

pub fn media_backend_payload_digest(receipt: &MediaBackendExecutionReceipt) -> String {
    let family_basis = receipt
        .executed_family_states
        .iter()
        .map(|(family, state)| format!("{family:?}:{state:?}"))
        .collect::<Vec<_>>()
        .join("\u{1f}");
    let observation_basis = receipt
        .graph
        .observations()
        .iter()
        .map(|observation| observation.provenance_digest.as_str())
        .collect::<Vec<_>>()
        .join("\u{1f}");
    stable_sha256(&format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}",
        receipt.source_sha256,
        receipt.backend_id,
        receipt.backend_version,
        receipt.model_cache_manifest_sha256,
        receipt.execution_plan.configuration_digest,
        family_basis,
        observation_basis
    ))
}

fn build_media_translation_envelope(
    source_sha256: &str,
    observations: &MultimodalEventGraph,
    plan: &MediaExecutionPlan,
) -> Result<CanonicalTranslationEnvelope, MediaError> {
    let source = format!("media-source-snapshot:{source_sha256}");
    let mut projection =
        ProjectionDefectGraphV3::new(SourceLedgerId(631_021), source_sha256, &source)
            .map_err(|_| MediaError::TranslationEnvelopeFailed)?;
    projection
        .add_obligation(Obligation {
            id: ObligationId(631_021),
            kind: ObligationKind::Evidence,
            strength: ObligationStrength::Must,
            polarity: ObligationPolarity::Positive,
            scope: "media-observation-provenance".into(),
            source_span: SourceSpan::checked(&source, 0, source.len())
                .map_err(|_| MediaError::TranslationEnvelopeFailed)?,
            source_text: source.clone(),
        })
        .map_err(|_| MediaError::TranslationEnvelopeFailed)?;
    let anchor = projection
        .obligation_anchor(ObligationId(631_021))
        .cloned()
        .ok_or(MediaError::TranslationEnvelopeFailed)?;
    for (index, observation) in observations.observations().iter().enumerate() {
        let target_id = TargetClaimId(index as u64 + 1);
        projection
            .add_target_claim(
                TargetClaim::checked(
                    target_id,
                    "lc631-media-observation",
                    observation.observation_id.clone(),
                    observation.provenance_digest.clone(),
                    &observation.label,
                )
                .map_err(|_| MediaError::TranslationEnvelopeFailed)?,
            )
            .map_err(|_| MediaError::TranslationEnvelopeFailed)?;
        let stage = if observation.kind == ObservationKind::TranscriptCandidate {
            ProjectionStage::AudioToTranscript
        } else {
            ProjectionStage::FrameToObservation
        };
        projection
            .add_edge(ProjectionEdgeV3 {
                id: ProjectionEdgeId(index as u64 + 1),
                obligation_id: Some(ObligationId(631_021)),
                source: Some(anchor.clone()),
                targets: vec![target_id],
                stage,
                state: PreservationState::Unresolved,
                rule_id: "media-candidate-provenance.v1".into(),
                verifier: VerificationReceipt {
                    verifier: VerifierKind::RuntimeObservation,
                    status: VerificationStatus::NeedsEvidence,
                    revision: "media-method-validation-unbound.v1".into(),
                    evidence_digest: Some(observation.provenance_digest.clone()),
                },
            })
            .map_err(|_| MediaError::TranslationEnvelopeFailed)?;
    }
    Ok(CanonicalTranslationEnvelope::from_graph(
        &projection,
        vec![DomainTranslationPayload {
            domain: "media".into(),
            schema_version: MEDIA_BACKEND_SCHEMA.into(),
            payload_digest: stable_sha256(&format!(
                "{}:{}",
                plan.configuration_digest, plan.model_cache_manifest_sha256
            )),
            unknowns: vec![
                "method-level semantic validation and cross-modal truth remain unbound".into(),
            ],
        }],
    ))
}

impl BackendFamily {
    fn all() -> [Self; 16] {
        [
            Self::Decode,
            Self::ShotBoundary,
            Self::ObjectDetection,
            Self::SegmentationTracking,
            Self::Pose,
            Self::FaceActionUnit,
            Self::ActionRecognition,
            Self::Ocr,
            Self::DepthMotion,
            Self::Asr,
            Self::Diarization,
            Self::AudioEvent,
            Self::CrossModalAlignment,
            Self::FrameDifference,
            Self::AcousticClustering,
            Self::TemporalCoPresence,
        ]
    }
}

fn parse_backend_family(value: &str) -> Result<BackendFamily, MediaError> {
    match value {
        "decode" => Ok(BackendFamily::Decode),
        "shot_boundary" => Ok(BackendFamily::ShotBoundary),
        "object_detection" => Ok(BackendFamily::ObjectDetection),
        "segmentation_tracking" => Ok(BackendFamily::SegmentationTracking),
        "pose" => Ok(BackendFamily::Pose),
        "face_action_unit" => Ok(BackendFamily::FaceActionUnit),
        "action_recognition" => Ok(BackendFamily::ActionRecognition),
        "ocr" => Ok(BackendFamily::Ocr),
        "depth_motion" => Ok(BackendFamily::DepthMotion),
        "asr" => Ok(BackendFamily::Asr),
        "diarization" => Ok(BackendFamily::Diarization),
        "audio_event" => Ok(BackendFamily::AudioEvent),
        "cross_modal_alignment" => Ok(BackendFamily::CrossModalAlignment),
        "frame_difference" => Ok(BackendFamily::FrameDifference),
        "acoustic_clustering" => Ok(BackendFamily::AcousticClustering),
        "temporal_co_presence" => Ok(BackendFamily::TemporalCoPresence),
        _ => Err(MediaError::UnknownBackendFamily(value.to_string())),
    }
}

fn backend_family_wire_name(family: BackendFamily) -> &'static str {
    match family {
        BackendFamily::Decode => "decode",
        BackendFamily::ShotBoundary => "shot_boundary",
        BackendFamily::ObjectDetection => "object_detection",
        BackendFamily::SegmentationTracking => "segmentation_tracking",
        BackendFamily::Pose => "pose",
        BackendFamily::FaceActionUnit => "face_action_unit",
        BackendFamily::ActionRecognition => "action_recognition",
        BackendFamily::Ocr => "ocr",
        BackendFamily::DepthMotion => "depth_motion",
        BackendFamily::Asr => "asr",
        BackendFamily::Diarization => "diarization",
        BackendFamily::AudioEvent => "audio_event",
        BackendFamily::CrossModalAlignment => "cross_modal_alignment",
        BackendFamily::FrameDifference => "frame_difference",
        BackendFamily::AcousticClustering => "acoustic_clustering",
        BackendFamily::TemporalCoPresence => "temporal_co_presence",
    }
}

fn expected_executed_method(family: BackendFamily) -> Option<&'static str> {
    match family {
        BackendFamily::Decode => Some("ffmpeg-cli"),
        BackendFamily::ShotBoundary => Some("opencv-hsv-histogram-bhattacharyya"),
        BackendFamily::ObjectDetection => Some("opencv-hog-default-people-detector"),
        BackendFamily::FrameDifference => Some("opencv-mean-absolute-frame-difference"),
        BackendFamily::Asr => Some("faster-whisper-local-cache"),
        BackendFamily::AcousticClustering => Some("pcm-feature-two-cluster"),
        BackendFamily::TemporalCoPresence => Some("timestamp-overlap-only"),
        _ => None,
    }
}

fn parse_observation_kind(value: &str) -> Result<ObservationKind, MediaError> {
    match value {
        "shot_boundary" => Ok(ObservationKind::ShotBoundary),
        "object_box" => Ok(ObservationKind::ObjectBox),
        "object_mask" => Ok(ObservationKind::ObjectMask),
        "anonymous_person_track" => Ok(ObservationKind::AnonymousPersonTrack),
        "pose_landmarks" => Ok(ObservationKind::PoseLandmarks),
        "face_landmarks" => Ok(ObservationKind::FaceLandmarks),
        "facial_action_unit" => Ok(ObservationKind::FacialActionUnit),
        "action_candidate" => Ok(ObservationKind::ActionCandidate),
        "ocr_text_candidate" => Ok(ObservationKind::OcrTextCandidate),
        "relative_depth_candidate" => Ok(ObservationKind::RelativeDepthCandidate),
        "motion_candidate" => Ok(ObservationKind::MotionCandidate),
        "transcript_candidate" => Ok(ObservationKind::TranscriptCandidate),
        "anonymous_speaker_segment" => Ok(ObservationKind::AnonymousSpeakerSegment),
        "audio_event_candidate" => Ok(ObservationKind::AudioEventCandidate),
        "cross_modal_relation_candidate" => Ok(ObservationKind::CrossModalRelationCandidate),
        "face_region_candidate" => Ok(ObservationKind::FaceRegionCandidate),
        "acoustic_cluster_candidate" => Ok(ObservationKind::AcousticClusterCandidate),
        "temporal_co_presence_candidate" => Ok(ObservationKind::TemporalCoPresenceCandidate),
        _ => Err(MediaError::UnknownObservationKind(value.to_string())),
    }
}

struct MediaProcessOutput {
    success: bool,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
    process_tree_kill_attempted: bool,
    process_tree_kill_verified: bool,
}

fn run_media_process(
    mut command: Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<MediaProcessOutput, MediaError> {
    let mut child = command
        .spawn()
        .map_err(|error| MediaError::BackendUnavailable(error.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| MediaError::Io("backend_stdout_unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| MediaError::Io("backend_stderr_unavailable".into()))?;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, stdout_limit));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, stderr_limit));
    let started = Instant::now();
    let status = loop {
        match child
            .try_wait()
            .map_err(|error| MediaError::Io(error.to_string()))?
        {
            Some(status) => break status,
            None if started.elapsed() > timeout => {
                let _kill_verified = kill_media_process_tree(&mut child);
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(MediaError::BackendTimeout);
            }
            None => thread::sleep(Duration::from_millis(25)),
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| MediaError::Io("stdout_reader_panicked".into()))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| MediaError::Io("stderr_reader_panicked".into()))?;
    Ok(MediaProcessOutput {
        success: status.success(),
        exit_code: status.code(),
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        process_tree_kill_attempted: false,
        process_tree_kill_verified: true,
    })
}

#[derive(Default)]
struct DrainedBytes {
    bytes: Vec<u8>,
    truncated: bool,
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> DrainedBytes {
    let mut output = DrainedBytes::default();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let remaining = limit.saturating_sub(output.bytes.len());
        let keep = remaining.min(read);
        output.bytes.extend_from_slice(&buffer[..keep]);
        output.truncated |= keep < read;
    }
    output
}

fn kill_media_process_tree(child: &mut Child) -> bool {
    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
    #[cfg(not(windows))]
    {
        Command::new("pkill")
            .args(["-TERM", "-P", &child.id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

fn sha256_directory(root: &Path) -> Result<String, MediaError> {
    let mut entries = Vec::new();
    collect_directory_entries(root, root, &mut entries)?;
    entries.sort();
    Ok(stable_sha256(&format!(
        "lc631-directory-manifest.v2\0{}",
        entries.join("\u{1f}")
    )))
}

pub fn model_cache_manifest_sha256(root: &Path) -> Result<String, MediaError> {
    sha256_directory(root)
}

fn collect_directory_entries(
    root: &Path,
    current: &Path,
    entries: &mut Vec<String>,
) -> Result<(), MediaError> {
    if entries.len() > 100_000 {
        return Err(MediaError::MediaBudgetExceeded);
    }
    let mut children = std::fs::read_dir(current)
        .map_err(|error| MediaError::Io(error.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| MediaError::Io(error.to_string()))?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|error| MediaError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(MediaError::BackendDescriptorMismatch);
        }
        if metadata.is_dir() {
            collect_directory_entries(root, &path, entries)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| MediaError::BackendDescriptorMismatch)?
                .to_string_lossy()
                .replace('\\', "/");
            entries.push(format!(
                "{}:{}:{}",
                relative,
                metadata.len(),
                sha256_file(&path)?
            ));
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, MediaError> {
    let mut file = File::open(path).map_err(|error| MediaError::Io(error.to_string()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| MediaError::Io(error.to_string()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format_digest(digest.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format_digest(Sha256::digest(bytes))
}

fn format_digest(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(7 + bytes.len() * 2);
    output.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn truncate_diagnostic(value: &str) -> String {
    value.chars().take(4_096).collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum MediaError {
    BackendFailed(String),
    BackendDescriptorMismatch,
    BackendMethodMismatch,
    BackendReceiptBindingMismatch,
    BackendTimeout,
    BackendUnavailable(String),
    DuplicateBackendFamily,
    DuplicateObservationId,
    EvidenceDigestMissing,
    IdentityPromotionForbidden,
    InvalidBackendJson(String),
    InvalidBackendObservation,
    InvalidTimeout,
    InternalStatePromotionForbidden,
    Io(String),
    LegacyBackendPathRejected,
    MediaBudgetExceeded,
    NegativeTimeSpan,
    SourceAuthorityDenied,
    SourceReceiptRejected,
    SourceSnapshotUnavailable,
    SourceSnapshotChanged,
    TranslationEnvelopeFailed,
    UnknownBackendFamily(String),
    UnknownObservationKind(String),
    ZeroTimescale,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_url_alone_cannot_authorize_media_bytes() {
        let authority = authorize_media_source(&MediaSourceRequest {
            kind: MediaSourceKind::YoutubeUrlUnacquired,
            locator: "https://www.youtube.com/watch?v=example".into(),
            user_asserts_rights: true,
            media_bytes_requested: true,
            network_requested: true,
        });
        assert!(!authority.authorized);
        assert!(authority
            .blockers
            .contains(&"youtube_url_alone_does_not_authorize_media_download".to_string()));
    }

    #[test]
    fn face_action_unit_cannot_commit_emotion_or_identity() {
        let time = RationalTime::checked(0, 1_000).unwrap();
        let observation = MediaObservation {
            observation_id: "au12".into(),
            kind: ObservationKind::FacialActionUnit,
            span: TimeSpan::checked(time, time).unwrap(),
            track_id: Some(1),
            label: "AU12 candidate".into(),
            backend_id: "libreface".into(),
            model_revision: "unconfigured".into(),
            evidence_digest: stable_sha256("au12"),
            source_fragment_digest: stable_sha256("frame-0"),
            provenance_digest: stable_sha256("au12-provenance"),
            configuration_digest: stable_sha256("config"),
            candidate_only: true,
            identity_committed: false,
            internal_state_committed: true,
        };
        assert_eq!(
            observation.validate(),
            Err(MediaError::InternalStatePromotionForbidden)
        );
    }

    #[test]
    fn backend_registry_reports_unavailable_without_model_and_license() {
        let descriptor =
            BackendDescriptor::configured(BackendFamily::Asr, "whisper", None, None, None);
        assert_eq!(descriptor.state, BackendState::Unavailable);
        assert_eq!(descriptor.closure_level, ClosureLevel::Typed);
    }

    #[test]
    fn method_catalog_covers_every_backend_family_without_bundling_models() {
        let catalog = default_method_catalog();
        let families = catalog
            .iter()
            .map(|item| item.family)
            .collect::<BTreeSet<_>>();
        assert_eq!(families.len(), 16);
        assert!(catalog
            .iter()
            .all(|item| item.backend_required && !item.bundled));
        assert!(catalog
            .iter()
            .all(|item| !item.expected_output.contains("identity")));
    }

    #[test]
    fn process_backend_cannot_run_from_metadata_only_authority() {
        let authority = authorize_media_source(&MediaSourceRequest {
            kind: MediaSourceKind::YoutubeMetadataOnly,
            locator: "metadata".into(),
            user_asserts_rights: false,
            media_bytes_requested: false,
            network_requested: false,
        });
        let result = execute_local_media_backend(
            &authority,
            &MediaBackendConfig {
                python_executable: PathBuf::from("missing-python"),
                backend_script: PathBuf::from("missing-backend"),
                ffmpeg_executable: PathBuf::from("missing-ffmpeg"),
                model_cache: PathBuf::from("missing-cache"),
                timeout_seconds: 1,
            },
        );
        assert_eq!(result, Err(MediaError::SourceAuthorityDenied));
    }
}
