//! Bounded canonical admission and rational accounting before comparison/hash/dispatch.
use super::*;
use lc631_substrate::measurement_numeric::{self as numeric, BigInt};
pub(super) fn limits(b: BreakCostBudget) -> Result<(), Error> {
    let pairs = [
        (b.max_cases, 8),
        (b.max_input_bytes_per_case, 4096),
        (b.max_input_bytes_total, 32768),
        (b.max_candidate_text_bytes, 1024),
        (b.max_output_bytes_per_case, 8192),
        (b.max_output_bytes_per_run, 65536),
    ];
    if pairs.iter().any(|(v, max)| *v == 0 || v > max)
        || b.max_weight_bits == 0
        || b.max_weight_bits > 64
        || b.max_intermediate_bits == 0
        || b.max_intermediate_bits > 4096
        || b.max_adapter_calls_per_run > 8
        || b.max_serializations_per_run > 8
        || b.max_property_checks_per_run > 64
        || b.max_soft_checks_per_run > 16
        || b.max_extra_hard_checks > 8
    {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}
pub(super) fn positive(q: &Rational, b: BreakCostBudget) -> Result<(), Error> {
    if q.numer().bits() > u64::from(b.max_weight_bits)
        || q.denom().bits() > u64::from(b.max_weight_bits)
    {
        return Err(Error::NumericBudget);
    }
    numeric::check(q).map_err(|_| Error::InvalidConfig)?;
    if q <= &Rational::from_integer(BigInt::from(0)) {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}
pub(super) fn bounded(q: &Rational, b: BreakCostBudget) -> Result<(), Error> {
    if q.numer().bits() > u64::from(b.max_intermediate_bits)
        || q.denom().bits() > u64::from(b.max_intermediate_bits)
    {
        return Err(Error::NumericBudget);
    }
    numeric::check(q).map_err(|_| Error::NumericBudget)
}
pub(super) fn rules(
    input: &[PropertyRule],
    b: BreakCostBudget,
) -> Result<Vec<PropertyRule>, Error> {
    if input.is_empty() || input.len() > 9 {
        return Err(Error::InvalidConfig);
    }
    let mut rules = resources::vector(input.len())?;
    for (index, rule) in input.iter().enumerate() {
        if input[..index].iter().any(|r| r.property == rule.property) {
            return Err(Error::InvalidSuite);
        }
        if rule.class == PropertyClass::Optional && !optional(rule.property) {
            return Err(Error::NonTradeableHard);
        }
        positive(&rule.weight, b)?;
        rules.push(rule.clone());
    }
    if !rules.iter().any(|r| r.class == PropertyClass::Optional) {
        return Err(Error::InvalidConfig);
    }
    rules.sort_by_key(|r| r.property as u8);
    Ok(rules)
}
pub(super) fn optional(p: LegacyProperty) -> bool {
    matches!(
        p,
        LegacyProperty::JsonObjectEnvelope | LegacyProperty::V1StrictConsumerAcceptance
    )
}
pub(super) fn cases(
    source: &SourceStore,
    cases: &[LegacyCase<'_>],
    b: BreakCostBudget,
) -> Result<(Vec<Vec<u8>>, Vec<usize>), Error> {
    if cases.is_empty() || cases.len() > b.max_cases {
        return Err(Error::InvalidSuite);
    }
    let expected_source = cached_legacy_source_revision(source)?;
    let mut payloads = resources::vector(cases.len())?;
    let mut order = resources::vector(cases.len())?;
    let mut total = 0usize;
    for (index, case) in cases.iter().enumerate() {
        if cases[..index].iter().any(|c| c.id == case.id)
            || case.canonical_key.is_empty()
            || case.canonical_key.len() > 4096
        {
            return Err(Error::InvalidSuite);
        }
        positive(case.weight, b)?;
        input(&expected_source, case.input, b)?;
        let cap = next_input_capacity(b.max_input_bytes_per_case, b.max_input_bytes_total, total)?;
        let bytes = output::encode(case.input, cap, Error::InputBudget)?;
        total = total.checked_add(bytes.len()).ok_or(Error::InputBudget)?;
        if total > b.max_input_bytes_total {
            return Err(Error::InputBudget);
        }
        if payloads.iter().any(|p| p == &bytes) {
            return Err(Error::InvalidSuite);
        }
        payloads.push(bytes);
        order.push(index);
    }
    // Full admitted boundary bytes define work identity; opaque case IDs/key labels never rank it.
    order.sort_by(|a, b| payloads[*a].cmp(&payloads[*b]));
    Ok((payloads, order))
}
// The real encoder receives this cap before it reserves its input-byte buffer.
fn next_input_capacity(per_case: usize, total: usize, used: usize) -> Result<usize, Error> {
    let remaining = total.checked_sub(used).ok_or(Error::InputBudget)?;
    let cap = per_case.min(remaining);
    if cap == 0 {
        return Err(Error::InputBudget);
    }
    Ok(cap)
}
#[cfg(test)]
#[path = "../../tests/tlcx22_breakcost_budget_support/input_capacity_unit.rs"]
mod input_capacity_unit;
#[cfg(test)]
#[path = "../../tests/tlcx22_breakcost_budget_support/source_digest_unit.rs"]
mod source_digest_unit;

fn cached_legacy_source_revision(source: &SourceStore) -> Result<String, Error> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::new();
    result
        .try_reserve_exact(71)
        .map_err(|_| Error::ProducerFailure)?;
    result.push_str("sha256:");
    for &byte in source.id().digest().as_bytes() {
        result.push(HEX[usize::from(byte >> 4)] as char);
        result.push(HEX[usize::from(byte & 15)] as char);
    }
    Ok(result)
}
fn input(
    expected_source: &str,
    raw: &DgclStandaloneCandidate,
    b: BreakCostBudget,
) -> Result<(), Error> {
    if raw.candidate_text.len() > b.max_candidate_text_bytes
        || !raw.requirement_realizations.is_empty()
    {
        return Err(Error::InputBudget);
    }
    if raw.source_revision.as_str() != expected_source
        || raw.candidate_digest != lc631_core::stable_sha256(&raw.candidate_text)
        || raw.candidate_byte_len != raw.candidate_text.len()
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}
pub(super) fn denominator(
    cases: &[LegacyCase<'_>],
    rules: &[PropertyRule],
    b: BreakCostBudget,
) -> Result<Rational, Error> {
    let mut sum = Rational::from_integer(BigInt::from(0));
    for case in cases {
        for rule in rules.iter().filter(|r| r.class == PropertyClass::Optional) {
            let w = case.weight * &rule.weight;
            bounded(&w, b)?;
            sum += w;
            bounded(&sum, b)?;
        }
    }
    Ok(sum)
}
