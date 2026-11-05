//! TEST-FIRST only: unresolved future API is intentional planned RED, not observed RED.
//! No implementation export, success stub, validation execution or closure claim.
#![forbid(unsafe_code)]
#[path = "tlcx22_reorganization_support/cuts.rs"]
mod cuts;
#[path = "tlcx22_reorganization_support/rebind.rs"]
mod rebind;
#[path = "tlcx22_reorganization_support/selection.rs"]
mod selection;
#[path = "tlcx22_reorganization_support/mod.rs"]
mod support;
