//! Section fills in Make2D's drawing (spec `make2d.md`: clipping-plane intersections "with the
//! section fill as a hatch"; `layouts.md` §3). Where a chosen clipping plane cuts a closed solid,
//! the cut face is filled with the solid's section style: a solid hatch in its fill colour, and
//! its pattern hatch over that.
//!
//! A fill is drawn in a flat view where the cut face looks towards the viewer. Not yet: a fill
//! partly covered by other objects in front of it is drawn whole, and a cut that another chosen
//! plane also crosses gets no fill.

use super::cuts::ahead;
use curva_document::Document;
use curva_document::annotation::{Annotation, Hatch};
use curva_document::clipping::{PlaneCut, SectionFill};
use curva_document::section::SectionStyle;
use curva_kernel::hidden::View;
use curva_kernel::{Curve, Point};

/// The name of the built-in pattern that fills solid.
const SOLID: &str = "Solid";

/// A cut face to fill: its outlines in the model, the side the plane keeps, and the style.
pub(super) struct Fill {
    loops: Vec<Curve>,
    keep: Point,
    style: SectionStyle,
    /// The solid's layer path, for keeping source layers.
    pub(super) source: String,
}

/// The fills for the cuts `cuts` make through the solid `id`, whose outlines where each cut
/// goes through it are `outlines` (`None` where it could not be cut): one for each cut whose
/// style shows something, whose outline closes, and which no other chosen plane crosses.
pub(super) fn fills_of(
    doc: &Document,
    id: u64,
    outlines: &[Option<Vec<Curve>>],
    cuts: &[PlaneCut],
    source: &str,
) -> Vec<Fill> {
    let mut fills = Vec::new();
    for (cut, outline) in cuts.iter().zip(outlines) {
        let Some(style) = doc.section_style(id, cut.frame).filter(SectionStyle::shows) else {
            continue;
        };
        let Some(loops) = outline.clone() else {
            continue;
        };
        let closed = |c: &Curve| c.is_closed().unwrap_or(false);
        let kept_whole = |c: &Curve| {
            c.sample().is_ok_and(|points| {
                cuts.iter()
                    .filter(|other| other.frame != cut.frame)
                    .all(|other| points.iter().all(|p| ahead(other, *p) >= -1e-9))
            })
        };
        if loops.is_empty() || !loops.iter().all(|c| closed(c) && kept_whole(c)) {
            continue;
        }
        fills.push(Fill {
            loops,
            keep: cut.keep,
            style,
            source: source.to_owned(),
        });
    }
    fills
}

/// Whether a fill's cut face shows in `view`: a flat view, looking at the face rather than
/// along it or at its back. The face's outside points away from the side the plane keeps.
fn shows_in(fill: &Fill, view: &View) -> bool {
    let facing: f64 = (0..3).map(|i| fill.keep[i] * view.forward[i]).sum();
    view.parallel && facing > 1e-6
}

/// The hatches that draw `fill` in a drawing: `to_drawing` takes the model into it, and its
/// plane starts at `base` with directions `x` and `y`. Each comes with the colour it is drawn
/// in, if its own.
pub(super) fn hatches(
    fill: &Fill,
    view: &View,
    to_drawing: &[[f64; 4]; 4],
    (base, x, y): (Point, Point, Point),
) -> Vec<(Hatch, Option<[f32; 3]>)> {
    if !shows_in(fill, view) {
        return Vec::new();
    }
    let Ok(loops) = fill
        .loops
        .iter()
        .map(|c| c.mapped_affine(*to_drawing))
        .collect::<Result<Vec<_>, _>>()
    else {
        return Vec::new();
    };
    let hatch = |pattern: &str, scale: f64, rotation: f64| Hatch {
        loops: loops.clone(),
        pattern: pattern.to_owned(),
        scale,
        rotation,
        base,
        x,
        y,
    };
    let mut out = Vec::new();
    if let SectionFill::Solid(colour) = fill.style.fill {
        out.push((hatch(SOLID, 1.0, 0.0), Some(colour)));
    }
    if let Some(pattern) = &fill.style.hatch {
        out.push((
            hatch(&pattern.pattern, pattern.scale, pattern.rotation),
            None,
        ));
    }
    out
}

/// Adds a hatch to the model with its colour, returning its id.
pub(super) fn add(doc: &mut Document, hatch: Hatch, colour: Option<[f32; 3]>) -> u64 {
    let id = doc.add_annotation(Annotation::Hatch(hatch));
    if colour.is_some() {
        doc.set_colour(&[id], colour);
    }
    id
}
