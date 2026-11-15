//! `Leader`, as in Rhino 8 (spec `annotation.md` §4; `rhino-reference.md`): the arrow's tip,
//! then each point the line passes through, Enter to end, then the note. The leader previews
//! with its arrowhead while its points are placed.

use crate::commands::unexpected;
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Leader as Made};
use curva_kernel::{Curve, Point};

const LANDING: &str = "Landing";

/// The running command.
#[derive(Default)]
pub struct Leader {
    points: Vec<Point>,
    /// Whether the points are all placed and the note is asked for.
    wording: bool,
    landing: bool,
}

impl Leader {
    fn leader(&self, doc: &Document, points: Vec<Point>, content: String) -> Made {
        let [x, y] = doc.work_plane();
        Made {
            points,
            content,
            x,
            y,
            landing: self.landing,
            style: doc.current_annotation_style(),
        }
    }

    fn ask(&self) -> Step {
        let labels = vec![format!(
            "{LANDING}={}",
            if self.landing { "Yes" } else { "No" }
        )];
        let request = if self.wording {
            Request::new("prompt.leader.words", Kind::Path)
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
            options: vec![LANDING],
            option_labels: labels,
            ..request
        })
    }
}

impl Command for Leader {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Option(LANDING)) => {
                self.landing = !self.landing;
                self.ask()
            }
            Some(Answer::Point(p)) if !self.wording => {
                self.points.push(p);
                self.ask()
            }
            Some(Answer::Empty) if !self.wording && self.points.len() >= 2 => {
                self.wording = true;
                self.ask()
            }
            Some(Answer::Path(words)) if self.wording => {
                // As in Rhino's -Text, \n starts a new line.
                let words = words.replace("\\n", "\n");
                if words.trim().is_empty() {
                    return self.ask();
                }
                let leader = self.leader(doc, self.points.clone(), words);
                doc.add_annotation(Annotation::Leader(leader));
                Step::Done(Some(Message::new("done.leader")))
            }
            Some(_) => unexpected(),
        }
    }

    /// The line so far with its arrowhead, out to the cursor.
    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let mut points = self.points.clone();
        if !self.wording {
            points.extend(live.cursor);
        }
        if points.len() < 2 {
            return Vec::new();
        }
        let leader = Annotation::Leader(self.leader(doc, points, String::new()));
        curva_document::shape::annotation_drawing(&leader, doc.state(), doc.space()).lines
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use crate::{Command, Live};
    use curva_document::annotation::Annotation;
    use curva_document::{Document, Shape};

    #[test]
    fn a_leader_runs_from_its_arrow_to_its_note() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["Leader", "0,0", "10,10", "20,10", "", "Beam\\nsteel"],
        );
        let Shape::Annotation(Annotation::Leader(leader)) = &doc.state().objects[0].shape else {
            panic!("not a leader");
        };
        assert_eq!(leader.points.len(), 3);
        assert_eq!(leader.content, "Beam\nsteel");
        assert!(!leader.landing);
        let drawing = curva_document::shape::annotation_drawing(
            &Annotation::Leader(leader.clone()),
            doc.state(),
            None,
        );
        // Two segments, the arrowhead and the note's letters.
        assert!(drawing.lines.len() > 3);
        assert!(drawing.fills.len() > 5);
        // The note is to the right of the last point, reading on from the line.
        let (low, _) = doc.state().objects[0].shape.bounds().unwrap();
        assert!(low[0] <= 0.0);
    }

    #[test]
    fn a_leader_needs_two_points_before_enter_ends_it() {
        let mut doc = Document::default();
        run(
            &mut doc,
            &["Leader", "Landing", "0,0", "", "10,0", "", "Note"],
        );
        let Shape::Annotation(Annotation::Leader(leader)) = &doc.state().objects[0].shape else {
            panic!("not a leader");
        };
        assert_eq!(leader.points.len(), 2);
        assert!(leader.landing);
    }

    #[test]
    fn a_leader_previews_out_to_the_cursor() {
        let mut doc = Document::default();
        let mut command = super::Leader::default();
        command.step(&mut doc, None);
        command.step(&mut doc, Some(crate::Answer::Point([0.0; 3])));
        let live = Live {
            cursor: Some([10.0, 5.0, 0.0]),
            ..Live::default()
        };
        // The segment and the arrowhead's outline.
        assert!(command.preview(&doc, &live).len() >= 2);
    }
}
