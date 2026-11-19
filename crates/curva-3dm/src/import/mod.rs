//! A [`Model`] read from a file brought into a Curva document.

use crate::model::{self, Brep, Geometry, Model, P3};
use crate::notes::note_in;
use curva_document::blocks::{BlockDefinition, Instance};
use curva_document::{
    Document, Layer, Linetype, Object, PrintWidth, SavedCPlane, SavedView, Shape, State, Unit,
};
use curva_kernel::record::{
    Brep as Record, Curve as RecordCurve, Curve2, Edge, Face, Loop, OrientedEdge, OrientedFace,
    Region, Shell, Surface as RecordSurface, Vertex,
};
use curva_kernel::{Body, Curve, KernelError, NurbsSurface, Spline};
use std::collections::BTreeMap;

mod annotation;
mod breps;
mod curves;
mod objects;
mod surfaces;
mod views;
use annotation::*;
pub(crate) use breps::*;
#[allow(clippy::wildcard_imports)]
use curves::*;
use objects::bring_in;
use surfaces::*;
use views::*;

/// What a read brought in and what it left out, for the command bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Objects brought in, by kind.
    pub brought: Vec<(String, usize)>,
    /// Things left out, by kind.
    pub skipped: Vec<(String, usize)>,
    /// Anything else worth saying, such as a change of unit.
    pub notes: Vec<String>,
}

impl Report {
    fn count(list: &mut Vec<(String, usize)>, kind: &str) {
        match list.iter_mut().find(|(k, _)| k == kind) {
            Some((_, n)) => *n += 1,
            None => list.push((kind.to_owned(), 1)),
        }
    }

    /// The report as one line: what came in, then what was left out.
    #[must_use]
    pub fn summary(&self) -> String {
        let list = |items: &[(String, usize)]| {
            items
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut out = if self.brought.is_empty() {
            "nothing brought in".to_owned()
        } else {
            format!("brought in {}", list(&self.brought))
        };
        if !self.skipped.is_empty() {
            out.push_str(&format!("; left out {}", list(&self.skipped)));
        }
        for note in &self.notes {
            out.push_str("; ");
            out.push_str(note);
        }
        out
    }
}

/// Millimetres in one of openNURBS' length units, if Curva knows it.
fn millimetres_per(unit: u32) -> Option<f64> {
    Some(match unit {
        1 => 1e-3,
        2 => 1.0,
        3 => 10.0,
        4 => 1000.0,
        5 => 1e6,
        6 => 2.54e-5,
        7 => 0.0254,
        8 => 25.4,
        9 => 304.8,
        10 => 1_609_344.0,
        12 => 1e-7,
        13 => 1e-6,
        14 => 100.0,
        15 => 1e4,
        16 => 1e5,
        17 => 1e9,
        18 => 1e12,
        19 => 914.4,
        20 => 25.4 / 72.0,
        21 => 25.4 / 6.0,
        22 => 1_852_000.0,
        _ => return None,
    })
}

fn unit_name(unit: u32) -> &'static str {
    match unit {
        1 => "microns",
        3 => "centimetres",
        5 => "kilometres",
        6 => "microinches",
        7 => "mils",
        8 => "inches",
        9 => "feet",
        10 => "miles",
        14 => "decimetres",
        19 => "yards",
        _ => "another unit",
    }
}

/// A file read as a new model, with the file's layers only.
#[must_use]
pub fn open(model: &Model) -> (Document, Report) {
    let mut report = Report::default();
    let mut state = State {
        layers: Vec::new(),
        next_layer: 0,
        ..State::default()
    };
    // Rhino's metres, inches and feet are Curva's own; any other unit comes in as millimetres.
    state.unit = match model.unit {
        4 => Unit::Metres,
        8 => Unit::Inches,
        9 => Unit::Feet,
        _ => Unit::Millimetres,
    };
    bring_in(model, &mut state, &mut report);
    if state.layers.is_empty() {
        state.layers = State::default().layers;
        state.next_layer = State::default().next_layer;
    }
    state.current_layer = state.layers[0].id;
    let mut doc = Document::default();
    doc.restore(state);
    add_views_and_planes(model, &mut doc, &mut report);
    (doc, report)
}

/// A file's contents added to a model: layers merged by name, everything scaled to its unit.
/// The caller records the change for undo.
pub fn import(model: &Model, doc: &mut Document) -> Report {
    let mut report = Report::default();
    let mut state = doc.state().clone();
    bring_in(model, &mut state, &mut report);
    doc.restore(state);
    add_views_and_planes(model, doc, &mut report);
    report
}

/// Each layer's full name, as Rhino shows nested layers: `Parent::Child`.
fn layer_names(layers: &[model::Layer]) -> Vec<String> {
    layers
        .iter()
        .map(|layer| {
            let mut name = layer.name.clone();
            let mut parent = layer.parent;
            let mut depth = 0;
            while parent != crate::archive::Uuid::NIL && depth < 32 {
                let Some(up) = layers.iter().find(|l| l.id == parent) else {
                    break;
                };
                name = format!("{}::{name}", up.name);
                parent = up.parent;
                depth += 1;
            }
            name
        })
        .collect()
}

/// A layer colour: Rhino's black, which would vanish on Curva's dark viewport, as Curva's own.
fn colour_of(c: [u8; 3]) -> [f32; 3] {
    if c == [0, 0, 0] {
        curva_document::STANDARD_COLOUR
    } else {
        c.map(|v| f32::from(v) / 255.0)
    }
}

fn scaled(shape: Shape, scale: f64) -> Result<Shape, String> {
    if (scale - 1.0).abs() < 1e-12 {
        return Ok(shape);
    }
    let m = [
        [scale, 0.0, 0.0, 0.0],
        [0.0, scale, 0.0, 0.0],
        [0.0, 0.0, scale, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    curva_document::blocks::shape_placed(&shape, &m).ok_or_else(|| "scaling failed".to_owned())
}

/// `solid` is the object's solid when it was built ahead (`objects::solids_ahead`).
fn shape_of(
    geometry: &Geometry,
    solid: Option<Result<Body, KernelError>>,
    blocks: &BTreeMap<crate::archive::Uuid, u64>,
    styles: &[model::DimStyle],
    patterns: &[model::HatchPattern],
    state: &mut State,
    file_mm: f64,
) -> Result<(Shape, &'static str), String> {
    use curva_document::annotation::Annotation;
    let text = |e: KernelError| e.0;
    Ok(match geometry {
        Geometry::Point(p) => (Shape::Curve(Curve::Point { at: *p }), "points"),
        Geometry::Radial(d) => (
            Shape::Annotation(Annotation::Radial(radial_in(d))),
            "dimensions",
        ),
        Geometry::Ordinate(d) => (
            Shape::Annotation(Annotation::Ordinate(ordinate_in(d))),
            "dimensions",
        ),
        Geometry::Angular(d) => (
            Shape::Annotation(Annotation::Angle(angular_in(d))),
            "dimensions",
        ),
        Geometry::Detail(_) => return Err("details come in with their layout".to_owned()),
        Geometry::Mesh(m) => (Shape::Mesh(m.clone()), "meshes"),
        Geometry::SubD(s) => (Shape::SubD(s.clone()), "SubDs"),
        Geometry::ClippingPlane(c) => (
            Shape::Curve(clipping_rectangle(c).map_err(text)?),
            "clipping planes",
        ),
        Geometry::Leader(l) => (
            Shape::Annotation(Annotation::Leader(leader_in(l))),
            "leaders",
        ),
        Geometry::Dimension(d) => (
            Shape::Annotation(Annotation::Dimension(dimension_in(d))),
            "dimensions",
        ),
        Geometry::Text(t) => (
            Shape::Annotation(Annotation::Text(note_in(t, styles))),
            "texts",
        ),
        Geometry::Hatch(h) => (
            Shape::Annotation(Annotation::Hatch(hatch_in(h, patterns, state, file_mm)?)),
            "hatches",
        ),
        Geometry::Dot {
            at,
            text,
            secondary,
            height,
            font,
        } => (dot_in(*at, text, secondary, *height, font), "dots"),
        Geometry::Curve(c) => (Shape::Curve(curve(c).map_err(text)?), "curves"),
        Geometry::Surface(s) => (Shape::Body(surface_body(s).map_err(text)?), "surfaces"),
        Geometry::Brep(b) => (
            Shape::Body(solid.unwrap_or_else(|| brep_body(b)).map_err(text)?),
            "polysurfaces",
        ),
        Geometry::Extrusion(e) => (
            Shape::Body(solid.unwrap_or_else(|| extrusion_body(e)).map_err(text)?),
            "extrusions",
        ),
        Geometry::Instance { block, xform } => {
            let definition = *blocks.get(block).ok_or("a block that is not defined")?;
            // openNURBS transforms columns; Curva's placements are rows.
            let placement = std::array::from_fn(|i| std::array::from_fn(|j| xform[j][i]));
            (
                Shape::Instance(Instance {
                    definition,
                    placement,
                }),
                "block instances",
            )
        }
    })
}

/// A Rhino dot as Curva's: its words, its second words, its height, and its font where it
/// names one.
fn dot_in(at: P3, text: &str, secondary: &str, height: i32, font: &str) -> Shape {
    Shape::Annotation(curva_document::annotation::Annotation::Dot(
        curva_document::annotation::Dot {
            at,
            content: text.to_owned(),
            secondary: secondary.to_owned(),
            height: f64::from(height),
            font: (!font.trim().is_empty()).then(|| font.to_owned()),
        },
    ))
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn lerp(a: P3, b: P3, t: f64) -> P3 {
    std::array::from_fn(|k| a[k] + t * (b[k] - a[k]))
}

/// Where `s` falls between `d[0]` and `d[1]`, from 0 to 1.
fn fraction(d: [f64; 2], s: f64) -> f64 {
    if (d[1] - d[0]).abs() < f64::MIN_POSITIVE {
        0.0
    } else {
        (s - d[0]) / (d[1] - d[0])
    }
}
