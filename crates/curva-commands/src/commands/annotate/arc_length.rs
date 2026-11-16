//! `DimArcLength`: the length along an arc, as AutoCAD's arc length dimension (spec
//! `annotation.md` §3; Rhino 8 has none). Pick an arc, then where the dimension arc goes; it
//! previews as it is placed.

use super::radial::picked_curve;
use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, ArcLengthDimension};
use curva_kernel::{Curve, Point};

/// The running command.
#[derive(Default)]
pub struct DimArcLength {
    /// The picked arc: its centre, start, end and plane.
    arc: Option<(Point, Point, Point, Point, Point)>,
}

impl DimArcLength {
    fn dimension(&self, doc: &Document, arc_at: Point) -> Option<ArcLengthDimension> {
        let (centre, start, end, x, y) = self.arc?;
        Some(ArcLengthDimension {
            centre,
            start,
            end,
            arc_at,
            x,
            y,
            style: doc.current_annotation_style(),
            text: None,
        })
    }
}

/// An arc's centre, start, end and plane (X towards its start).
fn arc_of(curve: &Curve) -> Option<(Point, Point, Point, Point, Point)> {
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
    let (s, c) = sweep.sin_cos();
    let start = std::array::from_fn(|i| centre[i] + radius * x[i]);
    let end = std::array::from_fn(|i| centre[i] + radius * (x[i] * c + y[i] * s));
    Some((*centre, start, end, *x, *y))
}

impl Command for DimArcLength {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => Step::Ask(Request::new("prompt.dimarclength.arc", Kind::PickPart)),
            Some(Answer::Point(at)) if self.arc.is_none() => {
                match picked_curve(doc, at).and_then(|(c, _)| arc_of(&c)) {
                    Some(arc) => {
                        self.arc = Some(arc);
                        Step::Ask(Request::new("prompt.dimarclength.place", Kind::Point))
                    }
                    None => Step::Ask(Request::new("prompt.dimarclength.not_arc", Kind::PickPart)),
                }
            }
            Some(Answer::Point(at)) => {
                let Some(dimension) = self.dimension(doc, at) else {
                    return unexpected();
                };
                let shown = doc.length_text(dimension.value());
                doc.add_annotation(Annotation::ArcLength(dimension));
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
            &Annotation::ArcLength(dimension),
            doc.state(),
            doc.space(),
        )
        .lines
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use crate::{Answer, Command, Live};
    use curva_document::annotation::Annotation;
    use curva_document::{Document, Shape};
    use curva_kernel::Curve;

    fn quarter(doc: &mut Document) {
        // A quarter of a circle of radius 10 about the origin, from +X to +Y.
        doc.add(
            Curve::arc(
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .unwrap(),
        );
    }

    #[test]
    fn an_arc_length_dimension_measures_a_picked_arc() {
        let mut doc = Document::default();
        quarter(&mut doc);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, 10.0, 0.0]);
        processor.click(&mut doc, [11.0, 11.0, 0.0]);
        let Shape::Annotation(Annotation::ArcLength(d)) = &doc.state().objects[1].shape else {
            panic!("no arc length dimension");
        };
        assert!((d.value() - 5.0 * std::f64::consts::PI).abs() < 1e-6);
        assert!(processor.log().iter().any(|l| l.contains("15.708")));
    }

    #[test]
    fn a_line_is_not_an_arc_and_the_dimension_previews() {
        let mut doc = Document::default();
        doc.add(Curve::line([0.0, -5.0, 0.0], [0.0, -20.0, 0.0]).unwrap());
        quarter(&mut doc);
        let mut processor = run(&mut doc, &["DimArcLength"]);
        processor.click(&mut doc, [0.0, -10.0, 0.0]);
        processor.click(&mut doc, [11.0, 11.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 2, "a line is refused");
        let mut command = super::DimArcLength::default();
        command.step(&mut doc, None);
        command.step(&mut doc, Some(Answer::Point([10.0, 0.0, 0.0])));
        let live = Live {
            cursor: Some([12.0, 12.0, 0.0]),
            ..Live::default()
        };
        assert!(command.preview(&doc, &live).len() > 32);
    }
}
