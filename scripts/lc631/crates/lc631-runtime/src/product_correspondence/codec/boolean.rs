//! Explicit native symbol-to-meaning map; no ID-derived semantic coordinates.
use super::{app, fields, list, sym};
use crate::{
    adversarial_measurement::SemanticAtom,
    product_correspondence::{ProductError, ProductLimits},
};
use lc631_solvers::typed_query::BoolExpr;
use lc631_substrate::semantic_term::Term;

pub(crate) fn boolean_term(
    expr: &BoolExpr,
    atoms: &[SemanticAtom<'_>],
    limits: ProductLimits,
) -> Result<Term, ProductError> {
    fn visit(
        expr: &BoolExpr,
        atoms: &[SemanticAtom<'_>],
        left: &mut usize,
        depth: usize,
        limits: ProductLimits,
    ) -> Result<Term, ProductError> {
        if *left == 0 || depth > limits.max_depth {
            return Err(ProductError::UnsupportedProjection);
        }
        *left -= 1;
        match expr {
            BoolExpr::Constant(value) => Ok(Term::Bool(*value)),
            BoolExpr::Variable(symbol) => {
                let atom = atoms
                    .iter()
                    .find(|a| a.symbol == *symbol)
                    .ok_or(ProductError::MissingMeaningDeclaration)?;
                let meaning = std::str::from_utf8(atom.meaning)
                    .map_err(|_| ProductError::UnsupportedProjection)?;
                if meaning.is_empty() || meaning.len() > 256 {
                    return Err(ProductError::UnsupportedProjection);
                }
                Ok(app("atom-v1", vec![sym(meaning)]))
            }
            BoolExpr::Not(child) => Ok(app(
                "not-v1",
                vec![visit(child, atoms, left, depth + 1, limits)?],
            )),
            BoolExpr::And(children) | BoolExpr::Or(children) => {
                if children.len() > *left {
                    return Err(ProductError::UnsupportedProjection);
                }
                let mut terms = crate::product_correspondence::reserved(children.len())?;
                for child in children {
                    terms.push(visit(child, atoms, left, depth + 1, limits)?);
                }
                Ok(app(
                    if matches!(expr, BoolExpr::And(_)) {
                        "and-v1"
                    } else {
                        "or-v1"
                    },
                    terms,
                ))
            }
            _ => Err(ProductError::UnsupportedProjection),
        }
    }
    let mut remaining = limits.max_response_nodes;
    visit(expr, atoms, &mut remaining, 0, limits)
}
pub(crate) fn validate(
    term: &Term,
    meanings: &[&[u8]],
    limits: ProductLimits,
) -> Result<(), ProductError> {
    fn visit(
        term: &Term,
        meanings: &[&[u8]],
        remaining: &mut usize,
        depth: usize,
        limits: ProductLimits,
    ) -> Result<(), ProductError> {
        if *remaining == 0 || depth > limits.max_depth {
            return Err(ProductError::UnsupportedProjection);
        }
        *remaining -= 1;
        match term {
            Term::Bool(_) => Ok(()),
            Term::Apply(name, _) if name == "atom-v1" => {
                let f = fields(term, "atom-v1", 1)?;
                match &f[0] {
                    Term::Symbol(s) if meanings.contains(&s.as_bytes()) => Ok(()),
                    _ => Err(ProductError::MissingMeaningDeclaration),
                }
            }
            Term::Apply(name, _) if name == "not-v1" => visit(
                &fields(term, "not-v1", 1)?[0],
                meanings,
                remaining,
                depth + 1,
                limits,
            ),
            Term::Apply(name, _) if name == "and-v1" || name == "or-v1" => {
                let children = list(term, name)?;
                if children.len() > *remaining {
                    return Err(ProductError::UnsupportedProjection);
                }
                for child in children {
                    visit(child, meanings, remaining, depth + 1, limits)?;
                }
                Ok(())
            }
            _ => Err(ProductError::UnsupportedProjection),
        }
    }
    let mut remaining = limits.max_response_nodes;
    visit(term, meanings, &mut remaining, 0, limits)
}
