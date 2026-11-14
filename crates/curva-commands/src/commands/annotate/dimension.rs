//! Linear dimensions, as in Rhino 8 (spec `annotation.md` §3; `rhino-reference.md`): `Dim`
//! (horizontal or vertical), `DimAligned` and `DimRotated`. Two points, then a third where the
//! dimension line goes; the finished dimension previews while it is placed.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Dimension, DimensionKind};
use curva_kernel::{Curve, Point};

const HORIZONTAL: &str = "Horizontal";
const VERTICAL: &str = "Vertical";

/// Which linear dimension the command makes.
#[derive(Clone, Copy, PartialEq)]
enum Making {
    /// `Dim`: horizontal or vertical, by where the line goes unless forced.
    Linear(Option<DimensionKind>),
    /// `DimAligned`.
    Aligned,
    /// `DimRotated`, once its angle is known.
    Rotated(Option<f64>),
}

/// The running command.
pub struct LinearDimension {
    making: Making,
    first: Option<Point>,
    second: Option<Point>,
}

impl LinearDimension {
    fn new(making: Making) -> Self {
        Self {
            making,
            first: None,
            second: None,
        }
    }

    /// The dimension to the points known so far, with its line through `line`.
    fn dimension(&self, doc: &Document, line: Point) -> Option<Dimension> {
        let (first, second) = (self.first?, self.second?);
        let [x, y] = doc.work_plane();
        let kind = match self.making {
            Making::Linear(Some(kind)) => kind,
            Making::Linear(None) => {
                // Moving away up or down from the points measures across; left or right, up.
                let middle: Point = std::array::from_fn(|i| (first[i] + second[i]) / 2.0);
                let off: Point = std::array::from_fn(|i| line[i] - middle[i]);
                let along = |axis: Point| (0..3).map(|i| off[i] * axis[i]).sum::<f64>().abs();
                if along(y) >= along(x) {
                    DimensionKind::Horizontal
                } else {
                    DimensionKind::Vertical
                }
            }
            Making::Aligned => DimensionKind::Aligned,
            Making::Rotated(angle) => DimensionKind::Rotated(angle?),
        };
        Some(Dimension {
            kind,
            first,
            second,
            line,
            x,
            y,
            style: doc.current_annotation_style(),
            text: None,
        })
    }

    fn ask(&self) -> Step {
        let (key, kind) = match (self.making, self.first, self.second) {
            (Making::Rotated(None), ..) => ("prompt.dim.angle", Kind::Number),
            (_, None, _) => ("prompt.dim.first", Kind::Point),
            (_, Some(_), None) => ("prompt.dim.second", Kind::Point),
            _ => ("prompt.dim.line", Kind::Point),
        };
        let options = match self.making {
            Making::Linear(_) => vec![HORIZONTAL, VERTICAL],
            _ => Vec::new(),
        };
        Step::Ask(Request {
            options,
            from: self.second.or(self.first),
            preview: self.first.into_iter().chain(self.second).collect(),
            ..Request::new(key, kind)
        })
    }
}

impl Command for LinearDimension {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Option(HORIZONTAL)) => {
                self.making = Making::Linear(Some(DimensionKind::Horizontal));
                self.ask()
            }
            Some(Answer::Option(VERTICAL)) => {
                self.making = Making::Linear(Some(DimensionKind::Vertical));
                self.ask()
            }
            Some(Answer::Number(degrees)) if self.making == Making::Rotated(None) => {
                self.making = Making::Rotated(Some(degrees.to_radians()));
                self.ask()
            }
            Some(Answer::Point(p)) if self.first.is_none() => {
                self.first = Some(p);
                self.ask()
            }
            Some(Answer::Point(p)) if self.second.is_none() => {
                self.second = Some(p);
                self.ask()
            }
            Some(Answer::Point(line)) => {
                let Some(dimension) = self.dimension(doc, line) else {
                    return unexpected();
                };
                if dimension.value() < 1e-9 {
                    return Step::Failed(Message::new("error.dim_zero"));
                }
                let shown = doc.length_text(dimension.value());
                doc.add_annotation(Annotation::Dimension(dimension));
                Step::Done(Some(Message::with("done.dim", vec![shown])))
            }
            Some(_) => unexpected(),
        }
    }

    /// The finished dimension, while its line is placed.
    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(dimension) = live.cursor.and_then(|at| self.dimension(doc, at)) else {
            return Vec::new();
        };
        curva_document::shape::annotation_drawing(
            &Annotation::Dimension(dimension),
            doc.state(),
            doc.space(),
        )
        .lines
    }
}

/// `Dim`.
pub struct Dim(LinearDimension);

impl Default for Dim {
    fn default() -> Self {
        Self(LinearDimension::new(Making::Linear(None)))
    }
}

/// `DimAligned`.
pub struct DimAligned(LinearDimension);

impl Default for DimAligned {
    fn default() -> Self {
        Self(LinearDimension::new(Making::Aligned))
    }
}

/// `DimRotated`.
pub struct DimRotated(LinearDimension);

impl Default for DimRotated {
    fn default() -> Self {
        Self(LinearDimension::new(Making::Rotated(None)))
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
forward!(Dim);
forward!(DimAligned);
forward!(DimRotated);

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::annotation::{Annotation, DimensionKind};
    use curva_document::{Document, Shape};

    fn only_dimension(doc: &Document) -> curva_document::annotation::Dimension {
        match &doc.state().objects[0].shape {
            Shape::Annotation(Annotation::Dimension(d)) => d.clone(),
            _ => panic!("not a dimension"),
        }
    }

    #[test]
    fn dim_is_horizontal_or_vertical_by_where_its_line_goes() {
        let mut doc = Document::default();
        run(&mut doc, &["Dim", "0,0", "30,40", "15,60"]);
        let d = only_dimension(&doc);
        assert_eq!(d.kind, DimensionKind::Horizontal);
        assert!((d.value() - 30.0).abs() < 1e-9);
        let mut doc = Document::default();
        run(&mut doc, &["Dim", "0,0", "30,40", "60,20"]);
        let d = only_dimension(&doc);
        assert_eq!(d.kind, DimensionKind::Vertical);
        assert!((d.value() - 40.0).abs() < 1e-9);
        let mut doc = Document::default();
        run(&mut doc, &["Dim", "Vertical", "0,0", "30,40", "15,60"]);
        assert_eq!(only_dimension(&doc).kind, DimensionKind::Vertical);
    }

    #[test]
    fn aligned_and_rotated_dimensions_measure_along_their_direction() {
        let mut doc = Document::default();
        run(&mut doc, &["DimAligned", "0,0", "30,40", "0,40"]);
        assert!((only_dimension(&doc).value() - 50.0).abs() < 1e-9);
        let mut doc = Document::default();
        run(&mut doc, &["DimRotated", "90", "0,0", "30,40", "50,20"]);
        assert!((only_dimension(&doc).value() - 40.0).abs() < 1e-9);
    }

    #[test]
    fn a_dimension_draws_its_lines_arrows_and_value() {
        let mut doc = Document::default();
        run(&mut doc, &["Dim", "0,0", "100,0", "50,20"]);
        let Shape::Annotation(annotation) = &doc.state().objects[0].shape else {
            panic!("not annotation");
        };
        let drawing = curva_document::shape::annotation_drawing(annotation, doc.state(), None);
        // Two extension lines, the dimension line, two arrowheads, and the letters of "100.00".
        assert!(drawing.lines.len() > 5);
        assert!(drawing.fills.len() >= 2 + 5);
    }
}
