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
    /// A polygon mesh (spec `mesh-and-subd.md` §2.1).
    Mesh(curva_mesh::Mesh),
    /// A SubD: a cage of points smoothed into a surface (spec `mesh-and-subd.md` §2.2).
    SubD(curva_mesh::SubD),
}

impl Shape {
    /// The curve, if this is one.
    #[must_use]
    pub fn curve(&self) -> Option<&Curve> {
        match self {
            Self::Curve(curve) => Some(curve),
            Self::Body(_)
            | Self::Instance(_)
            | Self::Annotation(_)
            | Self::Mesh(_)
            | Self::SubD(_) => None,
        }
    }

    /// The body, if this is one.
    #[must_use]
    pub fn body(&self) -> Option<&Body> {
        match self {
            Self::Body(body) => Some(body),
            Self::Curve(_)
            | Self::Instance(_)
            | Self::Annotation(_)
            | Self::Mesh(_)
            | Self::SubD(_) => None,
        }
    }

    /// The mesh, if this is one.
    #[must_use]
    pub fn mesh(&self) -> Option<&curva_mesh::Mesh> {
        match self {
            Self::Mesh(mesh) => Some(mesh),
            _ => None,
        }
    }

    /// The SubD, if this is one.
    #[must_use]
    pub fn subd(&self) -> Option<&curva_mesh::SubD> {
        match self {
            Self::SubD(subd) => Some(subd),
            _ => None,
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
            Self::Mesh(mesh) => mesh.bounds(),
            Self::SubD(subd) => subd.bounds(),
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
            Self::Annotation(annotation) => Self::Annotation(annotation.translated(by)),
            Self::Mesh(mesh) => Self::Mesh(mesh.translated(by)),
            Self::SubD(subd) => Self::SubD(subd.translated(by)),
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
            Self::Annotation(annotation) => Self::Annotation(annotation.mapped(change)),
            Self::Mesh(mesh) => Self::Mesh(mesh.mapped(change)),
            Self::SubD(subd) => Self::SubD(subd.mapped(change)),
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
            Self::Mesh(mesh) => mesh_edges(mesh),
            // A SubD's cage, the lines its points are moved by.
            Self::SubD(subd) => mesh_edges(subd.cage()),
        }
    }
}

/// A mesh's edges as lines, each once even where the mesh is unwelded (a MeshBox's edges,
/// #690), as Rhino 8 draws its wires ([`curva_mesh::Mesh::wire_edges`]).
#[must_use]
pub fn mesh_edges(mesh: &curva_mesh::Mesh) -> Vec<Curve> {
    let points = mesh.vertices();
    mesh.wire_edges()
        .into_iter()
        .filter_map(|[a, b]| Curve::line(*points.get(a as usize)?, *points.get(b as usize)?).ok())
        .collect()
}

/// The outlines of an annotation, for its extent: a text's letters; for a dimension, the lines
/// between its points (what it draws depends on its style, which [`annotation_drawing`] reads).
#[must_use]
pub fn annotation_curves(annotation: &crate::annotation::Annotation) -> Vec<Curve> {
    match annotation {
        crate::annotation::Annotation::Text(text) => {
            text_glyphs(text).into_iter().flatten().collect()
        }
        crate::annotation::Annotation::Dimension(d) => {
            Curve::polyline(vec![d.first, d.line, d.second], false)
                .into_iter()
                .collect()
        }
        crate::annotation::Annotation::Radial(r) => {
            Curve::polyline(vec![r.centre, r.on_curve, r.text_at], false)
                .into_iter()
                .collect()
        }
        crate::annotation::Annotation::Angle(a) => {
            Curve::polyline(vec![a.first, a.apex, a.second, a.arc_at], false)
                .into_iter()
                .collect()
        }
        crate::annotation::Annotation::ArcLength(a) => {
            Curve::polyline(vec![a.start, a.centre, a.end, a.arc_at], false)
                .into_iter()
                .collect()
        }
        crate::annotation::Annotation::Ordinate(o) => {
            // The kinks at their default for a text height of one, enough for an extent.
            let [first, second] = o.kink_points(2.0);
            Curve::polyline(vec![o.point, first, second, o.leader], false)
                .into_iter()
                .collect()
        }
        crate::annotation::Annotation::Leader(l) => Curve::polyline(l.points.clone(), false)
            .into_iter()
            .collect(),
        crate::annotation::Annotation::Dot(d) => vec![Curve::Point { at: d.at }],
        crate::annotation::Annotation::Hatch(h) => h.loops.clone(),
        crate::annotation::Annotation::ScaleBar(b) => crate::sheet::outline(&b.at, 25.0, 2.0),
        crate::annotation::Annotation::Table(t) => {
            let wide = t.width.map_or(30.0, |w| w / t.at.height.max(1e-12));
            crate::sheet::outline(
                &t.at,
                wide,
                if matches!(t.kind, crate::sheet::TableKind::SheetIndex) {
                    -4.0
                } else {
                    4.0
                },
            )
        }
    }
}

/// What an annotation draws: lines, and closed shapes to fill (letters, arrowheads).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnnotationDrawing {
    /// Lines, and the outlines of the filled shapes.
    pub lines: Vec<Curve>,
    /// Each filled shape's closed contours, the outer one with its holes.
    pub fills: Vec<Vec<Curve>>,
}

/// What an annotation in `space` draws, with its style's sizes: paper sizes on a layout, in the
/// model's unit in the model. A dimension's value shows in the model's unit and display.
#[must_use]
pub fn annotation_drawing(
    annotation: &crate::annotation::Annotation,
    state: &crate::State,
    space: Option<u64>,
) -> AnnotationDrawing {
    let text_drawing = |text: &crate::annotation::Text| {
        let fills = text_glyphs(text);
        AnnotationDrawing {
            lines: fills.iter().flatten().cloned().collect(),
            fills,
        }
    };
    use crate::annotation::Annotation;
    match annotation {
        Annotation::Text(text) => return text_drawing(text),
        // A dot is drawn on the screen; in the model it is its point, to pick and snap to.
        Annotation::Dot(d) => {
            return AnnotationDrawing {
                lines: vec![Curve::Point { at: d.at }],
                fills: Vec::new(),
            };
        }
        Annotation::Hatch(h) => return hatch_drawing(h, state, space),
        Annotation::ScaleBar(b) => return crate::sheet::scale_bar_drawing(b, state, space),
        Annotation::Table(t) => return crate::sheet::table_drawing(t, state),
        _ => {}
    }
    let Some(parts) = annotation_parts(annotation, state, space) else {
        return AnnotationDrawing::default();
    };
    let mut drawing = text_drawing(&parts.text);
    for (a, b) in &parts.lines {
        if let Ok(line) = Curve::line(*a, *b) {
            drawing.lines.push(line);
        }
    }
    for [tip, left, right] in &parts.arrows {
        if let Ok(head) = Curve::polyline(vec![*tip, *left, *right], true) {
            drawing.lines.push(head.clone());
            drawing.fills.push(vec![head]);
        }
    }
    drawing
}

/// A dimension or leader's parts as drawn in `space`: its lines, arrowheads and text, with its
/// style's sizes and its value shown as Curva shows it. `None` for other annotation.
#[must_use]
pub fn annotation_parts(
    annotation: &crate::annotation::Annotation,
    state: &crate::State,
    space: Option<u64>,
) -> Option<crate::annotation::DimensionParts> {
    use crate::annotation::Annotation;
    let styles = if state.annotation_styles.is_empty() {
        crate::annotation::standard_styles(matches!(
            state.unit,
            crate::Unit::Feet | crate::Unit::Inches
        ))
    } else {
        state.annotation_styles.clone()
    };
    let style = styles
        .iter()
        .find(|s| s.id == annotation.style())
        .or_else(|| styles.first())
        .cloned()?;
    let paper = space
        .and_then(|id| state.layouts.iter().find(|l| l.id == id))
        .map(|l| l.unit);
    let scale = style.scale_to(state.unit, paper);
    let length = |value: f64| match (state.unit, space) {
        (crate::Unit::Feet | crate::Unit::Inches, None) => {
            crate::units::format_length(value, state.unit, state.length_display)
        }
        _ => format!("{value:.*}", style.linear_resolution()),
    };
    // The value as shown, or typed text with `<>` standing for it, as in Rhino.
    let typed = |text: &Option<String>, shown: String| match text {
        Some(typed) => typed.replace("<>", &shown),
        None => shown,
    };
    let parts = match annotation {
        Annotation::Text(_)
        | Annotation::Dot(_)
        | Annotation::Hatch(_)
        | Annotation::ScaleBar(_)
        | Annotation::Table(_) => {
            return None;
        }
        Annotation::Dimension(d) => d.parts(&style, scale, typed(&d.text, length(d.value()))),
        Annotation::Radial(r) => {
            let mark = if r.diameter { "Ø" } else { "R" };
            let shown = format!("{mark}{}", length(r.value()));
            r.parts(&style, scale, typed(&r.text, shown))
        }
        Annotation::Angle(a) => {
            let shown = format!("{:.*}°", style.linear_resolution(), a.value());
            a.parts(&style, scale, typed(&a.text, shown))
        }
        Annotation::Ordinate(o) => o.parts(&style, scale, typed(&o.text, length(o.value()))),
        Annotation::ArcLength(a) => a.parts(&style, scale, typed(&a.text, length(a.value()))),
        Annotation::Leader(l) => l.parts(&style, scale, l.content.clone()),
    };
    Some(parts)
}
/// What a hatch draws: its outlines, and its pattern's lines clipped to them, or the outlines
/// filled for a solid pattern. Pattern sizes are millimetres, taken to the model's unit, or to
/// the paper's on a layout. A pattern no longer on offer draws solid.
fn hatch_drawing(
    hatch: &crate::annotation::Hatch,
    state: &crate::State,
    space: Option<u64>,
) -> AnnotationDrawing {
    let millimetres = space
        .and_then(|id| state.layouts.iter().find(|l| l.id == id))
        .map_or_else(|| state.unit.millimetres(), |l| l.unit.millimetres());
    let pattern = state
        .patterns()
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(&hatch.pattern));
    let mut drawing = AnnotationDrawing {
        lines: hatch.loops.clone(),
        fills: Vec::new(),
    };
    match pattern {
        Some(pattern) if !pattern.is_solid() => {
            // A pattern's dots (zero-length dashes) are left out for now: drawn as points they
            // would be marked as point objects.
            for (a, b) in hatch.pieces(&pattern, 1.0 / millimetres) {
                if let Ok(piece) = Curve::line(a, b) {
                    drawing.lines.push(piece);
                }
            }
        }
        _ => drawing.fills.push(hatch.loops.clone()),
    }
    drawing
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

#[cfg(test)]
mod mesh_tests {
    use super::*;
    use curva_mesh::{Frame, Mesh, SubD};

    #[test]
    fn a_mesh_and_a_subd_are_saved_moved_and_outlined_as_shapes() {
        let mesh = Mesh::mesh_box(&Frame::world(), [2.0, 3.0, 4.0], [1, 1, 1]);
        let subd = SubD::subd_box(&Frame::world(), [1.0, 1.0, 1.0], [1, 1, 1]);
        let mut doc = crate::Document::default();
        let m = doc.add_shape(Shape::Mesh(mesh.clone()));
        let s = doc.add_shape(Shape::SubD(subd.clone()));
        // Saved and opened again, each comes back as it was.
        let path = std::env::temp_dir().join(format!("curva-266-{}.curva", std::process::id()));
        doc.save(&path).unwrap();
        let opened = crate::Document::open(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let shape_of = |d: &crate::Document, id: u64| {
            d.state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .unwrap()
                .shape
                .clone()
        };
        assert_eq!(shape_of(&opened, m), Shape::Mesh(mesh.clone()));
        assert_eq!(shape_of(&opened, s), Shape::SubD(subd));
        // Moved and turned with every other shape, its box follows.
        let shape = Shape::Mesh(mesh);
        assert_eq!(shape.bounds(), Some(([0.0; 3], [2.0, 3.0, 4.0])));
        let moved = shape.translated([10.0, 0.0, 0.0]);
        assert_eq!(moved.bounds(), Some(([10.0, 0.0, 0.0], [12.0, 3.0, 4.0])));
        let doubled = shape.mapped(&Similarity::scaling([0.0; 3], 2.0));
        assert_eq!(doubled.bounds(), Some(([0.0; 3], [4.0, 6.0, 8.0])));
        // A box mesh's twelve edges are its outline, each once.
        assert_eq!(shape.outlines().len(), 12);
        assert!(shape.curve().is_none() && shape.body().is_none() && shape.mesh().is_some());
    }
}
