//! 日本語: 条件付きadmissionと実native対応を分離する。原式成功や権限は発行しない。
//! English: Separate conditional admission from actual native correspondence, without scores or grants.
#![forbid(unsafe_code)]
mod admission;
pub mod bool_program;
mod declarations;
mod pending;
mod resources;

pub use admission::{ConditionalCandidate, ConditionalRootPlan};
pub use declarations::*;
pub use lc631_analysis::creativity_measurement::formula::OriginalFormula;
use lc631_analysis::creativity_measurement::ComponentMetric;
pub use pending::{CurrentPending, PendingComposite};

fn required_metrics(formula: OriginalFormula) -> &'static [ComponentMetric] {
    // Central-owned crate-private seam into the existing strict F1/F2 registry.
    crate::original_score_pending::required_metrics(formula)
}
