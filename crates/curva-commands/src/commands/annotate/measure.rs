//! `DimCurveLength`, `DimArea` and `DimVolume`, as in Rhino 8 (spec `annotation.md` §3;
//! `rhino-reference.md`): an object, then where the text goes, or with `Type=Leader` the
//! leader's points as for `Leader`, Enter to end. As in Rhino, the words are the field for the
//! measure (`%<CurveLength("id")>%`, `Area`, `Volume`), so they follow the object as it changes.

use crate::commands::{ask_objects, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Leader, Text, TextAlign};
use curva_document::fields::{FieldContext, resolve};
use curva_document::shape::annotation_drawing;
use curva_kernel::{Curve, Point};

const TYPE: &str = "Type";

/// The running command, for one of the three measures.
pub struct Measure {
    /// The field's name, which is also what is measured.
    field: &'static str,
    /// The prompt for the object, and the message when it has nothing to measure.
    prompt: &'static str,
    refused: &'static str,
    object: Option<u64>,
    leader: bool,
    points: Vec<Point>,
}

impl Measure {
    fn new(field: &'static str, prompt: &'static str, refused: &'static str) -> Self {
        Self {
            field,
            prompt,
            refused,
            object: None,
            leader: false,
            points: Vec::new(),
        }
    }

    fn words(&self, id: u64) -> String {
        format!("%<{}(\"{id}\")>%", self.field)
    }

    /// The measure of the object as the field shows it, if the object has one.
    fn shown(&self, doc: &Document, id: u64) -> Option<String> {
        let shown = resolve(
            &self.words(id),
            doc.state(),
            doc.space(),
            &FieldContext::now(None),
        );
        shown.parse::<f64>().ok().map(|_| shown)
    }

    fn ask(&self) -> Step {
        let label = format!("{TYPE}={}", if self.leader { "Leader" } else { "Text" });
        let request = if self.object.is_none() {
            return ask_objects(self.prompt);
        } else if !self.leader {
            Request::new("prompt.dimmeasure.text", Kind::Point)
        } else {
            let key = match self.points.len() {
                0 => "prompt.leader.head",
                1 => "prompt.leader.next",
                _ => "prompt.leader.next_or_end",
            };
            Request {
                allow_empty: self.points.len() >= 2,
                from: self.points.last().copied(),
                preview: self.points.clone(),
                ..Request::new(key, Kind::Point)
            }
        };
        Step::Ask(Request {
            options: vec![TYPE],
            option_labels: vec![label],
            ..request
        })
    }

    fn done(&self, doc: &mut Document, annotation: Annotation, id: u64) -> Step {
        let shown = self.shown(doc, id).unwrap_or_default();
        doc.add_annotation(annotation);
        Step::Done(Some(Message::with("done.dimmeasure", vec![shown])))
    }
}

impl Command for Measure {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => {
                // One object already chosen is taken, as Rhino takes it.
                match doc.selection().as_slice() {
                    [id] if self.shown(doc, *id).is_some() => self.object = Some(*id),
                    _ => {}
                }
                self.ask()
            }
            Some(Answer::Objects(list)) if self.object.is_none() => {
                let Some(id) = list
                    .iter()
                    .copied()
                    .find(|id| self.shown(doc, *id).is_some())
                else {
                    return Step::Ask(Request::new(self.refused, Kind::Objects));
                };
                self.object = Some(id);
                self.ask()
            }
            Some(Answer::Option(TYPE)) => {
                self.leader = !self.leader;
                self.points.clear();
                self.ask()
            }
            Some(Answer::Point(at)) if self.object.is_some() && !self.leader => {
                match self.made(doc, &[at]) {
                    Some((made, id)) => self.done(doc, made, id),
                    None => unexpected(),
                }
            }
            Some(Answer::Point(p)) if self.object.is_some() => {
                self.points.push(p);
                self.ask()
            }
            Some(Answer::Empty) if self.leader && self.points.len() >= 2 => {
                match self.made(doc, &self.points) {
                    Some((made, id)) => self.done(doc, made, id),
                    None => unexpected(),
                }
            }
            Some(_) => unexpected(),
        }
    }

    /// The note at the pointer, or the leader so far out to it.
    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let mut points = if self.leader {
            self.points.clone()
        } else {
            Vec::new()
        };
        points.extend(live.cursor);
        match self.made(doc, &points) {
            Some((made, _)) => annotation_drawing(&made, doc.state(), doc.space()).lines,
            None => Vec::new(),
        }
    }
}

impl Measure {
    /// The note at `points[0]`, or the leader through `points`, with the object's id; `None`
    /// before an object is chosen or with too few points.
    fn made(&self, doc: &Document, points: &[Point]) -> Option<(Annotation, u64)> {
        let id = self.object?;
        let [x, y] = doc.work_plane();
        let style = doc.current_annotation_style();
        let made = if self.leader {
            if points.len() < 2 {
                return None;
            }
            Annotation::Leader(Leader {
                points: points.to_vec(),
                content: self.words(id),
                x,
                y,
                landing: false,
                style,
            })
        } else {
            Annotation::Text(Text {
                content: self.words(id),
                origin: *points.first()?,
                x,
                y,
                height: super::text::usual_height(doc),
                font: None,
                align: TextAlign::Left,
                style,
            })
        };
        Some((made, id))
    }
}

/// `DimCurveLength`.
pub struct DimCurveLength(Measure);

impl Default for DimCurveLength {
    fn default() -> Self {
        Self(Measure::new(
            "CurveLength",
            "prompt.dimcurvelength.object",
            "prompt.dimcurvelength.not_curve",
        ))
    }
}

/// `DimArea`.
pub struct DimArea(Measure);

impl Default for DimArea {
    fn default() -> Self {
        Self(Measure::new(
            "Area",
            "prompt.dimarea.object",
            "prompt.dimarea.no_area",
        ))
    }
}

/// `DimVolume`.
pub struct DimVolume(Measure);

impl Default for DimVolume {
    fn default() -> Self {
        Self(Measure::new(
            "Volume",
            "prompt.dimvolume.object",
            "prompt.dimvolume.no_volume",
        ))
    }
}

macro_rules! measuring {
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

measuring!(DimCurveLength);
measuring!(DimArea);
measuring!(DimVolume);

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::annotation::Annotation;
    use curva_document::fields::{FieldContext, resolve};
    use curva_document::{Document, Shape};
    use curva_kernel::{Body, Curve};

    fn last(doc: &Document) -> Annotation {
        match &doc.state().objects.last().unwrap().shape {
            Shape::Annotation(a) => a.clone(),
            other => panic!("not an annotation: {other:?}"),
        }
    }

    fn shown(doc: &Document, words: &str) -> String {
        resolve(words, doc.state(), None, &FieldContext::now(None))
    }

    #[test]
    fn a_curve_length_note_is_rhinos_field_and_follows_the_curve() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [3.0, 4.0, 0.0]).unwrap());
        doc.select(&[line]);
        run(&mut doc, &["DimCurveLength", "10,10"]);
        let Annotation::Text(note) = last(&doc) else {
            panic!("not a note");
        };
        assert_eq!(note.content, format!("%<CurveLength(\"{line}\")>%"));
        assert_eq!(note.origin, [10.0, 10.0, 0.0]);
        assert_eq!(shown(&doc, &note.content), "5");
    }

    #[test]
    fn with_type_leader_it_is_a_leader_to_the_measure() {
        let mut doc = Document::default();
        let square = doc.add(
            Curve::polyline(
                vec![[0.0; 3], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]],
                true,
            )
            .unwrap(),
        );
        doc.select(&[square]);
        run(&mut doc, &["DimArea", "Type", "1,1", "5,5", "8,5", ""]);
        let Annotation::Leader(leader) = last(&doc) else {
            panic!("not a leader");
        };
        assert_eq!(leader.points.len(), 3);
        assert_eq!(leader.content, format!("%<Area(\"{square}\")>%"));
        assert_eq!(shown(&doc, &leader.content), "4");
    }

    #[test]
    fn the_note_and_the_leader_preview_out_to_the_pointer() {
        use crate::{Answer, Command, Live};
        for mut command in [
            Box::<super::DimCurveLength>::default() as Box<dyn Command>,
            Box::<super::DimArea>::default(),
        ] {
            let mut doc = Document::default();
            let square = doc.add(
                Curve::polyline(
                    vec![[0.0; 3], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]],
                    true,
                )
                .unwrap(),
            );
            let live = Live {
                cursor: Some([10.0, 5.0, 0.0]),
                ..Live::default()
            };
            command.step(&mut doc, None);
            assert!(
                command.preview(&doc, &live).is_empty(),
                "nothing chosen yet"
            );
            command.step(&mut doc, Some(Answer::Objects(vec![square])));
            // The note's letters.
            assert!(!command.preview(&doc, &live).is_empty());
            command.step(&mut doc, Some(Answer::Option("Type")));
            command.step(&mut doc, Some(Answer::Point([1.0, 1.0, 0.0])));
            // The leader's segment and arrowhead.
            assert!(command.preview(&doc, &live).len() >= 2);
        }
    }

    #[test]
    fn a_volume_wants_a_closed_solid() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let block = doc.add_body(Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap());
        doc.select(&[line]);
        let processor = run(&mut doc, &["DimVolume"]);
        // A line has no volume, so it is not taken: the command asks for an object.
        assert_eq!(doc.state().objects.len(), 2);
        drop(processor);
        doc.select(&[block]);
        run(&mut doc, &["DimVolume", "0,0"]);
        let Annotation::Text(note) = last(&doc) else {
            panic!("not a note");
        };
        assert_eq!(shown(&doc, &note.content), "24");
    }
}
