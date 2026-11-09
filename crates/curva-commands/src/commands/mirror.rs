//! `Mirror`: adds a mirror image of objects across a line, keeping the originals as Rhino does.

use super::{ask_objects, ask_point, plane_normal, unexpected};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;
use curva_kernel::{Point, Similarity, cross, sub, unit};

/// The running command.
#[derive(Default)]
pub struct Mirror {
    ids: Option<Vec<u64>>,
    start: Option<Point>,
    axis: Point,
}

impl Command for Mirror {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(ids) = self.ids.clone() else {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            if chosen.is_empty() {
                return ask_objects("prompt.mirror.objects");
            }
            self.ids = Some(chosen);
            return ask_point("prompt.mirror.start", Vec::new());
        };
        match (self.start, answer) {
            (None, Some(Answer::Point(start))) => {
                self.start = Some(start);
                self.axis = plane_normal(doc);
                ask_point("prompt.mirror.end", vec![start])
            }
            (Some(start), Some(Answer::Point(end))) => {
                let Some(normal) = unit(cross(sub(end, start), self.axis)) else {
                    return Step::Failed(Message::new("error.reference_point"));
                };
                let made = doc.copy_transformed(&ids, &Similarity::mirror(start, normal));
                Step::Done(Some(Message::with(
                    "done.mirror",
                    vec![made.len().to_string()],
                )))
            }
            _ => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Curve, Point};

    fn selected_line() -> Document {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([1.0, 2.0, 0.0], [3.0, 2.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn ends(doc: &Document, index: usize) -> Vec<Point> {
        doc.state().objects[index].curve.sample().unwrap()
    }

    fn close(a: Point, b: Point) -> bool {
        (0..3).all(|axis| (a[axis] - b[axis]).abs() < 1e-9)
    }

    #[test]
    fn it_adds_a_mirror_image_and_keeps_the_original() {
        let mut doc = selected_line();
        run(&mut doc, &["mirror", "0,0", "0,10"]);
        assert_eq!(doc.state().objects.len(), 2);
        assert_eq!(ends(&doc, 0)[0], [1.0, 2.0, 0.0]);
        assert!(close(ends(&doc, 1)[0], [-1.0, 2.0, 0.0]));
        assert!(close(ends(&doc, 1)[1], [-3.0, 2.0, 0.0]));
    }

    #[test]
    fn it_mirrors_across_a_slanted_line() {
        let mut doc = selected_line();
        run(&mut doc, &["mirror", "0,0", "5,5"]);
        assert!(close(ends(&doc, 1)[0], [2.0, 1.0, 0.0]));
    }

    #[test]
    fn it_uses_the_plane_being_drawn_on() {
        let mut doc = selected_line();
        doc.set_work_plane([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        run(&mut doc, &["mirror", "0,0,0", "1,0,0"]);
        assert!(close(ends(&doc, 1)[0], [1.0, 2.0, 0.0]));
    }

    #[test]
    fn a_line_with_no_length_makes_nothing() {
        let mut doc = selected_line();
        run(&mut doc, &["mirror", "1,1", "1,1"]);
        assert_eq!(doc.state().objects.len(), 1);
    }
}
