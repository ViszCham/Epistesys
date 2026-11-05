//! Full native clocks/trace/rectangles checked directly against semantic comparands.
use super::*;
fn identity<'a>(
    w: &Operational<'_>,
    bindings: &'a Term,
    token: &Term,
    kind: &str,
    expected: &Term,
    op: &str,
) -> Result<&'a Term, CostError> {
    let e = fields(w, expected, op, 2)?;
    let row = association(w, bindings, token, kind)?;
    let path = term::symbol(&row[2])?;
    let role = match path {
        "Clock/primary" | "Origin/primary" | "Reference/primary" | "Frame/primary" => 0,
        "Clock/secondary" | "Frame/secondary" => 1,
        _ => return Err(CostError::CorrespondenceMismatch),
    };
    equal(w, &e[0], &int(role))?;
    equal(w, &e[1], &row[3])?;
    Ok(&row[3])
}
pub(super) fn temporal(
    w: &Operational<'_>,
    raw: &Term,
    expected: &Term,
    declarations: &Term,
    bindings: &Term,
) -> Result<(), CostError> {
    let a = fields(w, raw, "temporal-fields-v1", 7)?;
    let e = fields(w, expected, "temporal-model-v1", 5)?;
    equal(w, &a[0], &e[0])?;
    equal(w, &a[4], &Term::Bool(true))?;
    let meanings = term::children(&e[2], "atoms-v1")?;
    if meanings.len() > 4 || term::unsigned(&a[3])? != meanings.len() as u64 {
        return Err(CostError::CorrespondenceMismatch);
    }
    for (i, m) in meanings.iter().enumerate() {
        equal(
            w,
            declaration(
                w,
                declarations,
                "TemporalAtom",
                &format!("Temporal/Atom/{i}"),
            )?,
            m,
        )?;
    }
    let clock = fields(w, &a[5], "clock-fields-v1", 5)?;
    let ec = fields(w, &e[1], "clock-v1", 4)?;
    let row = association(w, bindings, &clock[0], "Clock")?;
    let role = match term::symbol(&row[2])? {
        "Clock/primary" => 0,
        "Clock/secondary" => 1,
        _ => return Err(CostError::CorrespondenceMismatch),
    };
    equal(w, &ec[0], &int(role))?;
    equal(w, &ec[1], &row[3])?;
    let meaning = fields(w, &row[3], "identity-meaning-v1", 2)?;
    let payload = fields(w, &meaning[1], "clock-meaning-v1", 3)?;
    equal(w, &clock[1], &payload[0])?;
    equal(w, &clock[4], &int(0))?;
    equal(w, &payload[1], &int(0))?;
    identity(w, bindings, &clock[2], "Origin", &ec[2], "origin-v1")?;
    identity(w, bindings, &clock[3], "Reference", &ec[3], "reference-v1")?;
    let steps = term::children(&a[6], "steps-fields-v1")?;
    let expected_steps = term::children(&e[4], "closed-trace-v1")?;
    if steps.is_empty() || steps.len() > 8 || steps.len() != expected_steps.len() {
        return Err(CostError::CorrespondenceMismatch);
    }
    for (a, e) in steps.iter().zip(expected_steps) {
        let a = fields(w, a, "step-fields-v1", 3)?;
        let e = fields(w, e, "step-v1", 3)?;
        equal(w, &a[0], &e[0])?;
        equal(w, &a[1], &clock[0])?;
        equal(w, &e[1], &int(role))?;
        equal(w, &a[2], &e[2])?;
        if term::children(&a[2], "truths-v1")?.len() != meanings.len() {
            return Err(CostError::CorrespondenceMismatch);
        }
    }
    nodes::temporal(
        w,
        term::children(&a[1], "nodes-v1")?,
        &a[2],
        &e[3],
        meanings,
    )
}
pub(super) fn spatial(
    w: &Operational<'_>,
    raw: &Term,
    expected: &Term,
    bindings: &Term,
) -> Result<(), CostError> {
    let a = fields(w, raw, "spatial-fields-v1", 4)?;
    let e = fields(w, expected, "spatial-pair-v1", 4)?;
    let meaning = identity(w, bindings, &a[0], "Frame", &e[0], "frame-v1")?;
    let m = fields(w, meaning, "identity-meaning-v1", 2)?;
    let m = fields(w, &m[1], "frame-meaning-v1", 6)?;
    equal(w, &a[1], &e[1])?;
    equal(w, &a[1], &m[0])?;
    let role = &fields(w, &e[0], "frame-v1", 2)?[0];
    for (a, e) in a[2..].iter().zip(&e[2..]) {
        match a {
            Term::Apply(n, v) if n == "none-v1" && v.is_empty() => {
                fields(w, e, "missing-rectangle-v1", 0)?;
            }
            Term::Apply(n, v) if n == "some-v1" && v.len() == 1 => {
                let r = fields(w, &v[0], "rectangle-fields-v1", 5)?;
                let e = fields(w, e, "rect-v1", 5)?;
                equal(w, &r[0], &fields(w, raw, "spatial-fields-v1", 4)?[0])?;
                equal(w, &e[0], role)?;
                for (a, e) in r[1..].iter().zip(&e[1..]) {
                    equal(w, a, e)?;
                }
            }
            _ => return Err(CostError::CorrespondenceMismatch),
        }
    }
    Ok(())
}
