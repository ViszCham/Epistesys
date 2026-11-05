//! Expected post-export E0277: report decoding cannot construct a current owner.
use lc631_runtime::transfer_measurement::OwnedTransferGain;
fn inject<'a, 'r, 'i, 'v>() {
    let _: OwnedTransferGain<'a, 'r, 'i, 'v> = serde_json::from_str("{}").unwrap();
}
fn main() {}
