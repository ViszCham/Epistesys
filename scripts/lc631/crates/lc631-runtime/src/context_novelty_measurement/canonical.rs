//! Root-global truth vectors and a DISTINCT Boolean AC construction grammar.
//! No candidate Γ conditions the truth vector; no parser/text/hash infers meaning.
use super::*;
pub(super) fn scan(
    expr: &BoolExpr,
    atoms: &[AtomMap],
    limits: Limits,
    work: &mut Work<'_>,
) -> Result<usize, Error> {
    let mut nodes = 0;
    visit(expr, atoms, limits, work, &mut nodes, 1)?;
    Ok(nodes)
}
fn visit(
    e: &BoolExpr,
    atoms: &[AtomMap],
    l: Limits,
    w: &mut Work<'_>,
    nodes: &mut usize,
    depth: usize,
) -> Result<(), Error> {
    w.charge(1)?;
    *nodes = nodes.checked_add(1).ok_or(Error::PayloadBudget)?;
    if *nodes > l.max_ast_nodes || depth > l.max_depth {
        return Err(Error::PayloadBudget);
    }
    match e {
        BoolExpr::Constant(_) => {}
        BoolExpr::Variable(id) => {
            meaning(*id, atoms, w)?;
        }
        BoolExpr::Not(child) => visit(child, atoms, l, w, nodes, depth + 1)?,
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            if children.is_empty() || children.len() > l.max_ast_nodes {
                return Err(Error::UnsupportedSignature);
            }
            for child in children {
                visit(child, atoms, l, w, nodes, depth + 1)?;
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Error::UnsupportedSignature)
        }
    }
    Ok(())
}
fn meaning(id: BoolSymbolId, atoms: &[AtomMap], w: &mut Work<'_>) -> Result<u8, Error> {
    w.charge(atoms.len() as u64 + 1)?;
    atoms
        .iter()
        .find(|m| m.local == id)
        .map(|m| m.meaning)
        .ok_or(Error::InvalidMap)
}
pub(super) fn eval(
    e: &BoolExpr,
    assignment: usize,
    atoms: &[AtomMap],
    w: &mut Work<'_>,
) -> Result<bool, Error> {
    w.charge(1)?;
    match e {
        BoolExpr::Constant(v) => Ok(*v),
        BoolExpr::Variable(id) => {
            let slot = usize::from(meaning(*id, atoms, w)?);
            let bit = atoms
                .len()
                .checked_sub(slot.checked_add(1).ok_or(Error::InvalidMap)?)
                .ok_or(Error::InvalidMap)?;
            Ok(assignment & (1usize << bit) != 0)
        }
        BoolExpr::Not(child) => Ok(!eval(child, assignment, atoms, w)?),
        BoolExpr::And(children) => {
            let mut v = true;
            for c in children {
                v &= eval(c, assignment, atoms, w)?;
            }
            Ok(v)
        }
        BoolExpr::Or(children) => {
            let mut v = false;
            for c in children {
                v |= eval(c, assignment, atoms, w)?;
            }
            Ok(v)
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            Err(Error::UnsupportedSignature)
        }
    }
}
pub(super) fn truth(
    e: &BoolExpr,
    atoms: &[AtomMap],
    l: Limits,
    w: &mut Work<'_>,
) -> Result<Vec<u8>, Error> {
    scan(e, atoms, l, w)?;
    let count = 1usize
        .checked_shl(atoms.len() as u32)
        .ok_or(Error::InvalidMap)?;
    let mut bytes = resources::vector(count, 256, w)?;
    for assignment in 0..count {
        w.assignment()?;
        bytes.push(u8::from(eval(e, assignment, atoms, w)?));
    }
    Ok(bytes)
}
fn append(out: &mut Vec<u8>, bytes: &[u8], cap: usize, w: &mut Work<'_>) -> Result<(), Error> {
    let end = out
        .len()
        .checked_add(bytes.len())
        .ok_or(Error::PayloadBudget)?;
    if end > cap {
        return Err(Error::PayloadBudget);
    }
    w.charge(bytes.len() as u64 + 1)?;
    out.extend_from_slice(bytes);
    Ok(())
}
fn collect<'e>(
    e: &'e BoolExpr,
    and: bool,
    out: &mut Vec<&'e BoolExpr>,
    cap: usize,
    w: &mut Work<'_>,
) -> Result<(), Error> {
    w.charge(1)?;
    match e {
        BoolExpr::And(c) if and => {
            for e in c {
                collect(e, and, out, cap, w)?;
            }
        }
        BoolExpr::Or(c) if !and => {
            for e in c {
                collect(e, and, out, cap, w)?;
            }
        }
        _ => {
            if out.len() >= cap {
                return Err(Error::PayloadBudget);
            }
            out.push(e);
        }
    }
    Ok(())
}
/// Prefix-free tags: const0/value, var1/meaning, not2/length/child,
/// And3 or Or4 / arity:u16 / (length:u32,child)*. Only And/Or flatten/sort.
pub(super) fn construction(
    e: &BoolExpr,
    atoms: &[AtomMap],
    l: Limits,
    w: &mut Work<'_>,
) -> Result<Vec<u8>, Error> {
    let nodes = scan(e, atoms, l, w)?;
    let cap = nodes
        .checked_mul(7)
        .and_then(|n| n.checked_add(8))
        .ok_or(Error::PayloadBudget)?
        .min(4096)
        .min(l.max_payload_bytes);
    let mut out = resources::vector(cap, 4096, w)?;
    match e {
        BoolExpr::Constant(v) => append(&mut out, &[0, u8::from(*v)], cap, w)?,
        BoolExpr::Variable(id) => {
            let m = meaning(*id, atoms, w)?;
            append(&mut out, &[1, m], cap, w)?;
        }
        BoolExpr::Not(child) => {
            let b = construction(child, atoms, l, w)?;
            append(&mut out, &[2], cap, w)?;
            append(&mut out, &(b.len() as u32).to_le_bytes(), cap, w)?;
            append(&mut out, &b, cap, w)?;
        }
        BoolExpr::And(_) | BoolExpr::Or(_) => {
            let and = matches!(e, BoolExpr::And(_));
            let mut flat = resources::vector(nodes, l.max_ast_nodes, w)?;
            collect(e, and, &mut flat, nodes, w)?;
            let mut children = resources::vector(flat.len(), l.max_ast_nodes, w)?;
            for child in flat {
                children.push(construction(child, atoms, l, w)?);
            }
            // Fallible bounded insertion sort: charge BEFORE payload comparison.
            for i in 1..children.len() {
                let mut j = i;
                while j > 0 {
                    w.charge((children[j - 1].len() + children[j].len()) as u64 + 1)?;
                    if children[j - 1] <= children[j] {
                        break;
                    }
                    children.swap(j - 1, j);
                    j -= 1;
                }
            }
            append(&mut out, &[if and { 3 } else { 4 }], cap, w)?;
            let arity = u16::try_from(children.len()).map_err(|_| Error::PayloadBudget)?;
            append(&mut out, &arity.to_le_bytes(), cap, w)?;
            for child in children {
                append(&mut out, &(child.len() as u32).to_le_bytes(), cap, w)?;
                append(&mut out, &child, cap, w)?;
            }
        }
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            return Err(Error::UnsupportedSignature)
        }
    }
    Ok(out)
}
