use super::{BinderKind, Term, TermError, TermLimits};
use num_bigint::Sign;
fn uint_len(mut n: usize) -> usize {
    let mut len = 1;
    while n >= 128 {
        n >>= 7;
        len += 1;
    }
    len
}
fn uint(mut n: usize, out: &mut Vec<u8>) {
    while n >= 128 {
        out.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
struct Size {
    bytes: usize,
    nodes: usize,
    limits: TermLimits,
}
impl Size {
    fn add(&mut self, n: usize) -> Result<(), TermError> {
        self.bytes = self.bytes.checked_add(n).ok_or(TermError::Budget)?;
        if self.bytes > self.limits.max_bytes {
            return Err(TermError::Budget);
        }
        Ok(())
    }
    fn node(&mut self, depth: usize) -> Result<(), TermError> {
        if depth > self.limits.max_depth || self.nodes >= self.limits.max_nodes {
            return Err(TermError::Budget);
        }
        self.nodes += 1;
        self.add(1)
    }
    fn symbol(&mut self, s: &str, depth: usize) -> Result<(), TermError> {
        self.node(depth)?;
        self.add(uint_len(s.len()))?;
        self.add(s.len())
    }
    fn term(&mut self, term: &Term, depth: usize, bound: usize) -> Result<(), TermError> {
        self.node(depth)?;
        match term {
            Term::Bool(_) => self.add(1)?,
            Term::Integer(n) => {
                let len = usize::try_from(n.bits().div_ceil(8))
                    .map_err(|_| TermError::Budget)?
                    .max(1);
                self.add(1 + uint_len(len))?;
                self.add(len)?;
            }
            Term::Symbol(s) => {
                self.add(uint_len(s.len()))?;
                self.add(s.len())?;
            }
            Term::Apply(op, args) => {
                self.symbol(op, depth + 1)?;
                self.add(uint_len(args.len()))?;
                for arg in args {
                    self.term(arg, depth + 1, bound)?;
                }
            }
            Term::Binder(_, sort, body) => {
                self.add(1)?;
                self.symbol(sort, depth + 1)?;
                self.term(body, depth + 1, bound + 1)?;
            }
            Term::BoundVar(index) => {
                if *index >= bound {
                    return Err(TermError::InvalidInput);
                }
                self.add(uint_len(*index))?;
            }
        }
        Ok(())
    }
}
pub(super) fn length(term: &Term, limits: TermLimits) -> Result<usize, TermError> {
    let mut s = Size {
        bytes: 0,
        nodes: 0,
        limits,
    };
    s.term(term, 1, 0)?;
    Ok(s.bytes)
}
pub(super) fn bundle_limits(parts: [&Term; 3], limits: TermLimits) -> Result<(), TermError> {
    let mut s = Size {
        bytes: 13,
        nodes: 2,
        limits,
    };
    if limits.max_nodes < 2 || limits.max_depth < 2 {
        return Err(TermError::Budget);
    }
    for part in parts {
        s.term(part, 2, 0)?;
    }
    Ok(())
}
fn symbol(s: &str, out: &mut Vec<u8>) {
    out.push(3);
    uint(s.len(), out);
    out.extend_from_slice(s.as_bytes());
}
pub(super) fn write(term: &Term, out: &mut Vec<u8>) {
    match term {
        Term::Bool(b) => {
            out.push(1);
            out.push(u8::from(*b));
        }
        Term::Integer(n) => {
            let (sign, mut bytes) = n.to_bytes_le();
            if bytes.is_empty() {
                bytes.push(0);
            }
            out.push(2);
            out.push(u8::from(sign == Sign::Minus));
            uint(bytes.len(), out);
            out.extend_from_slice(&bytes);
        }
        Term::Symbol(s) => symbol(s, out),
        Term::Apply(op, args) => {
            out.push(4);
            symbol(op, out);
            uint(args.len(), out);
            for arg in args {
                write(arg, out);
            }
        }
        Term::Binder(kind, sort, body) => {
            out.push(5);
            out.push(match kind {
                BinderKind::Forall => 0,
                BinderKind::Exists => 1,
                BinderKind::Lambda => 2,
            });
            symbol(sort, out);
            write(body, out);
        }
        Term::BoundVar(index) => {
            out.push(6);
            uint(*index, out);
        }
    }
}
