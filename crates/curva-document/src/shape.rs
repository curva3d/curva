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
    /// Text and other annotation (spec `annotation.md`).
    Annotation(crate::annotation::Annotation),
}

impl Shape {
    /// The curve, if this is one.
    #[must_use]
    pub fn curve(&self) -> Option<&Curve> {
        match self {
            Self::Curve(curve) => Some(curve),
            Self::Body(_) | Self::Instance(_) | Self::Annotation(_) => None,
        }
    }

    /// The body, if this is one.
    #[must_use]
    pub fn body(&self) -> Option<&Body> {
        match self {
            Self::Body(body) => Some(body),
            Self::Curve(_) | Self::Instance(_) | Self::Annotation(_) => None,
        }
    }

    /// The lowest and highest corners around it. An instance's extent needs its definition, so
    /// it has none here.
    #[must_use]
    pub fn bounds(&self) -> Option<(Point, Point)> {
        match self {
            Self::Instance(_) => None,
            Self::Annotation(annotation) => annotation_bounds(annotation),
            Self::Body(body) => body.bounds().ok(),
            Self::Curve(curve) => {
                let points = curve.sample().ok()?;
                let first = *points.first()?;
                Some(points.iter().fold((first, first), |(lo, hi), p| {
                    (
                        std::array::from_fn(|i| lo[i].min(p[i])),
                        std::array::from_fn(|i| hi[i].max(p[i])),
                    )
                }))
            }
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
            Self::Annotation(crate::annotation::Annotation::Text(text)) => {
                Self::Annotation(crate::annotation::Annotation::Text(text.translated(by)))
            }
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
            Self::Annotation(crate::annotation::Annotation::Text(text)) => {
                Self::Annotation(crate::annotation::Annotation::Text(text.mapped(change)))
            }
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
            Self::Annotation(annotation) => annotation_curves(annotation),
        }
    }
}

/// The outlines of an annotation's letters (and later its lines and arrows), where it lies.
#[must_use]
pub fn annotation_curves(annotation: &crate::annotation::Annotation) -> Vec<Curve> {
    match annotation {
        crate::annotation::Annotation::Text(text) => {
            text_glyphs(text).into_iter().flatten().collect()
        }
    }
}

/// Each glyph of a text with its closed contours, set line by line and aligned.
#[must_use]
pub fn text_glyphs(text: &crate::annotation::Text) -> Vec<Vec<Curve>> {
    use crate::annotation::{LINE_SPACING, TextAlign};
    let font = curva_kernel::font_bytes(text.font.as_deref());
    let mut glyphs = Vec::new();
    for (row, line) in text.content.lines().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let down = row as f64 * text.height * LINE_SPACING;
        let start: Point = std::array::from_fn(|i| text.origin[i] - text.y[i] * down);
        let Ok(set) = curva_kernel::text_outlines(line, &font, text.height, start, text.x, text.y)
        else {
            continue;
        };
        // The line's width along its direction, for centring or right alignment.
        let along = |p: Point| (0..3).map(|i| (p[i] - start[i]) * text.x[i]).sum::<f64>();
        let width = set
            .iter()
            .flatten()
            .filter_map(|c| c.sample().ok())
            .flatten()
            .map(along)
            .fold(0.0_f64, f64::max);
        let shift = match text.align {
            TextAlign::Left => 0.0,
            TextAlign::Centre => -width / 2.0,
            TextAlign::Right => -width,
        };
        let by: Point = text.x.map(|v| v * shift);
        glyphs.extend(
            set.into_iter()
                .map(|glyph| glyph.iter().map(|c| c.translated(by)).collect::<Vec<_>>()),
        );
    }
    glyphs
}

/// The box around an annotation's letters.
fn annotation_bounds(annotation: &crate::annotation::Annotation) -> Option<(Point, Point)> {
    let points: Vec<Point> = annotation_curves(annotation)
        .iter()
        .filter_map(|c| c.sample().ok())
        .flatten()
        .collect();
    let first = *points.first()?;
    Some(points.iter().fold((first, first), |(lo, hi), p| {
        (
            std::array::from_fn(|i| lo[i].min(p[i])),
            std::array::from_fn(|i| hi[i].max(p[i])),
        )
    }))
}
