//! Pure fixed-denominator K1 over privately observed property outcomes.
use super::*;
pub(super) fn state(
    root: &RootDefinition<'_, '_>,
    hard: LocalHardStatus,
    outcomes: &[Vec<PropertyOutcome>],
) -> Result<ComponentState, Error> {
    match hard {
        LocalHardStatus::Failed => Ok(ComponentState::Undefined(
            MeasurementError::PrerequisiteFailed,
        )),
        LocalHardStatus::Pending => Ok(ComponentState::Undefined(
            MeasurementError::PrerequisitePending,
        )),
        LocalHardStatus::NotRejected => k1(root, outcomes),
    }
}
fn k1(
    root: &RootDefinition<'_, '_>,
    outcomes: &[Vec<PropertyOutcome>],
) -> Result<ComponentState, Error> {
    let mut weights = resources::vector(16)?;
    let mut values = resources::vector(16)?;
    let mut keys = resources::vector(16)?;
    let mut missing = false;
    if root.order.len() != root.payloads.len() {
        return Err(Error::BindingMismatch);
    }
    for (case, &index) in root.order.iter().enumerate() {
        for (i, rule) in root
            .rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r.class == PropertyClass::Optional)
        {
            let weight = root.cases[index].weight * &rule.weight;
            admission::bounded(&weight, root.budget)?;
            let observed = *outcomes
                .get(case)
                .and_then(|r| r.get(i))
                .ok_or(Error::BindingMismatch)?;
            let value = match observed {
                PropertyOutcome::Preserved => RationalInterval::exact_ratio(0, 1),
                PropertyOutcome::ConfirmedViolation => RationalInterval::exact_ratio(1, 1),
                PropertyOutcome::NotObserved(_) => {
                    missing = true;
                    RationalInterval::from_ratio(0, 1, 1, 1)
                }
            }
            .map_err(|_| Error::NumericBudget)?;
            // Root-local slot follows the captured full-payload canonical order.
            // Admission retains/deduplicates complete payloads; this is no distance.
            let slot = u8::try_from(case).map_err(|_| Error::InvalidInput)?;
            let mut key = resources::vector(2)?;
            key.push(slot);
            key.push(rule.property as u8);
            keys.push(key);
            weights.push(weight);
            values.push(value);
        }
    }
    let mut probes = resources::vector(values.len())?;
    for i in 0..values.len() {
        probes.push(WeightedProbe {
            canonical_payload: &keys[i],
            weight: &weights[i],
            value: &values[i],
        });
    }
    let interval = weighted_assessment(&probes).map_err(|_| Error::NumericBudget)?;
    admission::bounded(interval.lower(), root.budget)?;
    admission::bounded(interval.upper(), root.budget)?;
    Ok(if missing {
        ComponentState::ResourceExhausted {
            partial: Some(interval),
        }
    } else {
        ComponentState::Measured(interval)
    })
}
