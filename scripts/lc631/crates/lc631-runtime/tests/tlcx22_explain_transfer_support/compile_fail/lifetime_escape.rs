//! Expected post-export diagnostic: local current-owner borrow cannot be static.
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_runtime::explanation_measurement::OwnedExplainGain;
fn escape<'a, 'r, 'i, 'c, 'v>(
    owner: &OwnedExplainGain<'a, 'r, 'i, 'c, 'v>,
) -> &'static ComponentReport {
    owner.historical_report()
}
fn main() {}
