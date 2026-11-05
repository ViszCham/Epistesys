//! Static validation includes unused bodies and the complete local call graph.
use super::super::{
    codec::{
        equal_str, forbidden_metered,
        package::{program, table, Function},
        Package,
    },
    term, CostError, CostLimits, Term,
};
use super::{Meter, Work};
pub(in crate::product_cost) fn validate_runtime(
    t: &Term,
    l: CostLimits,
    work: &mut Meter<'_>,
) -> Result<(), CostError> {
    forbidden_metered(t, work)?;
    let r = table(t, "runtime-v1", "operation-v1")?;
    validate(&[], &r, &[], l, true, work)
}
pub(in crate::product_cost) fn validate_package(
    p: &Package,
    l: CostLimits,
    work: &mut Meter<'_>,
) -> Result<(), CostError> {
    let h = table(&p.helpers, "helpers-v1", "function-v1")?;
    let r = table(&p.runtime, "runtime-v1", "operation-v1")?;
    let entries = [
        program(&p.adapter, "adapter")?,
        program(&p.projection, "projection")?,
        program(&p.inverse, "inverse")?,
    ];
    validate(&h, &r, &entries, l, false, work)
}
struct ValidationContext<'a, 'term> {
    limits: CostLimits,
    helpers: &'a [Function<'term>],
    runtime: &'a [Function<'term>],
    profile_only: bool,
}
fn validate(
    h: &[Function<'_>],
    r: &[Function<'_>],
    entries: &[&Term],
    l: CostLimits,
    profile_only: bool,
    work: &mut dyn Work,
) -> Result<(), CostError> {
    let n = h
        .len()
        .checked_add(r.len())
        .ok_or(CostError::ResourceExhausted)?;
    if n as u64 > l.local_entries {
        return Err(CostError::ResourceExhausted);
    }
    let context = ValidationContext {
        limits: l,
        helpers: h,
        runtime: r,
        profile_only,
    };
    let mut count = 0;
    for e in entries {
        expression(e, 2, 1, &mut count, &context, work)?
    }
    for f in h.iter().chain(r.iter()) {
        expression(f.body, f.arity, 1, &mut count, &context, work)?
    }
    // Cycle validation is an iterative graph walk. It does not reinterpret
    // Rust traversal frames as VM call depth; eval enforces actual VM depth.
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(n)
        .map_err(|_| CostError::ResourceExhausted)?;
    for f in h.iter().chain(r.iter()) {
        let mut calls = Vec::new();
        gather(f.body, &mut calls, 1, &context, work)?;
        edges.push(calls);
    }
    let mut colors = Vec::new();
    colors
        .try_reserve_exact(n)
        .map_err(|_| CostError::ResourceExhausted)?;
    colors.resize(n, 0u8);
    let mut stack = Vec::new();
    stack
        .try_reserve_exact(n)
        .map_err(|_| CostError::ResourceExhausted)?;
    for start in 0..n {
        if colors[start] != 0 {
            continue;
        }
        colors[start] = 1;
        stack.push((start, 0usize));
        while let Some((node, next)) = stack.last_mut() {
            work.debit_work(1)?;
            if *next == edges[*node].len() {
                colors[*node] = 2;
                stack.pop();
                continue;
            }
            let target = edges[*node][*next];
            *next += 1;
            match colors[target] {
                1 => return Err(CostError::InvalidProgram),
                2 => (),
                _ => {
                    colors[target] = 1;
                    stack.push((target, 0));
                }
            }
        }
    }
    Ok(())
}
fn lookup(name: &str, t: &[Function<'_>], work: &mut dyn Work) -> Result<Option<usize>, CostError> {
    for (i, f) in t.iter().enumerate() {
        if equal_str(name, f.name, work)? {
            return Ok(Some(i));
        }
    }
    Ok(None)
}
fn expression(
    e: &Term,
    arity: u64,
    depth: u64,
    count: &mut u64,
    context: &ValidationContext<'_, '_>,
    work: &mut dyn Work,
) -> Result<(), CostError> {
    let l = context.limits;
    let h = context.helpers;
    let r = context.runtime;
    let profile_only = context.profile_only;
    work.debit_work(1)?;
    if depth > l.depth {
        return Err(CostError::ResourceExhausted);
    }
    *count = count.checked_add(1).ok_or(CostError::ResourceExhausted)?;
    if *count > l.instructions {
        return Err(CostError::ResourceExhausted);
    }
    let (n, c) = match e {
        Term::Apply(n, c) => (n.as_str(), c.as_slice()),
        _ => return Err(CostError::InvalidProgram),
    };
    work.debit_work(n.len() as u64)?;
    match n {
        "op" | "helper" => {
            if c.is_empty() {
                return Err(CostError::InvalidProgram);
            }
            let name = term::symbol(&c[0])?;
            let t = if n == "op" { r } else { h };
            if let Some(i) = lookup(name, t, work)? {
                if t[i].arity as usize != c.len() - 1 {
                    return Err(CostError::InvalidProgram);
                }
            } else if !(profile_only && n == "helper") {
                return Err(if n == "op" {
                    CostError::InvalidProgram
                } else {
                    CostError::MissingDependency
                });
            }
            for x in &c[1..] {
                expression(x, arity, depth + 1, count, context, work)?
            }
        }
        _ => {
            let mut recurse = |xs: &[Term], a: u64, count: &mut u64| -> Result<(), CostError> {
                for x in xs {
                    expression(x, a, depth + 1, count, context, work)?
                }
                Ok(())
            };
            match n {
                "arg" => {
                    if c.len() != 1 || term::unsigned(&c[0])? >= arity {
                        return Err(CostError::InvalidProgram);
                    }
                }
                "literal" => {
                    if c.len() != 1 {
                        return Err(CostError::InvalidProgram);
                    }
                    forbidden_metered(&c[0], work)?
                }
                "child" | "equal" | "less" | "canonical-less" | "symbol-less" | "add"
                | "subtract" | "multiply" | "make" | "append" | "require" => {
                    if c.len() != 2 {
                        return Err(CostError::InvalidProgram);
                    }
                    recurse(c, arity, count)?
                }
                "arity" | "operator" | "tag" => {
                    if c.len() != 1 {
                        return Err(CostError::InvalidProgram);
                    }
                    recurse(c, arity, count)?
                }
                "if" => {
                    if c.len() != 3 {
                        return Err(CostError::InvalidProgram);
                    }
                    recurse(c, arity, count)?
                }
                "sequence" => recurse(c, arity, count)?,
                "dictionary" => {
                    if c.len() != 2 || term::unsigned(&c[0])? > 3 {
                        return Err(CostError::InvalidProgram);
                    }
                    recurse(&c[1..], arity, count)?
                }
                "fold" => {
                    if c.len() != 4 || term::unsigned(&c[0])? > l.iteration_bound {
                        return Err(CostError::InvalidProgram);
                    }
                    recurse(&c[1..3], arity, count)?;
                    expression(&c[3], 3, depth + 1, count, context, work)?
                }
                "fail" => {
                    if c.len() != 1 {
                        return Err(CostError::InvalidProgram);
                    }
                    term::symbol(&c[0])?;
                }
                "external-v1" => return Err(CostError::ExternalLookupForbidden),
                _ => return Err(CostError::InvalidProgram),
            }
        }
    }
    Ok(())
}
fn gather(
    e: &Term,
    calls: &mut Vec<usize>,
    depth: u64,
    context: &ValidationContext<'_, '_>,
    work: &mut dyn Work,
) -> Result<(), CostError> {
    let l = context.limits;
    let h = context.helpers;
    let r = context.runtime;
    let profile_only = context.profile_only;
    work.debit_work(1)?;
    if depth > l.depth {
        return Err(CostError::ResourceExhausted);
    }
    let (n, c) = match e {
        Term::Apply(n, c) => (n.as_str(), c.as_slice()),
        _ => return Err(CostError::InvalidProgram),
    };
    if n == "literal" || n == "arg" || n == "fail" {
        return Ok(());
    }
    let start = if n == "op" || n == "helper" {
        let name = term::symbol(&c[0])?;
        let t = if n == "op" { r } else { h };
        if let Some(i) = lookup(name, t, work)? {
            if calls.len() as u64 >= l.instructions {
                return Err(CostError::ResourceExhausted);
            }
            calls
                .try_reserve(1)
                .map_err(|_| CostError::ResourceExhausted)?;
            calls.push(if n == "op" { h.len() + i } else { i });
        } else if !(profile_only && n == "helper") {
            return Err(CostError::MissingDependency);
        }
        1
    } else if n == "fold" || n == "dictionary" {
        1
    } else {
        0
    };
    for x in &c[start..] {
        gather(x, calls, depth + 1, context, work)?
    }
    Ok(())
}
