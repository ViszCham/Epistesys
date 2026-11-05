//! Borrowed typed Given artifacts; no scalar/result constructors.
use super::{finite, Error};
use lc631_core::context::ContextRevision;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtStatusQuery};
use lc631_substrate::{source::SourceStore, Digest32};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnitId(pub u16);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleId(pub u16);
#[derive(Clone, Copy)]
pub struct BindingInput<'i> {
    pub unit: UnitId,
    pub role: RoleId,
    pub parameter: BoolSymbolId,
    pub body: &'i BoolExpr,
}
#[derive(Clone, Copy)]
pub struct MapBackInput<'i> {
    pub unit: UnitId,
    pub parameter: BoolSymbolId,
    pub body: &'i BoolExpr,
}
#[derive(Clone, Copy)]
pub struct RebindProbe<'i> {
    pub unit: UnitId,
    pub weight: u32,
    pub root: &'i BoolExpr,
}
pub enum UseExpr {
    MappedUnit(UnitId),
    RawInput(BoolSymbolId),
}
#[derive(Clone, Copy)]
pub struct RootUseInput<'i> {
    pub unit: UnitId,
    pub body: &'i UseExpr,
}

pub struct RoleModel<'i> {
    pub(super) source: &'i SourceStore,
    pub(super) context: ContextRevision,
    pub(super) theory: &'i SmtStatusQuery,
    pub(super) bindings: &'i [BindingInput<'i>],
    pub(super) uses: &'i [RootUseInput<'i>],
    pub(super) digest: Digest32,
    pub(super) nodes: u64,
}
impl<'i> RoleModel<'i> {
    pub fn checked(
        source: &'i SourceStore,
        context: ContextRevision,
        theory: &'i SmtStatusQuery,
        bindings: &'i [BindingInput<'i>],
        uses: &'i [RootUseInput<'i>],
    ) -> Result<Self, Error> {
        if source.id().digest() != context.source().digest() {
            return Err(Error::ForeignSourceOwner);
        }
        if bindings.is_empty()
            || bindings.len() > 16
            || uses.len() > 16
            || theory.boolean_symbols().is_empty()
            || theory.boolean_symbols().len() > 2
        {
            return Err(Error::InvalidModel);
        }
        if !theory.integer_symbols().is_empty() {
            return Err(Error::UnsupportedInteger);
        }
        if theory.assertions().len() > 64 {
            return Err(Error::ShapeLimit);
        }
        let mut nodes = 0u64;
        for (i, b) in bindings.iter().enumerate() {
            if bindings[..i].iter().any(|x| x.unit == b.unit) {
                return Err(Error::DuplicateBinding);
            }
            if !theory.boolean_symbols().contains(&b.parameter) {
                return Err(Error::UndeclaredSymbol);
            }
            let n = finite::scan(b.body, &[b.parameter], 64, 16)?;
            nodes = nodes.checked_add(n).ok_or(Error::ArithmeticOverflow)?;
        }
        for (i, u) in uses.iter().enumerate() {
            if uses[..i].iter().any(|x| x.unit == u.unit)
                || !bindings.iter().any(|b| b.unit == u.unit)
            {
                return Err(Error::DanglingUse);
            }
            match u.body {
                UseExpr::MappedUnit(unit) if !bindings.iter().any(|b| b.unit == *unit) => {
                    return Err(Error::DanglingUse);
                }
                UseExpr::RawInput(s) if !theory.boolean_symbols().contains(s) => {
                    return Err(Error::UndeclaredSymbol);
                }
                UseExpr::MappedUnit(_) | UseExpr::RawInput(_) => {}
            }
        }
        for a in theory.assertions() {
            nodes = nodes
                .checked_add(finite::scan(a, theory.boolean_symbols(), 64, 16)?)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        if bindings.len() != uses.len() {
            return Err(Error::DanglingUse);
        }
        if nodes.checked_mul(24).ok_or(Error::ArithmeticOverflow)? > 16 * 1024 {
            return Err(Error::PayloadLimit);
        }
        let digest = finite::model_digest(source, context, theory, bindings, uses);
        Ok(Self {
            source,
            context,
            theory,
            bindings,
            uses,
            digest,
            nodes,
        })
    }
    pub(super) fn binding(&self, unit: UnitId) -> Result<&BindingInput<'i>, Error> {
        self.bindings
            .iter()
            .find(|b| b.unit == unit)
            .ok_or(Error::UnitSetMismatch)
    }
    pub(super) fn use_input(&self, unit: UnitId) -> Result<&RootUseInput<'i>, Error> {
        self.uses
            .iter()
            .find(|u| u.unit == unit)
            .ok_or(Error::UseUnitMismatch)
    }
}
