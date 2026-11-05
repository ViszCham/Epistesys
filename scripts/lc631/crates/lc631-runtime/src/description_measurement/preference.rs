//! 日本語: 派生complexity値の限定比較。全F1/F2・goal推薦ではない。
//! English: Limited comparison of derived complexity values, never full F1/F2
//! ranking, JointPareto axes, goal-based recommendation or semantic equivalence.
use super::{Error, OwnedDescription};
use lc631_substrate::{measurement_numeric::checked_compare, resource::ResourceReservationView};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComplexityPreference {
    Left,
    Right,
    /// Equal arithmetic values under this fixed preference, not semantic equality.
    Tie,
    NeedsEvidence,
}
/// Check the actual shared root and every borrowed owner/target/current lease.
/// No caller values, arbitrary comparator, fake goal/TL, or ID tie-break enter.
/// A different root capture is rejected even if its numeric values coincide.
pub fn compare_current(
    left: &OwnedDescription<'_, '_, '_, '_>,
    right: &OwnedDescription<'_, '_, '_, '_>,
    view: &ResourceReservationView<'_>,
) -> Result<ComplexityPreference, Error> {
    left.check(view)?;
    right.check(view)?;
    if !std::ptr::eq(left.root, right.root) {
        return Err(Error::RootMismatch);
    }
    let preference = match (&left.match_value, &right.match_value) {
        (Some(a), Some(b)) => {
            // Every current K6 length and positive integer scale yields an exact
            // rational value; equality is kept as a tie without choosing an ID.
            match checked_compare(a.lower(), b.lower()).map_err(|e| Error::Arithmetic(e.into()))? {
                Ordering::Greater => ComplexityPreference::Left,
                Ordering::Less => ComplexityPreference::Right,
                Ordering::Equal => ComplexityPreference::Tie,
            }
        }
        (None, None) | (None, Some(_)) | (Some(_), None) => ComplexityPreference::NeedsEvidence,
    };
    left.check(view)?;
    right.check(view)?;
    Ok(preference)
}
