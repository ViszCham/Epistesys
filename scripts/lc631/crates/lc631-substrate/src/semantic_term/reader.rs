use super::{BinderKind, Term, TermError, TermLimits};
use crate::measurement_numeric::BigInt;
use num_bigint::Sign;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    nodes: usize,
    limits: TermLimits,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], limits: TermLimits) -> Self {
        Self {
            bytes,
            at: 0,
            nodes: 0,
            limits,
        }
    }
    fn byte(&mut self) -> Result<u8, TermError> {
        let b = *self.bytes.get(self.at).ok_or(TermError::InvalidInput)?;
        self.at += 1;
        Ok(b)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], TermError> {
        let end = self.at.checked_add(n).ok_or(TermError::Budget)?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or(TermError::InvalidInput)?;
        self.at = end;
        Ok(slice)
    }
    fn uint(&mut self) -> Result<usize, TermError> {
        let mut n = 0_usize;
        let mut shift = 0;
        loop {
            let b = self.byte()?;
            let low = usize::from(b & 127);
            if shift >= usize::BITS || low > (usize::MAX >> shift) {
                return Err(TermError::Budget);
            }
            n |= low << shift;
            if b & 128 == 0 {
                if shift > 0 && low == 0 {
                    return Err(TermError::InvalidInput);
                }
                return Ok(n);
            }
            shift += 7;
        }
    }
    fn node(&mut self, depth: usize) -> Result<(), TermError> {
        if depth > self.limits.max_depth || self.nodes >= self.limits.max_nodes {
            return Err(TermError::Budget);
        }
        self.nodes += 1;
        Ok(())
    }
    fn symbol(&mut self, depth: usize) -> Result<&'a str, TermError> {
        self.node(depth)?;
        if self.byte()? != 3 {
            return Err(TermError::InvalidInput);
        }
        let n = self.uint()?;
        std::str::from_utf8(self.take(n)?).map_err(|_| TermError::InvalidInput)
    }
    fn term(&mut self, depth: usize, bound: usize) -> Result<(), TermError> {
        self.node(depth)?;
        match self.byte()? {
            1 => {
                if self.byte()? > 1 {
                    return Err(TermError::InvalidInput);
                }
            }
            2 => {
                let sign = self.byte()?;
                if sign > 1 {
                    return Err(TermError::InvalidInput);
                }
                let n = self.uint()?;
                if n == 0 {
                    return Err(TermError::InvalidInput);
                }
                let mag = self.take(n)?;
                if (n > 1 && mag[n - 1] == 0) || (sign == 1 && n == 1 && mag[0] == 0) {
                    return Err(TermError::InvalidInput);
                }
            }
            3 => {
                let n = self.uint()?;
                std::str::from_utf8(self.take(n)?).map_err(|_| TermError::InvalidInput)?;
            }
            4 => {
                self.symbol(depth + 1)?;
                let n = self.uint()?;
                if n > self.limits.max_nodes.saturating_sub(self.nodes)
                    || n > self.bytes.len().saturating_sub(self.at) / 2
                {
                    return Err(TermError::Budget);
                }
                for _ in 0..n {
                    self.term(depth + 1, bound)?;
                }
            }
            5 => {
                if self.byte()? > 2 {
                    return Err(TermError::InvalidInput);
                }
                self.symbol(depth + 1)?;
                self.term(depth + 1, bound + 1)?;
            }
            6 => {
                if self.uint()? >= bound {
                    return Err(TermError::InvalidInput);
                }
            }
            _ => return Err(TermError::Unsupported),
        }
        Ok(())
    }
    // Only called after full validation. Reuses the same checked cursor; no
    // public unchecked parser or externally supplied success status exists.
    fn owned(&mut self) -> Result<Term, TermError> {
        Ok(match self.byte()? {
            1 => Term::Bool(self.byte()? == 1),
            2 => {
                let sign = self.byte()?;
                let n = self.uint()?;
                Term::Integer(BigInt::from_bytes_le(
                    if sign == 1 { Sign::Minus } else { Sign::Plus },
                    self.take(n)?,
                ))
            }
            3 => {
                let n = self.uint()?;
                Term::Symbol(copy_string(self.take(n)?)?)
            }
            4 => {
                let op = self.owned_symbol()?;
                let n = self.uint()?;
                let mut children = Vec::new();
                children
                    .try_reserve_exact(n)
                    .map_err(|_| TermError::Allocation)?;
                for _ in 0..n {
                    children.push(self.owned()?);
                }
                Term::Apply(op, children)
            }
            5 => {
                let kind = match self.byte()? {
                    0 => BinderKind::Forall,
                    1 => BinderKind::Exists,
                    2 => BinderKind::Lambda,
                    _ => return Err(TermError::InvalidInput),
                };
                let sort = self.owned_symbol()?;
                Term::Binder(kind, sort, Box::new(self.owned()?))
            }
            6 => Term::BoundVar(self.uint()?),
            _ => return Err(TermError::Unsupported),
        })
    }
    fn owned_symbol(&mut self) -> Result<String, TermError> {
        if self.byte()? != 3 {
            return Err(TermError::InvalidInput);
        }
        let n = self.uint()?;
        copy_string(self.take(n)?)
    }
}
fn copy_string(bytes: &[u8]) -> Result<String, TermError> {
    let s = std::str::from_utf8(bytes).map_err(|_| TermError::InvalidInput)?;
    let mut out = String::new();
    out.try_reserve_exact(s.len())
        .map_err(|_| TermError::Allocation)?;
    out.push_str(s);
    Ok(out)
}
pub(super) fn validate(bytes: &[u8], limits: TermLimits) -> Result<(), TermError> {
    if bytes.len() > limits.max_bytes {
        return Err(TermError::Budget);
    }
    let mut r = Reader::new(bytes, limits);
    r.term(1, 0)?;
    if r.at != bytes.len() {
        return Err(TermError::InvalidInput);
    }
    Ok(())
}
pub(super) fn materialize(bytes: &[u8], limits: TermLimits) -> Result<Term, TermError> {
    Reader::new(bytes, limits).owned()
}
