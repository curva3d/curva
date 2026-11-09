//! `Explode`: breaks joined curves and polylines back into separate curves.

use super::ask_objects;
use crate::{Answer, Command, Message, Step};
use curva_document::Document;
use curva_kernel::Curve;

/// The running command.
#[derive(Default)]
pub struct Explode;

impl Command for Explode {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let chosen = match answer {
            Some(Answer::Objects(list)) => list,
            _ => doc.selection(),
        };
        if chosen.is_empty() {
            return ask_objects("prompt.explode.objects");
        }
        let broken: Vec<(u64, Vec<Curve>)> = doc
            .state()
            .objects
            .iter()
            .filter(|object| chosen.contains(&object.id))
            .filter_map(|object| Some((object.id, object.curve.exploded()?)))
            .collect();
        if broken.is_empty() {
            return Step::Failed(Message::new("error.nothing_to_explode"));
        }
        let mut made = 0;
        for (id, parts) in broken {
            doc.remove(&[id]);
            made += parts.len();
            for part in parts {
                doc.add(part);
            }
        }
        Step::Done(Some(Message::with("done.explode", vec![made.to_string()])))
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn selected(curve: Curve) -> Document {
        let mut doc = Document::default();
        let id = doc.add(curve);
        doc.select(&[id]);
        doc
    }

    #[test]
    fn a_polyline_becomes_separate_lines() {
        let points = vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
        let mut doc = selected(Curve::polyline(points, false).unwrap());
        run(&mut doc, &["explode"]);
        assert_eq!(doc.state().objects.len(), 2);
    }

    #[test]
    fn it_is_one_undo_step() {
        let points = vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
        let mut doc = selected(Curve::polyline(points, false).unwrap());
        run(&mut doc, &["explode", "undo"]);
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn a_single_curve_stays_as_it_is() {
        let mut doc = selected(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let processor = run(&mut doc, &["explode"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(!processor.is_running());
    }
}
