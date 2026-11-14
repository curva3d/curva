//! Radius, diameter and angle dimensions, as in Rhino 8 (spec `annotation.md` §3;
//! `rhino-reference.md`). `DimRadius` and `DimDiameter` pick a circle or arc, where the arrow
//! lands, then where the text goes. `DimAngle` picks an arc, or two lines, or takes three points
//! with `Points`, then where the arc goes. Each previews as it is placed.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{AngleDimension, Annotation, RadialDimension};
use curva_kernel::{Curve, Point};

/// How near a click must be to a curve to pick it, as a share of the curve's size: the viewport
/// picks on the drawn outline, whose short segments lie a little inside a circle.
const PICK_SHARE: f64 = 0.02;
const POINTS: &str = "Points";

/// The curve nearest a pick, and the exact point on it nearest the pick, when the pick is
/// within [`PICK_SHARE`] of the curve's size.
fn picked_curve(doc: &Document, at: Point) -> Option<(Curve, Point)> {
    let mut best: Option<(f64, Curve, Point)> = None;
    for object in &doc.state().objects {
        let Some(curve) = object.curve() else {
            continue;
        };
        let (Ok(on), Some((lo, hi))) = (curve.closest_point(at), object.shape.bounds()) else {
            continue;
        };
        let reach = (PICK_SHARE * gap(lo, hi)).max(1e-3);
        let off = gap(on, at);
        if off <= reach && best.as_ref().is_none_or(|(b, ..)| off < *b) {
            best = Some((off, curve.clone(), on));
        }
    }
    best.map(|(_, curve, on)| (curve, on))
}

fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

fn gap(a: Point, b: Point) -> f64 {
    dot(sub(a, b), sub(a, b)).sqrt()
}

/// A circle's or arc's centre and plane.
fn round_curve(curve: &Curve) -> Option<(Point, Point, Point)> {
    match curve {
        Curve::Circle { centre, x, y, .. } | Curve::Arc { centre, x, y, .. } => {
            Some((*centre, *x, *y))
        }
        _ => None,
    }
}

/// The running `DimRadius` or `DimDiameter`.
pub struct Radial {
    diameter: bool,
    /// The picked circle's centre and plane, and where the arrow lands.
    picked: Option<(Point, Point, Point, Point)>,
}

impl Radial {
    fn dimension(&self, doc: &Document, text_at: Point) -> Option<RadialDimension> {
        let (centre, x, y, on_curve) = self.picked?;
        Some(RadialDimension {
            diameter: self.diameter,
            centre,
            on_curve,
            text_at,
            x,
            y,
            style: doc.current_annotation_style(),
            text: None,
        })
    }

    fn ask(&self) -> Step {
        match self.picked {
            None => Step::Ask(Request::new("prompt.dimradius.curve", Kind::PickPart)),
            Some((.., on_curve)) => Step::Ask(Request {
                from: Some(on_curve),
                ..Request::new("prompt.dimradius.text", Kind::Point)
            }),
        }
    }
}

impl Command for Radial {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Point(at)) if self.picked.is_none() => {
                let picked = picked_curve(doc, at)
                    .and_then(|(c, on)| round_curve(&c).map(|(centre, x, y)| (centre, x, y, on)));
                let Some(picked) = picked else {
                    return Step::Ask(Request::new("prompt.dimradius.not_round", Kind::PickPart));
                };
                self.picked = Some(picked);
                self.ask()
            }
            Some(Answer::Point(text_at)) => {
                let Some(dimension) = self.dimension(doc, text_at) else {
                    return unexpected();
                };
                let shown = doc.length_text(dimension.value());
                doc.add_annotation(Annotation::Radial(dimension));
                Step::Done(Some(Message::with("done.dim", vec![shown])))
            }
            Some(_) => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(dimension) = live.cursor.and_then(|at| self.dimension(doc, at)) else {
            return Vec::new();
        };
        curva_document::shape::annotation_drawing(
            &Annotation::Radial(dimension),
            doc.state(),
            doc.space(),
        )
        .lines
    }
}

/// `DimRadius`.
pub struct DimRadius(Radial);

impl Default for DimRadius {
    fn default() -> Self {
        Self(Radial {
            diameter: false,
            picked: None,
        })
    }
}

/// `DimDiameter`.
pub struct DimDiameter(Radial);

impl Default for DimDiameter {
    fn default() -> Self {
        Self(Radial {
            diameter: true,
            picked: None,
        })
    }
}

macro_rules! forward {
    ($kind:ty) => {
        impl Command for $kind {
            fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
                self.0.step(doc, answer)
            }
            fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
                self.0.preview(doc, live)
            }
        }
    };
}
forward!(DimRadius);
forward!(DimDiameter);

/// Where two lines meet (the middle of their closest points), or `None` when parallel.
fn meeting(a: (Point, Point), b: (Point, Point)) -> Option<Point> {
    let (da, db, w) = (sub(a.1, a.0), sub(b.1, b.0), sub(a.0, b.0));
    let (aa, ab, bb, aw, bw) = (
        dot(da, da),
        dot(da, db),
        dot(db, db),
        dot(da, w),
        dot(db, w),
    );
    let det = aa * bb - ab * ab;
    if det.abs() < 1e-12 * aa * bb {
        return None;
    }
    let s = (ab * bw - bb * aw) / det;
    let t = (aa * bw - ab * aw) / det;
    Some(std::array::from_fn(|i| {
        f64::midpoint(a.0[i] + s * da[i], b.0[i] + t * db[i])
    }))
}

/// A point along a side away from the apex: the click, or the line's far end when the click is
/// at the apex.
fn along_side(apex: Point, line: (Point, Point), click: Point) -> Point {
    if gap(click, apex) > 1e-6 {
        return click;
    }
    if gap(line.0, apex) > gap(line.1, apex) {
        line.0
    } else {
        line.1
    }
}

/// The running `DimAngle`.
#[derive(Default)]
pub struct DimAngle {
    /// Taking three points rather than picking curves.
    points: bool,
    /// Picked so far: points, or the first line and its click.
    clicks: Vec<Point>,
    first_line: Option<(Point, Point)>,
    /// The apex and the sides' points, once known, with the plane.
    sides: Option<(Point, Point, Point, Point, Point)>,
}

impl DimAngle {
    fn dimension(&self, doc: &Document, arc_at: Point) -> Option<AngleDimension> {
        let (apex, first, second, x, y) = self.sides?;
        Some(AngleDimension {
            apex,
            first,
            second,
            arc_at,
            x,
            y,
            style: doc.current_annotation_style(),
            text: None,
        })
    }

    fn ask(&self) -> Step {
        if self.sides.is_some() {
            let from = self.sides.map(|s| s.0);
            return Step::Ask(Request {
                from,
                ..Request::new("prompt.dimangle.arc", Kind::Point)
            });
        }
        if self.points {
            let key = match self.clicks.len() {
                0 => "prompt.dimangle.apex",
                1 => "prompt.dimangle.first",
                _ => "prompt.dimangle.second",
            };
            return Step::Ask(Request {
                from: self.clicks.first().copied(),
                preview: self.clicks.clone(),
                ..Request::new(key, Kind::Point)
            });
        }
        let key = if self.first_line.is_some() {
            "prompt.dimangle.second_line"
        } else {
            "prompt.dimangle.curve"
        };
        Step::Ask(Request {
            options: if self.first_line.is_some() {
                Vec::new()
            } else {
                vec![POINTS]
            },
            ..Request::new(key, Kind::PickPart)
        })
    }

    /// The plane through three points, or the work plane when they are in a line.
    fn plane(doc: &Document, apex: Point, first: Point, second: Point) -> (Point, Point) {
        let [wx, wy] = doc.work_plane();
        let (u, v) = (sub(first, apex), sub(second, apex));
        let normal: Point = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let length = dot(normal, normal).sqrt();
        if length < 1e-9 * dot(u, u).sqrt().max(1e-12) * dot(v, v).sqrt().max(1e-12) {
            return (wx, wy);
        }
        let mut n = normal.map(|c| c / length);
        // Keep the work plane's side up, so the plane's X follows the work plane's.
        let up: Point = [
            wx[1] * wy[2] - wx[2] * wy[1],
            wx[2] * wy[0] - wx[0] * wy[2],
            wx[0] * wy[1] - wx[1] * wy[0],
        ];
        if dot(n, up) < 0.0 {
            n = n.map(|c| -c);
        }
        let x = {
            let flat = sub(wx, n.map(|c| c * dot(wx, n)));
            let l = dot(flat, flat).sqrt();
            if l < 1e-9 {
                let l = dot(u, u).sqrt();
                u.map(|c| c / l)
            } else {
                flat.map(|c| c / l)
            }
        };
        let y = [
            n[1] * x[2] - n[2] * x[1],
            n[2] * x[0] - n[0] * x[2],
            n[0] * x[1] - n[1] * x[0],
        ];
        (x, y)
    }

    fn picked(&mut self, doc: &Document, at: Point) -> Step {
        let Some((curve, at)) = picked_curve(doc, at) else {
            return Step::Ask(Request::new("prompt.dimangle.not_line", Kind::PickPart));
        };
        match (&curve, self.first_line) {
            (
                Curve::Arc {
                    centre,
                    radius,
                    x,
                    y,
                    sweep,
                },
                None,
            ) => {
                let end: Point = std::array::from_fn(|i| {
                    centre[i] + radius * (x[i] * sweep.cos() + y[i] * sweep.sin())
                });
                let start: Point = std::array::from_fn(|i| centre[i] + radius * x[i]);
                // Its middle, to pick the angle the arc spans.
                let half = sweep / 2.0;
                let middle: Point = std::array::from_fn(|i| {
                    centre[i] + radius * (x[i] * half.cos() + y[i] * half.sin())
                });
                if *sweep >= std::f64::consts::PI {
                    return Step::Failed(Message::new("error.dimangle_arc"));
                }
                self.sides = Some((*centre, start, end, *x, *y));
                self.clicks = vec![middle];
                self.ask()
            }
            (Curve::Line { start, end }, None) => {
                self.first_line = Some((*start, *end));
                self.clicks = vec![at];
                self.ask()
            }
            (Curve::Line { start, end }, Some(first)) => {
                let second = (*start, *end);
                let Some(apex) = meeting(first, second) else {
                    return Step::Failed(Message::new("error.dimangle_parallel"));
                };
                let a = along_side(apex, first, self.clicks[0]);
                let b = along_side(apex, second, at);
                let (x, y) = Self::plane(doc, apex, a, b);
                self.sides = Some((apex, a, b, x, y));
                self.ask()
            }
            _ => Step::Ask(Request::new("prompt.dimangle.not_line", Kind::PickPart)),
        }
    }
}

impl Command for DimAngle {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Option(POINTS)) => {
                self.points = true;
                self.ask()
            }
            Some(Answer::Point(arc_at)) if self.sides.is_some() => {
                let Some(dimension) = self.dimension(doc, arc_at) else {
                    return unexpected();
                };
                let shown = format!("{:.2}°", dimension.value());
                doc.add_annotation(Annotation::Angle(dimension));
                Step::Done(Some(Message::with("done.dim", vec![shown])))
            }
            Some(Answer::Point(p)) if self.points => {
                self.clicks.push(p);
                if let [apex, first, second] = self.clicks[..] {
                    if gap(apex, first) < 1e-9 || gap(apex, second) < 1e-9 {
                        return Step::Failed(Message::new("error.dimangle_points"));
                    }
                    let (x, y) = Self::plane(doc, apex, first, second);
                    self.sides = Some((apex, first, second, x, y));
                }
                self.ask()
            }
            Some(Answer::Point(at)) => self.picked(doc, at),
            Some(_) => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(dimension) = live.cursor.and_then(|at| self.dimension(doc, at)) else {
            return Vec::new();
        };
        curva_document::shape::annotation_drawing(
            &Annotation::Angle(dimension),
            doc.state(),
            doc.space(),
        )
        .lines
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use crate::{Command, Live};
    use curva_document::annotation::Annotation;
    use curva_document::{Document, Shape};

    fn only(doc: &Document) -> Annotation {
        match &doc.state().objects.last().unwrap().shape {
            Shape::Annotation(a) => a.clone(),
            _ => panic!("not an annotation"),
        }
    }

    #[test]
    fn radius_and_diameter_dimensions_measure_a_picked_circle() {
        let mut doc = Document::default();
        run(&mut doc, &["Circle", "0,0", "5"]);
        let mut processor = run(&mut doc, &["DimRadius"]);
        processor.click(&mut doc, [5.0, 0.0, 0.0]);
        processor.click(&mut doc, [9.0, 3.0, 0.0]);
        let Annotation::Radial(r) = only(&doc) else {
            panic!("not radial");
        };
        assert!(!r.diameter);
        assert!((r.value() - 5.0).abs() < 1e-9);
        let mut processor = run(&mut doc, &["DimDiameter"]);
        processor.click(&mut doc, [0.0, 5.0, 0.0]);
        processor.click(&mut doc, [3.0, 9.0, 0.0]);
        let Annotation::Radial(d) = only(&doc) else {
            panic!("not radial");
        };
        assert!(d.diameter);
        assert!((d.value() - 10.0).abs() < 1e-9);
        let drawing =
            curva_document::shape::annotation_drawing(&Annotation::Radial(d), doc.state(), None);
        assert!(drawing.fills.len() > 2);
    }

    #[test]
    fn a_radius_dimension_previews_while_its_text_is_placed() {
        let mut doc = Document::default();
        run(&mut doc, &["Circle", "0,0", "5"]);
        let mut command = super::DimRadius::default();
        command.step(&mut doc, None);
        command.step(&mut doc, Some(crate::Answer::Point([5.0, 0.0, 0.0])));
        let live = Live {
            cursor: Some([9.0, 2.0, 0.0]),
            ..Live::default()
        };
        assert!(!command.preview(&doc, &live).is_empty());
    }

    #[test]
    fn a_radius_dimension_wants_a_round_curve() {
        let mut doc = Document::default();
        run(&mut doc, &["Line", "0,0", "10,0"]);
        let mut processor = run(&mut doc, &["DimRadius"]);
        processor.click(&mut doc, [5.0, 0.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn angle_dimensions_from_points_lines_and_arcs() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["DimAngle", "Points", "0,0", "10,0", "10,10", "8,3"],
        );
        let Annotation::Angle(a) = only(&doc) else {
            panic!("not an angle");
        };
        assert!((a.value() - 45.0).abs() < 1e-9);

        let mut doc = Document::default();
        run(&mut doc, &["Line", "0,0", "10,0"]);
        run(&mut doc, &["Line", "0,0", "0,10"]);
        let mut processor = run(&mut doc, &["DimAngle"]);
        processor.click(&mut doc, [6.0, 0.0, 0.0]);
        processor.click(&mut doc, [0.0, 6.0, 0.0]);
        processor.click(&mut doc, [3.0, 3.0, 0.0]);
        let Annotation::Angle(a) = only(&doc) else {
            panic!("not an angle");
        };
        assert!((a.value() - 90.0).abs() < 1e-9);

        // An arc of 60 degrees, from 5,0 about the origin.
        let mut doc = Document::default();
        run(&mut doc, &["Arc", "0,0", "5,0", "60"]);
        let Some(Shape::Curve(arc)) = doc.state().objects.first().map(|o| o.shape.clone()) else {
            panic!("no arc");
        };
        let on = arc.sample().unwrap()[1];
        let mut processor = run(&mut doc, &["DimAngle"]);
        processor.click(&mut doc, on);
        processor.click(&mut doc, [6.0, 2.0, 0.0]);
        let Annotation::Angle(a) = only(&doc) else {
            panic!("not an angle");
        };
        assert!((a.value() - 60.0).abs() < 1e-6, "{}", a.value());
    }

    #[test]
    fn the_angle_measured_is_the_one_holding_the_arc() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["DimAngle", "Points", "0,0", "10,0", "0,10", "-5,5"],
        );
        let Annotation::Angle(a) = only(&doc) else {
            panic!("not an angle");
        };
        assert!((a.value() - 90.0).abs() < 1e-9);
        let mut doc = Document::default();
        run(
            &mut doc,
            &["DimAngle", "Points", "0,0", "10,0", "10,5", "-5,-1"],
        );
        let Annotation::Angle(a) = only(&doc) else {
            panic!("not an angle");
        };
        let small = 5.0_f64.atan2(10.0).to_degrees();
        assert!((a.value() - small).abs() < 1e-9, "{}", a.value());
    }
}
