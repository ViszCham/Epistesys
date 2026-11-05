//! K8 rational outward Poincare distance; arithmetic precision grants no authority.
#![forbid(unsafe_code)]

use lc631_core::measurement_numeric::{
    check, checked_add, checked_compare, checked_divide, checked_multiply, checked_subtract,
    sqrt_enclosure, BigInt, NumericBudget, NumericError, NumericEvaluation, Rational,
    RationalInterval,
};

const INNER_PRECISIONS: [u32; 7] = [80, 112, 144, 176, 208, 240, 256];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct K8Budget {
    maximum_inner_precision: u32,
    maximum_series_index: u32,
}

impl Default for K8Budget {
    fn default() -> Self {
        Self {
            maximum_inner_precision: 256,
            maximum_series_index: 256,
        }
    }
}

impl K8Budget {
    /// Reduced budgets support explicit exhaustion; output precision stays 64 bits.
    pub fn new(
        maximum_inner_precision: u32,
        maximum_series_index: u32,
    ) -> Result<Self, NumericError> {
        if !INNER_PRECISIONS.contains(&maximum_inner_precision) || maximum_series_index > 256 {
            return Err(NumericError::InvalidPrecision);
        }
        Ok(Self {
            maximum_inner_precision,
            maximum_series_index,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct K8Report {
    pub evaluation: NumericEvaluation,
    pub lower_endpoint_precision: u32,
    pub upper_endpoint_precision: u32,
    pub lower_endpoint_slack: Option<Rational>,
    pub upper_endpoint_slack: Option<Rational>,
}

struct Endpoint {
    bounds: Option<RationalInterval>,
    complete: bool,
    precision: u32,
}

fn integer(n: u32) -> Rational {
    Rational::from_integer(BigInt::from(n))
}

fn target() -> Rational {
    Rational::new(BigInt::from(1), BigInt::from(1) << 64)
}

fn complete_sqrt(
    value: &Rational,
    precision: u32,
) -> Result<Option<RationalInterval>, NumericError> {
    match sqrt_enclosure(value, NumericBudget::new(precision)?)? {
        NumericEvaluation::Complete(bounds) => Ok(Some(bounds)),
        NumericEvaluation::ResourceExhausted { partial: _ } => Ok(None),
    }
}

fn exhausted_error(error: NumericError) -> Result<(), NumericError> {
    match error {
        NumericError::NumericBudget | NumericError::ResourceExhausted => Ok(()),
        NumericError::DivisionByZero
        | NumericError::ReversedInterval
        | NumericError::InvalidInput
        | NumericError::InvalidPrecision
        | NumericError::NoApplicable => Err(error),
    }
}

/// Next S_N enclosure and valid geometric-series remainder, with no float arithmetic.
fn series_bounds(
    q: &RationalInterval,
    root_curvature: &RationalInterval,
    lower_sum: &Rational,
    upper_sum: &Rational,
    upper_next_power: &Rational,
    next_denominator: u32,
) -> Result<RationalInterval, NumericError> {
    let upper_square = checked_multiply(q.upper(), q.upper())?;
    let tail_denominator = checked_multiply(
        &integer(next_denominator),
        &checked_subtract(&integer(1), &upper_square)?,
    )?;
    let tail = checked_divide(upper_next_power, &tail_denominator)?;
    let atanh = RationalInterval::new(lower_sum.clone(), checked_add(upper_sum, &tail)?)?;
    atanh
        .multiply(&RationalInterval::exact_ratio(2, 1)?)?
        .divide(root_curvature)
}

fn series(
    q: &RationalInterval,
    root_curvature: &RationalInterval,
    maximum_index: u32,
    precision: u32,
) -> Result<Endpoint, NumericError> {
    let lower_square = checked_multiply(q.lower(), q.lower())?;
    let upper_square = checked_multiply(q.upper(), q.upper())?;
    let mut lower_power = q.lower().clone();
    let mut upper_power = q.upper().clone();
    let mut lower_sum = integer(0);
    let mut upper_sum = integer(0);
    let mut retained = None;
    for n in 0..=maximum_index {
        let step = (|| {
            let denominator = integer(2 * n + 1);
            lower_sum = checked_add(&lower_sum, &checked_divide(&lower_power, &denominator)?)?;
            upper_sum = checked_add(&upper_sum, &checked_divide(&upper_power, &denominator)?)?;
            let lower_next = checked_multiply(&lower_power, &lower_square)?;
            let upper_next = checked_multiply(&upper_power, &upper_square)?;
            let bounds = series_bounds(
                q,
                root_curvature,
                &lower_sum,
                &upper_sum,
                &upper_next,
                2 * n + 3,
            )?;
            lower_power = lower_next;
            upper_power = upper_next;
            Ok::<_, NumericError>(bounds)
        })();
        let bounds = match step {
            Ok(bounds) => bounds,
            Err(error) => {
                exhausted_error(error)?;
                return Ok(Endpoint {
                    bounds: retained,
                    complete: false,
                    precision,
                });
            }
        };
        let convergence = checked_subtract(bounds.upper(), bounds.lower())
            .and_then(|slack| checked_compare(&slack, &target()))
            .map(|order| !order.is_gt());
        let converged = match convergence {
            Ok(converged) => converged,
            Err(error) => {
                exhausted_error(error)?;
                return Ok(Endpoint {
                    bounds: Some(bounds),
                    complete: false,
                    precision,
                });
            }
        };
        retained = Some(bounds);
        if converged {
            return Ok(Endpoint {
                bounds: retained,
                complete: true,
                precision,
            });
        }
    }
    Ok(Endpoint {
        bounds: retained,
        complete: false,
        precision,
    })
}

fn endpoint(
    t: &Rational,
    curvature: &Rational,
    budget: K8Budget,
) -> Result<Endpoint, NumericError> {
    if t == &integer(0) {
        return Ok(Endpoint {
            bounds: Some(RationalInterval::exact_ratio(0, 1)?),
            complete: true,
            precision: 80,
        });
    }
    let q_squared = checked_divide(t, &checked_add(&integer(1), t)?)?;
    let mut retained = Endpoint {
        bounds: None,
        complete: false,
        precision: 80,
    };
    for precision in INNER_PRECISIONS {
        if precision > budget.maximum_inner_precision {
            break;
        }
        let roots = (|| {
            let q = complete_sqrt(&q_squared, precision)?;
            let curvature_root = complete_sqrt(curvature, precision)?;
            Ok::<_, NumericError>((q, curvature_root))
        })();
        let (q, curvature_root) = match roots {
            Ok((Some(q), Some(root))) => (q, root),
            Ok((None, _)) | Ok((_, None)) => continue,
            Err(error) => {
                exhausted_error(error)?;
                break;
            }
        };
        // A rounded q upper of 1, or sqrt(kappa) lower of 0, needs refinement.
        if q.upper() >= &integer(1) || curvature_root.lower() <= &integer(0) {
            continue;
        }
        let current = series(&q, &curvature_root, budget.maximum_series_index, precision)?;
        if current.complete {
            return Ok(current);
        }
        if current.bounds.is_some() {
            retained = current;
        }
    }
    Ok(retained)
}

fn slack(endpoint: &Endpoint) -> Result<Option<Rational>, NumericError> {
    match endpoint
        .bounds
        .as_ref()
        .map(|bounds| checked_subtract(bounds.upper(), bounds.lower()))
    {
        Some(Ok(slack)) => Ok(Some(slack)),
        Some(Err(error)) => {
            exhausted_error(error)?;
            Ok(None)
        }
        None => Ok(None),
    }
}

fn bounded_endpoint(
    t: &Rational,
    curvature: &Rational,
    budget: K8Budget,
) -> Result<Endpoint, NumericError> {
    match endpoint(t, curvature, budget) {
        Ok(endpoint) => Ok(endpoint),
        Err(error) => {
            exhausted_error(error)?;
            Ok(Endpoint {
                bounds: None,
                complete: false,
                precision: 80,
            })
        }
    }
}

pub fn evaluate(
    t: &RationalInterval,
    curvature: &Rational,
    budget: K8Budget,
) -> Result<K8Report, NumericError> {
    check(curvature)?;
    if curvature <= &integer(0) || t.lower() < &integer(0) {
        return Err(NumericError::InvalidInput);
    }
    let lower = bounded_endpoint(t.lower(), curvature, budget)?;
    let upper = if t.is_exact() {
        Endpoint {
            bounds: lower.bounds.clone(),
            complete: lower.complete,
            precision: lower.precision,
        }
    } else {
        bounded_endpoint(t.upper(), curvature, budget)?
    };
    let lower_endpoint_slack = slack(&lower)?;
    let upper_endpoint_slack = slack(&upper)?;
    let bounds = match upper.bounds.as_ref() {
        Some(upper_bounds) => {
            let lo = lower
                .bounds
                .as_ref()
                .map(|b| b.lower().clone())
                .unwrap_or_else(|| integer(0));
            match RationalInterval::new(lo, upper_bounds.upper().clone()) {
                Ok(bounds) => Some(bounds),
                Err(error) => {
                    exhausted_error(error)?;
                    None
                }
            }
        }
        None => None,
    };
    let complete = lower.complete
        && upper.complete
        && lower_endpoint_slack.is_some()
        && upper_endpoint_slack.is_some();
    let evaluation = match (complete, bounds) {
        (true, Some(bounds)) => NumericEvaluation::Complete(bounds),
        (true, None) | (false, None) => NumericEvaluation::ResourceExhausted { partial: None },
        (false, Some(bounds)) => NumericEvaluation::ResourceExhausted {
            partial: Some(bounds),
        },
    };
    Ok(K8Report {
        evaluation,
        lower_endpoint_precision: lower.precision,
        upper_endpoint_precision: upper.precision,
        lower_endpoint_slack,
        upper_endpoint_slack,
    })
}

pub fn poincare_distance(
    t: &RationalInterval,
    curvature: &Rational,
) -> Result<NumericEvaluation, NumericError> {
    Ok(evaluate(t, curvature, K8Budget::default())?.evaluation)
}
