//! 日本語: hash一致でなく全入力refs/payloadと実canonical mapsを照合する。
//! English: Compare complete input occurrences/payloads and actual canonical maps, never hashes alone.
use super::*;

pub(super) fn validate(
    coordinate: &BoolCoordinateSpec<'_>,
    target: BoolTarget<'_>,
    native: &NativeProgramView<'_, '_, '_, '_>,
    expected: NativeExpected<'_, '_, '_, '_>,
) -> Result<(), Error> {
    if coordinate.composite_goal.as_bytes() == &[0; 32]
        || coordinate.declared_relation.is_empty()
        || coordinate.declared_relation.len() > 256
    {
        return Err(Error::InvalidDeclaration);
    }
    if !std::ptr::eq(native.plan, expected.plan) {
        return Err(Error::ForeignNativePlan);
    }
    if !std::ptr::eq(native.program, expected.program) {
        return Err(Error::ForeignNativeProgram);
    }
    input(coordinate.native, native.input)?;
    if !maps(target.forward, native.forward) || !maps(target.inverse, native.inverse) {
        return Err(Error::CoordinatePayloadMismatch);
    }
    Ok(())
}
fn input(expected: c::PlanInput<'_>, actual: c::PlanInput<'_>) -> Result<(), Error> {
    let queries = [
        (expected.root_goal, actual.root_goal),
        (expected.source_theory, actual.source_theory),
        (expected.target_theory, actual.target_theory),
        (expected.source_use, actual.source_use),
        (expected.target_use, actual.target_use),
    ];
    if !std::ptr::eq(expected.source, actual.source)
        || !std::ptr::eq(expected.target, actual.target)
        || !std::ptr::eq(expected.expected_mapping, actual.expected_mapping)
        || !std::ptr::eq(expected.cases, actual.cases)
        || queries.iter().any(|(a, b)| !std::ptr::eq(*a, *b))
    {
        return Err(Error::CoordinateInputMismatch);
    }
    // Native constructors admitted literal queries and exactly four three-port cases.
    if actual.source.text().len() > 4096
        || actual.target.text().len() > 4096
        || actual.expected_mapping.len() != 3
        || actual.cases.len() != 4
        || expected.source.text() != actual.source.text()
        || expected.target.text() != actual.target.text()
        || expected.source_context != actual.source_context
        || expected.target_context != actual.target_context
        || expected.scope != actual.scope
        || expected.source_hard != actual.source_hard
        || expected.target_hard != actual.target_hard
        || expected.pins != actual.pins
        || expected.expected_mapping != actual.expected_mapping
        || queries.iter().any(|(a, b)| *a != *b)
    {
        return Err(Error::CoordinateInputMismatch);
    }
    for (a, b) in expected.cases.iter().zip(actual.cases) {
        if b.input.len() != 3
            || !std::ptr::eq(a.input, b.input)
            || a.input != b.input
            || a.weight != b.weight
        {
            return Err(Error::CoordinateInputMismatch);
        }
    }
    Ok(())
}
fn maps(declared: &[c::PortMap], actual: &[c::PortMap; 3]) -> bool {
    // Actual maps use constructor-admitted symbol order. Reordering declarations
    // changes no semantics; each admitted source must still have its exact target.
    declared.len() == 3
        && actual
            .iter()
            .all(|row| declared.iter().find(|other| other.source == row.source) == Some(row))
}
