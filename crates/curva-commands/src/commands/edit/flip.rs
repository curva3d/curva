//! `Flip`: reverses the direction of curves and turns surfaces over, as Rhino 8 does (#768).

use crate::commands::{ask_objects, bodies_of, curves_of, objects_or_selection};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Flip;

impl Command for Flip {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(chosen) = objects_or_selection(doc, answer) else {
            return ask_objects("prompt.flip.objects");
        };
        let mut flipped = 0;
        for (id, curve) in curves_of(doc, &chosen) {
            doc.replace(id, curve.reversed());
            flipped += 1;
        }
        // A surface turns over; a closed solid can't, and is left as it is.
        for (id, body) in bodies_of(doc, &chosen) {
            if let Ok(turned) = body.turned_over() {
                doc.replace(id, curva_document::Shape::Body(turned));
                flipped += 1;
            }
        }
        Step::Done(Some(Message::with("done.flip", vec![flipped.to_string()])))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn a_curve_runs_the_other_way_and_is_the_same_object() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["flip"]);
        assert_eq!(doc.state().objects[0].id, id);
        assert_eq!(
            doc.state().objects[0].curve().unwrap().sample().unwrap()[0],
            [1.0, 0.0, 0.0]
        );
    }

    #[test]
    fn a_surface_is_turned_over_as_in_rhino() {
        let mut doc = Document::default();
        let plane = curva_kernel::Body::ruled(
            &Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap(),
            &Curve::line([0.0, 10.0, 0.0], [10.0, 10.0, 0.0]).unwrap(),
            false,
        )
        .unwrap();
        let up = plane.face_normal_near(0, [5.0, 5.0, 0.0]).unwrap()[2];
        let id = doc.add_body(plane);
        doc.select(&[id]);
        run(&mut doc, &["flip"]);
        // Rhino 8: the face's normal turns the other way.
        let body = doc.state().objects[0].shape.body().unwrap();
        let now = body.face_normal_near(0, [5.0, 5.0, 0.0]).unwrap()[2];
        assert!(up * now < 0.0, "{up} {now}");
    }

    #[test]
    fn it_can_be_undone() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["flip", "undo"]);
        assert_eq!(
            doc.state().objects[0].curve().unwrap().sample().unwrap()[0],
            [0.0; 3]
        );
    }
}
