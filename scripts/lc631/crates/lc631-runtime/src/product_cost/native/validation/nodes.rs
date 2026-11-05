//! Direct native-index tree↔semantic-tree checks, including exact node coverage.
use super::*;
pub(super) fn boolean(
    w: &Operational<'_>,
    raw: &Term,
    expected: &Term,
    meanings: &[Term],
) -> Result<(), CostError> {
    let f = fields(w, raw, "bool-ast-v1", 2)?;
    let nodes = term::children(&f[0], "nodes-v1")?;
    let mut seen = vec![false; nodes.len()];
    bool_node(
        w,
        nodes,
        index(w, &f[1], nodes.len())?,
        expected,
        meanings,
        &mut seen,
        0,
    )?;
    if seen.contains(&false) {
        return Err(CostError::CorrespondenceMismatch);
    }
    Ok(())
}
fn mark(w: &Operational<'_>, seen: &mut [bool], i: usize, depth: usize) -> Result<(), CostError> {
    w.bump(1)?;
    if depth > 64 || seen.len() > 32 || i >= seen.len() || seen[i] {
        return Err(CostError::UnsupportedProfile);
    }
    seen[i] = true;
    Ok(())
}
fn bool_node(
    w: &Operational<'_>,
    nodes: &[Term],
    i: usize,
    e: &Term,
    m: &[Term],
    seen: &mut [bool],
    depth: usize,
) -> Result<(), CostError> {
    mark(w, seen, i, depth)?;
    match &nodes[i] {
        Term::Apply(n, f) if n == "bool-constant-v1" && f.len() == 1 => equal(w, &f[0], e),
        Term::Apply(n, f) if n == "bool-variable-v1" && f.len() == 1 => {
            let e = fields(w, e, "atom-v1", 1)?;
            equal(w, &m[index(w, &f[0], m.len())?], &e[0])
        }
        Term::Apply(n, f) if n == "bool-not-v1" && f.len() == 1 => {
            let e = fields(w, e, "not-v1", 1)?;
            bool_node(
                w,
                nodes,
                index(w, &f[0], nodes.len())?,
                &e[0],
                m,
                seen,
                depth + 1,
            )
        }
        Term::Apply(n, f) if matches!(n.as_str(), "bool-and-v1" | "bool-or-v1") && f.len() == 1 => {
            let raw = term::children(&f[0], "indices-v1")?;
            let e = term::children(
                e,
                if n == "bool-and-v1" {
                    "and-v1"
                } else {
                    "or-v1"
                },
            )?;
            if raw.len() != e.len() {
                return Err(CostError::CorrespondenceMismatch);
            }
            for (i, e) in raw.iter().zip(e) {
                bool_node(w, nodes, index(w, i, nodes.len())?, e, m, seen, depth + 1)?;
            }
            Ok(())
        }
        _ => Err(CostError::CorrespondenceMismatch),
    }
}
pub(super) fn temporal(
    w: &Operational<'_>,
    nodes: &[Term],
    root: &Term,
    e: &Term,
    m: &[Term],
) -> Result<(), CostError> {
    let mut seen = vec![false; nodes.len()];
    time_node(w, nodes, index(w, root, nodes.len())?, e, m, &mut seen, 0)?;
    if seen.contains(&false) {
        return Err(CostError::UnsupportedProfile);
    }
    Ok(())
}
fn time_node(
    w: &Operational<'_>,
    nodes: &[Term],
    i: usize,
    e: &Term,
    m: &[Term],
    seen: &mut [bool],
    depth: usize,
) -> Result<(), CostError> {
    mark(w, seen, i, depth)?;
    let (name, f) = match &nodes[i] {
        Term::Apply(n, f) => (n.as_str(), f.as_slice()),
        _ => return Err(CostError::CorrespondenceMismatch),
    };
    match (name, f) {
        ("time-constant-v1", [v]) => equal(w, v, e),
        ("time-atom-v1", [v]) => {
            let e = fields(w, e, "atom-v1", 1)?;
            equal(w, &m[index(w, v, m.len())?], &e[0])
        }
        (name, [child]) => {
            let op = match name {
                "time-not-v1" => "not-v1",
                "time-strong-next-v1" => "strong-next-v1",
                "time-eventually-v1" => "eventually-v1",
                "time-always-v1" => "always-v1",
                _ => return Err(CostError::CorrespondenceMismatch),
            };
            time_node(
                w,
                nodes,
                index(w, child, nodes.len())?,
                &fields(w, e, op, 1)?[0],
                m,
                seen,
                depth + 1,
            )
        }
        (name, [left, right]) => {
            let op = match name {
                "time-and-v1" => "and-v1",
                "time-or-v1" => "or-v1",
                "time-until-v1" => "until-v1",
                _ => return Err(CostError::CorrespondenceMismatch),
            };
            let e = fields(w, e, op, 2)?;
            time_node(
                w,
                nodes,
                index(w, left, nodes.len())?,
                &e[0],
                m,
                seen,
                depth + 1,
            )?;
            time_node(
                w,
                nodes,
                index(w, right, nodes.len())?,
                &e[1],
                m,
                seen,
                depth + 1,
            )
        }
        _ => Err(CostError::CorrespondenceMismatch),
    }
}
