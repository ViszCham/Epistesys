//! Allocation-free K6 admission and exact package structure checks.
use super::{term, CostError, CostLimits, Term};
pub(super) mod package;
pub(super) use package::Package;
pub(super) struct Scan<'a, 'm> {
    pub bytes: &'a [u8],
    pub at: usize,
    pub nodes: u64,
    pub limits: CostLimits,
    pub work: &'m mut dyn super::machine::Work,
}
impl<'a, 'm> Scan<'a, 'm> {
    pub fn new(
        bytes: &'a [u8],
        limits: CostLimits,
        work: &'m mut dyn super::machine::Work,
    ) -> Result<Self, CostError> {
        if bytes.len() as u64 > limits.bundle_bytes {
            return Err(CostError::ResourceExhausted);
        }
        Ok(Self {
            bytes,
            at: 0,
            nodes: 0,
            limits,
            work,
        })
    }
    pub fn byte(&mut self) -> Result<u8, CostError> {
        self.work.debit_work(1)?;
        let b = *self
            .bytes
            .get(self.at)
            .ok_or(CostError::InvalidCanonicalEncoding)?;
        self.at += 1;
        Ok(b)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], CostError> {
        self.work.debit_work(n as u64)?;
        let end = self.at.checked_add(n).ok_or(CostError::ResourceExhausted)?;
        let s = self
            .bytes
            .get(self.at..end)
            .ok_or(CostError::InvalidCanonicalEncoding)?;
        self.at = end;
        Ok(s)
    }
    fn uint(&mut self) -> Result<usize, CostError> {
        let (mut n, mut shift) = (0usize, 0);
        loop {
            let b = self.byte()?;
            let v = (b & 127) as usize;
            if shift >= usize::BITS || v > (usize::MAX >> shift) {
                return Err(CostError::InvalidCanonicalEncoding);
            }
            n |= v << shift;
            if b & 128 == 0 {
                if shift > 0 && v == 0 {
                    return Err(CostError::InvalidCanonicalEncoding);
                }
                return Ok(n);
            }
            shift += 7;
        }
    }
    fn node(&mut self, d: u64) -> Result<(), CostError> {
        self.work.debit_work(1)?;
        if d > self.limits.depth || self.nodes >= self.limits.total_nodes {
            return Err(CostError::ResourceExhausted);
        }
        self.nodes += 1;
        Ok(())
    }
    fn symbol(&mut self, d: u64) -> Result<&'a str, CostError> {
        self.node(d)?;
        if self.byte()? != 3 {
            return Err(CostError::InvalidCanonicalEncoding);
        }
        let n = self.uint()?;
        std::str::from_utf8(self.take(n)?).map_err(|_| CostError::InvalidCanonicalEncoding)
    }
    pub fn term(&mut self, d: u64, bound: usize) -> Result<(), CostError> {
        self.node(d)?;
        match self.byte()? {
            1 => {
                if self.byte()? > 1 {
                    return Err(CostError::InvalidCanonicalEncoding);
                }
            }
            2 => {
                let sign = self.byte()?;
                let n = self.uint()?;
                if sign > 1 || n == 0 {
                    return Err(CostError::InvalidCanonicalEncoding);
                }
                let m = self.take(n)?;
                if (n > 1 && m[n - 1] == 0) || (sign == 1 && n == 1 && m[0] == 0) {
                    return Err(CostError::InvalidCanonicalEncoding);
                }
            }
            3 => {
                let n = self.uint()?;
                std::str::from_utf8(self.take(n)?)
                    .map_err(|_| CostError::InvalidCanonicalEncoding)?;
            }
            4 => {
                self.symbol(d + 1)?;
                let n = self.uint()?;
                if n as u64 > self.limits.total_nodes - self.nodes {
                    return Err(CostError::ResourceExhausted);
                }
                for _ in 0..n {
                    self.term(d + 1, bound)?
                }
            }
            5 => {
                if self.byte()? > 2 {
                    return Err(CostError::InvalidCanonicalEncoding);
                }
                self.symbol(d + 1)?;
                self.term(d + 1, bound + 1)?
            }
            6 => {
                if self.uint()? >= bound {
                    return Err(CostError::InvalidCanonicalEncoding);
                }
            }
            _ => return Err(CostError::InvalidCanonicalEncoding),
        }
        Ok(())
    }
    pub fn header(&mut self, name: &str, count: usize, depth: u64) -> Result<(), CostError> {
        self.node(depth)?;
        if self.byte()? != 4 || self.symbol(depth + 1)? != name || self.uint()? != count {
            return Err(CostError::InvalidProgram);
        }
        Ok(())
    }
    pub fn span(&mut self, depth: u64) -> Result<std::ops::Range<usize>, CostError> {
        let start = self.at;
        self.term(depth, 0)?;
        Ok(start..self.at)
    }
}
pub(super) fn scan(
    bytes: &[u8],
    limits: CostLimits,
    work: &mut dyn super::machine::Work,
) -> Result<u64, CostError> {
    let mut s = Scan::new(bytes, limits, work)?;
    s.term(1, 0)?;
    if s.at != bytes.len() {
        return Err(CostError::InvalidCanonicalEncoding);
    }
    Ok(s.nodes)
}
pub(super) fn forbidden_metered(
    t: &Term,
    work: &mut dyn super::machine::Work,
) -> Result<(), CostError> {
    work.debit_work(1)?;
    match t {
        Term::Apply(n, c) => {
            if equal_str(n, "external-v1", work)? {
                return Err(CostError::ExternalLookupForbidden);
            }
            if equal_str(n, "opaque-v1", work)? {
                return Err(CostError::InvalidProgram);
            }
            for x in c {
                forbidden_metered(x, work)?
            }
        }
        Term::Binder(_, _, b) => forbidden_metered(b, work)?,
        _ => (),
    }
    Ok(())
}
pub(super) fn input_allowed(
    t: &Term,
    work: &mut dyn super::machine::Work,
) -> Result<(), CostError> {
    work.debit_work(1)?;
    match t {
        Term::Apply(n, c) => {
            if equal_str(n, "external-v1", work)? {
                return Err(CostError::ExternalLookupForbidden);
            }
            if n == "opaque-v1" {
                if c.len() != 2 {
                    return Err(CostError::InvalidProgram);
                }
                term::symbol(&c[0])?;
                term::unsigned(&c[1])?;
            } else {
                for x in c {
                    input_allowed(x, work)?
                }
            }
        }
        Term::Binder(_, _, b) => input_allowed(b, work)?,
        _ => (),
    }
    Ok(())
}
pub(super) fn equal_str(
    a: &str,
    b: &str,
    work: &mut dyn super::machine::Work,
) -> Result<bool, CostError> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.bytes().zip(b.bytes()) {
        work.debit_work(2)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(super) fn equal_term(
    a: &Term,
    b: &Term,
    work: &mut dyn super::machine::Work,
) -> Result<bool, CostError> {
    work.debit_work(2)?;
    Ok(match (a, b) {
        (Term::Bool(a), Term::Bool(b)) => a == b,
        (Term::Integer(a), Term::Integer(b)) => {
            work.debit_work(a.bits().div_ceil(8) + b.bits().div_ceil(8))?;
            a == b
        }
        (Term::Symbol(a), Term::Symbol(b)) => equal_str(a, b, work)?,
        (Term::Apply(na, a), Term::Apply(nb, b)) => {
            if !equal_str(na, nb, work)? || a.len() != b.len() {
                false
            } else {
                let mut same = true;
                for (a, b) in a.iter().zip(b) {
                    same &= equal_term(a, b, work)?
                }
                same
            }
        }
        (Term::Binder(ka, sa, a), Term::Binder(kb, sb, b)) => {
            ka == kb && equal_str(sa, sb, work)? && equal_term(a, b, work)?
        }
        (Term::BoundVar(a), Term::BoundVar(b)) => a == b,
        _ => false,
    })
}
pub(super) fn declarations(t: &Term) -> Result<&[Term], CostError> {
    term::children(t, "declarations-v1")
}
pub(super) fn declaration_rows<'a>(
    t: &'a Term,
    ordered: bool,
    limits: CostLimits,
    work: &mut dyn super::machine::Work,
) -> Result<&'a [Term], CostError> {
    let rows = term::children(t, "declarations-v1")?;
    let mut previous: Option<(&str, &str)> = None;
    if rows.len() as u64 > limits.local_entries {
        return Err(CostError::ResourceExhausted);
    }
    for (index, row) in rows.iter().enumerate() {
        work.debit_work(1)?;
        let c = term::children(row, "declaration-v1")?;
        if c.len() != 3 {
            return Err(CostError::InvalidProgram);
        }
        let k = term::symbol(&c[0])?;
        let p = term::symbol(&c[1])?;
        if ![
            "CaseSpec",
            "CaseReference",
            "BooleanAtom",
            "TemporalAtom",
            "Clock",
            "Origin",
            "Reference",
            "Frame",
        ]
        .contains(&k)
        {
            return Err(CostError::InvalidProgram);
        }
        if ordered {
            if let Some(old) = previous {
                work.debit_work((old.0.len() + old.1.len() + k.len() + p.len()) as u64)?;
                if old >= (k, p) {
                    return Err(CostError::InvalidProgram);
                }
            }
        }
        previous = Some((k, p));
        forbidden_metered(&c[2], work)?;
        if !ordered {
            for other in &rows[..index] {
                let other = term::children(other, "declaration-v1")?;
                if other.len() != 3 {
                    return Err(CostError::InvalidProgram);
                }
                if equal_term(&other[0], &c[0], work)? && equal_term(&other[1], &c[1], work)? {
                    return Err(CostError::CorrespondenceMismatch);
                }
            }
        }
    }
    Ok(rows)
}
