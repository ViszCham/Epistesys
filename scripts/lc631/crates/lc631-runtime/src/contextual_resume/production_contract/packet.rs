//! Whole finite packet shape, not authenticated history or current authority.
#![forbid(unsafe_code)]
use crate::contextual_resume::transition::{
    parse_digest, UntrustedTransitionCheckpoint, BOUND_TRANSITION_BYTES,
};
use crate::journal::{decode_segment, JournalRecordKind};
use lc631_analysis::{
    coding_resume_wire::*, dgcl_repair_application_payload_digest, dgcl_repair_request_digest,
    lifecycle_binds_target, DgclRepairApplication, DgclRepairRequest,
};
use lc631_receipt_kernel::UntrustedReceipt;
use lc631_substrate::{
    codec::{decode_envelope_borrowed, SectionView},
    resource::{
        ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView,
    },
    Digest32,
};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodingResumePacketError {
    Limit,
    Shape,
    BindingMismatch,
    NeedsEvidence,
    Resource(ResourceError),
}
impl From<ResourceError> for CodingResumePacketError {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}
pub struct UntrustedCodingResumePacket<'a>(&'a [u8]);
impl<'a> UntrustedCodingResumePacket<'a> {
    pub fn borrowed(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}
pub struct CodingResumePacketLimits {
    frame: usize,
    sections: usize,
}
impl CodingResumePacketLimits {
    pub fn finite_v1() -> Self {
        Self {
            frame: 1048576,
            sections: 10,
        }
    }
}
pub struct CodingResumeOrigin {
    pub(super) pins: [Digest32; 15],
    pub(super) frame: Digest32,
}
impl CodingResumeOrigin {
    pub fn checked(pins: [Digest32; 15], frame: Digest32) -> Result<Self, CodingResumePacketError> {
        let zero = Digest32::from_bytes([0; 32]);
        if frame == zero || pins.contains(&zero) {
            return Err(CodingResumePacketError::NeedsEvidence);
        }
        Ok(Self { pins, frame })
    }
}
pub struct CheckedCodingResumePacket<'a, 'res> {
    source: &'a str,
    frame: Digest32,
    segment: &'a [u8],
    request: DgclRepairRequest,
    application: DgclRepairApplication,
    attestation: UntrustedReceipt,
    opaque: Vec<SectionView<'a>>,
    _funding: ResourceClaim<'res>,
}
impl<'a> CheckedCodingResumePacket<'a, '_> {
    pub fn source(&self) -> &str {
        self.source
    }
    pub fn frame_digest(&self) -> Digest32 {
        self.frame
    }
    pub fn segment(&self) -> &[u8] {
        self.segment
    }
    pub fn request(&self) -> &DgclRepairRequest {
        &self.request
    }
    pub fn application(&self) -> &DgclRepairApplication {
        &self.application
    }
    pub fn attestation(&self) -> &UntrustedReceipt {
        &self.attestation
    }
    pub fn opaque_sections(&self) -> &[SectionView<'a>] {
        &self.opaque
    }
    pub fn claim_boundary(&self) -> &'static str {
        "untrusted shape/crossbinding only; authenticate through inspect_transition_chain; no authority/current owner"
    }
}
fn shape<T>(_: T) -> CodingResumePacketError {
    CodingResumePacketError::Shape
}
fn hash(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
pub fn decode_coding_resume_packet<'a, 'res>(
    input: UntrustedCodingResumePacket<'a>,
    origin: &CodingResumeOrigin,
    limits: CodingResumePacketLimits,
    resources: &'res ResourceReservationView<'_>,
) -> Result<CheckedCodingResumePacket<'a, 'res>, CodingResumePacketError> {
    resources.check()?;
    let bytes = input.0;
    if bytes.len() > limits.frame {
        return Err(CodingResumePacketError::Limit);
    }
    let amount = bytes
        .len()
        .checked_mul(8)
        .and_then(|n| n.checked_add(65536))
        .ok_or(CodingResumePacketError::Limit)?;
    let funding = resources.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(u64::try_from(amount).map_err(shape)?)?,
    )])?;
    let frame = decode_envelope_borrowed(bytes).map_err(shape)?;
    if frame.minor() != 0 || frame.section_count() > limits.sections {
        return Err(CodingResumePacketError::Limit);
    }
    if frame.digest() != origin.frame {
        return Err(CodingResumePacketError::BindingMismatch);
    }
    let mut required: [Option<SectionView<'a>>; 6] = std::array::from_fn(|_| None);
    let mut opaque = Vec::new();
    opaque.try_reserve_exact(4).map_err(shape)?;
    for i in 0..frame.section_count() {
        let s = frame
            .section(i)
            .map_err(shape)?
            .ok_or(CodingResumePacketError::Shape)?;
        if (0x3200..=0x3205).contains(&s.tag()) {
            if s.version() != 1 || s.count() != 1 || s.flags() != 0 {
                return Err(CodingResumePacketError::Shape);
            }
            let index = usize::from(s.tag() - 0x3200);
            if required[index].is_some() {
                return Err(CodingResumePacketError::Shape);
            }
            required[index] = Some(s);
        } else {
            if opaque.len() == 4 {
                return Err(CodingResumePacketError::Limit);
            }
            opaque.push(s);
        }
    }
    let [descriptor, task, request, application, receipt, journal] = required;
    let get =
        |s: Option<SectionView<'a>>| s.map(|s| s.payload()).ok_or(CodingResumePacketError::Shape);
    let descriptor = get(descriptor)?;
    let task = get(task)?;
    let journal = get(journal)?;
    if descriptor.len() != 480 || task.len() > 262144 || journal.len() > 65536 {
        return Err(CodingResumePacketError::Limit);
    }
    for (i, pin) in origin.pins.iter().enumerate() {
        if descriptor.get(i * 32..(i + 1) * 32) != Some(pin.as_bytes().as_slice()) {
            return Err(CodingResumePacketError::BindingMismatch);
        }
    }
    let source = std::str::from_utf8(task).map_err(shape)?;
    let request = decode_coding_resume_request(get(request)?).map_err(shape)?;
    let application = decode_coding_resume_application(get(application)?).map_err(shape)?;
    let attestation = decode_coding_resume_receipt(get(receipt)?).map_err(shape)?;
    crossbind(&origin.pins, source, &request, &application, journal)?;
    funding.check()?;
    Ok(CheckedCodingResumePacket {
        source,
        frame: frame.digest(),
        segment: journal,
        request,
        application,
        attestation,
        opaque,
        _funding: funding,
    })
}
fn crossbind(
    p: &[Digest32; 15],
    source: &str,
    r: &DgclRepairRequest,
    a: &DgclRepairApplication,
    journal: &[u8],
) -> Result<(), CodingResumePacketError> {
    let parse = |s: &str| parse_digest(s).map_err(shape);
    if !a.applied
        || hash(source.as_bytes()) != p[1]
        || parse(&r.source_revision)? != p[1]
        || parse(
            r.repository_root_digest
                .as_deref()
                .ok_or(CodingResumePacketError::NeedsEvidence)?,
        )? != p[0]
        || parse(&r.plan_digest)? != p[4]
        || hash(r.target_ref.as_bytes()) != p[8]
        || a.attempt != r.attempt
        || a.gap_id != r.gap_id
        || a.source_revision != r.source_revision
        || a.plan_digest != r.plan_digest
        || a.target_ref != r.target_ref
        || a.target_digest_before != r.target_digest_before
        || a.candidate_digest != r.candidate_digest
        || a.request_digest != dgcl_repair_request_digest(r)
        || parse(&a.after_snapshot.build_input_digest)? != p[10]
        || !lifecycle_binds_target(&a.after_snapshot, &r.target_ref, &a.target_digest_after)
    {
        return Err(CodingResumePacketError::BindingMismatch);
    }
    let segment = decode_segment(journal).map_err(shape)?;
    if segment.records().len() != 1 {
        return Err(CodingResumePacketError::Shape);
    }
    let record = &segment.records()[0];
    if record.kind() != JournalRecordKind::Evidence {
        return Err(CodingResumePacketError::Shape);
    }
    let checkpoint = UntrustedTransitionCheckpoint::decode(record.payload()).map_err(shape)?;
    if record.object_digest() != checkpoint.payload_digest() {
        return Err(CodingResumePacketError::BindingMismatch);
    }
    let raw = checkpoint.as_bytes();
    if raw.len() != BOUND_TRANSITION_BYTES {
        return Err(CodingResumePacketError::NeedsEvidence);
    }
    let signed_pins = [
        p[1],
        p[2],
        p[3],
        p[7],
        p[5],
        p[6],
        parse(&dgcl_repair_request_digest(r))?,
        p[4],
        p[8],
        parse(&a.target_digest_before)?,
        parse(&a.target_digest_after)?,
        parse(&a.after_snapshot.snapshot_digest)?,
        parse(&dgcl_repair_application_payload_digest(a))?,
        parse(
            &a.attestation
                .as_ref()
                .ok_or(CodingResumePacketError::NeedsEvidence)?
                .receipt_digest(),
        )?,
    ];
    for (i, pin) in signed_pins.iter().enumerate() {
        if raw.get(16 + i * 32..16 + (i + 1) * 32) != Some(pin.as_bytes().as_slice()) {
            return Err(CodingResumePacketError::BindingMismatch);
        }
    }
    if raw.get(472..504) != Some(p[9].as_bytes().as_slice()) {
        return Err(CodingResumePacketError::BindingMismatch);
    }
    // No MAC/key, clock, permit or current observation is created here.
    Ok(())
}
