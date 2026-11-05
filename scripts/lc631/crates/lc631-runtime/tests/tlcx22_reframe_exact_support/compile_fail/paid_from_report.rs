//! After B export, require E0599 for caller-created paid-map evidence.
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_runtime::reorganization_measurement::reframe as f;
fn inject(report: ComponentReport) {
    let _ = f::PaidFrameMap::from_report(report);
    let _ = f::PaidFrameMap::from_success(true);
    let _ = f::PaidFrameMap::from_work(0);
}
fn main() {}
