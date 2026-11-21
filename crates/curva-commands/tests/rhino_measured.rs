//! Commands against Rhino 8 itself (#514, the owner's rule: Rhino is the standard). Each case was
//! run in Rhino 8 with the same typed inputs through its own command line (`_-Command ...`, by the
//! Rhino MCP), and every object it left was measured: its extent, and a curve's length and whether
//! it is closed, or a solid's volume. Curva runs the same inputs here and must leave the same.
//!
//! To add a case: run it in Rhino 8 the same way, measure it the same way, and add a row. A case
//! that fails is a difference from Rhino: fix the command, never the numbers.
//!
//! Rhino remembers each command's options between runs and sessions, so a case that depends on
//! an option sets it explicitly in Rhino (`_Solid=_Yes`) rather than trusting its current value.
//! Only objects that can be seen are compared: Curva's live booleans keep their inputs inside the
//! result, hidden, where Rhino deletes them; both show one solid.

use curva_commands::Processor;
use curva_document::{Document, Shape};
use curva_kernel::Curve;

/// What is in the model before the command: nothing, a line from (0,0) to (10,0), a closed
/// 10 × 10 square at the origin, or an open polyline (0,0), (10,0), (10,10); whatever is there
/// is selected.
#[derive(Clone, Copy)]
enum Start {
    Empty,
    Line,
    Square,
    OpenL,
    TwoLines,
    TwoCircles,
    TwoBoxes,
}

/// One object as Rhino measured it: its extent's size and lowest corner, and its length and
/// closedness if it is a curve, or its volume if it is a closed solid.
#[derive(Debug, Clone, Copy)]
struct Measured {
    size: [f64; 3],
    min: [f64; 3],
    length: Option<f64>,
    closed: Option<bool>,
    volume: Option<f64>,
}

const fn curve(size: [f64; 3], min: [f64; 3], length: f64, closed: bool) -> Measured {
    Measured {
        size,
        min,
        length: Some(length),
        closed: Some(closed),
        volume: None,
    }
}

const fn solid(size: [f64; 3], min: [f64; 3], volume: f64) -> Measured {
    Measured {
        size,
        min,
        length: None,
        closed: None,
        volume: Some(volume),
    }
}

const fn surface(size: [f64; 3], min: [f64; 3]) -> Measured {
    Measured {
        size,
        min,
        length: None,
        closed: None,
        volume: None,
    }
}

const L: Measured = curve([10.0, 0.0, 0.0], [0.0, 0.0, 0.0], 10.0, false);
const SQ: Measured = curve([10.0, 10.0, 0.0], [0.0, 0.0, 0.0], 40.0, true);

/// The cases: a name, what the model starts with, what is typed (the command first), and what
/// Rhino 8 left, measured 21 November 2026.
fn cases() -> Vec<(&'static str, Start, Vec<&'static str>, Vec<Measured>)> {
    use Start::{Empty, Line, OpenL, Square, TwoBoxes, TwoCircles, TwoLines};
    vec![
        (
            "line",
            Empty,
            vec!["line", "0,0,0", "10,5,0"],
            vec![curve([10.0, 5.0, 0.0], [0.0; 3], 11.1803, false)],
        ),
        (
            "polyline",
            Empty,
            vec!["polyline", "0,0,0", "10,0,0", "10,5,0", ""],
            vec![curve([10.0, 5.0, 0.0], [0.0; 3], 15.0, false)],
        ),
        (
            "polyline close",
            Empty,
            vec!["polyline", "0,0,0", "10,0,0", "10,5,0", "close"],
            vec![curve([10.0, 5.0, 0.0], [0.0; 3], 26.1803, true)],
        ),
        (
            "rectangle corners",
            Empty,
            vec!["rectangle", "0,0,0", "4,3,0"],
            vec![curve([4.0, 3.0, 0.0], [0.0; 3], 14.0, true)],
        ),
        (
            "rectangle center",
            Empty,
            vec!["rectangle", "center", "0,0,0", "4,3,0"],
            vec![curve([8.0, 6.0, 0.0], [-4.0, -3.0, 0.0], 28.0, true)],
        ),
        (
            "rectangle length",
            Empty,
            vec!["rectangle", "0,0,0", "10", "3"],
            vec![curve([10.0, 3.0, 0.0], [0.0; 3], 26.0, true)],
        ),
        (
            "circle",
            Empty,
            vec!["circle", "0,0,0", "5"],
            vec![curve([10.0, 10.0, 0.0], [-5.0, -5.0, 0.0], 31.4159, true)],
        ),
        (
            "circle diameter",
            Empty,
            vec!["circle", "0,0,0", "diameter", "10"],
            vec![curve([10.0, 10.0, 0.0], [-5.0, -5.0, 0.0], 31.4159, true)],
        ),
        (
            "circle 2point",
            Empty,
            vec!["circle", "2point", "0,0,0", "10,0,0"],
            vec![curve([10.0, 10.0, 0.0], [0.0, -5.0, 0.0], 31.4159, true)],
        ),
        (
            "arc centre",
            Empty,
            vec!["arc", "0,0,0", "5,0,0", "0,5,0"],
            vec![curve([5.0, 5.0, 0.0], [0.0; 3], 7.854, false)],
        ),
        (
            "ellipse",
            Empty,
            vec!["ellipse", "0,0,0", "6,0,0", "0,2,0"],
            vec![curve([12.0, 4.0, 0.0], [-6.0, -2.0, 0.0], 26.7298, true)],
        ),
        (
            "polygon",
            Empty,
            vec!["polygon", "0,0,0", "5,0,0"],
            vec![curve(
                [9.0451, 9.5106, 0.0],
                [-4.0451, -4.7553, 0.0],
                29.3893,
                true,
            )],
        ),
        (
            "box corners",
            Empty,
            vec!["box", "0,0,0", "4,2,0", "3"],
            vec![solid([4.0, 2.0, 3.0], [0.0; 3], 24.0)],
        ),
        (
            "sphere",
            Empty,
            vec!["sphere", "0,0,0", "5"],
            vec![solid([10.0; 3], [-5.0; 3], 523.599)],
        ),
        (
            "cylinder",
            Empty,
            vec!["cylinder", "0,0,0", "2", "5"],
            vec![solid([4.0, 4.0, 5.0], [-2.0, -2.0, 0.0], 62.832)],
        ),
        (
            "cone",
            Empty,
            vec!["cone", "0,0,0", "2", "5"],
            vec![solid([4.0, 4.0, 5.0], [-2.0, -2.0, 0.0], 20.944)],
        ),
        (
            "torus",
            Empty,
            vec!["torus", "0,0,0", "10", "2"],
            vec![solid([24.0, 24.0, 4.0], [-12.0, -12.0, -2.0], 789.568)],
        ),
        (
            "move",
            Line,
            vec!["move", "0,0,0", "5,5,0"],
            vec![curve([10.0, 0.0, 0.0], [5.0, 5.0, 0.0], 10.0, false)],
        ),
        (
            "copy",
            Line,
            vec!["copy", "0,0,0", "0,5,0", "0,10,0", ""],
            vec![
                L,
                curve([10.0, 0.0, 0.0], [0.0, 5.0, 0.0], 10.0, false),
                curve([10.0, 0.0, 0.0], [0.0, 10.0, 0.0], 10.0, false),
            ],
        ),
        (
            "rotate",
            Line,
            vec!["rotate", "0,0,0", "90"],
            vec![curve([0.0, 10.0, 0.0], [0.0; 3], 10.0, false)],
        ),
        (
            "scale",
            Line,
            vec!["scale", "0,0,0", "2"],
            vec![curve([20.0, 0.0, 0.0], [0.0; 3], 20.0, false)],
        ),
        (
            "mirror",
            Line,
            vec!["mirror", "0,0,0", "0,1,0"],
            vec![curve([10.0, 0.0, 0.0], [-10.0, 0.0, 0.0], 10.0, false), L],
        ),
        (
            "offset",
            Line,
            vec!["offset", "2", "5,5,0"],
            vec![L, curve([10.0, 0.0, 0.0], [0.0, 2.0, 0.0], 10.0, false)],
        ),
        (
            "offset closed",
            Square,
            vec!["offset", "2", "5,5,0"],
            vec![SQ, curve([6.0, 6.0, 0.0], [2.0, 2.0, 0.0], 24.0, true)],
        ),
        // Rhino run with _Solid=_Yes and _Solid=_No set explicitly (Rhino remembers options).
        (
            "extrudecrv solid",
            Square,
            vec!["extrudecrv", "5"],
            vec![SQ, solid([10.0, 10.0, 5.0], [0.0; 3], 500.0)],
        ),
        (
            "extrudecrv open",
            Square,
            vec!["extrudecrv", "solid", "5"],
            vec![SQ, surface([10.0, 10.0, 5.0], [0.0; 3])],
        ),
        (
            "arraylinear",
            Line,
            vec!["arraylinear", "3", "0,0,0", "0,5,0"],
            vec![
                L,
                curve([10.0, 0.0, 0.0], [0.0, 5.0, 0.0], 10.0, false),
                curve([10.0, 0.0, 0.0], [0.0, 10.0, 0.0], 10.0, false),
            ],
        ),
        (
            "filletcorners square",
            Square,
            vec!["filletcorners", "2"],
            vec![curve([10.0, 10.0, 0.0], [0.0; 3], 36.5664, true)],
        ),
        (
            "filletcorners open polyline",
            OpenL,
            vec!["filletcorners", "2"],
            vec![curve([10.0, 10.0, 0.0], [0.0; 3], 19.1416, false)],
        ),
        (
            "scale1d",
            Line,
            vec!["scale1d", "0,0,0", "2", "1,0,0"],
            vec![curve([20.0, 0.0, 0.0], [0.0; 3], 20.0, false)],
        ),
        (
            "scale2d",
            Square,
            vec!["scale2d", "0,0,0", "2"],
            vec![curve([20.0, 20.0, 0.0], [0.0; 3], 80.0, true)],
        ),
        (
            "box center",
            Empty,
            vec!["box", "center", "0,0,0", "2,1,0", "3"],
            vec![solid([4.0, 2.0, 3.0], [-2.0, -1.0, 0.0], 24.0)],
        ),
        (
            "circle 3point",
            Empty,
            vec!["circle", "3point", "0,0,0", "10,0,0", "5,5,0"],
            vec![curve([10.0, 10.0, 0.0], [0.0, -5.0, 0.0], 31.4159, true)],
        ),
        (
            "rectangle 3point",
            Empty,
            vec!["rectangle", "3point", "0,0,0", "4,0,0", "4,3,0"],
            vec![curve([4.0, 3.0, 0.0], [0.0; 3], 14.0, true)],
        ),
        (
            "arc startpoint",
            Empty,
            vec!["arc", "startpoint", "0,0,0", "10,0,0", "5,5,0"],
            vec![curve([10.0, 5.0, 0.0], [0.0; 3], 15.708, false)],
        ),
        (
            "join",
            TwoLines,
            vec!["join"],
            vec![curve([10.0, 10.0, 0.0], [0.0; 3], 20.0, false)],
        ),
        (
            "explode",
            OpenL,
            vec!["explode"],
            vec![L, curve([0.0, 10.0, 0.0], [10.0, 0.0, 0.0], 10.0, false)],
        ),
        (
            "arraypolar",
            Line,
            vec!["arraypolar", "0,0,0", "4", "360"],
            vec![
                curve([10.0, 0.0, 0.0], [-10.0, 0.0, 0.0], 10.0, false),
                curve([0.0, 10.0, 0.0], [0.0, -10.0, 0.0], 10.0, false),
                curve([0.0, 10.0, 0.0], [0.0; 3], 10.0, false),
                L,
            ],
        ),
        (
            "loft",
            TwoCircles,
            vec!["loft"],
            vec![
                curve([10.0, 10.0, 0.0], [-5.0, -5.0, 0.0], 31.4159, true),
                surface([10.0, 10.0, 10.0], [-5.0, -5.0, 0.0]),
                curve([6.0, 6.0, 0.0], [-3.0, -3.0, 10.0], 18.8496, true),
            ],
        ),
        (
            "booleanunion",
            TwoBoxes,
            vec!["booleanunion"],
            vec![solid([3.0; 3], [0.0; 3], 15.0)],
        ),
        (
            "box diagonal",
            Empty,
            vec!["box", "diagonal", "0,0,0", "4,2,3"],
            vec![solid([4.0, 2.0, 3.0], [0.0; 3], 24.0)],
        ),
        (
            "box 3point",
            Empty,
            vec!["box", "3point", "0,0,0", "4,0,0", "4,2,0", "3"],
            vec![solid([4.0, 2.0, 3.0], [0.0; 3], 24.0)],
        ),
        (
            "box vertical",
            Empty,
            vec!["box", "vertical", "0,0,0", "4,0,0", "4,0,2", "3"],
            vec![solid([4.0, 3.0, 2.0], [0.0, -3.0, 0.0], 24.0)],
        ),
    ]
}

fn start(kind: Start) -> Document {
    let mut doc = Document::default();
    let add_all = |doc: &mut Document, shapes: Vec<Shape>| {
        let ids: Vec<u64> = shapes.into_iter().map(|s| doc.add_shape(s)).collect();
        doc.select(&ids);
    };
    match kind {
        Start::TwoLines => {
            add_all(
                &mut doc,
                vec![
                    Shape::Curve(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap()),
                    Shape::Curve(Curve::line([10.0, 0.0, 0.0], [10.0, 10.0, 0.0]).unwrap()),
                ],
            );
            return doc;
        }
        Start::TwoCircles => {
            add_all(
                &mut doc,
                vec![
                    Shape::Curve(Curve::circle([0.0; 3], 5.0).unwrap()),
                    Shape::Curve(Curve::circle([0.0, 0.0, 10.0], 3.0).unwrap()),
                ],
            );
            return doc;
        }
        Start::TwoBoxes => {
            add_all(
                &mut doc,
                vec![
                    Shape::Body(curva_kernel::Body::cuboid([0.0; 3], [2.0; 3]).unwrap()),
                    Shape::Body(curva_kernel::Body::cuboid([1.0; 3], [2.0; 3]).unwrap()),
                ],
            );
            return doc;
        }
        _ => {}
    }
    let shape = match kind {
        Start::Empty | Start::TwoLines | Start::TwoCircles | Start::TwoBoxes => return doc,
        Start::Line => Curve::line([0.0; 3], [10.0, 0.0, 0.0]),
        Start::Square => Curve::polyline(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        ),
        Start::OpenL => Curve::polyline(vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], false),
    };
    let id = doc.add(shape.unwrap());
    doc.select(&[id]);
    doc
}

/// What Curva left, measured as Rhino's objects were, in the same order (by lowest corner, then
/// size).
fn measured(doc: &Document) -> Vec<Measured> {
    let mut found: Vec<Measured> = doc
        .state()
        .objects
        .iter()
        // What can be seen, as Rhino's objects are: a live boolean's inputs stay inside it, hidden.
        .filter(|object| !object.hidden)
        .filter_map(|object| {
            let (lo, hi) = object.shape.bounds()?;
            let mut m = surface(std::array::from_fn(|i| hi[i] - lo[i]), lo);
            match &object.shape {
                Shape::Curve(curve) => {
                    m.length = curve.length().ok();
                    m.closed = curve.is_closed().ok();
                }
                Shape::Body(body) if body.is_solid() => m.volume = body.volume().ok(),
                _ => {}
            }
            Some(m)
        })
        .collect();
    let key = |m: &Measured| (m.min.map(round), m.size.map(round));
    found.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap());
    found
}

fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Whether two measurements agree: extents within 0.01 (curves are measured by their samples),
/// lengths and volumes within 0.1%.
fn agrees(a: &Measured, b: &Measured) -> bool {
    let near = |x: f64, y: f64| (x - y).abs() <= 1e-2;
    let close = |x: Option<f64>, y: Option<f64>| match (x, y) {
        (Some(x), Some(y)) => (x - y).abs() <= 1e-3 * y.abs().max(1.0),
        (None, None) => true,
        _ => false,
    };
    (0..3).all(|i| near(a.size[i], b.size[i]) && near(a.min[i], b.min[i]))
        && close(a.length, b.length)
        && a.closed == b.closed
        && close(a.volume, b.volume)
}

#[test]
fn commands_leave_what_rhino_8_leaves() {
    let mut differences = Vec::new();
    for (name, from, typed, rhino) in cases() {
        let mut doc = start(from);
        let mut processor = Processor::default();
        for line in &typed {
            processor.submit(&mut doc, line);
        }
        if processor.is_running() {
            processor.submit(&mut doc, "");
        }
        let curva = measured(&doc);
        let same =
            curva.len() == rhino.len() && curva.iter().zip(&rhino).all(|(c, r)| agrees(c, r));
        if !same {
            differences.push(format!(
                "{name} ({}):\n  Rhino 8: {rhino:?}\n  Curva:   {curva:?}",
                typed.join(" ")
            ));
        }
    }
    assert!(
        differences.is_empty(),
        "{} of {} cases differ from Rhino 8:\n{}",
        differences.len(),
        cases().len(),
        differences.join("\n")
    );
}
