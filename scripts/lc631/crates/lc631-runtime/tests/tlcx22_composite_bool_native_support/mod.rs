//! 日本語: 実B producerのfixtureを再利用するorchestration。報告scoreは注入しない。
//! English: Orchestrate existing actual B producers; never inject reported scores.
pub use lc631_analysis::creativity_measurement::{formula::OriginalFormula, ComponentMetric as M};
pub use lc631_runtime::composite_original::bool_program as join;
pub use lc631_runtime::{bridge_gain_measurement as b, compatibility_measurement as c};
#[allow(dead_code)]
#[path = "../tlcx22_bridge_actual_support/mod.rs"]
pub mod native;
pub use native::*;
pub const F2_WITHOUT_BRIDGE: &[M] = &[
    M::DescriptionBits,
    M::FrameworkDistance,
    M::SemanticConfigurationDistance,
    M::Compatibility,
    M::Coverage,
    M::Compression,
    M::Prediction,
    M::Redundancy,
    M::Preserve,
    M::Rebind,
    M::Reframe,
    M::BreakCost,
    M::Explain,
    M::Design,
    M::Transfer,
    M::Solve,
    M::CounterexampleRisk,
    M::BridgeCost,
];
pub const F2_ALL: &[M] = &[
    M::DescriptionBits,
    M::FrameworkDistance,
    M::SemanticConfigurationDistance,
    M::Compatibility,
    M::Coverage,
    M::Compression,
    M::Bridge,
    M::Prediction,
    M::Redundancy,
    M::Preserve,
    M::Rebind,
    M::Reframe,
    M::BreakCost,
    M::Explain,
    M::Design,
    M::Transfer,
    M::Solve,
    M::CounterexampleRisk,
    M::BridgeCost,
];
// Native relation is fixed BEFORE program capture/measurement; composite applicability is unassessed.
macro_rules! actual_pair {
    ($fx:ident,$cases:ident,$view:ident;$root:ident,$program:ident,$branch:ident,$gain:ident,$coordinate:ident,$target:ident) => {
        let baseline = $fx.forward(true);
        let baseline_inverse = Fixture::inverse(&baseline);
        let good = $fx.forward(false);
        let inverse = Fixture::inverse(&good);
        let $coordinate = join::BoolCoordinateSpec {
            native: $fx.input($cases),
            composite_goal: d(0x91),
            declared_relation:
                b"Given finite port contract; relation to composite coding goal remains unassessed",
        };
        let $target = join::BoolTarget {
            forward: &good,
            inverse: &inverse,
        };
        let $root = b::BridgeRoot::given(
            root_input($coordinate.native, spec(&baseline, &baseline_inverse), 48),
            $view,
        )
        .unwrap();
        let baseline_program = $root.baseline_program($view).unwrap();
        let $program = $root
            .candidate_program(spec($target.forward, $target.inverse), $view)
            .unwrap();
        let baseline_branch = $root.prepare(&baseline_program, $view).unwrap();
        let $branch = $root.prepare(&$program, $view).unwrap();
        let baseline_uses = baseline_branch
            .execute_uses($view)
            .unwrap_or_else(|_| panic!("actual baseline uses failed"));
        let candidate_uses = $branch
            .execute_uses($view)
            .unwrap_or_else(|_| panic!("actual candidate uses failed"));
        let $gain = b::measure_pair(&$root, &baseline_uses, &candidate_uses, $view).unwrap();
    };
}
pub(crate) use actual_pair;
