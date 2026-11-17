//! `DimOrdinate`, as in Rhino 8 (spec `annotation.md` §3; `rhino-reference.md`): a point, then
//! where its leader ends, which gives X when the leader runs up or down and Y when it runs across
//! (unless `XDatum` or `YDatum` says which); one dimension after another until Enter. Distances are
//! from the base point, the world origin until `Basepoint` moves it.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, OrdinateAxis, OrdinateDimension};
use curva_kernel::{Curve, Point};

const X_DATUM: &str = "XDatum";
const Y_DATUM: &str = "YDatum";
const BASE_POINT: &str = "Basepoint";
const KINK_OFFSET: &str = "KinkOffset";

/// What the next answer is for.
#[derive(Clone, Copy, PartialEq)]
enum Asking {
    Point,
    Leader,
    Base,
    Kink,
}

/// The running command.
pub struct DimOrdinate {
    base: Point,
    axis: Option<OrdinateAxis>,
    kinks: Option<[f64; 2]>,
    point: Option<Point>,
    /// Where the measured point is tied, if it snapped to an object.
    tie: Option<curva_document::tie::Tie>,
    asking: Asking,
    placed: usize,
}

impl Default for DimOrdinate {
    fn default() -> Self {
        Self {
            base: [0.0; 3],
            axis: None,
            kinks: None,
            point: None,
            tie: None,
            asking: Asking::Point,
            placed: 0,
        }
    }
}

impl DimOrdinate {
    /// The dimension of the picked point with its leader ending at `leader`.
    fn dimension(&self, doc: &Document, leader: Point) -> Option<OrdinateDimension> {
        let point = self.point?;
        let [x, y] = doc.work_plane();
        let mut dimension = OrdinateDimension {
            base: self.base,
            point,
            leader,
            axis: OrdinateAxis::X,
            kinks: self.kinks,
            x,
            y,
            style: doc.current_annotation_style(),
            text: None,
        };
        dimension.axis = self.axis.unwrap_or_else(|| {
            OrdinateAxis::implied(dimension.local(point), dimension.local(leader))
        });
        Some(dimension)
    }

    fn ask(&self) -> Step {
        let request = match self.asking {
            Asking::Point => Request {
                allow_empty: self.placed > 0,
                ..Request::new("prompt.dimordinate.point", Kind::Point)
            },
            Asking::Leader => Request {
                from: self.point,
                ..Request::new("prompt.dimordinate.leader", Kind::Point)
            },
            Asking::Base => {
                return Step::Ask(Request::new("prompt.dimordinate.base", Kind::Point));
            }
            Asking::Kink => {
                return Step::Ask(Request::new("prompt.dimordinate.kink", Kind::Number));
            }
        };
        let axis = match self.axis {
            Some(OrdinateAxis::X) => "X",
            Some(OrdinateAxis::Y) => "Y",
            None => "Auto",
        };
        Step::Ask(Request {
            options: vec![X_DATUM, Y_DATUM, BASE_POINT, KINK_OFFSET],
            option_labels: vec![
                format!("{X_DATUM}{}", if axis == "X" { "=Yes" } else { "" }),
                format!("{Y_DATUM}{}", if axis == "Y" { "=Yes" } else { "" }),
                BASE_POINT.to_owned(),
                KINK_OFFSET.to_owned(),
            ],
            ..request
        })
    }

    fn done(&self) -> Step {
        Step::Done(Some(Message::with(
            "done.dimordinate",
            vec![self.placed.to_string()],
        )))
    }
}

impl Command for DimOrdinate {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            // The datum options choose the coordinate; chosen again, they go back to implied.
            Some(Answer::Option(X_DATUM)) => {
                self.axis = (self.axis != Some(OrdinateAxis::X)).then_some(OrdinateAxis::X);
                self.ask()
            }
            Some(Answer::Option(Y_DATUM)) => {
                self.axis = (self.axis != Some(OrdinateAxis::Y)).then_some(OrdinateAxis::Y);
                self.ask()
            }
            Some(Answer::Option(BASE_POINT)) => {
                self.asking = Asking::Base;
                self.ask()
            }
            Some(Answer::Option(KINK_OFFSET)) => {
                self.asking = Asking::Kink;
                self.ask()
            }
            Some(Answer::Point(at)) if self.asking == Asking::Base => {
                self.base = at;
                self.asking = if self.point.is_some() {
                    Asking::Leader
                } else {
                    Asking::Point
                };
                self.ask()
            }
            Some(Answer::Number(offset)) if self.asking == Asking::Kink => {
                if !(offset >= 0.0 && offset.is_finite()) {
                    return Step::Failed(Message::new("error.dimordinate.kink"));
                }
                self.kinks = Some([offset; 2]);
                self.asking = if self.point.is_some() {
                    Asking::Leader
                } else {
                    Asking::Point
                };
                self.ask()
            }
            Some(Answer::Point(at)) if self.asking == Asking::Point => {
                self.point = Some(at);
                self.tie = doc.pick_tie_at(at);
                self.asking = Asking::Leader;
                self.ask()
            }
            Some(Answer::Point(leader)) if self.asking == Asking::Leader => {
                let Some(dimension) = self.dimension(doc, leader) else {
                    return unexpected();
                };
                let id = doc.add_annotation(Annotation::Ordinate(dimension));
                doc.set_ties(id, vec![self.tie.take()]);
                self.placed += 1;
                self.point = None;
                self.asking = Asking::Point;
                self.ask()
            }
            Some(Answer::Empty) if self.asking == Asking::Point && self.placed > 0 => self.done(),
            Some(_) => unexpected(),
        }
    }

    /// The dimension while its leader's end is placed.
    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        if self.asking != Asking::Leader {
            return Vec::new();
        }
        let Some(dimension) = live.cursor.and_then(|at| self.dimension(doc, at)) else {
            return Vec::new();
        };
        curva_document::shape::annotation_drawing(
            &Annotation::Ordinate(dimension),
            doc.state(),
            doc.space(),
        )
        .lines
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_document::Shape;
    use curva_document::annotation::{Annotation, OrdinateAxis};

    fn ordinates(doc: &Document) -> Vec<curva_document::annotation::OrdinateDimension> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| match &o.shape {
                Shape::Annotation(Annotation::Ordinate(d)) => Some(d.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn ordinate_dimensions_follow_their_leaders_until_enter() {
        let mut doc = Document::default();
        let processor = run(
            &mut doc,
            &["DimOrdinate", "30,15", "32,40", "30,15", "-10,12", ""],
        );
        let made = ordinates(&doc);
        assert_eq!(made.len(), 2, "{:?}", processor.log());
        assert_eq!(made[0].axis, OrdinateAxis::X, "a leader up gives X");
        assert!((made[0].value() - 30.0).abs() < 1e-12);
        assert_eq!(made[1].axis, OrdinateAxis::Y, "a leader across gives Y");
        assert!((made[1].value() - 15.0).abs() < 1e-12);
        assert!(made[0].kinks.is_none(), "kinks at the default");
    }

    #[test]
    fn an_ordinate_dimension_previews_while_its_leader_is_placed() {
        use crate::{Command, Live};
        let mut doc = Document::default();
        let mut command = super::DimOrdinate::default();
        command.step(&mut doc, None);
        command.step(&mut doc, Some(crate::Answer::Point([30.0, 15.0, 0.0])));
        let live = Live {
            cursor: Some([32.0, 40.0, 0.0]),
            ..Live::default()
        };
        assert!(!command.preview(&doc, &live).is_empty());
    }

    #[test]
    fn the_base_point_datum_and_kinks_can_be_set() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &[
                "DimOrdinate",
                "Basepoint",
                "10,10",
                "YDatum",
                "KinkOffset",
                "4",
                "30,15",
                "32,40",
                "",
            ],
        );
        let made = ordinates(&doc);
        assert_eq!(made.len(), 1);
        assert_eq!(
            made[0].axis,
            OrdinateAxis::Y,
            "forced, though the leader runs up"
        );
        assert!((made[0].value() - 5.0).abs() < 1e-12);
        assert_eq!(made[0].kinks, Some([4.0, 4.0]));
    }
}
