//! Actual immutable native correspondence and hard checks, BEFORE creativity metrics.
use super::*;
pub(super) fn query(
    q: &SmtStatusQuery,
    atoms: &[AtomMap],
    l: Limits,
    w: &mut Work<'_>,
) -> Result<(), Error> {
    w.charge(q.boolean_symbols().len() as u64 + 1)?;
    if !q.integer_symbols().is_empty()
        || q.boolean_symbols().len() != atoms.len()
        || q.assertions().len() > l.max_assertions
    {
        return Err(Error::UnsupportedSignature);
    }
    for id in q.boolean_symbols() {
        if !atoms.iter().any(|m| m.local == *id) {
            return Err(Error::InvalidMap);
        }
    }
    for e in q.assertions() {
        canonical::scan(e, atoms, l, w)?;
    }
    Ok(())
}
pub(super) fn input(
    input: &RootInput<'_>,
    d: &Declaration<'_>,
    w: &mut Work<'_>,
) -> Result<(Vec<bool>, usize), Error> {
    let l = d.limits;
    w.charge(256)?;
    if d.facts.len() > l.max_features || d.constructions.len() > l.max_features {
        return Err(Error::PayloadBudget);
    }
    if let ArchiveInput::Read(v) = d.archive {
        if v.len() > l.max_archive_entries {
            return Err(Error::PayloadBudget);
        }
    }
    let original = input.original.state();
    if input.source.text().len() > 262144
        || original.revision().source().digest() != input.source.id().digest()
    {
        return Err(Error::SourceMismatch);
    }
    if input.premises.len() != input.source_query.assertions().len()
        || input.premises.len() != original.premises().len()
        || input.premises.is_empty()
        || input.premises.len() > l.max_assertions
    {
        return Err(Error::ExpectedInputMismatch);
    }
    let mut mutable = resources::vector(input.premises.len(), 16, w)?;
    let mut target = None;
    for (i, p) in input.premises.iter().enumerate() {
        w.charge(input.premises.len() as u64 + 1)?;
        if !std::ptr::eq(p.formula, &input.source_query.assertions()[i]) {
            return Err(Error::ExpectedInputMismatch);
        }
        let original_p = original
            .premises()
            .iter()
            .find(|v| v.id() == p.id)
            .ok_or(Error::ExpectedInputMismatch)?;
        if original_p.source_span() != Some(p.span) || input.source.slice(p.span).is_err() {
            return Err(Error::SourceMismatch);
        }
        mutable.push(original_p.changeable());
        if p.id == input.target {
            if !original_p.changeable() {
                return Err(Error::HardInvariant);
            }
            target = Some(i);
        }
    }
    let target = target.ok_or(Error::HardInvariant)?;
    if d.atoms.len() > usize::from(l.max_atoms) || d.atoms.len() > 8 {
        return Err(Error::InvalidMap);
    }
    for (i, m) in d.atoms.iter().enumerate() {
        w.charge(d.atoms.len() as u64 + 1)?;
        if usize::from(m.meaning) >= d.atoms.len()
            || d.atoms[..i]
                .iter()
                .any(|v| v.local == m.local || v.meaning == m.meaning)
            || m.grounding.start() == m.grounding.end()
            || input.source.slice(m.grounding).is_err()
        {
            return Err(Error::InvalidMap);
        }
    }
    query(input.source_query, d.atoms, l, w)?;
    query(input.goal.query, d.atoms, l, w)?;
    if input.source_query.boolean_symbols() != input.goal.query.boolean_symbols() {
        return Err(Error::UnsupportedSignature);
    }
    feasible(input.source_query, input.goal.query, d.atoms, w)?;
    Ok((mutable, target))
}
pub(super) fn feasible(
    theory: &SmtStatusQuery,
    goal: &SmtStatusQuery,
    atoms: &[AtomMap],
    w: &mut Work<'_>,
) -> Result<(), Error> {
    let count = 1usize
        .checked_shl(atoms.len() as u32)
        .ok_or(Error::InvalidMap)?;
    let mut feasible = false;
    for assignment in 0..count {
        w.assignment()?;
        let mut admitted = true;
        for e in theory.assertions() {
            admitted &= canonical::eval(e, assignment, atoms, w)?;
        }
        if admitted {
            feasible = true;
            for e in goal.assertions() {
                if !canonical::eval(e, assignment, atoms, w)? {
                    return Err(Error::HardInvariant);
                }
            }
        }
    }
    if !feasible {
        return Err(Error::ContextInconsistent);
    }
    Ok(())
}
pub(super) fn candidate(
    root: &RootPlan<'_, '_>,
    q: &SmtStatusQuery,
    w: &mut Work<'_>,
) -> Result<(), Error> {
    let s = &root.state;
    let d = &s.declaration;
    query(q, d.atoms, d.limits, w)?;
    if q.boolean_symbols() != s.source_query.boolean_symbols()
        || q.assertions().len() != s.premises.len()
    {
        return Err(Error::UnsupportedSignature);
    }
    let mut next = 0;
    for (i, p) in s.premises.iter().enumerate() {
        if i == s.target_index {
            continue;
        }
        w.charge(
            (d.limits.max_ast_nodes as u64)
                .checked_mul(2)
                .ok_or(Error::WorkBudget)?,
        )?;
        let actual = q.assertions().get(next).ok_or(Error::HardInvariant)?;
        if actual != p.formula {
            return Err(Error::HardInvariant);
        }
        next += 1;
    }
    // Only the one declared mutable target occupies the final assertion slot;
    // all immutable source assertions were checked above, not self-reported.
    feasible(q, s.goal.query, d.atoms, w)
}
