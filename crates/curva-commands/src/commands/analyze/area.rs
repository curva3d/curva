//! `Area`, as in Rhino: measures surfaces, polysurfaces and solids, meshes, and the area inside
//! closed flat curves.

use crate::commands::{ask_objects, bodies_of, curves_of, geometry_failed, objects_or_selection};
use crate::{Answer, Command, Message, Step};
use curva_document::Document;

/// The running command.
#[derive(Default)]
pub struct Area;

impl Command for Area {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let Some(chosen) = objects_or_selection(doc, answer) else {
            return ask_objects("prompt.area.objects");
        };
        let mut total = 0.0;
        for (_, curve) in curves_of(doc, &chosen) {
            match curve.area() {
                Ok(area) => total += area,
                Err(error) => return geometry_failed(&error),
            }
        }
        for (_, body) in bodies_of(doc, &chosen) {
            match body.area() {
                Ok(area) => total += area,
                Err(error) => return geometry_failed(&error),
            }
        }
        for object in doc
            .state()
            .objects
            .iter()
            .filter(|o| chosen.contains(&o.id))
        {
            if let Some(mesh) = object.shape.mesh() {
                total += mesh.area();
            }
        }
        Step::Done(Some(Message::with(
            "done.area",
            vec![curva_document::units::format_area(total, doc.state().unit)],
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn it_adds_up_closed_curves() {
        let mut doc = Document::default();
        let a = doc.add(Curve::circle([0.0; 3], 1.0).unwrap());
        let b = doc.add(
            Curve::polyline(
                vec![[5.0, 0.0, 0.0], [7.0, 0.0, 0.0], [7.0, 2.0, 0.0]],
                true,
            )
            .unwrap(),
        );
        doc.select(&[a, b]);
        let processor = run(&mut doc, &["area"]);
        assert!(
            processor.log()[0].contains("5.142"),
            "{:?}",
            processor.log()
        );
    }

    #[test]
    fn it_measures_surfaces_and_solids_as_rhino_does() {
        for (steps, area) in [
            (vec!["box", "0,0,0", "10,5,0", "3"], "190"),
            (vec!["plane", "0,0,0", "10,5,0"], "50"),
        ] {
            let mut doc = Document::default();
            run(&mut doc, &steps);
            let id = doc.state().objects[0].id;
            doc.select(&[id]);
            let processor = run(&mut doc, &["area"]);
            assert!(processor.log()[0].contains(area), "{:?}", processor.log());
        }
    }

    #[test]
    fn an_open_curve_is_reported() {
        let mut doc = Document::default();
        let id = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        let processor = run(&mut doc, &["area"]);
        assert!(processor.log()[0].contains("not closed"));
    }
}
