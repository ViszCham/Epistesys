//! TLCX-24 arithmetic geometry; verified embedding and scheduler consumers remain separate.
#![forbid(unsafe_code)]

pub mod arithmetic;
pub mod embedding;
pub mod graph;
pub mod k8;

use arithmetic::{one, point, zero, Interval, NumericError, Rational};

pub const OPERATIONAL_REVISION: &str = "OperationalMeasurement/finite-formal-v1.4";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SchedulerNoveltyPolicyId {
    #[default]
    DistanceToFrontierUnionV1,
    NearestVerifiedCandidateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryError {
    Numeric(NumericError),
    InvalidConfiguration,
    DimensionMismatch,
    OutsideBallMargin,
    ResourceExhausted,
}

impl From<NumericError> for GeometryError {
    fn from(value: NumericError) -> Self {
        Self::Numeric(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryUnavailable {
    EmbeddingPolicyNotRegistered,
    EmbeddingPolicyUnverified,
    NumericKernelNotConnected,
    EmptyVerifiedFrontier,
}

/// Presence records reported pins only; it cannot certify graph or held-out checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportedEmbeddingPins {
    pub transition_snapshot: [u8; 32],
    pub edge_evidence: [u8; 32],
    pub feature_correspondence: [u8; 32],
    pub objective: [u8; 32],
    pub initialization: [u8; 32],
    pub optimizer_version_seed: [u8; 32],
    pub stopping_budget: [u8; 32],
    pub held_out_checks: [u8; 32],
    pub dimension_curvature_scale_margin_coordinates: [u8; 32],
}

pub fn embedding_readiness(pins: Option<&ReportedEmbeddingPins>) -> GeometryUnavailable {
    match pins {
        None => GeometryUnavailable::EmbeddingPolicyNotRegistered,
        Some(_) => GeometryUnavailable::EmbeddingPolicyUnverified,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BallPolicy {
    curvature: Rational,
    margin: Rational,
    novelty_scale: Rational,
    dimension_limit: usize,
}

impl BallPolicy {
    pub fn new(
        curvature: Rational,
        margin: Rational,
        novelty_scale: Rational,
        dimension_limit: usize,
    ) -> Result<Self, GeometryError> {
        lc631_core::measurement_numeric::check(&curvature)?;
        lc631_core::measurement_numeric::check(&margin)?;
        lc631_core::measurement_numeric::check(&novelty_scale)?;
        if curvature <= zero()
            || margin <= zero()
            || margin >= one()
            || novelty_scale <= zero()
            || dimension_limit == 0
            || dimension_limit > 256
        {
            return Err(GeometryError::InvalidConfiguration);
        }
        Ok(Self {
            curvature,
            margin,
            novelty_scale,
            dimension_limit,
        })
    }

    pub fn novelty_scale(&self) -> &Rational {
        &self.novelty_scale
    }

    fn validate_box(&self, coordinates: &[Interval]) -> Result<Interval, GeometryError> {
        if coordinates.is_empty() {
            return Err(GeometryError::DimensionMismatch);
        }
        if coordinates.len() > self.dimension_limit {
            return Err(GeometryError::ResourceExhausted);
        }
        let mut norm = point(zero())?;
        for coordinate in coordinates {
            norm = norm.add(&coordinate.square()?)?;
        }
        let scaled = norm.multiply(&point(self.curvature.clone())?)?;
        if scaled.upper() > &(one() - &self.margin) {
            return Err(GeometryError::OutsideBallMargin);
        }
        Ok(scaled)
    }
}

/// Exact rational outer t enclosure, NOT a computed Poincare distance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedPoincare {
    t: Interval,
    curvature: Rational,
}

impl PreparedPoincare {
    pub fn t_enclosure(&self) -> &Interval {
        &self.t
    }
    pub fn curvature(&self) -> &Rational {
        &self.curvature
    }

    pub fn distance(
        &self,
    ) -> Result<lc631_core::measurement_numeric::NumericEvaluation, NumericError> {
        k8::poincare_distance(&self.t, &self.curvature)
    }
}

pub fn prepare_poincare(
    policy: &BallPolicy,
    x: &[Interval],
    y: &[Interval],
) -> Result<PreparedPoincare, GeometryError> {
    if x.len() != y.len() {
        return Err(GeometryError::DimensionMismatch);
    }
    let x_norm = policy.validate_box(x)?;
    let y_norm = policy.validate_box(y)?;
    let mut difference_norm = point(zero())?;
    for (a, b) in x.iter().zip(y) {
        difference_norm = difference_norm.add(&a.subtract(b)?.square()?)?;
    }
    let one = point(one())?;
    let denominator = one.subtract(&x_norm)?.multiply(&one.subtract(&y_norm)?)?;
    let t = difference_norm
        .multiply(&point(policy.curvature.clone())?)?
        .divide(&denominator)?;
    Ok(PreparedPoincare {
        t,
        curvature: policy.curvature.clone(),
    })
}

/// No geometry proposal is admitted before checked embedding and K8 consumers exist.
pub fn scheduling_readiness(
    pins: Option<&ReportedEmbeddingPins>,
    frontier_count: usize,
) -> GeometryUnavailable {
    if frontier_count == 0 {
        return GeometryUnavailable::EmptyVerifiedFrontier;
    }
    embedding_readiness(pins)
}
