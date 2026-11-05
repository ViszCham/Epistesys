use super::{AxisKind, AxisValue, MissingAxis, SelectionError};
use lc631_analysis::creativity_measurement::{
    ComponentAssessment, ComponentMetric, ComponentReport, ComponentState, MeasurementGap,
};

/// A borrowed projection retains all five-field component metadata and its
/// distinct basis/validation/independence fields; it verifies no oracle claim.
pub struct ReportedComponentAxis<'a> {
    pub kind: AxisKind,
    pub value: AxisValue<'a>,
    pub report: &'a ComponentReport,
}

/// Borrow all primitive axes from one admitted candidate without synthesizing
/// common producer/result metadata. Every original report remains attached.
/// This reported adapter does not replace each producer's current-owner check.
pub fn project_assessment_reported<'a>(
    assessment: &'a ComponentAssessment<'a>,
) -> impl ExactSizeIterator<Item = Result<ReportedComponentAxis<'a>, SelectionError>> + 'a {
    assessment
        .reports()
        .iter()
        .copied()
        .map(project_component_reported)
}

/// Only JointPareto primitive leaves are admitted. Original F1/F2 and nested
/// aggregates cannot silently add the same TL or gain a second time.
pub fn project_component_reported(
    report: &ComponentReport,
) -> Result<ReportedComponentAxis<'_>, SelectionError> {
    use ComponentMetric as M;
    let kind = match report.metric {
        M::Coverage => AxisKind::CoverageGain,
        M::Compression => AxisKind::CompressionGain,
        M::Bridge => AxisKind::BridgeGain,
        M::Prediction => AxisKind::PredictionGain,
        M::Preserve => AxisKind::Preserve,
        M::Rebind => AxisKind::Rebind,
        M::Reframe => AxisKind::Reframe,
        M::Explain => AxisKind::ExplainGain,
        M::Design => AxisKind::DesignGain,
        M::Transfer => AxisKind::TransferGain,
        M::Solve => AxisKind::SolveGain,
        M::Redundancy => AxisKind::RedundancyCost,
        M::BreakCost => AxisKind::BreakCost,
        M::CounterexampleRisk => AxisKind::CounterexampleCost,
        M::BridgeCost => AxisKind::MigrationCost,
        M::ContextDistance
        | M::FrameworkDistance
        | M::SemanticConfigurationDistance
        | M::Compatibility
        | M::ComplexityMatch
        | M::Knownness
        | M::NoveltyCompatibility
        | M::StructuralGain
        | M::ReorganizationGain
        | M::UtilityGain
        | M::DescriptionBits
        | M::ComplexityParity
        | M::OriginalV1
        | M::OriginalV2 => return Err(SelectionError::InvalidAxes),
    };
    let value = match &report.state {
        ComponentState::Measured(bounds) if bounds.is_exact() => AxisValue::Exact(bounds),
        ComponentState::Measured(bounds) => AxisValue::Bounded(bounds),
        ComponentState::Estimated { bounds, .. } => AxisValue::Estimated(bounds),
        ComponentState::Unknown(gap) => AxisValue::Unavailable(match gap {
            MeasurementGap::MissingObservation => MissingAxis::Unknown,
            MeasurementGap::MissingEvaluator => MissingAxis::MissingEvaluator,
            MeasurementGap::Unsupported => MissingAxis::Unsupported,
            MeasurementGap::Timeout => MissingAxis::Timeout,
            MeasurementGap::Conflict => MissingAxis::Conflict,
            MeasurementGap::Uncalibrated => MissingAxis::Uncalibrated,
        }),
        ComponentState::NotApplicable { .. } => AxisValue::Unavailable(MissingAxis::NotApplicable),
        ComponentState::Incomparable { .. } => AxisValue::Unavailable(MissingAxis::Incomparable),
        ComponentState::Undefined(_) => AxisValue::Unavailable(MissingAxis::Undefined),
        ComponentState::InvalidConfig(_) => AxisValue::Unavailable(MissingAxis::InvalidConfig),
        ComponentState::ResourceExhausted { .. } => {
            AxisValue::Unavailable(MissingAxis::ResourceExhausted)
        }
    };
    Ok(ReportedComponentAxis {
        kind,
        value,
        report,
    })
}
