#![forbid(unsafe_code)]

pub mod attestation_v2;
pub mod attestation_v3;
mod model_scan;
mod source_ingress;
pub mod time_boundary;
use model_scan::sha256_file_with_resources;
pub use model_scan::{model_cache_manifest_sha256_bounded, ModelScanLimits};
use source_ingress::{observe_regular_source, RegularSourceObservation};

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
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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
    #[serde(skip)]
    authenticated_source: Option<AuthenticatedMediaSource>,
    #[serde(skip)]
    source_observation: Option<RegularSourceObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthenticatedMediaSource {
    request: MediaSourceRequest,
    path: PathBuf,
    snapshot: String,
    bytes: u64,
    source_digest: String,
    payload_digest: String,
    issuer_fingerprint: String,
    file_identity: lc631_process_supervisor::filesystem::FileIdentity,
    admitted_at: Instant,
    admitted_epoch: u64,
}

impl MediaSourceAuthority {
    pub fn attestation_verified(&self) -> bool {
        self.authorized
            && self.attestation_receipt_digest.is_some()
            && self.source_attestation.is_some()
            && self.authenticated_source.as_ref().is_some_and(|binding| {
                self.local_path.as_ref() == Some(&binding.path)
                    && self.source_snapshot_sha256.as_ref() == Some(&binding.snapshot)
                    && self.source_bytes == Some(binding.bytes)
                    && self.source_digest == binding.source_digest
                    && !self.metadata_only
            })
    }

    fn elapsed_anchor(&self, supplied: u64) -> Result<(u64, Instant), MediaError> {
        let binding = self
            .authenticated_source
            .as_ref()
            .ok_or(MediaError::SourceReceiptRejected)?;
        let elapsed = binding.admitted_at.elapsed().as_secs();
        // A fresher trusted epoch may raise the origin; retain the same Instant
        // so fractional preparation time is never discarded at dispatch.
        let origin = binding.admitted_epoch.max(supplied.saturating_sub(elapsed));
        origin
            .checked_add(elapsed)
            .ok_or(MediaError::SourceReceiptRejected)?;
        Ok((origin, binding.admitted_at))
    }

    fn current_epoch(&self, supplied: u64) -> Result<u64, MediaError> {
        let (origin, started) = self.elapsed_anchor(supplied)?;
        origin
            .checked_add(started.elapsed().as_secs())
            .ok_or(MediaError::SourceReceiptRejected)
    }

    fn revalidate_source<'a>(
        &self,
        resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
    ) -> Result<RegularSourceObservation, MediaError> {
        let binding = self
            .authenticated_source
            .as_ref()
            .ok_or(MediaError::SourceReceiptRejected)?;
        let observed = observe_regular_source(
            &binding.path,
            Some((&binding.snapshot, binding.bytes)),
            resources,
        )?;
        if observed.identity != binding.file_identity || observed.canonical_path != binding.path {
            return Err(MediaError::SourceSnapshotChanged);
        }
        Ok(observed)
    }
}

pub fn authorize_media_source(request: &MediaSourceRequest) -> MediaSourceAuthority {
    authorize_media_source_read(request, None)
}

fn authorize_media_source_read<'a>(
    request: &MediaSourceRequest,
    resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
) -> MediaSourceAuthority {
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
    // Observe the original locator before canonicalization erases link/occurrence facts.
    let source_observation = local_path.as_deref().and_then(|path| {
        match observe_regular_source(path, None, resources) {
            Ok(observed) => Some(observed),
            Err(error) => {
                blockers.push(format!("source_snapshot_unavailable:{error:?}"));
                None
            }
        }
    });
    let local_path = source_observation
        .as_ref()
        .map(|observed| observed.canonical_path.clone());
    let source_digest = stable_sha256(&format!(
        "media-source-locator.v2\0{:?}\0{}\0{}",
        request.kind,
        request.locator,
        local_path
            .as_ref()
            .and_then(|path| path.to_str())
            .unwrap_or("metadata-only")
    ));
    let source_snapshot_sha256 = source_observation
        .as_ref()
        .map(|observed| observed.sha256.clone());
    let source_bytes = source_observation.as_ref().map(|observed| observed.bytes);
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
        authenticated_source: None,
        source_observation,
    }
}

pub fn authorize_media_source_attested(
    request: &MediaSourceRequest,
    attestation: UntrustedReceipt,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<MediaSourceAuthority, MediaError> {
    authorize_media_source_attested_read(request, attestation, verifier, replay, now_epoch, None)
}

pub fn authorize_media_source_attested_with_resources<'a>(
    request: &MediaSourceRequest,
    attestation: UntrustedReceipt,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    resources: &'a lc631_process_supervisor::ResourceReservationView<'a>,
) -> Result<MediaSourceAuthority, MediaError> {
    authorize_media_source_attested_read(
        request,
        attestation,
        verifier,
        replay,
        now_epoch,
        Some(resources),
    )
}

fn authorize_media_source_attested_read<'a>(
    request: &MediaSourceRequest,
    attestation: UntrustedReceipt,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
) -> Result<MediaSourceAuthority, MediaError> {
    let admitted_at = Instant::now();
    if let Some(view) = resources {
        view.check()
            .map_err(|error| MediaError::Io(format!("media_source_resource:{error:?}")))?;
    }
    let mut authority = authorize_media_source_read(request, resources);
    if let Some(view) = resources {
        view.check()
            .map_err(|error| MediaError::Io(format!("media_source_resource:{error:?}")))?;
    }
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
                now_epoch
                    .checked_add(admitted_at.elapsed().as_secs())
                    .ok_or(MediaError::SourceReceiptRejected)?,
            ),
            replay,
        )
        .map_err(|_| MediaError::SourceReceiptRejected)?;
    authority.authorized = true;
    authority
        .blockers
        .retain(|blocker| blocker != "authenticated_media_source_receipt_required");
    authority.attestation_receipt_digest = Some(verified.receipt_digest());
    authority.authenticated_source = Some(AuthenticatedMediaSource {
        request: request.clone(),
        path: authority
            .local_path
            .clone()
            .ok_or(MediaError::SourceSnapshotUnavailable)?,
        snapshot: authority
            .source_snapshot_sha256
            .clone()
            .ok_or(MediaError::SourceSnapshotUnavailable)?,
        bytes: authority
            .source_bytes
            .ok_or(MediaError::SourceSnapshotUnavailable)?,
        source_digest: authority.source_digest.clone(),
        payload_digest: media_source_payload_digest(request, &authority)?,
        issuer_fingerprint: verified.verifier_key_fingerprint().into(),
        file_identity: authority
            .source_observation
            .as_ref()
            .ok_or(MediaError::SourceSnapshotUnavailable)?
            .identity,
        admitted_at,
        admitted_epoch: now_epoch,
    });
    authority.source_attestation = Some(verified.into_untrusted());
    Ok(authority)
}

/// Revalidating a source attestation is not consuming a one-shot side effect.
/// The caller supplies trusted current time; host revocation remains a separate
/// host contract rather than being inferred from this cached diagnostic token.
pub fn validate_current_media_authority(
    authority: &MediaSourceAuthority,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
) -> Result<(), MediaError> {
    if !authority.attestation_verified() {
        return Err(MediaError::SourceReceiptRejected);
    }
    let binding = authority
        .authenticated_source
        .as_ref()
        .ok_or(MediaError::SourceReceiptRejected)?;
    if binding.issuer_fingerprint != verifier.key_fingerprint() {
        return Err(MediaError::SourceReceiptRejected);
    }
    let now_epoch = authority.current_epoch(now_epoch)?;
    let verified = verifier
        .verify(
            authority
                .source_attestation
                .clone()
                .ok_or(MediaError::SourceReceiptRejected)?,
            &ReceiptPolicy::exact(
                ReceiptClass::MediaSource,
                SubjectRevision::checked(&binding.snapshot)
                    .map_err(|_| MediaError::SourceReceiptRejected)?,
                ReceiptScope::checked(media_source_scope(binding.request.kind))
                    .map_err(|_| MediaError::SourceReceiptRejected)?,
                &binding.payload_digest,
                now_epoch,
            ),
            &mut ReplayGuard::default(),
        )
        .map_err(|_| MediaError::SourceReceiptRejected)?;
    if authority.attestation_receipt_digest.as_deref() != Some(verified.receipt_digest().as_str()) {
        return Err(MediaError::SourceReceiptRejected);
    }
    Ok(())
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
    FaceDetection,
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
            BackendFamily::FaceDetection,
            "opencv-haar-frontalface",
            "anonymous face-region candidates",
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
        match self.entries.entry(descriptor.family) {
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(MediaError::DuplicateBackendFamily)
            }
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(descriptor);
                Ok(())
            }
        }
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
        // Public RationalTime fields can bypass its constructor.
        if start.timescale == 0 || end.timescale == 0 {
            return Err(MediaError::ZeroTimescale);
        }
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
    /// Classify the current public span without asserting clock provenance.
    pub fn time_extent(
        &self,
    ) -> Result<time_boundary::TemporalExtent, time_boundary::TimeBoundaryError> {
        time_boundary::classify(&self.span)
    }

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
        // Public spans may have changed after construction. Use the same
        // instant/half-open classification exposed to graph consumers.
        self.time_extent().map_err(|error| match error {
            time_boundary::TimeBoundaryError::ZeroTimescale => MediaError::ZeroTimescale,
            time_boundary::TimeBoundaryError::Reversed
            | time_boundary::TimeBoundaryError::EmptyInterval => MediaError::NegativeTimeSpan,
        })?;
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

    /// A typed view of a stored observation, not evidence of clock calibration.
    pub fn time_extent(
        &self,
        index: usize,
    ) -> Result<Option<time_boundary::TemporalExtent>, time_boundary::TimeBoundaryError> {
        self.observations
            .get(index)
            .map(MediaObservation::time_extent)
            .transpose()
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
    pub memory_limit_bytes: Option<usize>,
    pub memory_limit_kind: String,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RawMediaObservation {
    observation_id: String,
    kind: String,
    start_ms: i64,
    end_ms: i64,
    track_id: Option<u64>,
    label: String,
    evidence_digest: String,
    #[serde(default)]
    label_truncated: bool,
    #[serde(default)]
    original_label_bytes: Option<usize>,
    #[serde(default)]
    temporal_overlap: Option<RawTemporalOverlap>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawTemporalOverlap {
    profile: String,
    clock_digest: String,
    transcript_id: String,
    frame_digest: String,
    frame_index: u64,
    frame_start_ms: i64,
    frame_end_ms: i64,
    pcm_end_ms: i64,
}

fn bound_source_fragment(
    observation: &RawMediaObservation,
    by_id: &BTreeMap<&str, &RawMediaObservation>,
    source: &str,
    budget: &MediaResourceBudget,
) -> Result<String, MediaError> {
    if observation.kind != "temporal_co_presence_candidate" {
        return Ok(stable_sha256(&format!(
            "{}:{}:{}",
            source, observation.start_ms, observation.end_ms,
        )));
    }
    let invalid = || MediaError::InvalidBackendObservation;
    let overlap = observation.temporal_overlap.as_ref().ok_or_else(invalid)?;
    let frame_start = overlap
        .frame_index
        .checked_mul(500)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(invalid)?;
    let frame_limit = frame_start.checked_add(500).ok_or_else(invalid)?;
    if overlap.profile != "ffmpeg-shared-origin-fps2-v1"
        || !is_sha256(&overlap.clock_digest)
        || !is_sha256(&overlap.frame_digest)
        || usize::try_from(overlap.frame_index)
            .map_or(true, |index| index >= budget.max_decoded_frames)
        || overlap.frame_start_ms != frame_start
        || overlap.frame_end_ms <= frame_start
        || overlap.frame_end_ms > frame_limit
        || overlap.pcm_end_ms <= 0
        || overlap.pcm_end_ms as u128 > (budget.max_pcm_samples as u128 * 1000 / 16_000)
    {
        return Err(invalid());
    }
    let transcript = by_id
        .get(overlap.transcript_id.as_str())
        .copied()
        .ok_or_else(invalid)?;
    if transcript.kind != "transcript_candidate"
        || transcript.start_ms < 0
        || transcript.end_ms < transcript.start_ms
        || transcript.label.len() > 4096
    {
        return Err(invalid());
    }
    let start = transcript.start_ms.max(frame_start);
    let end = transcript
        .end_ms
        .min(overlap.frame_end_ms)
        .min(overlap.pcm_end_ms);
    if start >= end || observation.start_ms != start || observation.end_ms != end {
        return Err(invalid());
    }
    // Bind the reported frame witness and exact referenced transcript occurrence.
    // The declared clock/profile is not independent calibration or semantic proof.
    let binding =
        serde_json::to_string(&(source, start, end, overlap, transcript)).map_err(|_| invalid())?;
    Ok(stable_sha256(&format!(
        "media-temporal-fragment.v1\0{binding}"
    )))
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
    execute_media_attested_read(
        authority,
        descriptor,
        config,
        budget,
        issuer,
        verifier,
        replay,
        now_epoch,
        None,
        MediaAttestationProfile::JsonV2,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn execute_local_media_backend_attested_with_resources<'a>(
    authority: &MediaSourceAuthority,
    descriptor: &BackendDescriptor,
    config: &MediaBackendConfig,
    budget: &MediaResourceBudget,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    resources: &'a lc631_process_supervisor::ResourceReservationView<'a>,
) -> Result<MediaBackendExecutionReceipt, MediaError> {
    execute_media_attested_read(
        authority,
        descriptor,
        config,
        budget,
        issuer,
        verifier,
        replay,
        now_epoch,
        Some(resources),
        MediaAttestationProfile::JsonV2,
        None,
    )
}

/// Explicit binary-v3 issuance. Existing entrypoints continue to issue v2.
/// Authenticated process output remains candidate data, not model truth.
#[allow(clippy::too_many_arguments)]
pub fn execute_local_media_backend_attested_v3(
    authority: &MediaSourceAuthority,
    descriptor: &BackendDescriptor,
    config: &MediaBackendConfig,
    budget: &MediaResourceBudget,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<MediaBackendExecutionReceipt, MediaError> {
    execute_media_attested_read(
        authority,
        descriptor,
        config,
        budget,
        issuer,
        verifier,
        replay,
        now_epoch,
        None,
        MediaAttestationProfile::BinaryV3,
        None,
    )
}

/// Funded binary-v3 execution and current-source verification in one view.
#[allow(clippy::too_many_arguments)]
pub fn execute_local_media_backend_attested_v3_with_resources<'a>(
    authority: &MediaSourceAuthority,
    descriptor: &BackendDescriptor,
    config: &MediaBackendConfig,
    budget: &MediaResourceBudget,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    resources: &'a lc631_process_supervisor::ResourceReservationView<'a>,
) -> Result<MediaBackendExecutionReceipt, MediaError> {
    execute_media_attested_read(
        authority,
        descriptor,
        config,
        budget,
        issuer,
        verifier,
        replay,
        now_epoch,
        Some(resources),
        MediaAttestationProfile::BinaryV3,
        None,
    )
}

/// Funded v3 execution with a declared configuration checked against the same
/// observed launch plan before any backend child, replay or receipt issuance.
/// The expectation is caller input, not a source or execution authority token.
#[allow(clippy::too_many_arguments)]
pub fn execute_registered_media_backend_v3_with_resources<'a>(
    authority: &MediaSourceAuthority,
    descriptor: &BackendDescriptor,
    config: &MediaBackendConfig,
    budget: &MediaResourceBudget,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    resources: &'a lc631_process_supervisor::ResourceReservationView<'a>,
    expected_configuration: &str,
) -> Result<MediaBackendExecutionReceipt, MediaError> {
    execute_media_attested_read(
        authority,
        descriptor,
        config,
        budget,
        issuer,
        verifier,
        replay,
        now_epoch,
        Some(resources),
        MediaAttestationProfile::BinaryV3,
        Some(expected_configuration),
    )
}

#[derive(Clone, Copy)]
enum MediaAttestationProfile {
    JsonV2,
    BinaryV3,
}

#[allow(clippy::too_many_arguments)]
fn execute_media_attested_read<'a>(
    authority: &MediaSourceAuthority,
    descriptor: &BackendDescriptor,
    config: &MediaBackendConfig,
    budget: &MediaResourceBudget,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
    resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
    attestation_profile: MediaAttestationProfile,
    expected_configuration: Option<&str>,
) -> Result<MediaBackendExecutionReceipt, MediaError> {
    if let Some(view) = resources {
        view.check()
            .map_err(|error| MediaError::Io(format!("media_resource:{error:?}")))?;
    }
    if !authority.authorized || authority.metadata_only {
        return Err(MediaError::SourceAuthorityDenied);
    }
    validate_current_media_authority(authority, verifier, now_epoch)?;
    let (now_epoch, execution_started) = authority.elapsed_anchor(now_epoch)?;
    let source = authority
        .local_path
        .as_ref()
        .ok_or(MediaError::SourceAuthorityDenied)?;
    let observed_source = authority.revalidate_source(resources)?;
    let source_bytes = observed_source.bytes;
    if source_bytes == 0 || source_bytes > MAX_MEDIA_BYTES {
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
    let source_sha256 = observed_source.sha256;
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
    let source_snapshot = MediaSourceSnapshot::capture(
        source,
        &source_sha256,
        MAX_MEDIA_BYTES,
        budget.max_temporary_bytes,
        resources,
    )?;
    let prepared: Result<MediaBackendExecutionReceipt, MediaError> = (|| {
        let decoder_temporary_bytes = budget
            .max_temporary_bytes
            .checked_sub(source_snapshot.bytes())
            .filter(|remaining| *remaining > 0)
            .ok_or(MediaError::MediaBudgetExceeded)?;
        let python_sha256 = sha256_file_with_resources(&config.python_executable, resources)?;
        let backend_script_sha256 = sha256_file_with_resources(&config.backend_script, resources)?;
        let ffmpeg_sha256 = sha256_file_with_resources(&config.ffmpeg_executable, resources)?;
        let model_cache_manifest_sha256 = model_cache_manifest_sha256_bounded(
            &config.model_cache,
            ModelScanLimits::default(),
            resources,
        )?;
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
        // Reuse the registered-script equality boundary, without adding an unrelated runtime API.
        let registered_script = descriptor
            .executable_or_model
            .as_deref()
            .ok_or(MediaError::BackendDescriptorMismatch)?
            .canonicalize()
            .map_err(|_| MediaError::BackendDescriptorMismatch)?;
        let dispatched_script = config
            .backend_script
            .canonicalize()
            .map_err(|_| MediaError::BackendDescriptorMismatch)?;
        if registered_script != dispatched_script {
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
            source_bytes,
            python_sha256,
            backend_script_sha256,
            ffmpeg_sha256,
            model_cache_manifest_sha256: model_cache_manifest_sha256.clone(),
            configuration_digest: configuration_digest.clone(),
            resource_budget: budget.clone(),
            network_policy:
                "offline_environment_and_local_model_path; os_network_isolation_unobserved".into(),
            network_isolation_os_enforced: false,
        };
        if expected_configuration
            .is_some_and(|expected| expected != execution_plan.configuration_digest.as_str())
        {
            return Err(MediaError::RegistrationConfigurationMismatch);
        }
        let mut command = Command::new(&config.python_executable);
        command
            // Per-child prevention, not deletion of shared model/receipt caches.
            .arg("-B")
            .arg(&config.backend_script)
            .arg("--media")
            .arg(source_snapshot.path())
            .arg("--ffmpeg")
            .arg(&config.ffmpeg_executable)
            .arg("--model-cache")
            .arg(&config.model_cache)
            .arg("--mode")
            .arg("all")
            .arg("--max-temp-bytes")
            .arg(decoder_temporary_bytes.to_string())
            .arg("--max-frames")
            .arg(budget.max_decoded_frames.to_string())
            .arg("--max-pcm-samples")
            .arg(budget.max_pcm_samples.to_string())
            .arg("--max-observations")
            .arg(budget.max_observations.to_string())
            .arg("--max-model-entries")
            .arg(ModelScanLimits::default().max_entries.to_string())
            .arg("--max-model-file-bytes")
            .arg(ModelScanLimits::default().max_file_bytes.to_string())
            .arg("--max-model-total-bytes")
            .arg(ModelScanLimits::default().max_total_bytes.to_string())
            .arg("--max-model-metadata-bytes")
            .arg(ModelScanLimits::default().max_metadata_bytes.to_string())
            .env("PYTHONUTF8", "1")
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("NO_PROXY", "*")
            .env("no_proxy", "*")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        authority.revalidate_source(resources)?;
        let launch_epoch = authority.current_epoch(
            now_epoch
                .checked_add(execution_started.elapsed().as_secs())
                .ok_or(MediaError::SourceReceiptRejected)?,
        )?;
        validate_current_media_authority(authority, verifier, launch_epoch)?;
        let remaining_authority_seconds = authority
            .source_attestation
            .as_ref()
            .and_then(|receipt| receipt.claims().expires_at_epoch())
            .map_or(config.timeout_seconds, |expiry| {
                expiry.saturating_sub(launch_epoch)
            });
        let output = run_media_process(
            command,
            Duration::from_secs(config.timeout_seconds.min(remaining_authority_seconds)),
            budget.max_stdout_bytes,
            budget.max_stderr_bytes,
            resources,
        )?;
        let process_output = output.outcome();
        validate_current_media_authority(
            authority,
            verifier,
            now_epoch
                .checked_add(execution_started.elapsed().as_secs())
                .ok_or(MediaError::SourceReceiptRejected)?,
        )?;
        authority.revalidate_source(resources)?;
        if sha256_file_with_resources(&config.python_executable, resources)?
            != execution_plan.python_sha256
            || sha256_file_with_resources(&config.backend_script, resources)?
                != execution_plan.backend_script_sha256
            || sha256_file_with_resources(&config.ffmpeg_executable, resources)?
                != execution_plan.ffmpeg_sha256
        {
            return Err(MediaError::BackendReceiptBindingMismatch);
        }
        if process_output.exit_code != Some(0) {
            return Err(MediaError::BackendFailed(truncate_diagnostic(
                &String::from_utf8_lossy(&process_output.stderr),
            )));
        }
        if process_output.stdout_truncated || process_output.stderr_truncated {
            return Err(MediaError::MediaBudgetExceeded);
        }
        let mut raw: RawBackendReceipt = serde_json::from_slice(&process_output.stdout)
            .map_err(|error| MediaError::InvalidBackendJson(error.to_string()))?;
        let process_receipt = MediaProcessReceipt {
            exit_code: process_output.exit_code,
            stdout_digest: sha256_bytes(&process_output.stdout),
            stderr_digest: sha256_bytes(&process_output.stderr),
            stdout_truncated: process_output.stdout_truncated,
            stderr_truncated: process_output.stderr_truncated,
            process_tree_kill_attempted: process_output.process_tree_kill_attempted,
            process_tree_kill_verified: process_output.process_tree_kill_verified,
            memory_limit_bytes: process_output.memory_limit_bytes,
            memory_limit_kind: process_output.memory_limit_kind.into(),
        };
        drop(output);
        if raw.backend_id.len() > 256
            || raw.backend_version.len() > 256
            || raw.model_revision.len() > 256
            || raw.model_card_license_hint.len() > 4096
            || raw.diagnostics.len() > 1024
            || raw.diagnostics.iter().any(|message| message.len() > 4096)
        {
            return Err(MediaError::MediaBudgetExceeded);
        }
        if raw.schema_version != MEDIA_BACKEND_SCHEMA || raw.source_sha256 != source_sha256 {
            return Err(MediaError::BackendReceiptBindingMismatch);
        }
        if raw.model_cache_manifest_sha256 != model_cache_manifest_sha256
            || model_cache_manifest_sha256_bounded(
                &config.model_cache,
                ModelScanLimits::default(),
                resources,
            )? != model_cache_manifest_sha256
            || raw.backend_id != descriptor.backend_id
        {
            return Err(MediaError::BackendReceiptBindingMismatch);
        }
        if raw.observations.len() > budget.max_observations {
            return Err(MediaError::MediaBudgetExceeded);
        }
        for observation in &raw.observations {
            if observation.label_truncated {
                let original = observation
                    .original_label_bytes
                    .filter(|length| *length > observation.label.len())
                    .ok_or(MediaError::InvalidBackendObservation)?;
                raw.diagnostics.push(format!(
                    "label_truncated:{}:original_utf8_bytes={original}",
                    observation.observation_id
                ));
            } else if observation
                .original_label_bytes
                .is_some_and(|length| length != observation.label.len())
            {
                return Err(MediaError::InvalidBackendObservation);
            }
        }
        let executed = raw
            .executed_families
            .iter()
            .map(|family| parse_backend_family(family))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let source_fragments = {
            let by_id = raw
                .observations
                .iter()
                .map(|observation| (observation.observation_id.as_str(), observation))
                .collect::<BTreeMap<_, _>>();
            raw.observations
                .iter()
                .map(|observation| {
                    bound_source_fragment(observation, &by_id, &source_sha256, budget)
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut graph = MultimodalEventGraph::default();
        for (observation, source_fragment_digest) in
            raw.observations.into_iter().zip(source_fragments)
        {
            if observation.label.len() > 4_096
                || observation.start_ms < 0
                || observation.end_ms < observation.start_ms
            {
                return Err(MediaError::InvalidBackendObservation);
            }
            let start = RationalTime::checked(observation.start_ms, 1_000)?;
            let end = RationalTime::checked(observation.end_ms, 1_000)?;
            let kind = parse_observation_kind(&observation.kind)?;
            validate_observation_family(&kind, &executed)?;
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
        let receipt = MediaBackendExecutionReceipt {
        schema_version: MEDIA_BACKEND_SCHEMA,
        source_sha256: source_sha256.clone(),
        backend_id: raw.backend_id,
        backend_version: raw.backend_version,
        model_revision: raw.model_revision,
        model_cache_manifest_sha256: raw.model_cache_manifest_sha256,
        model_card_license_hint: raw.model_card_license_hint,
        backend_receipt_sha256: process_receipt.stdout_digest.clone(),
        execution_plan,
        process: process_receipt,
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
        Ok(receipt)
    })();
    // Always close the owned snapshot before publishing or returning an error.
    let closed = source_snapshot.close();
    let mut receipt = match (prepared, closed) {
        (Ok(receipt), Ok(())) => receipt,
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(cleanup)) => return Err(cleanup),
        (Err(error), Err(cleanup)) => {
            return Err(MediaError::Io(format!(
                "media_execution:{error:?}; snapshot_cleanup:{cleanup:?}"
            )))
        }
    };
    let observed_epoch = authority.current_epoch(
        now_epoch
            .checked_add(execution_started.elapsed().as_secs())
            .ok_or(MediaError::SourceReceiptRejected)?,
    )?;
    validate_current_media_authority(authority, verifier, observed_epoch)?;
    let (payload, scope) = match attestation_profile {
        MediaAttestationProfile::JsonV2 => (
            attestation_v2::media_backend_payload_digest_v2(&receipt)
                .map_err(|_| MediaError::BackendReceiptBindingMismatch)?,
            attestation_v2::media_backend_scope_v2(&receipt.backend_id),
        ),
        MediaAttestationProfile::BinaryV3 => (
            attestation_v3::media_backend_payload_digest_v3(&receipt)
                .map_err(|_| MediaError::BackendReceiptBindingMismatch)?,
            attestation_v3::media_backend_scope_v3(&receipt.backend_id),
        ),
    };
    let subject = SubjectRevision::checked(source_sha256)
        .map_err(|_| MediaError::BackendReceiptBindingMismatch)?;
    let scope =
        ReceiptScope::checked(scope).map_err(|_| MediaError::BackendReceiptBindingMismatch)?;
    // Preserve v2's historical issue-time profile. Explicit v3 samples again
    // after report hashing, then revalidates the parent before issuance.
    let issued_epoch = match attestation_profile {
        MediaAttestationProfile::JsonV2 => observed_epoch,
        MediaAttestationProfile::BinaryV3 => {
            let current = authority.current_epoch(
                now_epoch
                    .checked_add(execution_started.elapsed().as_secs())
                    .ok_or(MediaError::SourceReceiptRejected)?,
            )?;
            validate_current_media_authority(authority, verifier, current)?;
            current
        }
    };
    let wire = issuer
        .issue(
            ReceiptClass::MediaBackend,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            issued_epoch,
            None,
            authority.attestation_receipt_digest.clone(),
        )
        .map_err(|_| MediaError::BackendReceiptBindingMismatch)?;
    if matches!(attestation_profile, MediaAttestationProfile::BinaryV3) {
        receipt.authenticity_receipt_digest = Some(wire.receipt_digest());
        receipt.authenticity_attestation = Some(wire);
        // Keep one original anchor: separately flooring execution and
        // verification elapsed time could lose a second across expiry.
        let verified = attestation_v3::verify_from_execution(
            &receipt,
            authority,
            verifier,
            replay,
            now_epoch,
            execution_started,
            resources,
        );
        verified.map_err(|_| MediaError::BackendReceiptBindingMismatch)?;
        return Ok(receipt);
    }
    let verified = verifier
        .verify(
            wire,
            &ReceiptPolicy::exact(
                ReceiptClass::MediaBackend,
                subject,
                scope,
                payload,
                observed_epoch,
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
    fn all() -> [Self; 17] {
        [
            Self::Decode,
            Self::ShotBoundary,
            Self::ObjectDetection,
            Self::FaceDetection,
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
        "face_detection" => Ok(BackendFamily::FaceDetection),
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
        BackendFamily::FaceDetection => "face_detection",
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
        BackendFamily::FaceDetection => Some("opencv-haar-frontalface"),
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

fn validate_observation_family(
    kind: &ObservationKind,
    executed: &BTreeSet<BackendFamily>,
) -> Result<(), MediaError> {
    use BackendFamily as Family;
    use ObservationKind as Kind;
    let family = match kind {
        Kind::ShotBoundary => Family::ShotBoundary,
        Kind::ObjectBox => Family::ObjectDetection,
        Kind::ObjectMask | Kind::AnonymousPersonTrack => Family::SegmentationTracking,
        Kind::PoseLandmarks => Family::Pose,
        Kind::FaceLandmarks | Kind::FacialActionUnit => Family::FaceActionUnit,
        Kind::FaceRegionCandidate => Family::FaceDetection,
        Kind::ActionCandidate => Family::ActionRecognition,
        Kind::OcrTextCandidate => Family::Ocr,
        Kind::RelativeDepthCandidate => Family::DepthMotion,
        Kind::MotionCandidate if executed.contains(&Family::FrameDifference) => {
            Family::FrameDifference
        }
        Kind::MotionCandidate => Family::DepthMotion,
        Kind::TranscriptCandidate => Family::Asr,
        Kind::AnonymousSpeakerSegment => Family::Diarization,
        Kind::AudioEventCandidate => Family::AudioEvent,
        Kind::CrossModalRelationCandidate => Family::CrossModalAlignment,
        Kind::AcousticClusterCandidate => Family::AcousticClustering,
        Kind::TemporalCoPresenceCandidate => Family::TemporalCoPresence,
    };
    if executed.contains(&family) {
        Ok(())
    } else {
        Err(MediaError::BackendMethodMismatch)
    }
}

enum MediaSourceSnapshot<'a> {
    Legacy(lc631_process_supervisor::AuthenticatedSourceSnapshot),
    Reserved(lc631_process_supervisor::ReservedSourceSnapshot<'a>),
}
impl<'a> MediaSourceSnapshot<'a> {
    fn capture(
        source: &Path,
        expected: &str,
        max_bytes: u64,
        temporary: u64,
        resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
    ) -> Result<Self, MediaError> {
        match resources {
            Some(view) => {
                lc631_process_supervisor::AuthenticatedSourceSnapshot::capture_with_resources(
                    source, expected, max_bytes, temporary, view,
                )
                .map(Self::Reserved)
                .map_err(|error| MediaError::Io(format!("media_snapshot_resource:{error:?}")))
            }
            None => lc631_process_supervisor::AuthenticatedSourceSnapshot::capture(
                source, expected, max_bytes, temporary,
            )
            .map(Self::Legacy)
            .map_err(|error| MediaError::Io(error.to_string())),
        }
    }
    fn path(&self) -> &Path {
        match self {
            Self::Legacy(snapshot) => snapshot.path(),
            Self::Reserved(snapshot) => snapshot.path(),
        }
    }
    fn bytes(&self) -> u64 {
        match self {
            Self::Legacy(snapshot) => snapshot.bytes(),
            Self::Reserved(snapshot) => snapshot.bytes(),
        }
    }
    fn close(self) -> Result<(), MediaError> {
        match self {
            Self::Legacy(snapshot) => snapshot
                .close()
                .map_err(|error| MediaError::Io(error.to_string())),
            Self::Reserved(snapshot) => snapshot
                .close()
                .map_err(|error| MediaError::Io(format!("media_snapshot_resource:{error:?}"))),
        }
    }
}

enum MediaProcessOutput<'a> {
    Legacy(lc631_process_supervisor::RunOutcome),
    Reserved(lc631_process_supervisor::ResourceRunOutcome<'a>),
}
impl MediaProcessOutput<'_> {
    fn outcome(&self) -> &lc631_process_supervisor::RunOutcome {
        match self {
            Self::Legacy(output) => output,
            Self::Reserved(output) => output.outcome(),
        }
    }
}

fn run_media_process<'a>(
    command: Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    resources: Option<&'a lc631_process_supervisor::ResourceReservationView<'a>>,
) -> Result<MediaProcessOutput<'a>, MediaError> {
    use std::ffi::OsString;
    let arguments = command.get_args().map(OsString::from).collect::<Vec<_>>();
    let mut environment = Vec::new();
    for key in [
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "COMSPEC",
    ] {
        if let Some(value) = std::env::var_os(key) {
            environment.push((OsString::from(key), value));
        }
    }
    environment.extend(
        command
            .get_envs()
            .filter_map(|(key, value)| value.map(|value| (key.into(), value.into()))),
    );
    let cwd = std::env::current_dir().map_err(|error| MediaError::Io(error.to_string()))?;
    let request = lc631_process_supervisor::RunRequest {
        executable: command.get_program(),
        arguments: &arguments,
        current_dir: command.get_current_dir().unwrap_or(&cwd),
        environment: &environment,
        stdin: None,
        timeout,
        stdout_limit,
        stderr_limit,
    };
    let guarded = match resources {
        Some(view) => lc631_process_supervisor::run_bounded_with_resources(request, view)
            .map(MediaProcessOutput::Reserved)
            .map_err(|error| MediaError::Io(format!("media_child_resource:{error:?}")))?,
        None => lc631_process_supervisor::run_bounded(request)
            .map(MediaProcessOutput::Legacy)
            .map_err(|error| MediaError::Io(error.to_string()))?,
    };
    let output = guarded.outcome();
    if output.timed_out {
        return Err(MediaError::BackendTimeout);
    }
    if !output.io_cleanup_complete
        || !output.process_tree_kill_verified
        || output.diagnostic.is_some()
    {
        return Err(MediaError::Io("media_process_cleanup_unverified".into()));
    }
    Ok(guarded)
}

fn sha256_directory(root: &Path) -> Result<String, MediaError> {
    model_cache_manifest_sha256_bounded(root, ModelScanLimits::default(), None)
}

pub fn model_cache_manifest_sha256(root: &Path) -> Result<String, MediaError> {
    sha256_directory(root)
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
    RegistrationConfigurationMismatch,
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
        // Face detection is distinct from object detection; no family may be
        // silently omitted or duplicated by the public method catalog.
        assert_eq!(families.len(), 17);
        assert_eq!(families, BTreeSet::from(BackendFamily::all()));
        assert_eq!(catalog.len(), families.len());
        let method_ids = catalog
            .iter()
            .map(|item| item.method_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(method_ids.len(), catalog.len());
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
