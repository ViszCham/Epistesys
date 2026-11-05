use super::super::{machine, term, CostError, CostLimits, InlinePackage, Term};
use super::{declaration_rows, declarations, forbidden_metered, scan};
pub(in crate::product_cost) struct Function<'a> {
    pub name: &'a str,
    pub arity: u64,
    pub body: &'a Term,
}
pub(in crate::product_cost) struct Package {
    pub adapter: Term,
    pub projection: Term,
    pub inverse: Term,
    pub helpers: Term,
    pub runtime: Term,
    pub dictionary: Term,
}
pub(in crate::product_cost) fn table<'a>(
    t: &'a Term,
    outer: &str,
    entry: &str,
) -> Result<Vec<Function<'a>>, CostError> {
    let rows = term::children(t, outer)?;
    if rows.len() > 1024 {
        return Err(CostError::ResourceExhausted);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(rows.len())
        .map_err(|_| CostError::ResourceExhausted)?;
    let mut last: Option<&str> = None;
    for row in rows {
        let c = term::children(row, entry)?;
        if c.len() != 3 {
            return Err(CostError::InvalidProgram);
        }
        let name = term::symbol(&c[0])?;
        if name.is_empty() || last.is_some_and(|v| v >= name) {
            return Err(CostError::InvalidProgram);
        }
        let arity = term::unsigned(&c[1])?;
        if arity > 1024 {
            return Err(CostError::InvalidProgram);
        }
        last = Some(name);
        out.push(Function {
            name,
            arity,
            body: &c[2],
        });
    }
    Ok(out)
}
pub(in crate::product_cost) fn program<'a>(t: &'a Term, name: &str) -> Result<&'a Term, CostError> {
    let c = term::children(t, "program-v1")?;
    if c.len() != 3 || term::symbol(&c[0])? != name || term::unsigned(&c[1])? != 2 {
        return Err(CostError::InvalidProgram);
    }
    Ok(&c[2])
}
impl Package {
    pub fn parse(
        p: InlinePackage<'_>,
        l: CostLimits,
        m: &mut machine::Meter<'_>,
        host_budget: u64,
        other_host: u64,
    ) -> Result<Self, CostError> {
        let pieces = [
            p.adapter,
            p.projection,
            p.inverse,
            p.helpers,
            p.runtime,
            p.dictionary,
        ];
        if pieces.iter().any(|b| b.is_empty()) {
            return Err(CostError::MissingDependency);
        }
        let total = pieces.iter().try_fold(0u64, |a, b| {
            a.checked_add(b.len() as u64)
                .ok_or(CostError::ResourceExhausted)
        })?;
        if total > l.bundle_bytes {
            return Err(CostError::ResourceExhausted);
        }
        let mut aggregate = 0u64;
        let mut counts = [0u64; 6];
        for (index, bytes) in pieces.iter().enumerate() {
            let nodes = scan(bytes, l, m)?;
            counts[index] = nodes;
            aggregate = aggregate
                .checked_add(nodes)
                .ok_or(CostError::ResourceExhausted)?;
            if aggregate > l.total_nodes {
                return Err(CostError::ResourceExhausted);
            }
        }
        // Logical upper bounds include owned Terms/strings/child vectors, local
        // tables/frames and output staging; allocator/RSS is not asserted.
        let host = total
            .checked_mul(8)
            .and_then(|v| aggregate.checked_mul(256).and_then(|n| v.checked_add(n)))
            .and_then(|v| v.checked_add(2 * 1024 * 1024))
            .and_then(|v| v.checked_add(other_host))
            .ok_or(CostError::ResourceExhausted)?;
        if host > host_budget {
            return Err(CostError::ResourceExhausted);
        }
        let mut parsed = Vec::new();
        parsed
            .try_reserve_exact(6)
            .map_err(|_| CostError::ResourceExhausted)?;
        for (bytes, nodes) in pieces.into_iter().zip(counts) {
            m.resources.check()?;
            m.chunks(2 * (bytes.len() as u64 + nodes))?;
            let t = term::decode(bytes, l)?;
            forbidden_metered(&t, m)?;
            parsed.push(t);
        }
        let mut it = parsed.into_iter();
        let package = Self {
            adapter: it.next().ok_or(CostError::MissingDependency)?,
            projection: it.next().ok_or(CostError::MissingDependency)?,
            inverse: it.next().ok_or(CostError::MissingDependency)?,
            helpers: it.next().ok_or(CostError::MissingDependency)?,
            runtime: it.next().ok_or(CostError::MissingDependency)?,
            dictionary: it.next().ok_or(CostError::MissingDependency)?,
        };
        package.validate(l, m)?;
        Ok(package)
    }
    pub fn validate(&self, l: CostLimits, m: &mut machine::Meter<'_>) -> Result<(), CostError> {
        program(&self.adapter, "adapter")?;
        program(&self.projection, "projection")?;
        program(&self.inverse, "inverse")?;
        let h = table(&self.helpers, "helpers-v1", "function-v1")?;
        let r = table(&self.runtime, "runtime-v1", "operation-v1")?;
        let d = term::children(&self.dictionary, "dictionary-v1")?;
        if d.len() != 4 {
            return Err(CostError::InvalidProgram);
        }
        declaration_rows(&d[0], true, l, m)?;
        let mut entries = h.len() + r.len();
        for (t, name) in d.iter().zip([
            "declarations-v1",
            "constants-v1",
            "helper-data-v1",
            "residuals-v1",
        ]) {
            entries = entries
                .checked_add(term::children(t, name)?.len())
                .ok_or(CostError::ResourceExhausted)?;
        }
        if entries as u64 > l.local_entries {
            return Err(CostError::ResourceExhausted);
        }
        machine::validate_package(self, l, m)
    }
    pub fn dictionary_rows(&self) -> Result<&[Term], CostError> {
        let c = term::children(&self.dictionary, "dictionary-v1")?;
        declarations(&c[0])
    }
}
