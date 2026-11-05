//! Compile cut for a live borrowed-output consumer; behavior is tested by the
//! callback discipline tests and normal CLI connection integration separately.
use lc631_analysis::{
    observe_connection_plan_from_cli_with_consumer, CodingConnectionPlan, ConnectionError,
    VerifiedConnectionObservation,
};
use lc631_process_supervisor::ResourceReservationView;

#[test]
fn tlcx05_connection_consumer_api_returns_owned_result_not_captured_bytes() {
    fn build(
        root: &std::path::Path,
        plan: &CodingConnectionPlan,
        view: &ResourceReservationView<'_>,
    ) -> Result<(VerifiedConnectionObservation, usize), ConnectionError> {
        observe_connection_plan_from_cli_with_consumer(root, plan, view, |report, stdout| {
            assert!(report.runtime_command_observed);
            Ok(stdout.len())
        })
    }
    let _signature = build;
}
