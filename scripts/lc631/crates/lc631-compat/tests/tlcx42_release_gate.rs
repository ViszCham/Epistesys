use lc631_compat::tlcx_release_gate::*;
use std::io::{self, Cursor, Read};

fn digest(n: u8) -> ReleaseDigest {
    ReleaseDigest::from_bytes([n; 32])
}
fn artifact(kind: ArtifactKind, bytes: &[u8]) -> ObservedArtifact {
    ObservedArtifact::read(kind, &mut Cursor::new(bytes), 1024).unwrap()
}
fn observations() -> [ObservedArtifact; 5] {
    ArtifactKind::ALL.map(|kind| artifact(kind, &[kind as u8, 42]))
}
fn identity(observed: &[ObservedArtifact; 5]) -> CandidateIdentity {
    CandidateIdentity::checked(std::array::from_fn(|i| ArtifactPin {
        kind: observed[i].kind(),
        digest: observed[i].digest(),
    }))
    .unwrap()
}
fn requirements() -> Vec<GateKey> {
    GateId::ALL
        .into_iter()
        .map(|gate| GateKey {
            gate,
            platform: Platform::WindowsX64,
            backend: Backend::Core,
            profile: digest(gate as u8),
        })
        .collect()
}
fn records(id: &CandidateIdentity, requirements: &[GateKey]) -> Vec<ReportedGateRecord> {
    requirements
        .iter()
        .map(|key| ReportedGateRecord {
            key: *key,
            candidate: id.digest(),
            source_snapshot: id.source_snapshot(),
            run: digest(90),
            observed_at: 10,
            expires_at: 30,
            outcome: ReportedOutcome::Passed,
            receipt: Some(digest(91)),
        })
        .collect()
}
fn plan<'a>(id: CandidateIdentity, required: &'a [GateKey]) -> ReleasePlan<'a> {
    ReleasePlan::checked(id, required, digest(90), 20).unwrap()
}

#[test]
fn tlcx42_actual_bytes_match_but_reported_passes_cannot_release() {
    let observed = observations();
    let id = identity(&observed);
    let required = requirements();
    let records = records(&id, &required);
    let report = assess(&plan(id, &required), &observed, &records, 20).unwrap();
    assert!(report
        .artifacts
        .iter()
        .all(|r| r.status == ArtifactStatus::BytesMatch));
    assert!(report
        .profiles
        .iter()
        .all(|r| r.status == ProfileStatus::ReportedPassNeedsVerification));
    assert_eq!(
        report.disposition,
        CandidateDisposition::CandidateNotFullyReleased
    );
}

#[test]
fn tlcx42_source_only_and_changed_binary_do_not_pass_identity() {
    let mut observed = observations();
    let id = identity(&observed);
    let required = requirements();
    let records = records(&id, &required);
    let p = plan(id, &required);
    let report = assess(&p, &observed[..1], &records, 20).unwrap();
    assert_eq!(report.disposition, CandidateDisposition::Blocked);
    assert!(report
        .artifacts
        .iter()
        .any(|r| r.status == ArtifactStatus::Missing));
    observed[1] = artifact(ArtifactKind::Binary, b"different binary");
    let report = assess(&p, &observed, &records, 20).unwrap();
    assert_eq!(report.artifacts[1].status, ArtifactStatus::DigestMismatch);
    assert_eq!(report.disposition, CandidateDisposition::Blocked);
}

#[test]
fn tlcx42_skipped_stale_foreign_snapshot_and_run_are_distinct() {
    let observed = observations();
    let id = identity(&observed);
    let required = requirements();
    let mut records = records(&id, &required);
    records[0].outcome = ReportedOutcome::Skipped;
    records[1].expires_at = 20;
    records[2].source_snapshot = digest(99);
    records[3].run = digest(98);
    records[4].candidate = digest(97);
    records[5].observed_at = 21;
    let report = assess(&plan(id, &required), &observed, &records, 20).unwrap();
    for (index, status) in [
        ProfileStatus::Skipped,
        ProfileStatus::Stale,
        ProfileStatus::SnapshotMismatch,
        ProfileStatus::RunMismatch,
        ProfileStatus::CandidateMismatch,
        ProfileStatus::FutureObservation,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(report.profiles[index].status, status);
    }
}

#[test]
fn tlcx42_required_profiles_and_host_absence_never_vacuously_succeed() {
    let observed = observations();
    let id = identity(&observed);
    let required = requirements();
    assert!(ReleasePlan::checked(id, &[], digest(90), 20).is_err());
    let incomplete = &required[..15];
    assert!(ReleasePlan::checked(id, incomplete, digest(90), 20).is_err());
    let report = assess(&plan(id, &required), &observed, &[], 20).unwrap();
    assert_eq!(report.profiles[13].status, ProfileStatus::HostPending);
    assert_eq!(report.profiles[15].status, ProfileStatus::HostPending);
    assert_eq!(report.profiles[0].status, ProfileStatus::Missing);
    assert_eq!(
        report.disposition,
        CandidateDisposition::CandidateNotFullyReleased
    );
}

#[test]
fn tlcx42_duplicate_or_unregistered_records_are_rejected() {
    let observed = observations();
    let id = identity(&observed);
    let required = requirements();
    let mut rs = records(&id, &required);
    rs.push(rs[0]);
    assert_eq!(
        assess(&plan(id, &required), &observed, &rs, 20).err(),
        Some(GateError::DuplicateRecord)
    );
    rs.pop();
    rs[0].key.profile = digest(99);
    assert_eq!(
        assess(&plan(id, &required), &observed, &rs, 20).err(),
        Some(GateError::UnregisteredProfile)
    );
    let duplicate = [observed[0], observed[0]];
    assert_eq!(
        assess(&plan(id, &required), &duplicate, &[], 20).err(),
        Some(GateError::DuplicateArtifact)
    );
}

#[test]
fn tlcx42_reader_caps_interruption_and_independent_sha_golden() {
    let value = artifact(ArtifactKind::Source, b"abc");
    assert_eq!(
        value.digest().bytes(),
        &[
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad
        ]
    );
    assert_eq!(
        ObservedArtifact::read(ArtifactKind::Binary, &mut Cursor::new(b"four"), 3).err(),
        Some(GateError::ReadLimit)
    );
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Interrupted.into())
        }
    }
    assert_eq!(
        ObservedArtifact::read(ArtifactKind::Binary, &mut Interrupted, 3).err(),
        Some(GateError::InterruptedLimit)
    );
}

#[test]
fn tlcx42_missing_receipt_failure_and_declared_freshness_are_preserved() {
    let observed = observations();
    let id = identity(&observed);
    let required = requirements();
    let mut rs = records(&id, &required);
    rs[0].receipt = None;
    rs[1].outcome = ReportedOutcome::Failed;
    rs[2].observed_at = 0;
    let p = ReleasePlan::checked(id, &required, digest(90), 10).unwrap();
    let report = assess(&p, &observed, &rs, 20).unwrap();
    assert_eq!(report.profiles[0].status, ProfileStatus::MissingReceipt);
    assert_eq!(report.profiles[1].status, ProfileStatus::Failed);
    assert_eq!(report.profiles[2].status, ProfileStatus::Stale);
}

#[test]
fn tlcx42_explicit_os_backend_profiles_cannot_substitute_for_each_other() {
    let observed = observations();
    let id = identity(&observed);
    let mut required = requirements();
    required.push(GateKey {
        gate: GateId::G03,
        platform: Platform::UbuntuX64,
        backend: Backend::Smt,
        profile: digest(70),
    });
    required.push(GateKey {
        gate: GateId::G03,
        platform: Platform::WindowsX64,
        backend: Backend::Asp,
        profile: digest(71),
    });
    let mut rs = records(&id, &required);
    rs.pop();
    let report = assess(&plan(id, &required), &observed, &rs, 20).unwrap();
    assert_eq!(
        report.profiles[16].status,
        ProfileStatus::ReportedPassNeedsVerification
    );
    assert_eq!(report.profiles[17].status, ProfileStatus::Missing);
    assert_eq!(
        report.disposition,
        CandidateDisposition::CandidateNotFullyReleased
    );
}
