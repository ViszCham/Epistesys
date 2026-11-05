use lc631_process_supervisor::read_bounded_bytes;
use lc631_substrate::resource::{ReportedResourceLease, ResourceKind, ResourceReservationView};
use std::io::{Cursor, Read};

#[test]
fn tlcx05_production_reader_signature_requires_live_reservation_not_a_report() {
    // Actual report non-acceptance is checked by the runtime target's rustc
    // positive/negative control; no report is forged into a live lease here.
    fn accepts_only_live<'a>(reader: &mut dyn Read, view: &'a ResourceReservationView<'a>) {
        let _ = read_bounded_bytes(reader, view, ResourceKind::Output);
    }
    let _entry = accepts_only_live;
    assert_ne!(
        std::any::type_name::<ReportedResourceLease>(),
        std::any::type_name::<ResourceReservationView<'static>>()
    );
}

#[test]
fn tlcx05_bounded_reader_accepts_non_file_readers_without_metadata_authority() {
    fn cursor_entry<'a>(reader: &mut Cursor<&[u8]>, view: &'a ResourceReservationView<'a>) {
        let _ = read_bounded_bytes(reader, view, ResourceKind::Output);
    }
    let _entry = cursor_entry;
}
