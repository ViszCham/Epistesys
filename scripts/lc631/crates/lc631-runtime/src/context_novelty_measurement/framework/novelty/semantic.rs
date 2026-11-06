//! 日本語: 同一の実P1/P2 ownerから、有限の意味差・方向付き接続・T_ncを検査する。
//! English: Recheck finite semantic change, directional bridges and T_nc on the same live P1/P2 owners.
//! 日本語: 入力対応はGivenであり、完全なF2・権限・host配信は別に扱う。
//! English: The input correspondence is Given. Full F2, authority and host delivery remain separate.
#![forbid(unsafe_code)]
mod admission;
mod bank;
mod bridge;
mod current;
mod identity;
mod metrics;
mod root;
mod status;
mod types;

use super as m;
use super::{
    f, n, ComponentMetric, ComponentReport, ComponentState, Digest32, MeasurementError, Rational,
    RationalInterval, ResourceClaim, ResourceError, ResourceReservationView,
};
use lc631_substrate::measurement_numeric as numeric;
use std::cell::Cell;

pub use bridge::{BridgeResult, BridgeWitness};
pub use current::{Current, Owned};
pub use root::{ExpectedPair, Root};
pub use status::{Query, QueryRow};
pub use types::*;

impl From<m::Error> for Error {
    fn from(e: m::Error) -> Self {
        match e {
            m::Error::Resource(e) => Self::Resource(e),
            m::Error::ForeignReservation => Self::ForeignReservation,
            m::Error::ExpectedInputMismatch => Self::ExpectedInputMismatch,
            m::Error::WorkBudget => Self::WorkBudget,
            other => Self::Novelty(other),
        }
    }
}
fn framework(e: f::Error) -> Error {
    m::framework(e).into()
}
fn native(e: n::Error) -> Error {
    m::native(e).into()
}
fn reserve<T>(n: usize, max: usize) -> Result<Vec<T>, Error> {
    m::budget::vector(n, max).map_err(Into::into)
}
const ROLES: [f::QueryRole; 3] = [
    f::QueryRole::GoalEcho,
    f::QueryRole::Placement,
    f::QueryRole::BridgeLaw,
];
