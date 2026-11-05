//! 両試験が共有する所有root/資源fixture。証拠の自動削除なし。
//! Shared owned-root/resource fixtures; no automatic evidence deletion.
#![allow(dead_code)]
use super::budget::*;
use lc631_host::JournalBundleLimits;
use lc631_runtime::{
    journal,
    resource::{
        Cancellation, Deadline, Reservation, ResourceAmount, ResourceBroker, ResourceKind,
        ResourceLimits, ResourceRequest, RunResources,
    },
};
use lc631_substrate::{codec::decode_envelope_borrowed, Digest32};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn binding() -> BudgetBinding {
    BudgetBinding {
        run_family: d(1),
        source: d(2),
        context: d(3),
        scope: d(4),
        goal: d(5),
        policy: d(6),
        root_target: d(7),
        initial_snapshot: d(8),
    }
}
pub fn caps() -> BudgetCaps {
    BudgetCaps {
        attempts: 4,
        per_attempt_delta_bytes: 100,
        total_delta_bytes: 50,
    }
}
pub fn proposal(delta_bytes: u64) -> ReservationProposal {
    ReservationProposal {
        reservation: d(9),
        candidate: d(10),
        before_snapshot: d(8),
        intended_after_snapshot: d(11),
        authority_binding: d(12),
        intent_binding: d(13),
        delta_bytes,
    }
}
pub fn report() -> ReportedSettlementProposal {
    ReportedSettlementProposal {
        observed_snapshot: d(11),
        report_digest: d(14),
    }
}
pub fn root(label: &str) -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().canonicalize().unwrap();
    for attempt in 0..32 {
        let p = base.join(format!(
            "epistesys-tlcx32-budget-{label}-{}-{now}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&p) {
            Ok(()) => {
                eprintln!("retained owned budget fixture: {}", p.display());
                return p;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => panic!("owned fixture creation: {e}"),
        }
    }
    panic!("unique root unavailable")
}
pub fn part(root: &Path, name: &str, seq: u64) -> PathBuf {
    root.join(format!("{name}-{seq:020}.bin"))
}
pub fn resources(id: u64) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            id,
            binding().source,
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(
            &[ResourceKind::Ram, ResourceKind::Output, ResourceKind::Disk].map(|kind| {
                ResourceRequest::new(kind, ResourceAmount::checked(64 * 1024 * 1024).unwrap())
            }),
        )
        .unwrap();
    (broker, run, reservation)
}
pub fn limits() -> JournalBundleLimits {
    JournalBundleLimits::checked(20, 1024 * 1024).unwrap()
}
pub fn actual_head(root: &Path, seq: u64) -> BudgetHead {
    let bytes = fs::read(part(root, "segment", seq)).unwrap();
    let cursor = journal::decode_segment(&bytes).unwrap().cursor();
    let head = fs::read(part(root, "local-head", seq)).unwrap();
    BudgetHead {
        sequence: cursor.sequence(),
        record_digest: cursor.record_digest(),
        local_head_frame_digest: decode_envelope_borrowed(&head).unwrap().digest(),
    }
}
