//! `DimArcLength`: the length along an arc, as AutoCAD's arc length dimension (spec
//! `annotation.md` §3; Rhino 8 has none). Pick an arc, then where the dimension arc goes; it
//! previews as it is placed. As AutoCAD's DIMARC, `Partial` measures between two points chosen on
//! the arc, and `Leader` adds an arrow from the dimension in to the arc, for a turn over a quarter.

use super::radial::picked_curve;
use crate::Message;
use crate::commands::arguments::point;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_document::annotation::{Annotation, ArcLengthDimension};
use curva_kernel::{Curve, Point, dot, sub};
use std::f64::consts::{FRAC_PI_2, TAU};

const PARTIAL: &str = "Partial";
const LEADER: &str = "Leader";

/// `DimArcLength`, on the command framework.
#[derive(Default)]
pub struct DimArcLength;

/// A picked arc: its centre, radius, plane, and where it starts and how far it turns (radians,
/// counter-clockwise on the plane from X).
#[derive(Clone, Copy)]
struct Arc {
    centre: Point,
    radius: f64,
    x: Point,
    y: Point,
    sweep: f64,
}

impl Arc {
    /// The arc picked at `at`, if what is there is an arc.
    fn picked(cx: &Context<'_>, at: Point) -> Option<Self> {
        let (curve, _) = picked_curve(cx.doc, at)?;
        let Curve::Arc {
            centre,
            radius,
            x,
            y,
            sweep,
        } = curve
        else {
            return None;
        };
        Some(Self {
            centre,
            radius,
            x,
            y,
            sweep,
        })
    }

    /// How far round from the arc's start a point lies, seen from the centre, in radians.
    fn turn_to(&self, p: Point) -> f64 {
        let d = sub(p, self.centre);
        dot(d, self.y).atan2(dot(d, self.x)).rem_euclid(TAU)
    }

    /// Whether a point lies within the arc's turn, seen from the centre.
    fn spans(&self, p: Point) -> bool {
        self.turn_to(p) <= self.sweep + 1e-9
    }

    /// The point on the arc this far round from its start.
    fn at(&self, turn: f64) -> Point {
        let (s, c) = turn.sin_cos();
        std::array::from_fn(|i| self.centre[i] + self.radius * (self.x[i] * c + self.y[i] * s))
    }
}

impl DimArcLength {
    /// Where the measured part starts and ends, as turns from the arc's start: the whole arc, or
    /// between the two points chosen with `Partial`, in the arc's own direction.
    fn measured(cx: &Context<'_>, arc: &Arc) -> Option<(f64, f64)> {
        if cx.mode != Some(PARTIAL) {
            return Some((0.0, arc.sweep));
        }
        let (a, b) = (arc.turn_to(point(cx, 1)?), arc.turn_to(point(cx, 2)?));
        Some((a.min(b), a.max(b)))
    }
}

impl Recipe for DimArcLength {
    const NAME: &'static str = "dimarclength";
    const OPTIONS: &'static [OptionDef] =
        &[OptionDef::mode(PARTIAL), OptionDef::toggle(LEADER, false)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::pick("prompt.dimarclength.arc").offering(&[])];
        let Some(arc) = point(cx, 0).and_then(|at| Arc::picked(cx, at)) else {
            return steps;
        };
        if cx.mode == Some(PARTIAL) {
            steps.push(StepDef::point("prompt.dimarclength.first").offering(&[]));
            steps.push(StepDef::point("prompt.dimarclength.second").offering(&[]));
        }
        // As AutoCAD, a leader is offered only for a turn over a quarter.
        let mut offered = Vec::new();
        if cx.mode != Some(PARTIAL) {
            offered.push(PARTIAL);
        }
        if arc.sweep > FRAC_PI_2 {
            offered.push(LEADER);
        }
        steps.push(StepDef::point("prompt.dimarclength.place").offering(&offered));
        steps
    }

    fn accepts(&self, cx: &Context<'_>) -> bool {
        let Some(arc) = point(cx, 0).and_then(|at| Arc::picked(cx, at)) else {
            return false;
        };
        // The points of a partial one must lie on the arc's turn, and not both in one place.
        match (cx.mode == Some(PARTIAL), cx.values.len()) {
            (true, 2) => point(cx, 1).is_some_and(|p| arc.spans(p)),
            (true, 3) => point(cx, 2).is_some_and(|p| {
                arc.spans(p)
                    && point(cx, 1).is_some_and(|q| (arc.turn_to(p) - arc.turn_to(q)).abs() > 1e-9)
            }),
            _ => true,
        }
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(arc) = point(cx, 0).and_then(|at| Arc::picked(cx, at)) else {
            return Ok(Made::default());
        };
        let place = if cx.mode == Some(PARTIAL) { 3 } else { 1 };
        let (Some((from, to)), Some(arc_at)) = (Self::measured(cx, &arc), point(cx, place)) else {
            return Ok(Made::default());
        };
        let dimension = ArcLengthDimension {
            centre: arc.centre,
            start: arc.at(from),
            end: arc.at(to),
            arc_at,
            x: arc.x,
            y: arc.y,
            style: cx.doc.current_annotation_style(),
            text: None,
            partial: cx.mode == Some(PARTIAL),
            leader: cx.toggle(LEADER) && arc.sweep > FRAC_PI_2,
        };
        let shown = cx.doc.length_text(dimension.value());
        Ok(Made {
            add: vec![Shape::Annotation(Annotation::ArcLength(dimension))],
            message: Some(Message::with("done.dim", vec![shown])),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::annotation::{Annotation, ArcLengthDimension};
    use curva_document::{Document, Shape};
    use curva_kernel::Curve;
    use std::f64::consts::PI;

    /// An arc of radius `r` about the origin, from +X round to `degrees`.
    fn arc(doc: &mut Document, r: f64, degrees: f64) {
        let (s, c) = degrees.to_radians().sin_cos();
        doc.add(
            Curve::arc(
                [0.0; 3],
                [r, 0.0, 0.0],
                [r * c, r * s, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .unwrap(),
        );
    }

    fn made(doc: &Document) -> &ArcLengthDimension {
        let Some(Shape::Annotation(Annotation::ArcLength(d))) =
            doc.state().objects.last().map(|o| &o.shape)
        else {
            panic!("no arc length dimension");
        };
        d
    }

    #[test]
    fn an_arc_length_dimension_measures_a_picked_arc() {
        let mut doc = Document::default();
        arc(&mut doc, 10.0, 90.0);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        processor.click(&mut doc, [11.0, 11.0, 0.0]);
        let d = made(&doc);
        assert!((d.value() - 5.0 * PI).abs() < 1e-6);
        assert!(!d.partial && !d.leader);
        assert!(processor.log().iter().any(|l| l.contains("15.708")));
    }

    #[test]
    fn a_line_is_not_an_arc_and_is_asked_again() {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0, -5.0, 0.0], [0.0, -20.0, 0.0]).unwrap());
        arc(&mut doc, 10.0, 90.0);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, -10.0, 0.0]);
        // The click on the line was dropped and the arc is asked for again.
        assert!(
            processor
                .prompt()
                .is_some_and(|p| p.contains("Select an arc"))
        );
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        processor.click(&mut doc, [11.0, 11.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
    }

    #[test]
    fn the_preview_is_the_dimension_as_drawn() {
        let mut doc = Document::default();
        arc(&mut doc, 10.0, 90.0);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        // Extension lines, 32 pieces of dimension arc and the arc symbol, not three bare lines.
        assert!(processor.preview(&doc, Some([12.0, 12.0, 0.0]), "").len() > 32);
    }

    #[test]
    fn partial_measures_between_two_points_on_the_arc() {
        let mut doc = Document::default();
        arc(&mut doc, 10.0, 180.0);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        processor.submit(&mut doc, "Partial");
        // Chosen the other way round: it still measures in the arc's own direction.
        let (s, c) = 150f64.to_radians().sin_cos();
        processor.click(&mut doc, [10.0 * c, 10.0 * s, 0.0]);
        let (s, c) = 30f64.to_radians().sin_cos();
        processor.click(&mut doc, [10.0 * c, 10.0 * s, 0.0]);
        processor.click(&mut doc, [0.0, 14.0, 0.0]);
        let d = made(&doc);
        assert!(d.partial);
        assert!((d.value() - 10.0 * 120f64.to_radians()).abs() < 1e-9);
        assert!((d.start[0] - 10.0 * 30f64.to_radians().cos()).abs() < 1e-9);
    }

    #[test]
    fn a_partial_point_off_the_arcs_turn_is_asked_again() {
        let mut doc = Document::default();
        arc(&mut doc, 10.0, 90.0);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        processor.submit(&mut doc, "Partial");
        processor.click(&mut doc, [-10.0, -1.0, 0.0]);
        processor.click(&mut doc, [7.0, 7.0, 0.0]);
        processor.click(&mut doc, [10.0, 0.0, 0.0]);
        processor.click(&mut doc, [12.0, 12.0, 0.0]);
        let d = made(&doc);
        assert!((d.value() - 10.0 * PI / 4.0).abs() < 1e-9, "{}", d.value());
    }

    #[test]
    fn leader_on_an_arc_over_a_quarter() {
        let mut doc = Document::default();
        arc(&mut doc, 10.0, 180.0);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        processor.submit(&mut doc, "Leader");
        processor.click(&mut doc, [0.0, 15.0, 0.0]);
        let d = made(&doc);
        assert!(d.leader);
        let (from, to) = d.leader_points().unwrap();
        assert!((from[1] - 15.0).abs() < 1e-9 && (to[1] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn no_leader_on_a_quarter_or_less() {
        let mut doc = Document::default();
        arc(&mut doc, 10.0, 180.0);
        // Turned on once, it is remembered; a quarter arc still gets none.
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        processor.submit(&mut doc, "Leader");
        processor.cancel(&mut doc);
        arc(&mut doc, 20.0, 90.0);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        let (s, c) = 45f64.to_radians().sin_cos();
        processor.click(&mut doc, [20.0 * c, 20.0 * s, 0.0]);
        processor.click(&mut doc, [24.0, 24.0, 0.0]);
        assert!(!made(&doc).leader);
        // The 180° arc again: the remembered Leader is used without choosing it.
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        processor.click(&mut doc, [0.0, 15.0, 0.0]);
        assert!(made(&doc).leader);
    }
}
