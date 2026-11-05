//! Sealed identity values and bounded immutable trees. No public token access.
use super::super::{term, CostError, CostLimits, Term};
use super::Resources;
use lc631_substrate::semantic_term::BinderKind;
use std::{cell::Cell, rc::Rc};
pub(in crate::product_cost) struct Node {
    pub(super) kind: Kind,
    pub(super) bytes: u64,
    pub(super) nodes: u64,
    pub(super) depth: u64,
    pub(super) tainted: bool,
    charge: u64,
    arena: Rc<Arena>,
}
pub(super) enum Kind {
    Atom(Term),
    Apply(String, Vec<Rc<Node>>),
    Binder(BinderKind, String, Rc<Node>),
    Opaque(String, u64),
}
pub(super) struct Arena {
    used: Cell<u64>,
    cap: u64,
    live: Cell<u64>,
    cells: u64,
}
impl Arena {
    pub fn new(l: CostLimits) -> Rc<Self> {
        Rc::new(Self {
            used: Cell::new(0),
            cap: l.additional_ram_bytes / 4,
            live: Cell::new(0),
            cells: l.value_cells,
        })
    }
}
impl Drop for Node {
    fn drop(&mut self) {
        self.arena.used.set(self.arena.used.get() - self.charge)
    }
}
pub(super) struct Value {
    pub node: Rc<Node>,
    arena: Rc<Arena>,
}
impl Drop for Value {
    fn drop(&mut self) {
        self.arena.live.set(self.arena.live.get() - 1)
    }
}
impl Value {
    pub fn new(node: Rc<Node>, arena: &Rc<Arena>) -> Result<Self, CostError> {
        let n = arena
            .live
            .get()
            .checked_add(1)
            .ok_or(CostError::ResourceExhausted)?;
        if n > arena.cells {
            return Err(CostError::ResourceExhausted);
        }
        arena.live.set(n);
        Ok(Self {
            node,
            arena: Rc::clone(arena),
        })
    }
}
fn uleb(mut n: u64) -> u64 {
    let mut out = 1;
    while n >= 128 {
        n >>= 7;
        out += 1
    }
    out
}
fn add(a: u64, b: u64) -> Result<u64, CostError> {
    a.checked_add(b).ok_or(CostError::ResourceExhausted)
}
pub(super) fn make(kind: Kind, a: &Rc<Arena>, l: CostLimits) -> Result<Rc<Node>, CostError> {
    let (mut bytes, mut nodes, mut depth, mut tainted, charge) = (0, 1, 1, false, 128u64);
    let charge = match &kind {
        Kind::Atom(t) => {
            bytes = match t {
                Term::BoundVar(i) => 1 + uleb(*i as u64),
                _ => term::length(t, l)?,
            };
            add(charge, bytes)?
        }
        Kind::Opaque(k, _) => {
            tainted = true;
            add(charge, k.len() as u64)?
        }
        Kind::Apply(op, c) => {
            bytes = add(
                add(2, uleb(op.len() as u64))?,
                add(op.len() as u64, uleb(c.len() as u64))?,
            )?;
            nodes = 2;
            depth = 2;
            for x in c {
                bytes = add(bytes, x.bytes)?;
                nodes = add(nodes, x.nodes)?;
                depth = depth.max(add(x.depth, 1)?);
                tainted |= x.tainted;
            }
            add(
                add(charge, op.len() as u64)?,
                (c.len() as u64)
                    .checked_mul(8)
                    .ok_or(CostError::ResourceExhausted)?,
            )?
        }
        Kind::Binder(_, sort, b) => {
            bytes = add(
                add(add(3, uleb(sort.len() as u64))?, sort.len() as u64)?,
                b.bytes,
            )?;
            nodes = add(2, b.nodes)?;
            depth = add(b.depth, 1)?.max(2);
            tainted = b.tainted;
            add(charge, sort.len() as u64)?
        }
    };
    if bytes > l.bundle_bytes || nodes > l.total_nodes || depth > l.depth {
        return Err(CostError::ResourceExhausted);
    }
    let next = add(a.used.get(), charge)?;
    if next > a.cap {
        return Err(CostError::ResourceExhausted);
    }
    a.used.set(next);
    Ok(Rc::new(Node {
        kind,
        bytes,
        nodes,
        depth,
        tainted,
        charge,
        arena: Rc::clone(a),
    }))
}
pub(super) fn input(
    t: &Term,
    a: &Rc<Arena>,
    l: CostLimits,
    allow_opaque: bool,
    resources: &dyn Resources,
) -> Result<Rc<Node>, CostError> {
    resources.check()?;
    let kind = match t {
        Term::Apply(n, c) if n == "opaque-v1" => {
            if !allow_opaque || c.len() != 2 {
                return Err(CostError::InvalidProgram);
            }
            Kind::Opaque(
                copy_text(term::symbol(&c[0])?, resources)?,
                term::unsigned(&c[1])?,
            )
        }
        Term::Apply(n, c) => {
            let mut xs = Vec::new();
            xs.try_reserve_exact(c.len())
                .map_err(|_| CostError::ResourceExhausted)?;
            for x in c {
                xs.push(input(x, a, l, allow_opaque, resources)?)
            }
            Kind::Apply(copy_text(n, resources)?, xs)
        }
        Term::Binder(k, s, b) => Kind::Binder(
            *k,
            copy_text(s, resources)?,
            input(b, a, l, allow_opaque, resources)?,
        ),
        Term::Symbol(s) => Kind::Atom(Term::Symbol(copy_text(s, resources)?)),
        _ => Kind::Atom(t.clone()),
    };
    resources.check()?;
    make(kind, a, l)
}
pub(super) fn copy_text(s: &str, r: &dyn Resources) -> Result<String, CostError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(s.len())
        .map_err(|_| CostError::ResourceExhausted)?;
    for c in s.as_bytes().chunks(1024) {
        r.check()?;
        bytes.extend_from_slice(c);
        r.check()?
    }
    String::from_utf8(bytes).map_err(|_| CostError::InvalidProgram)
}
pub(super) fn scalar(n: &Node) -> Result<&Term, CostError> {
    match &n.kind {
        Kind::Atom(t) => Ok(t),
        _ => Err(CostError::InvalidProgram),
    }
}
pub(super) fn unsigned(n: &Node) -> Result<u64, CostError> {
    term::unsigned(scalar(n)?)
}
pub(super) fn boolean(n: &Node) -> Result<bool, CostError> {
    match scalar(n)? {
        Term::Bool(b) => Ok(*b),
        _ => Err(CostError::InvalidProgram),
    }
}
pub(super) fn symbol(n: &Node) -> Result<&str, CostError> {
    term::symbol(scalar(n)?)
}
pub(super) fn seq(n: &Node) -> Result<&[Rc<Node>], CostError> {
    match &n.kind {
        Kind::Apply(s, c) if s == "seq" => Ok(c),
        _ => Err(CostError::InvalidProgram),
    }
}
pub(super) fn tag(n: &Node) -> &'static str {
    match &n.kind {
        Kind::Opaque(..) => "Opaque",
        Kind::Apply(..) => "Apply",
        Kind::Binder(..) => "Binder",
        Kind::Atom(Term::Bool(_)) => "Bool",
        Kind::Atom(Term::Integer(_)) => "Integer",
        Kind::Atom(Term::Symbol(_)) => "Symbol",
        Kind::Atom(Term::BoundVar(_)) => "BoundVar",
        Kind::Atom(_) => "Invalid",
    }
}
pub(super) fn encoded(
    n: &Node,
    _limits: CostLimits,
    resources: &dyn Resources,
) -> Result<Vec<u8>, CostError> {
    if n.tainted {
        return Err(CostError::InvalidProgram);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(n.bytes as usize)
        .map_err(|_| CostError::ResourceExhausted)?;
    // Node construction enforces limits; serialization uses that checked tree.
    write(n, &mut out, resources)?;
    resources.check()?;
    Ok(out)
}
fn uint(mut n: u64, b: &mut Vec<u8>) {
    loop {
        let low = (n & 127) as u8;
        n >>= 7;
        b.push(if n == 0 { low } else { low | 128 });
        if n == 0 {
            break;
        }
    }
}
fn copy_bytes(bytes: &[u8], b: &mut Vec<u8>, r: &dyn Resources) -> Result<(), CostError> {
    for c in bytes.chunks(1024) {
        r.check()?;
        b.extend_from_slice(c);
        r.check()?
    }
    Ok(())
}
fn symbol_bytes(s: &str, b: &mut Vec<u8>, r: &dyn Resources) -> Result<(), CostError> {
    b.push(3);
    uint(s.len() as u64, b);
    copy_bytes(s.as_bytes(), b, r)
}
fn write(n: &Node, b: &mut Vec<u8>, r: &dyn Resources) -> Result<(), CostError> {
    r.check()?;
    match &n.kind {
        Kind::Atom(Term::Bool(v)) => {
            b.push(1);
            b.push(u8::from(*v));
        }
        Kind::Atom(Term::Integer(n)) => {
            let (_, mut magnitude) = n.to_bytes_le();
            r.check()?;
            if magnitude.is_empty() {
                magnitude.push(0)
            }
            b.push(2);
            b.push(u8::from(
                n < &lc631_substrate::measurement_numeric::BigInt::from(0),
            ));
            uint(magnitude.len() as u64, b);
            copy_bytes(&magnitude, b, r)?;
        }
        Kind::Atom(Term::Symbol(s)) => symbol_bytes(s, b, r)?,
        Kind::Atom(Term::BoundVar(n)) => {
            b.push(6);
            uint(*n as u64, b)
        }
        Kind::Atom(_) => return Err(CostError::InvalidProgram),
        Kind::Apply(op, c) => {
            b.push(4);
            symbol_bytes(op, b, r)?;
            uint(c.len() as u64, b);
            for x in c {
                write(x, b, r)?
            }
        }
        Kind::Binder(k, s, x) => {
            b.push(5);
            b.push(match k {
                BinderKind::Forall => 0,
                BinderKind::Exists => 1,
                BinderKind::Lambda => 2,
            });
            symbol_bytes(s, b, r)?;
            write(x, b, r)?
        }
        Kind::Opaque(..) => return Err(CostError::InvalidProgram),
    }
    Ok(())
}
