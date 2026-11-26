//! `ScaleBar`: a scale bar for a detail view (spec `layouts.md` §4), Serpentine3D's command, as
//! Rhino has none. It follows the detail's scale when that changes.

use crate::commands::annotate::text::usual_height;
use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Text, TextAlign};
use curva_document::sheet::ScaleBar as Bar;
use curva_kernel::{Curve, Point};

const LENGTH: &str = "Length";
const DIVISIONS: &str = "Divisions";

/// What the command asks next.
#[derive(Clone, Copy, Default, PartialEq)]
enum Asking {
    #[default]
    Place,
    Detail,
    Length,
    Divisions,
}

/// The running command: on a layout, the detail to measure (asked only when the sheet has more
/// than one scaled detail), then where the bar's lower left corner goes. `Length` sets the model
/// length it stands for (0 picks a round one); `Divisions` how many blocks (0 picks).
#[derive(Default)]
pub struct ScaleBar {
    asking: Asking,
    detail: Option<u64>,
    length: f64,
    divisions: u32,
}

/// The scaled details on the open layout, by their frames.
fn scaled_details(doc: &Document) -> Vec<u64> {
    doc.space()
        .and_then(|id| doc.layout(id))
        .map(|l| {
            l.details
                .iter()
                .filter(|d| d.scale.is_some())
                .map(|d| d.frame)
                .collect()
        })
        .unwrap_or_default()
}

impl ScaleBar {
    fn bar(&self, doc: &Document, at: Point, detail: u64) -> Bar {
        let [x, y] = doc.work_plane();
        Bar {
            at: Text {
                content: String::new(),
                origin: at,
                x,
                y,
                height: usual_height(doc),
                font: None,
                align: TextAlign::Left,
                style: doc.current_annotation_style(),
            },
            detail,
            length: (self.length > 0.0).then_some(self.length),
            divisions: self.divisions,
        }
    }

    fn ask(&self, doc: &Document) -> Step {
        let length = if self.length > 0.0 {
            doc.length_text(self.length)
        } else {
            "Auto".to_owned()
        };
        let divisions = if self.divisions > 0 {
            self.divisions.to_string()
        } else {
            "Auto".to_owned()
        };
        let request = match self.asking {
            Asking::Place => Request::new("prompt.scalebar.place", Kind::Point),
            Asking::Detail => Request::new("prompt.scalebar.detail", Kind::Objects),
            Asking::Length => Request::new("prompt.scalebar.length", Kind::Number),
            Asking::Divisions => Request::new("prompt.scalebar.divisions", Kind::Number),
        };
        Step::Ask(Request {
            options: vec![LENGTH, DIVISIONS],
            option_labels: vec![
                format!("{LENGTH}={length}"),
                format!("{DIVISIONS}={divisions}"),
            ],
            ..request
        })
    }
}

impl Command for ScaleBar {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if doc.space().is_none() {
            return Step::Failed(Message::new("error.sheet_only"));
        }
        let details = scaled_details(doc);
        match answer {
            None => {
                match details.as_slice() {
                    [] => return Step::Failed(Message::new("error.scalebar.no_detail")),
                    [only] => self.detail = Some(*only),
                    _ => self.asking = Asking::Detail,
                }
                self.ask(doc)
            }
            Some(Answer::Option(LENGTH)) => {
                self.asking = Asking::Length;
                self.ask(doc)
            }
            Some(Answer::Option(DIVISIONS)) => {
                self.asking = Asking::Divisions;
                self.ask(doc)
            }
            Some(Answer::Number(value)) if self.asking == Asking::Length => {
                self.length = value.max(0.0);
                self.asking = self.next();
                self.ask(doc)
            }
            Some(Answer::Number(value)) if self.asking == Asking::Divisions => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let whole = value.round().clamp(0.0, 100.0) as u32;
                self.divisions = whole;
                self.asking = self.next();
                self.ask(doc)
            }
            Some(Answer::Objects(ids)) if self.asking == Asking::Detail => {
                match ids.iter().find(|id| details.contains(id)) {
                    Some(frame) => {
                        self.detail = Some(*frame);
                        self.asking = Asking::Place;
                    }
                    None => return Step::Failed(Message::new("error.scalebar.not_a_detail")),
                }
                self.ask(doc)
            }
            Some(Answer::Point(at)) if self.asking == Asking::Place => {
                let Some(detail) = self.detail else {
                    return unexpected();
                };
                doc.add_annotation(Annotation::ScaleBar(self.bar(doc, at, detail)));
                Step::Done(Some(Message::new("done.scalebar")))
            }
            Some(_) => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let (Some(detail), Some(at), Asking::Place) = (self.detail, live.cursor, self.asking)
        else {
            return Vec::new();
        };
        let bar = Annotation::ScaleBar(self.bar(doc, at, detail));
        curva_document::shape::annotation_drawing(&bar, doc.state(), doc.space()).lines
    }
}

impl ScaleBar {
    /// The step after an option's value: the detail if none is chosen yet, else the place.
    fn next(&self) -> Asking {
        if self.detail.is_some() {
            Asking::Place
        } else {
            Asking::Detail
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use crate::{Command, Live};
    use curva_document::annotation::Annotation;
    use curva_document::{Document, Shape};

    /// A layout with a scaled detail open, as the Detail command makes it.
    fn sheet_with_detail() -> Document {
        let mut doc = Document::default();
        run(&mut doc, &["Layout", "", "0", "Detail", "20,20", "220,160"]);
        doc
    }

    fn only_bar(doc: &Document) -> curva_document::sheet::ScaleBar {
        doc.state()
            .objects
            .iter()
            .find_map(|o| match &o.shape {
                Shape::Annotation(Annotation::ScaleBar(bar)) => Some(bar.clone()),
                _ => None,
            })
            .expect("a scale bar")
    }

    #[test]
    fn a_bar_measures_the_only_detail_where_it_is_placed() {
        let mut doc = sheet_with_detail();
        let frame = doc.layouts()[0].details[0].frame;
        run(&mut doc, &["ScaleBar", "240,20"]);
        let bar = only_bar(&doc);
        assert_eq!(bar.detail, frame);
        assert_eq!(bar.at.origin, [240.0, 20.0, 0.0]);
        assert!(
            (bar.at.height - 3.5).abs() < 1e-9,
            "the style's paper height"
        );
        assert_eq!((bar.length, bar.divisions), (None, 0));
    }

    #[test]
    fn length_and_divisions_can_be_typed() {
        let mut doc = sheet_with_detail();
        run(
            &mut doc,
            &["ScaleBar", "Length", "5000", "Divisions", "5", "240,20"],
        );
        let bar = only_bar(&doc);
        assert_eq!((bar.length, bar.divisions), (Some(5000.0), 5));
    }

    #[test]
    fn with_two_details_the_detail_is_picked_first() {
        let mut doc = sheet_with_detail();
        run(&mut doc, &["Detail", "230,20", "400,160"]);
        let second = doc.layouts()[0].details[1].frame;
        let mut command = super::ScaleBar::default();
        command.step(&mut doc, None);
        command.step(&mut doc, Some(crate::Answer::Objects(vec![second])));
        let live = Live {
            cursor: Some([240.0, 200.0, 0.0]),
            number: None,
        };
        assert!(
            !command.preview(&doc, &live).is_empty(),
            "the bar follows the cursor"
        );
        command.step(&mut doc, Some(crate::Answer::Point([240.0, 200.0, 0.0])));
        assert_eq!(only_bar(&doc).detail, second);
    }

    #[test]
    fn a_bar_needs_a_sheet_with_a_scaled_detail() {
        let mut doc = Document::default();
        run(&mut doc, &["ScaleBar", "0,0"]);
        run(&mut doc, &["Layout", "", "0", "ScaleBar", "0,0"]);
        assert!(
            doc.state()
                .objects
                .iter()
                .all(|o| !matches!(o.shape, Shape::Annotation(_)))
        );
    }
}
