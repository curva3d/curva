//! What an object is: a curve, or a body (a surface or solid).

use curva_kernel::{Body, Curve, Point, Similarity};

/// An object's geometry.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A curve or point.
    Curve(Curve),
    /// A surface or solid.
    Body(Body),
    /// A placed copy of a block definition.
    Instance(crate::blocks::Instance),
}

impl Shape {
    /// The curve, if this is one.
    #[must_use]
    pub fn curve(&self) -> Option<&Curve> {
        match self {
            Self::Curve(curve) => Some(curve),
            Self::Body(_) | Self::Instance(_) => None,
        }
    }

    /// The body, if this is one.
    #[must_use]
    pub fn body(&self) -> Option<&Body> {
        match self {
            Self::Body(body) => Some(body),
            Self::Curve(_) | Self::Instance(_) => None,
        }
    }

    /// The shape moved by `by`.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        match self {
            Self::Curve(curve) => Self::Curve(curve.translated(by)),
            Self::Body(body) => Self::Body(body.translated(by)),
            Self::Instance(i) => Self::Instance(crate::blocks::Instance {
                definition: i.definition,
                placement: crate::blocks::followed_by(&i.placement, &crate::blocks::moved_by(by)),
            }),
        }
    }

    /// The shape turned, mirrored or resized.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        match self {
            Self::Curve(curve) => Self::Curve(curve.mapped(change)),
            Self::Body(body) => Self::Body(body.mapped(change)),
            Self::Instance(i) => Self::Instance(crate::blocks::Instance {
                definition: i.definition,
                placement: crate::blocks::followed_by(
                    &i.placement,
                    &crate::blocks::of_similarity(change),
                ),
            }),
        }
    }

    /// The lines that draw it in wireframe: the curve itself, or a body's edges.
    #[must_use]
    pub fn outlines(&self) -> Vec<Curve> {
        match self {
            Self::Curve(curve) => vec![curve.clone()],
            Self::Body(body) => body.edge_curves().unwrap_or_default(),
            // An instance's lines come from its definition, which the document resolves.
            Self::Instance(_) => Vec::new(),
        }
    }
}
