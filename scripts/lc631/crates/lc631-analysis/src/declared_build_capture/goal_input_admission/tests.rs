//! Private counter boundary tests; independent literal caps, not report gold.
use super::*;
use lc631_substrate::{resource::*, Digest32};
use std::time::Duration;
fn owner(cancel: Cancellation) -> ResourceLease {
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let reported = ReportedResourceLease::checked(
        1,
        Digest32::from_bytes([6; 32]),
        &[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(65_536).unwrap(),
        )],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        cancel,
    )
    .unwrap();
    issuer.issue_admitted(1, reported).unwrap()
}
#[test]
fn tlcx22_h4_private_exact_nodes_and_depth_then_one_over_are_rejected() {
    let owner = owner(Cancellation::default());
    let view = owner.view();
    let mut b = budget::Budget::new();
    for _ in 0..32_768 {
        b.enter(Node::Null, 64, &view).unwrap();
    }
    assert_eq!(b.nodes, 32_768);
    assert_eq!(b.enter(Node::Null, 64, &view), Err(Error::InventoryLimit));
    let mut b = budget::Budget::new();
    assert_eq!(b.enter(Node::Null, 65, &view), Err(Error::InventoryLimit));
    assert_eq!(b.nodes, 0);
}
#[test]
fn tlcx22_h4_private_exact_leaf_aggregate_and_encoded_are_individual_caps() {
    let owner = owner(Cancellation::default());
    let view = owner.view();
    let leaf = "x".repeat(262_144);
    let too_large = "x".repeat(262_145);
    let mut b = budget::Budget::new();
    assert_eq!(
        b.enter(Node::Str(&too_large), 0, &view),
        Err(Error::InventoryLimit)
    );
    assert_eq!(b.nodes, 0);
    for _ in 0..4 {
        b.enter(Node::Str(&leaf), 0, &view).unwrap();
    }
    assert_eq!(b.strings, 1_048_576);
    assert_eq!(
        b.enter(Node::Str("x"), 0, &view),
        Err(Error::InventoryLimit)
    );
    let mut b = budget::Budget::new();
    b.encoded = 8_388_608 - 32;
    b.enter(Node::Number, 0, &view).unwrap();
    assert_eq!(b.encoded, 8_388_608);
    assert_eq!(b.enter(Node::Null, 0, &view), Err(Error::InventoryLimit));
    b.encoded = usize::MAX;
    assert_eq!(b.enter(Node::Number, 0, &view), Err(Error::InventoryLimit));
}
#[test]
fn tlcx22_h4_private_known_length_preflights_before_any_child() {
    let owner = owner(Cancellation::default());
    let view = owner.view();
    let legal = vec![0; 16_384];
    let illegal = vec![0; 16_385];
    let mut b = budget::Budget::new();
    assert_eq!(
        b.enter(Node::Seq(Sequence::U32(&illegal)), 0, &view),
        Err(Error::InventoryLimit)
    );
    assert_eq!(b.nodes, 0);
    b.enter(Node::Seq(Sequence::U32(&legal)), 0, &view).unwrap();
    assert_eq!(b.nodes, 1);
    let mut b = budget::Budget::new();
    b.nodes = 32_768 - 16_384;
    assert_eq!(
        b.enter(Node::Seq(Sequence::U32(&legal)), 0, &view),
        Err(Error::InventoryLimit)
    );
    let mut b = budget::Budget::new();
    assert_eq!(
        b.enter(Node::Seq(Sequence::U32(&legal)), 64, &view),
        Err(Error::InventoryLimit)
    );
}
#[test]
fn tlcx22_h4_private_resource_check_at_sixty_four_and_fixed_stack_capacity() {
    let cancel = Cancellation::default();
    let owner = owner(cancel.clone());
    let view = owner.view();
    let mut b = budget::Budget::new();
    for _ in 0..63 {
        b.enter(Node::Null, 0, &view).unwrap();
    }
    cancel.cancel();
    assert_eq!(
        b.enter(Node::Null, 0, &view),
        Err(Error::Resource(ResourceError::Cancelled))
    );
    assert_eq!(FRAMES, 256);
    assert!(std::mem::size_of::<Frame<'_>>().checked_mul(256).unwrap() <= 65_536);
}
