//! Independent outer reference checks. NEVER builds S, sorts fields or feeds A.
//! This is verification work, separate from observed A/P/I execution fuel.
use super::*;
mod models;
mod nodes;
pub(super) fn check(
    raw: &Term,
    expected: &Term,
    view: &ResourceReservationView<'_>,
) -> Result<u64, CostError> {
    let op = Operational::new(view);
    let w = &op;
    let stream = fields(w, raw, "field-stream-v1", 3)?;
    let native = fields(w, &stream[0], "native-fields-v1", 3)?;
    let product = fields(w, expected, "tlcx-finite-product-v1", 3)?;
    cases(w, &native[0], &product[0], &stream[1], &stream[2])?;
    models::temporal(w, &native[1], &product[1], &stream[1], &stream[2])?;
    models::spatial(w, &native[2], &product[2], &stream[2])?;
    w.resources.check()?;
    Ok(w.used.get())
}
fn fields<'a>(
    w: &Operational<'_>,
    term: &'a Term,
    name: &str,
    n: usize,
) -> Result<&'a [Term], CostError> {
    w.bump(1 + name.len() as u64)?;
    let f = term::children(term, name)?;
    if f.len() != n {
        return Err(CostError::CorrespondenceMismatch);
    }
    Ok(f)
}
fn equal(w: &Operational<'_>, a: &Term, b: &Term) -> Result<(), CostError> {
    if !same(w, a, b)? {
        Err(CostError::CorrespondenceMismatch)
    } else {
        Ok(())
    }
}
fn index(w: &Operational<'_>, t: &Term, n: usize) -> Result<usize, CostError> {
    w.bump(1)?;
    let i = usize::try_from(term::unsigned(t)?).map_err(|_| CostError::ResourceExhausted)?;
    if i >= n {
        return Err(CostError::CorrespondenceMismatch);
    }
    Ok(i)
}
fn cases(
    w: &Operational<'_>,
    raw: &Term,
    expected: &Term,
    declarations: &Term,
    bindings: &Term,
) -> Result<(), CostError> {
    let actual = term::children(raw, "cases-v1")?;
    let slots = term::children(expected, "response-cases-v1")?;
    if actual.len() != slots.len() || actual.is_empty() || actual.len() > 2 {
        return Err(CostError::CorrespondenceMismatch);
    }
    let mut seen = vec![false; actual.len()];
    for (slot, expected) in slots.iter().enumerate() {
        let e = fields(w, expected, "case-slot-v1", 3)?;
        equal(w, &e[0], &int(slot as i128))?;
        let mut found = None;
        for (i, case) in actual.iter().enumerate() {
            let c = fields(w, case, "case-fields-v1", 8)?;
            if same(w, &c[7], &e[1])? && found.replace((i, c)).is_some() {
                return Err(CostError::CorrespondenceMismatch);
            }
        }
        let (i, c) = found.ok_or(CostError::CorrespondenceMismatch)?;
        if seen[i] {
            return Err(CostError::CorrespondenceMismatch);
        }
        seen[i] = true;
        let spec = fields(w, &e[1], "stage-b-case-spec-v1", 5)?;
        equal(w, &spec[0], &sym("BooleanReferenceAgreement"))?;
        equal(w, &c[5], &spec[3])?;
        equal(w, &c[6], &spec[4])?;
        let association_row = association(w, bindings, &c[0], "CaseSpec")?;
        equal(w, &association_row[2], &sym(&format!("Case/{slot}/Spec")))?;
        equal(w, &association_row[3], &e[1])?;
        let atoms = term::children(&c[1], "atoms-fields-v1")?;
        let meanings = term::children(&spec[1], "atoms-v1")?;
        if atoms.len() != meanings.len() || atoms.len() > 4 {
            return Err(CostError::CorrespondenceMismatch);
        }
        for (i, (a, m)) in atoms.iter().zip(meanings).enumerate() {
            let a = fields(w, a, "atom-fields-v1", 3)?;
            equal(w, &a[1], &int(i as i128))?;
            equal(w, &a[2], m)?;
        }
        let theory = term::children(&c[2], "theory-fields-v1")?;
        let expected_theory = term::children(&spec[2], "theory-v1")?;
        if theory.len() != expected_theory.len() {
            return Err(CostError::CorrespondenceMismatch);
        }
        for (raw, e) in theory.iter().zip(expected_theory) {
            nodes::boolean(w, raw, e, meanings)?;
        }
        // CaseReference is a FULL semantic declaration; never infer oracle content.
        let reference = declaration(
            w,
            declarations,
            "CaseReference",
            &format!("Case/{slot}/Reference"),
        )?;
        nodes::boolean(w, &c[3], reference, meanings)?;
        nodes::boolean(w, &c[4], &e[2], meanings)?;
    }
    if seen.contains(&false) {
        return Err(CostError::CorrespondenceMismatch);
    }
    Ok(())
}
fn association<'a>(
    w: &Operational<'_>,
    bindings: &'a Term,
    token: &Term,
    kind: &str,
) -> Result<&'a [Term], CostError> {
    let mut found = None;
    for b in term::children(bindings, "admission-bindings-v1")? {
        let b = fields(w, b, "admission-binding-v1", 4)?;
        if same(w, &b[1], &sym(kind))? && same(w, &b[0], token)? && found.replace(b).is_some() {
            return Err(CostError::CorrespondenceMismatch);
        }
    }
    found.ok_or(CostError::CorrespondenceMismatch)
}
fn declaration<'a>(
    w: &Operational<'_>,
    rows: &'a Term,
    kind: &str,
    path: &str,
) -> Result<&'a Term, CostError> {
    let mut found = None;
    for r in term::children(rows, "declarations-v1")? {
        let r = fields(w, r, "declaration-v1", 3)?;
        if same(w, &r[0], &sym(kind))?
            && same(w, &r[1], &sym(path))?
            && found.replace(&r[2]).is_some()
        {
            return Err(CostError::CorrespondenceMismatch);
        }
    }
    found.ok_or(CostError::CorrespondenceMismatch)
}

/// Bounded outer verification work, not adapter fuel and never semantic input.
struct Operational<'a> {
    resources: &'a dyn machine::Resources,
    used: std::cell::Cell<u64>,
}
impl<'a> Operational<'a> {
    fn new(resources: &'a dyn machine::Resources) -> Self {
        Self {
            resources,
            used: std::cell::Cell::new(0),
        }
    }
    fn bump(&self, n: u64) -> Result<(), CostError> {
        let mut left = n;
        while left > 0 {
            self.resources.check()?;
            let chunk = left.min(1024);
            self.used.set(
                self.used
                    .get()
                    .checked_add(chunk)
                    .ok_or(CostError::ResourceExhausted)?,
            );
            left -= chunk;
        }
        self.resources.check()
    }
}
impl machine::Work for &Operational<'_> {
    fn debit_work(&mut self, n: u64) -> Result<(), CostError> {
        self.bump(n)
    }
}
fn same(w: &Operational<'_>, a: &Term, b: &Term) -> Result<bool, CostError> {
    let mut meter = w;
    codec::equal_term(a, b, &mut meter)
}
pub(super) fn bytes_equal(
    a: &[u8],
    b: &[u8],
    view: &ResourceReservationView<'_>,
) -> Result<u64, CostError> {
    let w = Operational::new(view);
    w.bump(1)?;
    if a.len() != b.len() {
        return Err(CostError::CorrespondenceMismatch);
    }
    for (a, b) in a.chunks(1024).zip(b.chunks(1024)) {
        w.bump(
            (a.len() as u64)
                .checked_mul(2)
                .ok_or(CostError::ResourceExhausted)?,
        )?;
        if a != b {
            return Err(CostError::CorrespondenceMismatch);
        }
        w.resources.check()?;
    }
    Ok(w.used.get())
}
