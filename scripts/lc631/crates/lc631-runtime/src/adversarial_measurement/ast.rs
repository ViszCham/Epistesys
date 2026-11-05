//! 日本語: bounded ASTの構造変換のみ。新しい真偽評価器・parserではない。
//! English: Bounded AST structural lowering only; no new truth evaluator/parser.
use super::*;
use lc631_solvers::typed_query::SmtEmitLimits;
pub(super) fn preflight_case(c: &AttackCase<'_>, bytes: &mut usize) -> Result<(), Error> {
    if !c.theory.integer_symbols().is_empty() {
        return Err(Error::UnsupportedFragment);
    }
    if c.atoms.len() > MAX_ATOMS || c.atoms.len() != c.theory.boolean_symbols().len() {
        return Err(Error::InvalidMap);
    }
    for (i, a) in c.atoms.iter().enumerate() {
        if a.meaning.is_empty()
            || !c.theory.boolean_symbols().contains(&a.symbol)
            || c.atoms[..i]
                .iter()
                .any(|b| a.symbol == b.symbol || a.meaning == b.meaning)
        {
            return Err(Error::InvalidMap);
        }
        *bytes = bytes
            .checked_add(a.meaning.len())
            .ok_or(Error::InvalidCase)?;
    }
    let mut count = 0;
    for e in c.theory.assertions() {
        scan(e, c.theory.boolean_symbols(), &mut count, 1)?;
    }
    scan(c.reference, c.theory.boolean_symbols(), &mut count, 1)?;
    Ok(())
}
fn scan(
    e: &BoolExpr,
    symbols: &[BoolSymbolId],
    nodes: &mut usize,
    depth: usize,
) -> Result<(), Error> {
    *nodes = nodes.checked_add(1).ok_or(Error::AstLimit)?;
    if *nodes > MAX_NODES || depth > MAX_DEPTH {
        return Err(Error::AstLimit);
    }
    match e {
        BoolExpr::Constant(_) => Ok(()),
        BoolExpr::Variable(id) => {
            if symbols.contains(id) {
                Ok(())
            } else {
                Err(Error::InvalidMap)
            }
        }
        BoolExpr::Not(e) => scan(e, symbols, nodes, depth + 1),
        BoolExpr::And(es) | BoolExpr::Or(es) => {
            if es.is_empty() {
                return Err(Error::UnsupportedFragment);
            }
            for e in es {
                scan(e, symbols, nodes, depth + 1)?;
            }
            Ok(())
        }
        _ => Err(Error::UnsupportedFragment),
    }
}
pub(super) fn preflight_response(e: &BoolExpr, atoms: &[Atom]) -> Result<(), Error> {
    let mut ids = [BoolSymbolId::new(0); MAX_ATOMS];
    for (i, a) in atoms.iter().enumerate() {
        ids[i] = a.symbol;
    }
    scan(e, &ids[..atoms.len()], &mut 0, 1)
}
fn copy(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len())
        .map_err(|_| Error::AllocationFailed)?;
    out.extend_from_slice(bytes);
    Ok(out)
}
pub(super) fn own_case(c: &AttackCase<'_>) -> Result<Case, Error> {
    let mut atoms = Vec::new();
    atoms
        .try_reserve_exact(c.atoms.len())
        .map_err(|_| Error::AllocationFailed)?;
    for a in c.atoms {
        atoms.push(Atom {
            symbol: a.symbol,
            meaning: copy(a.meaning)?,
        });
    }
    atoms.sort_by(|a, b| a.meaning.cmp(&b.meaning));
    let mut assertions = Vec::new();
    assertions
        .try_reserve_exact(c.theory.assertions().len())
        .map_err(|_| Error::AllocationFailed)?;
    for e in c.theory.assertions() {
        assertions.push(normalize(e, &atoms)?);
    }
    let reference = normalize(c.reference, &atoms)?;
    let theory = query(atoms.len(), assertions)?;
    let map = (0..atoms.len())
        .map(|i| {
            let id = BoolSymbolId::new(i as u32);
            (id, id)
        })
        .collect();
    Ok(Case {
        key: copy(c.key)?,
        atoms,
        theory,
        reference,
        weight: c.weight.clone(),
        map,
        registration_assignments: 0,
        registration_visits: 0,
    })
}
/// Called only AFTER full preflight on borrowed inputs. Positions correspond to
/// sorted declared semantic atom payloads, not opaque source symbol numbers.
pub(super) fn normalize(e: &BoolExpr, atoms: &[Atom]) -> Result<BoolExpr, Error> {
    match e {
        BoolExpr::Constant(b) => Ok(BoolExpr::Constant(*b)),
        BoolExpr::Variable(id) => Ok(BoolExpr::Variable(BoolSymbolId::new(
            atoms
                .iter()
                .position(|a| a.symbol == *id)
                .ok_or(Error::InvalidMap)? as u32,
        ))),
        BoolExpr::Not(e) => Ok(BoolExpr::Not(Box::new(normalize(e, atoms)?))),
        BoolExpr::And(es) | BoolExpr::Or(es) => {
            let mut out = Vec::new();
            out.try_reserve_exact(es.len())
                .map_err(|_| Error::AllocationFailed)?;
            for e in es {
                out.push(normalize(e, atoms)?);
            }
            Ok(if matches!(e, BoolExpr::And(_)) {
                BoolExpr::And(out)
            } else {
                BoolExpr::Or(out)
            })
        }
        _ => Err(Error::UnsupportedFragment),
    }
}
pub(super) fn query(atoms: usize, assertions: Vec<BoolExpr>) -> Result<SmtStatusQuery, Error> {
    let symbols = (0..atoms).map(|i| BoolSymbolId::new(i as u32)).collect();
    SmtStatusQuery::checked(
        symbols,
        vec![],
        assertions,
        SmtEmitLimits::checked(1024 * 1024, MAX_ATOMS, 0, 1024, 16384, 128)
            .map_err(Error::Query)?,
    )
    .map_err(Error::Query)
}
pub(super) fn request<'a>(
    b: CatalogBinding,
    c: &'a Case,
    left: &'a BoolExpr,
    right: &'a BoolExpr,
) -> formal::Request<'a> {
    formal::Request {
        source: &c.theory,
        target: &c.theory,
        source_predicate: left,
        target_predicate: right,
        symbols: &c.map,
        unconstrained: formal::UnconstrainedPolicy::ExplicitlyGiven,
        binding: formal::Binding {
            source: b.selection.source,
            target: b.selection.source,
            source_context: b.context,
            target_context: b.context,
            probe_set: b.probe_set,
            probe_map: Digest32::from_bytes([0; 32]),
        },
    }
}
pub(super) fn agreement(response: &BoolExpr, expected: &BoolExpr) -> BoolExpr {
    BoolExpr::Or(vec![
        BoolExpr::And(vec![response.clone(), expected.clone()]),
        BoolExpr::And(vec![
            BoolExpr::Not(Box::new(response.clone())),
            BoolExpr::Not(Box::new(expected.clone())),
        ]),
    ])
}
/// Complete failure proposition Γ ∧ ¬(response ↔ expected). Γ is fixed BOTH sides.
pub(super) fn failure(c: &Case, property: &BoolExpr) -> Result<SmtStatusQuery, Error> {
    let mut assertions = c.theory.assertions().to_vec();
    assertions.push(BoolExpr::Not(Box::new(property.clone())));
    query(c.atoms.len(), assertions)
}
pub(super) fn hash_expr(h: &mut Sha256, e: &BoolExpr) {
    match e {
        BoolExpr::Constant(b) => h.update([0, u8::from(*b)]),
        BoolExpr::Variable(id) => {
            h.update([1]);
            h.update(id.get().to_le_bytes());
        }
        BoolExpr::Not(e) => {
            h.update([2]);
            hash_expr(h, e);
        }
        BoolExpr::And(es) | BoolExpr::Or(es) => {
            h.update([if matches!(e, BoolExpr::And(_)) { 3 } else { 4 }]);
            h.update((es.len() as u64).to_le_bytes());
            for e in es {
                hash_expr(h, e);
            }
        }
        _ => unreachable!("private lowered Boolean fragment"),
    }
}
