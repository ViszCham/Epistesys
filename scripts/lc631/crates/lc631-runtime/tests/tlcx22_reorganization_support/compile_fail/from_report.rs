//! Expected after exports: E0599; reported metadata cannot construct an owner.
use lc631_analysis::creativity_measurement::ComponentReport;
use lc631_runtime::reorganization_measurement::OwnedRebind;
fn inject(report: ComponentReport) {
    let _ = OwnedRebind::from_report(report);
}
fn main() {}
