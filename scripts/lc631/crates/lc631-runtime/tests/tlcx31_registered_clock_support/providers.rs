//! Raw local actors, never checked capabilities. Only future registered factories
//! may turn their real calls plus verified registrations into clock observations.
use lc631_analysis::declared_build_capture::CurrentSnapshotAuthority;
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier};
use lc631_runtime::goal_clock_domain as c;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
#[path = "../tlcx31_coding_support/mod.rs"]
pub mod native;

pub struct Clock {
    pub epoch: AtomicU64,
    pub calls: AtomicUsize,
    pub unavailable: AtomicBool,
    script: Mutex<VecDeque<u64>>,
}
impl Clock {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            epoch: AtomicU64::new(100),
            calls: AtomicUsize::new(0),
            unavailable: AtomicBool::new(false),
            script: Mutex::new(VecDeque::new()),
        })
    }
    pub fn set(&self, n: u64) {
        self.epoch.store(n, Ordering::SeqCst);
    }
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    pub fn sequence(&self, values: &[u64], tail: u64) {
        assert!(values.len() <= 64);
        *self.script.lock().unwrap() = values.iter().copied().collect();
        self.set(tail);
    }
}
impl c::ClockEpochSource for Clock {
    fn read_epoch(&self) -> Result<u64, c::ProviderUnavailable> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.unavailable.load(Ordering::SeqCst) {
            Err(c::ProviderUnavailable)
        } else {
            Ok(self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| self.epoch.load(Ordering::SeqCst)))
        }
    }
}
pub struct Registry {
    pub verifier: ReceiptVerifier,
    pub now: AtomicU64,
    pub revision: AtomicU64,
    pub revoked: Mutex<Vec<String>>,
    pub calls: AtomicUsize,
}
impl Registry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            verifier: ReceiptVerifier::from_key_bytes("registered-clock-local", &[135; 32])
                .unwrap(),
            now: AtomicU64::new(100),
            revision: AtomicU64::new(1),
            revoked: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
        })
    }
    pub fn issuer() -> ReceiptIssuer {
        ReceiptIssuer::from_key_bytes("registered-clock-local", &[135; 32]).unwrap()
    }
    pub fn revoke(&self, digest: String) {
        let mut r = self.revoked.lock().unwrap();
        assert!(r.len() < 8);
        r.push(digest);
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
}
impl c::RegistryAuthority for Registry {
    fn with_current(
        &self,
        f: &mut dyn FnMut(c::RegistryView<'_>) -> Result<(), c::Error>,
    ) -> Result<(), c::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let r = self.revoked.lock().unwrap();
        f(c::RegistryView {
            verifier: &self.verifier,
            now_epoch: self.now.load(Ordering::SeqCst),
            revision: 1,
            revocation_revision: self.revision.load(Ordering::SeqCst),
            revoked_receipt_digests: &r,
        })
    }
}
pub struct Grant(pub Mutex<native::FixtureSnapshotAuthority>);
impl c::SnapshotGrantSource for Grant {
    fn with_current(
        &self,
        f: &mut dyn FnMut(c::SnapshotGrantView<'_>) -> Result<(), c::Error>,
    ) -> Result<(), c::Error> {
        let mut a = self.0.lock().unwrap();
        let v = a.current().map_err(c::Error::Snapshot)?;
        f(c::SnapshotGrantView {
            permit: v.permit,
            context: v.context,
            expected_principal: v.expected_principal,
        })
    }
}
