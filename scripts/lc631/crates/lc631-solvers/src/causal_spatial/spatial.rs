//! 宣言2D frame内の閉矩形。 / Closed rectangles within a declared 2D frame.
#![forbid(unsafe_code)]
use super::{report, Binding, Error, Evaluation, Profile, Report, UnknownReason};
use lc631_substrate::context_identity::ContextRevision;
use sha2::{Digest, Sha256};

/// Opaque declared frame identity, requiring external axes/unit/origin binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rectangle {
    frame: FrameId,
    min: [i64; 2],
    max: [i64; 2],
}

impl Rectangle {
    /// Nondegenerate closed axis-aligned rectangles in a declared 2D frame.
    pub fn new(frame: FrameId, min: [i64; 2], max: [i64; 2]) -> Result<Self, Error> {
        if min[0] >= max[0] || min[1] >= max[1] {
            return Err(Error::InvalidRectangle);
        }
        Ok(Self { frame, min, max })
    }

    pub const fn frame(self) -> FrameId {
        self.frame
    }
    pub const fn min(self) -> [i64; 2] {
        self.min
    }
    pub const fn max(self) -> [i64; 2] {
        self.max
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialRelation {
    Contains,
    /// Nonempty intersection of closed sets, including boundaries and overlap.
    Connected,
    /// Boundary contact with no overlap of interiors.
    Touches,
    StrictlyLeftOf,
}

pub fn spatial(
    left: Option<Rectangle>,
    right: Option<Rectangle>,
    relation: SpatialRelation,
) -> Result<Evaluation<bool>, Error> {
    let (Some(a), Some(b)) = (left, right) else {
        return Ok(Evaluation::Unknown(UnknownReason::MissingSpatialObject));
    };
    if a.frame != b.frame {
        return Err(Error::FrameMismatch);
    }
    let intersects = (0..2).all(|axis| a.min[axis] <= b.max[axis] && b.min[axis] <= a.max[axis]);
    let interiors = (0..2).all(|axis| a.min[axis] < b.max[axis] && b.min[axis] < a.max[axis]);
    let value = match relation {
        SpatialRelation::Contains => {
            (0..2).all(|axis| a.min[axis] <= b.min[axis] && a.max[axis] >= b.max[axis])
        }
        SpatialRelation::Connected => intersects,
        SpatialRelation::Touches => intersects && !interiors,
        SpatialRelation::StrictlyLeftOf => a.max[0] < b.min[0],
    };
    Ok(Evaluation::Computed(value))
}

pub fn evaluate_spatial(
    context: ContextRevision,
    left: Option<Rectangle>,
    right: Option<Rectangle>,
    relation: SpatialRelation,
) -> Result<Report<bool>, Error> {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx16.rectangles-2d.v1\0");
    for rectangle in [left, right] {
        match rectangle {
            None => hash.update([0]),
            Some(rectangle) => {
                hash.update([1]);
                hash.update(rectangle.frame.0);
                for value in rectangle.min.into_iter().chain(rectangle.max) {
                    hash.update(value.to_le_bytes());
                }
            }
        }
    }
    let model = hash.finalize().into();
    let mut hash = Sha256::new();
    hash.update(b"epistesys.tlcx16.spatial-query.v1\0");
    hash.update([match relation {
        SpatialRelation::Contains => 0,
        SpatialRelation::Connected => 1,
        SpatialRelation::Touches => 2,
        SpatialRelation::StrictlyLeftOf => 3,
    }]);
    let binding = Binding {
        context,
        query: hash.finalize().into(),
        model,
    };
    Ok(report(
        binding,
        Profile::ClosedIntegerRectangles2dV1,
        spatial(left, right, relation)?,
    ))
}
