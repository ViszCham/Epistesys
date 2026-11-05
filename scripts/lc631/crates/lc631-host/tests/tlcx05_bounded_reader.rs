use lc631_host::bounded_reader::read_text;
use lc631_process_supervisor::{ResourceKind, ResourceReservationView};
use std::io::Read;

#[test]
fn tlcx05_host_reader_accepts_only_an_injected_borrowed_view() {
    // Signature-only check. Behavioral accounting/growth tests live in the
    // CLI package, which owns an actual runtime broker. No issuer is forged.
    fn injected<'a>(input: &mut dyn Read, view: &'a ResourceReservationView<'a>) {
        let _ = read_text(input, view, ResourceKind::Output);
    }
    let _entry = injected;
}

#[test]
fn tlcx05_legacy_key_file_keeps_an_explicit_actual_byte_cap() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "epistesys-tlcx05-key-cap-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("receipt-root-v1.key"),
        format!("{}{}", " ".repeat(257), "a".repeat(64)),
    )
    .unwrap();
    match lc631_host::HostReceiptContext::load_existing(&root) {
        Err(lc631_host::HostError::Io(message)) => {
            assert!(message.contains("host_input_budget_exceeded"))
        }
        _ => panic!("legacy key read must reject excessive raw bytes before trim/decode"),
    }
}
