//! The owner coordination artifact is not a wildcard for arbitrary files.
use lc631_analysis::{inspect_resume, ResumeError};
use std::path::PathBuf;
fn root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "tlcx05-checkpoint-{label}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
#[test]
fn tlcx05_history_accepts_only_exact_regular_zero_byte_coordination_locks() {
    let path = root("locks");
    std::fs::write(path.join(".writer.lock"), []).unwrap();
    std::fs::write(path.join(".resource-owner-v1.lock"), []).unwrap();
    assert!(inspect_resume(&path, "source", "plan", "authority")
        .unwrap()
        .latest_sequence
        .is_none());
    std::fs::write(path.join(".resource-owner-v1.lock"), b"not empty").unwrap();
    assert_eq!(
        inspect_resume(&path, "source", "plan", "authority").err(),
        Some(ResumeError::RootUnsafe)
    );
    let unknown = root("unknown-lock");
    std::fs::write(unknown.join(".arbitrary.lock"), []).unwrap();
    assert_eq!(
        inspect_resume(&unknown, "source", "plan", "authority").err(),
        Some(ResumeError::CorruptCheckpoint)
    );
}
#[test]
fn tlcx05_history_rejects_1001_payloads_before_parsing_or_materializing_them() {
    let path = root("count");
    for index in 0..1001 {
        std::fs::write(path.join(format!("checkpoint-{index:020}.json")), []).unwrap();
    }
    assert_eq!(
        inspect_resume(&path, "source", "plan", "authority").err(),
        Some(ResumeError::BudgetExceeded)
    );
}
