//! Shape/source/map admission is not archive completion or payload eligibility.
use super::*;
pub(super) fn inputs(
    root: &f::FrameworkRoot<'_, '_>,
    r: &Registration<'_>,
    w: &mut Work<'_>,
) -> Result<(), Error> {
    budget::limits(r.limits)?;
    if !root.native.bridge.proposals().is_empty() {
        return Err(Error::AlreadyGenerated);
    }
    if r.constructions.is_empty() || r.constructions.len() > r.limits.max_features {
        return Err(Error::PayloadBudget);
    }
    for f in root.declaration.universe {
        budget::weight(f.weight, r.limits, w)?;
    }
    if let ArchiveInput::Read(entries) = r.archive {
        if entries.len() > r.limits.max_entries {
            return Err(Error::PayloadBudget);
        }
        for e in entries {
            w.charge(256).map_err(native)?;
            if !std::ptr::eq(e.source, root.native.state.source) {
                return Err(Error::ForeignSource);
            }
            if e.query.assertions().is_empty() {
                return Err(Error::EmptyConstruction);
            }
            n::identity::query_shape(e.query, budget::native_limits(root, r.limits), w)
                .map_err(native)?;
            atoms(root, e)?;
        }
    }
    Ok(())
}
fn atoms(root: &f::FrameworkRoot<'_, '_>, e: &ArchiveEntry<'_>) -> Result<(), Error> {
    let native_atoms = root.native.state.declaration.atoms;
    if e.atoms.len() != native_atoms.len()
        || e.query.boolean_symbols().len() != native_atoms.len()
        || !e.query.integer_symbols().is_empty()
    {
        return Err(Error::InvalidMap);
    }
    let mut meanings = [false; 3];
    for (i, a) in e.atoms.iter().enumerate() {
        let slot = usize::from(a.meaning);
        if slot >= native_atoms.len()
            || meanings[slot]
            || !e.query.boolean_symbols().contains(&a.local)
            || e.atoms[..i].iter().any(|b| b.local == a.local)
        {
            return Err(Error::InvalidMap);
        }
        let original = native_atoms
            .iter()
            .find(|v| v.meaning == a.meaning)
            .ok_or(Error::InvalidMap)?;
        if original.grounding != a.grounding {
            return Err(Error::InvalidMap);
        }
        meanings[slot] = true;
    }
    Ok(())
}
