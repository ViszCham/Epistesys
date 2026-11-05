//! Complete two-dimensional closed rectangle payload; no inferred calibration.
use super::{app, fields, int, integer, roles, sym};
use crate::{
    product_correspondence::{IdentityKind, PlanData, ProductError},
    spatial_preservation::{SpatialModelInput, SpatialUnit},
};
use lc631_solvers::causal_spatial::Rectangle;
use lc631_substrate::semantic_term::Term;

pub(crate) fn project(data: &PlanData, input: SpatialModelInput) -> Result<Term, ProductError> {
    if input.context != data.context || input.scope != data.scope {
        return Err(ProductError::UnsupportedProjection);
    }
    let frame = roles::native(data, IdentityKind::Frame, input.frame.0)?;
    let unit = match input.unit {
        SpatialUnit::Pixel => "Pixel",
        SpatialUnit::Millimeter => "Millimeter",
    };
    if roles::frame_unit(frame)? != unit {
        return Err(ProductError::UnsupportedProjection);
    }
    let left = rectangle(input.left, input.frame.0, frame.local_role)?;
    let right = rectangle(input.right, input.frame.0, frame.local_role)?;
    Ok(app(
        "spatial-pair-v1",
        vec![
            app(
                "frame-v1",
                vec![int(i128::from(frame.local_role)), frame.meaning.clone()],
            ),
            sym(unit),
            left,
            right,
        ],
    ))
}
fn rectangle(value: Option<Rectangle>, frame: [u8; 32], role: u16) -> Result<Term, ProductError> {
    match value {
        None => Ok(app("missing-rectangle-v1", vec![])),
        Some(r) => {
            if r.frame().0 != frame {
                return Err(ProductError::IdentityMismatch);
            }
            let min = r.min();
            let max = r.max();
            if min[0] >= max[0] || min[1] >= max[1] {
                return Err(ProductError::UnsupportedProjection);
            }
            Ok(app(
                "rect-v1",
                vec![
                    int(i128::from(role)),
                    int(i128::from(min[0])),
                    int(i128::from(min[1])),
                    int(i128::from(max[0])),
                    int(i128::from(max[1])),
                ],
            ))
        }
    }
}
pub(crate) fn validate(data: &PlanData, term: &Term) -> Result<(), ProductError> {
    let f = fields(term, "spatial-pair-v1", 4)?;
    let frame = roles::validate_reference(data, &f[0], IdentityKind::Frame, "frame-v1")?;
    if f[1] != sym(roles::frame_unit(frame)?) {
        return Err(ProductError::UnsupportedProjection);
    }
    for t in [&f[2], &f[3]] {
        if matches!(t,Term::Apply(name,fields) if name == "missing-rectangle-v1" && fields.is_empty())
        {
            continue;
        }
        let r = fields(t, "rect-v1", 5)?;
        if integer(&r[0])? != i128::from(frame.local_role) {
            return Err(ProductError::IdentityMismatch);
        }
        let mut coordinates = [0i64; 4];
        for (out, t) in coordinates.iter_mut().zip(&r[1..]) {
            *out = i64::try_from(integer(t)?).map_err(|_| ProductError::UnsupportedProjection)?;
        }
        if coordinates[0] >= coordinates[2] || coordinates[1] >= coordinates[3] {
            return Err(ProductError::UnsupportedProjection);
        }
    }
    Ok(())
}
