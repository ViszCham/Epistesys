//! Expected post-export diagnostic: only closed program enum, no score/callback.
use lc631_runtime::transfer_measurement::Program;
fn inject() {
    let _ = Program::FromScore(1);
    let _ = Program::Callback(|| true);
    let _ = Program::CaseIdLookup(vec![("t2", true)]);
}
fn main() {}
