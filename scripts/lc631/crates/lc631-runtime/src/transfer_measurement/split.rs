//! Exact bounded alpha/And-Or-AC task normalization, with PREDEBITED work.
//! Gamma premise boundaries remain task roles; no arbitrary Boolean equivalence.
use super::*;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId};
pub(super) fn validate(input: TransferRootInput<'_>) -> Result<Digest32, Error> {
    if input.registry.is_empty() || input.registry.len() > 40 {
        return Err(Error::InvalidInput(e::InvalidInput::SizeLimit));
    }
    let mut remaining = input.limits.max_split_work;
    let mut canonical = k::vector(input.registry.len()).map_err(mapped)?;
    for (i, entry) in input.registry.iter().enumerate() {
        for bytes in [
            entry.key,
            entry.family,
            entry.clone_family,
            entry.source_payload,
        ] {
            if bytes.is_empty() || bytes.len() > 4096 {
                return Err(Error::InvalidInput(e::InvalidInput::SizeLimit));
            }
        }
        if input.registry[..i].iter().any(|p| p.key == entry.key) {
            return Err(Error::InvalidInput(e::InvalidInput::DuplicateCase));
        }
        // Admitted before allocation/normalization/sorting. Unit weight is local.
        let one = Rational::from_integer(1.into());
        k::admit_case(entry.key, entry.theory, entry.goal, &one).map_err(mapped)?;
        let mut nodes = 0;
        for a in entry.theory.assertions() {
            count(a, &mut nodes);
        }
        count(entry.goal, &mut nodes);
        let n = entry.theory.boolean_symbols().len();
        let mut permutations = 1u64;
        for v in 1..=n {
            permutations = permutations
                .checked_mul(v as u64)
                .ok_or(Error::SplitBudgetExhausted)?;
        }
        // Includes serialization/AC sorting overhead conservatively; no refund.
        let cost = permutations
            .checked_mul(
                (nodes as u64)
                    .checked_mul(64)
                    .and_then(|x| x.checked_add(512))
                    .ok_or(Error::SplitBudgetExhausted)?,
            )
            .ok_or(Error::SplitBudgetExhausted)?;
        if cost > remaining {
            return Err(Error::SplitBudgetExhausted);
        }
        remaining -= cost;
        canonical.push(canonical_task(entry)?);
    }
    for (i, entry) in input.registry.iter().enumerate() {
        for (j, prior) in input.registry[..i].iter().enumerate() {
            if canonical[i] == canonical[j] {
                return Err(Error::SplitLeakage(SplitLeakage::CanonicalContent));
            }
            if entry.role != prior.role {
                if entry.family == prior.family {
                    return Err(Error::SplitLeakage(SplitLeakage::Family));
                }
                if entry.clone_family == prior.clone_family {
                    return Err(Error::SplitLeakage(SplitLeakage::CloneFamily));
                }
                if entry.source_payload == prior.source_payload {
                    return Err(Error::SplitLeakage(SplitLeakage::SourceContent));
                }
            }
        }
    }
    let mut h = Sha256::new();
    h.update(b"predebited-split-normalization.v1");
    h.update(input.split_revision.as_bytes());
    h.update(input.normalizer_revision.as_bytes());
    for (entry, key) in input.registry.iter().zip(&canonical) {
        k::field(&mut h, entry.key);
        k::field(&mut h, entry.family);
        k::field(&mut h, entry.clone_family);
        k::field(&mut h, entry.source_payload);
        k::field(&mut h, key);
        h.update([match entry.role {
            e::DatasetRole::Training => 0,
            e::DatasetRole::Search => 1,
            e::DatasetRole::ExplanationValidation => 2,
            e::DatasetRole::TransferValidation => 3,
            e::DatasetRole::FinalResearchHoldout => 4,
        }]);
    }
    Ok(Digest32::from_bytes(h.finalize().into()))
}
fn count(expr: &BoolExpr, n: &mut usize) {
    *n += 1; // only after bounded AST admission, <=256
    match expr {
        BoolExpr::Not(c) => count(c, n),
        BoolExpr::And(v) | BoolExpr::Or(v) => {
            for c in v {
                count(c, n);
            }
        }
        _ => {}
    }
}
fn canonical_task(entry: &SplitEntry<'_>) -> Result<Vec<u8>, Error> {
    let n = entry.theory.boolean_symbols().len();
    let mut permutation = [0u8, 1, 2, 3, 4, 5, 6, 7];
    let mut best: Option<Vec<u8>> = None;
    loop {
        let mut premises = k::vector(entry.theory.assertions().len()).map_err(mapped)?;
        for expr in entry.theory.assertions() {
            premises.push(norm(expr, entry.theory.boolean_symbols(), &permutation)?);
        }
        premises.sort_unstable();
        let mut key = bytes()?;
        key.push(n as u8);
        key.push(premises.len() as u8);
        for p in premises {
            append(&mut key, &p);
        }
        append(
            &mut key,
            &norm(entry.goal, entry.theory.boolean_symbols(), &permutation)?,
        );
        if best.as_ref().is_none_or(|b| key < *b) {
            best = Some(key);
        }
        if !next(&mut permutation[..n]) {
            break;
        }
    }
    best.ok_or(Error::InvalidInput(e::InvalidInput::SizeLimit))
}
fn bytes() -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    out.try_reserve_exact(2048)
        .map_err(|_| Error::AllocationFailed)?;
    Ok(out)
}
pub(super) fn order_key(expr: &BoolExpr, symbols: &[BoolSymbolId]) -> Result<Vec<u8>, Error> {
    norm(expr, symbols, &[0, 1, 2, 3, 4, 5, 6, 7])
}
fn append(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u16).to_le_bytes());
    out.extend_from_slice(value);
}
fn norm(expr: &BoolExpr, symbols: &[BoolSymbolId], map: &[u8; 8]) -> Result<Vec<u8>, Error> {
    let mut out = bytes()?;
    match expr {
        BoolExpr::Constant(v) => out.extend_from_slice(&[0, u8::from(*v)]),
        BoolExpr::Variable(id) => {
            let index = symbols
                .iter()
                .position(|v| v == id)
                .ok_or(Error::InvalidInput(e::InvalidInput::UndeclaredAtom))?;
            out.extend_from_slice(&[1, map[index]]);
        }
        BoolExpr::Not(child) => {
            out.push(2);
            append(&mut out, &norm(child, symbols, map)?);
        }
        BoolExpr::And(_) | BoolExpr::Or(_) => {
            let and = matches!(expr, BoolExpr::And(_));
            let mut children = k::vector(256).map_err(mapped)?;
            flatten(expr, and, symbols, map, &mut children)?;
            children.sort_unstable();
            out.push(if and { 3 } else { 4 });
            out.extend_from_slice(&(children.len() as u16).to_le_bytes());
            for child in children {
                append(&mut out, &child);
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Error::UnsupportedFragment)
        }
    }
    Ok(out)
}
fn flatten(
    expr: &BoolExpr,
    and: bool,
    symbols: &[BoolSymbolId],
    map: &[u8; 8],
    out: &mut Vec<Vec<u8>>,
) -> Result<(), Error> {
    match expr {
        BoolExpr::And(children) if and => {
            for c in children {
                flatten(c, and, symbols, map, out)?;
            }
        }
        BoolExpr::Or(children) if !and => {
            for c in children {
                flatten(c, and, symbols, map, out)?;
            }
        }
        _ => out.push(norm(expr, symbols, map)?),
    }
    Ok(())
}
fn next(p: &mut [u8]) -> bool {
    if p.len() < 2 {
        return false;
    }
    let Some(i) = (0..p.len() - 1).rev().find(|&i| p[i] < p[i + 1]) else {
        return false;
    };
    let Some(j) = (i + 1..p.len()).rev().find(|&j| p[i] < p[j]) else {
        return false;
    };
    p.swap(i, j);
    p[i + 1..].reverse();
    true
}
