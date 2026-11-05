//! 固定長LE payload。旧checkpoint bytes/tagの解釈を変えない。
//! Fixed-width LE profile inside Evidence records; legacy tags remain unchanged.
use super::{zero, BudgetBinding, BudgetCaps, BudgetError, ReservationProposal};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

pub(super) const PAYLOAD_BYTES: usize = 648;
const MAGIC: &[u8; 8] = b"ETBUD001";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Initialize = 1,
    Reserve = 2,
    SettledNeedsFreshValidation = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Event {
    pub kind: Kind,
    pub binding: BudgetBinding,
    pub caps: BudgetCaps,
    pub prior_sequence: u64,
    pub prior_record: Digest32,
    pub prior_head_frame: Digest32,
    pub proposal: ReservationProposal,
    pub report: Digest32,
    pub observed_snapshot: Digest32,
    pub charged_attempts: u64,
    pub charged_delta: u64,
}

fn put(out: &mut [u8], position: &mut usize, bytes: &[u8]) {
    // Every call has a fixed schema size, and out is PAYLOAD_BYTES long.
    let end = *position + bytes.len();
    out[*position..end].copy_from_slice(bytes);
    *position = end;
}

pub(super) fn encode(event: Event) -> [u8; PAYLOAD_BYTES] {
    let mut out = [0; PAYLOAD_BYTES];
    let mut at = 0;
    put(&mut out, &mut at, MAGIC);
    put(&mut out, &mut at, &1_u16.to_le_bytes());
    put(&mut out, &mut at, &(event.kind as u16).to_le_bytes());
    put(&mut out, &mut at, &(PAYLOAD_BYTES as u32).to_le_bytes());
    for value in event.binding.values() {
        put(&mut out, &mut at, value.as_bytes());
    }
    for value in [
        event.caps.attempts,
        event.caps.per_attempt_delta_bytes,
        event.caps.total_delta_bytes,
        event.prior_sequence,
    ] {
        put(&mut out, &mut at, &value.to_le_bytes());
    }
    let p = event.proposal;
    for value in [
        event.prior_record,
        event.prior_head_frame,
        p.reservation,
        p.candidate,
        p.before_snapshot,
        p.intended_after_snapshot,
        p.authority_binding,
        p.intent_binding,
        event.report,
        event.observed_snapshot,
    ] {
        put(&mut out, &mut at, value.as_bytes());
    }
    for value in [p.delta_bytes, event.charged_attempts, event.charged_delta] {
        put(&mut out, &mut at, &value.to_le_bytes());
    }
    out
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn read<const N: usize>(&mut self) -> Result<[u8; N], BudgetError> {
        let end = self.at.checked_add(N).ok_or(BudgetError::InvalidPayload)?;
        let bytes = self
            .bytes
            .get(self.at..end)
            .ok_or(BudgetError::InvalidPayload)?;
        self.at = end;
        bytes.try_into().map_err(|_| BudgetError::InvalidPayload)
    }
    fn u64(&mut self) -> Result<u64, BudgetError> {
        Ok(u64::from_le_bytes(self.read()?))
    }
    fn digest(&mut self) -> Result<Digest32, BudgetError> {
        Ok(Digest32::from_bytes(self.read()?))
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<Event, BudgetError> {
    if bytes.len() != PAYLOAD_BYTES {
        return Err(BudgetError::InvalidPayload);
    }
    let mut r = Reader { bytes, at: 0 };
    if &r.read::<8>()? != MAGIC || u16::from_le_bytes(r.read()?) != 1 {
        return Err(BudgetError::UnsupportedPayload);
    }
    let kind = match u16::from_le_bytes(r.read()?) {
        1 => Kind::Initialize,
        2 => Kind::Reserve,
        3 => Kind::SettledNeedsFreshValidation,
        _ => return Err(BudgetError::UnsupportedPayload),
    };
    if u32::from_le_bytes(r.read()?) != PAYLOAD_BYTES as u32 {
        return Err(BudgetError::InvalidPayload);
    }
    let binding = BudgetBinding {
        run_family: r.digest()?,
        source: r.digest()?,
        context: r.digest()?,
        scope: r.digest()?,
        goal: r.digest()?,
        policy: r.digest()?,
        root_target: r.digest()?,
        initial_snapshot: r.digest()?,
    };
    let caps = BudgetCaps {
        attempts: r.u64()?,
        per_attempt_delta_bytes: r.u64()?,
        total_delta_bytes: r.u64()?,
    };
    let prior_sequence = r.u64()?;
    let prior_record = r.digest()?;
    let prior_head_frame = r.digest()?;
    let proposal = ReservationProposal {
        reservation: r.digest()?,
        candidate: r.digest()?,
        before_snapshot: r.digest()?,
        intended_after_snapshot: r.digest()?,
        authority_binding: r.digest()?,
        intent_binding: r.digest()?,
        delta_bytes: 0,
    };
    let report = r.digest()?;
    let observed_snapshot = r.digest()?;
    let event = Event {
        kind,
        binding,
        caps,
        prior_sequence,
        prior_record,
        prior_head_frame,
        proposal: ReservationProposal {
            delta_bytes: r.u64()?,
            ..proposal
        },
        report,
        observed_snapshot,
        charged_attempts: r.u64()?,
        charged_delta: r.u64()?,
    };
    binding.check()?;
    caps.check()?;
    if r.at != bytes.len() {
        return Err(BudgetError::InvalidPayload);
    }
    Ok(event)
}

pub(super) fn payload_digest(bytes: &[u8]) -> Digest32 {
    let mut hasher = Sha256::new();
    hasher.update(b"Epistesys/TLCX/RepairBudget/Evidence/v1\0");
    hasher.update(bytes);
    Digest32::from_bytes(hasher.finalize().into())
}

pub(super) fn empty_proposal() -> ReservationProposal {
    ReservationProposal {
        reservation: zero(),
        candidate: zero(),
        before_snapshot: zero(),
        intended_after_snapshot: zero(),
        authority_binding: zero(),
        intent_binding: zero(),
        delta_bytes: 0,
    }
}
