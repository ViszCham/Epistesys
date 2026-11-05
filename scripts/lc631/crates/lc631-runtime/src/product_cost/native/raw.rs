//! 日本語: owner の raw shape のみをコピー。意味整列/lowering は課金 A の仕事。
//! English: Copy native shape only; charged A owns semantic ordering/lowering.
use super::*;
use crate::product_correspondence::{IdentityKind, PlanData};
mod boolean;
pub(super) mod cases;
mod models;
pub(super) use cases::collect_cases;
pub(super) use models::{spatial, temporal};

pub(super) struct Registry {
    entries: Vec<(&'static str, Vec<u8>)>,
    work: std::cell::Cell<u64>,
}
impl Registry {
    pub(super) fn declared(data: &PlanData) -> Result<Self, CostError> {
        let mut result = Self {
            entries: Vec::new(),
            work: std::cell::Cell::new(0),
        };
        for identity in &data.identities {
            result.token(kind(identity.kind), &identity.native_id)?;
        }
        for case in &data.cases {
            result.token("Case", &case.native_key)?;
            for atom in &case.native_atoms {
                let key = symbol_key(&case.native_key, atom.symbol.get())?;
                result.token("BooleanSymbol", &key)?;
            }
        }
        Ok(result)
    }
    fn token(&mut self, kind: &'static str, bytes: &[u8]) -> Result<Term, CostError> {
        let index = match self.find(kind, bytes)? {
            Some(i) => i,
            None => {
                let i = self.entries.len();
                self.entries.push((kind, term::copy(bytes)?));
                i
            }
        };
        Ok(app("opaque-v1", vec![sym(kind), int(index as i128)]))
    }
    pub(super) fn existing(&self, kind: &str, bytes: &[u8]) -> Result<Term, CostError> {
        let i = self
            .find(kind, bytes)?
            .ok_or(CostError::CorrespondenceMismatch)?;
        Ok(app("opaque-v1", vec![sym(kind), int(i as i128)]))
    }
    fn find(&self, kind: &str, bytes: &[u8]) -> Result<Option<usize>, CostError> {
        for (index, (k, b)) in self.entries.iter().enumerate() {
            // Observed operational equality work, separate from semantic A fuel.
            let work = 1u64
                .checked_add(k.len() as u64)
                .and_then(|n| {
                    n.checked_add(if *k == kind {
                        b.len().min(bytes.len()) as u64
                    } else {
                        0
                    })
                })
                .ok_or(CostError::ResourceExhausted)?;
            self.work.set(
                self.work
                    .get()
                    .checked_add(work)
                    .ok_or(CostError::ResourceExhausted)?,
            );
            if *k == kind && b == bytes {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
    pub(super) fn work(&self) -> u64 {
        self.work.get()
    }
    pub(super) fn symbol(&self, case: &[u8], raw: u32) -> Result<Term, CostError> {
        self.existing("BooleanSymbol", &symbol_key(case, raw)?)
    }
    pub(super) fn bindings(&self, data: &PlanData) -> Result<Term, CostError> {
        let mut rows = Vec::new();
        for identity in &data.identities {
            let path = identity_path(&identity.meaning)?;
            rows.push(app(
                "admission-binding-v1",
                vec![
                    self.existing(kind(identity.kind), &identity.native_id)?,
                    sym(kind(identity.kind)),
                    sym(path),
                    identity.meaning.clone(),
                ],
            ));
        }
        for (slot, case) in data.cases.iter().enumerate() {
            rows.push(app(
                "admission-binding-v1",
                vec![
                    self.existing("Case", &case.native_key)?,
                    sym("CaseSpec"),
                    sym(&format!("Case/{slot}/Spec")),
                    case.spec.clone(),
                ],
            ));
        }
        Ok(app("admission-bindings-v1", rows))
    }
}
fn symbol_key(case: &[u8], raw: u32) -> Result<Vec<u8>, CostError> {
    let mut key = term::copy(case)?;
    key.extend_from_slice(&raw.to_le_bytes());
    Ok(key)
}
pub(super) fn kind(k: IdentityKind) -> &'static str {
    match k {
        IdentityKind::Clock => "Clock",
        IdentityKind::Origin => "Origin",
        IdentityKind::Reference => "Reference",
        IdentityKind::Frame => "Frame",
    }
}
pub(super) fn identity_path(t: &Term) -> Result<&str, CostError> {
    let f = term::children(t, "identity-meaning-v1")?;
    if f.len() != 2 {
        return Err(CostError::CorrespondenceMismatch);
    }
    term::symbol(&f[0])
}
pub(super) fn declaration(kind: &str, path: &str, meaning: Term) -> Term {
    app("declaration-v1", vec![sym(kind), sym(path), meaning])
}
/// No host semantic sort: raw case/atom traversal is retained in this inventory.
pub(super) fn remaining_declarations(
    data: &PlanData,
    rows: &mut Vec<Term>,
) -> Result<(), CostError> {
    for (i, meaning) in data.temporal_atom_meanings.iter().enumerate() {
        rows.push(declaration(
            "TemporalAtom",
            &format!("Temporal/Atom/{i}"),
            meaning.clone(),
        ));
    }
    for identity in &data.identities {
        rows.push(declaration(
            kind(identity.kind),
            identity_path(&identity.meaning)?,
            identity.meaning.clone(),
        ));
    }
    Ok(())
}
