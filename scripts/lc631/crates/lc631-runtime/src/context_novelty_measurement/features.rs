//! Fixed typed universes and actual AST extraction; no ID-distance or NLP.
use super::*;
pub(super) fn facts(d: &Declaration<'_>, w: &mut Work<'_>) -> Result<Universe, Error> {
    build(
        d.facts.iter().map(|f| (f.formula, f.weight)),
        d.facts.len(),
        true,
        d,
        w,
    )
}
pub(super) fn constructions(d: &Declaration<'_>, w: &mut Work<'_>) -> Result<Universe, Error> {
    build(
        d.constructions.iter().map(|f| (f.formula, f.weight)),
        d.constructions.len(),
        false,
        d,
        w,
    )
}
fn build<'a>(
    items: impl Iterator<Item = (&'a BoolExpr, &'a Rational)>,
    n: usize,
    fact: bool,
    d: &Declaration<'_>,
    w: &mut Work<'_>,
) -> Result<Universe, Error> {
    if n > d.limits.max_features {
        return Err(Error::PayloadBudget);
    }
    let mut features: Vec<CanonicalFeature> = resources::vector(n, 256, w)?;
    let mut total = 0usize;
    for (formula, weight) in items {
        numeric::weight(weight, d.limits, w)?;
        let payload = if fact {
            canonical::truth(formula, d.atoms, d.limits, w)?
        } else {
            canonical::construction(formula, d.atoms, d.limits, w)?
        };
        total = total
            .checked_add(payload.len())
            .ok_or(Error::PayloadBudget)?;
        if payload.len() > 4096 || total > d.limits.max_payload_bytes {
            return Err(Error::PayloadBudget);
        }
        w.charge((weight.numer().bits() + weight.denom().bits()) / 8 + 2)?;
        features.push(CanonicalFeature {
            payload,
            weight: weight.clone(),
        });
    }
    for i in 1..features.len() {
        let mut j = i;
        while j > 0 {
            w.charge((features[j - 1].payload.len() + features[j].payload.len()) as u64 + 1)?;
            if features[j - 1].payload == features[j].payload {
                return Err(Error::DuplicateFeature);
            }
            if features[j - 1].payload < features[j].payload {
                break;
            }
            features.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok(Universe { features })
}
fn insert(
    set: &mut FeatureSet,
    payload: &[u8],
    u: &Universe,
    w: &mut Work<'_>,
) -> Result<(), Error> {
    let mut found = None;
    for (i, f) in u.features.iter().enumerate() {
        w.charge((payload.len() + f.payload.len()) as u64 + 1)?;
        if f.payload == payload {
            found = Some(i);
            break;
        }
    }
    let index = found.ok_or(Error::UniverseMismatch)?;
    w.charge(set.len() as u64 + 1)?;
    if !set.contains(&index) {
        set.push(index);
    }
    Ok(())
}
fn finish(set: &mut [usize], w: &mut Work<'_>) -> Result<(), Error> {
    for i in 1..set.len() {
        let mut j = i;
        while j > 0 {
            w.charge(1)?;
            if set[j - 1] < set[j] {
                break;
            }
            set.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok(())
}
pub(super) fn source_facts(
    input: &RootInput<'_>,
    mutable: &[bool],
    u: &Universe,
    d: &Declaration<'_>,
    w: &mut Work<'_>,
) -> Result<FeatureSet, Error> {
    let mut set = resources::vector(input.premises.len(), 16, w)?;
    for (p, changeable) in input.premises.iter().zip(mutable) {
        if *changeable {
            let payload = canonical::truth(p.formula, d.atoms, d.limits, w)?;
            insert(&mut set, &payload, u, w)?;
        }
    }
    finish(&mut set, w)?;
    Ok(set)
}
pub(super) fn candidate_facts(
    root: &RootPlan<'_, '_>,
    q: &SmtStatusQuery,
    w: &mut Work<'_>,
) -> Result<FeatureSet, Error> {
    let s = &root.state;
    let d = &s.declaration;
    let mut set = resources::vector(s.premises.len(), 16, w)?;
    let mut next = 0;
    for (i, mutable) in s.mutable.iter().enumerate() {
        let slot = if i == s.target_index {
            q.assertions()
                .len()
                .checked_sub(1)
                .ok_or(Error::HardInvariant)?
        } else {
            let v = next;
            next += 1;
            v
        };
        if *mutable {
            let e = q.assertions().get(slot).ok_or(Error::HardInvariant)?;
            let payload = canonical::truth(e, d.atoms, d.limits, w)?;
            insert(&mut set, &payload, &s.facts, w)?;
        }
    }
    finish(&mut set, w)?;
    Ok(set)
}
pub(super) fn construction_set(
    q: &SmtStatusQuery,
    u: &Universe,
    d: &Declaration<'_>,
    w: &mut Work<'_>,
) -> Result<FeatureSet, Error> {
    let mut set = resources::vector(q.assertions().len(), d.limits.max_assertions, w)?;
    for e in q.assertions() {
        let payload = canonical::construction(e, d.atoms, d.limits, w)?;
        insert(&mut set, &payload, u, w)?;
    }
    finish(&mut set, w)?;
    Ok(set)
}
pub(super) fn kernel_universe<'a>(
    u: &'a Universe,
    w: &mut Work<'_>,
) -> Result<Vec<lc631_analysis::creativity_measurement::Feature<'a>>, Error> {
    let mut v = resources::vector(u.features.len(), 256, w)?;
    for f in &u.features {
        v.push(lc631_analysis::creativity_measurement::Feature {
            canonical_payload: &f.payload,
            weight: &f.weight,
        });
    }
    Ok(v)
}
pub(super) fn keys<'a>(
    set: &FeatureSet,
    u: &'a Universe,
    w: &mut Work<'_>,
) -> Result<Vec<&'a [u8]>, Error> {
    let mut v = resources::vector(set.len(), 256, w)?;
    for i in set {
        w.charge(1)?;
        v.push(
            u.features
                .get(*i)
                .ok_or(Error::UniverseMismatch)?
                .payload
                .as_slice(),
        );
    }
    Ok(v)
}
pub(super) fn distance(
    u: &Universe,
    left: &FeatureSet,
    right: &FeatureSet,
    l: Limits,
    w: &mut Work<'_>,
) -> Result<(RationalInterval, Rational), Error> {
    let denominator = numeric::pair(u, left, right, l, w)?;
    let universe = kernel_universe(u, w)?;
    let left = keys(left, u, w)?;
    let right = keys(right, u, w)?;
    let actual = lc631_analysis::creativity_measurement::set_distance(&universe, &left, &right)
        .map_err(Error::Arithmetic)?;
    Ok((actual, denominator))
}
