//! Transfer admission never turns missing semantics into numeric success.
use super::*;
#[test]
fn tlcx22_transfer_negative_source_empty_weight_and_inconsistent_gamma() {
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let entries = tf::entries(&f);
    let ri = tf::root(&cases, &entries);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let mut wrong = ri;
    wrong.binding.selection.source = SourceRevision::of("not the resource source");
    assert!(matches!(
        t::capture_transfer(wrong, &view),
        Err(t::Error::Resource(ResourceError::SourceMismatch))
    ));
    assert!(matches!(
        t::capture_transfer(tf::root(&[], &entries), &view),
        Err(t::Error::InvalidInput(_))
    ));
    let zero = q(0, 1);
    let zero_cases = [
        t::TransferCase {
            weight: &zero,
            ..cases[0]
        },
        cases[1],
    ];
    assert!(matches!(
        t::capture_transfer(tf::root(&zero_cases, &entries), &view),
        Err(t::Error::InvalidInput(_))
    ));
    let contradiction = theory(vec![f.p.clone(), BoolExpr::Not(Box::new(f.p.clone()))], 3);
    let inconsistent_cases = [
        t::TransferCase {
            theory: &contradiction,
            ..cases[0]
        },
        t::TransferCase {
            theory: &contradiction,
            ..cases[1]
        },
    ];
    let mut inconsistent_entries = entries;
    inconsistent_entries[2].theory = &contradiction;
    inconsistent_entries[3].theory = &contradiction;
    assert!(matches!(
        t::capture_transfer(tf::root(&inconsistent_cases, &inconsistent_entries), &view),
        Err(t::Error::NoComparable(e::NoComparable::ContextInconsistent))
    ));
}
