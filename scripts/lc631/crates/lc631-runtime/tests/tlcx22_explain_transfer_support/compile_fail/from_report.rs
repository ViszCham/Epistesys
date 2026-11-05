//! Expected post-export diagnostic: no public constructor from a report.
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_runtime::explanation_measurement::OwnedExplainGain;
fn inject(report: ComponentReport) {
    let _ = OwnedExplainGain::from_report(report);
}
fn main() {}
