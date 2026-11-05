use lc631_compat::{audit_v630_root_with_reader, hash_capped_reader, CompatError};
use std::io::Cursor;

#[test]
fn tlcx05_baseline_caps_actual_bytes_independent_of_a_stale_length_hint() {
    let mut grown = Cursor::new(b"abcde");
    assert!(matches!(
        hash_capped_reader(&mut grown, 4),
        Err(CompatError::BudgetExceeded)
    ));
    assert_eq!(
        grown.position(),
        5,
        "one sentinel byte, never an unbounded read"
    );
    let mut exact = Cursor::new(b"abcd");
    assert_eq!(
        hash_capped_reader(&mut exact, 4).unwrap(),
        "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589"
    );
}

#[test]
fn tlcx05_baseline_callback_failure_is_not_a_matching_digest_or_silenced_success() {
    let mut calls = 0;
    let result = audit_v630_root_with_reader(std::path::Path::new("unused"), &mut |_path, cap| {
        calls += 1;
        assert!(cap > 0 && cap <= 1024 * 1024);
        Err::<Option<String>, _>("cancelled")
    });
    assert!(matches!(result, Err("cancelled")));
    assert_eq!(calls, 1);
}
