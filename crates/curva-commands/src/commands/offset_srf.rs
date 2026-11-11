//! `OffsetSrf`: a copy of surfaces or solids at a distance, as in Rhino. Option `Solid`
//! thickens an open surface into a solid between where it was and the copy.

use super::{ask_objects, bodies_of, geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, Shape};
use curva_kernel::{Body, Curve};
use std::cell::RefCell;

/// The distance used until another is typed, in millimetres.
const STARTING_DISTANCE: f64 = 1.0;
const DISTANCE: &str = "offsetsrf.distance";
const SOLID: &str = "offsetsrf.solid";
const SOLID_OPTION: &str = "Solid";

/// The running command.
#[derive(Default)]
pub struct OffsetSrf {
    ids: Option<Vec<u64>>,
    /// The last preview worked out, with the distance and setting it was for, as working out an
    /// offset takes a moment and the preview is asked for every frame.
    shown: RefCell<Option<(f64, bool, Vec<Curve>)>>,
}

fn distance_of(doc: &Document) -> f64 {
    doc.remembered(DISTANCE).unwrap_or(STARTING_DISTANCE)
}

fn solid_on(doc: &Document) -> bool {
    doc.remembered(SOLID).is_some_and(|value| value != 0.0)
}

impl OffsetSrf {
    fn ask(doc: &Document) -> Step {
        let solid = if solid_on(doc) { "Yes" } else { "No" };
        Step::Ask(Request {
            prompt: Message::with(
                "prompt.offsetsrf.distance",
                vec![distance_of(doc).to_string(), solid.to_owned()],
            ),
            options: vec![SOLID_OPTION],
            allow_empty: true,
            ..Request::new("prompt.offsetsrf.distance", Kind::Number)
        })
    }

    fn offsets(
        doc: &Document,
        ids: &[u64],
        distance: f64,
        solid: bool,
    ) -> Result<Vec<(u64, Body)>, curva_kernel::KernelError> {
        bodies_of(doc, ids)
            .into_iter()
            .map(|(id, body)| Ok((id, body.offset(distance, solid)?)))
            .collect()
    }

    fn make(&self, doc: &mut Document, distance: f64) -> Step {
        let Some(ids) = &self.ids else {
            return unexpected();
        };
        let made = match Self::offsets(doc, ids, distance, solid_on(doc)) {
            Ok(made) => made,
            Err(error) => return geometry_failed(&error),
        };
        let count = made.len();
        for (source, body) in made {
            let layer = doc.layer_of(source);
            doc.add_shape_on(Shape::Body(body), layer);
        }
        Step::Done(Some(Message::with(
            "done.offsetsrf",
            vec![count.to_string()],
        )))
    }
}

impl Command for OffsetSrf {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.ids.is_none() {
            let chosen = match answer {
                Some(Answer::Objects(list)) => list,
                _ => doc.selection(),
            };
            let bodies: Vec<u64> = bodies_of(doc, &chosen).iter().map(|(id, _)| *id).collect();
            if bodies.is_empty() {
                return ask_objects("prompt.offsetsrf.objects");
            }
            self.ids = Some(bodies);
            return Self::ask(doc);
        }
        match answer {
            Some(Answer::Option(_)) => {
                let flipped = !solid_on(doc);
                doc.remember(SOLID, f64::from(u8::from(flipped)));
                Self::ask(doc)
            }
            Some(Answer::Empty) => {
                let distance = distance_of(doc);
                self.make(doc, distance)
            }
            Some(Answer::Number(distance)) if distance.abs() > 1e-9 => {
                doc.remember(DISTANCE, distance);
                self.make(doc, distance)
            }
            Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_distance")),
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        let Some(ids) = &self.ids else {
            return Vec::new();
        };
        let distance = live
            .number
            .filter(|d| d.abs() > 1e-9)
            .unwrap_or_else(|| distance_of(doc));
        let solid = solid_on(doc);
        if let Some((held, held_solid, curves)) = &*self.shown.borrow()
            && *held == distance
            && *held_solid == solid
        {
            return curves.clone();
        }
        let curves: Vec<Curve> = Self::offsets(doc, ids, distance, solid)
            .map(|made| {
                made.iter()
                    .flat_map(|(_, body)| body.edge_curves().unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default();
        *self.shown.borrow_mut() = Some((distance, solid, curves.clone()));
        curves
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    fn selected_sheet(doc: &mut Document) {
        let square = Curve::polyline(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let id = doc.add_body(Body::planar(&[square]).unwrap());
        doc.select(&[id]);
    }

    #[test]
    fn a_surface_is_copied_at_the_distance() {
        let mut doc = Document::default();
        selected_sheet(&mut doc);
        let mut processor = run(&mut doc, &["offsetsrf"]);
        assert!(!processor.preview(&doc, None, "3").is_empty());
        processor.submit(&mut doc, "3");
        assert_eq!(doc.state().objects.len(), 2);
        let (low, _) = doc.state().objects[1]
            .shape
            .body()
            .unwrap()
            .bounds()
            .unwrap();
        assert!((low[2].abs() - 3.0).abs() < 1e-6);
        processor.submit(&mut doc, "undo");
        assert_eq!(doc.state().objects.len(), 1);
    }

    #[test]
    fn solid_thickens_the_surface() {
        let mut doc = Document::default();
        selected_sheet(&mut doc);
        run(&mut doc, &["offsetsrf", "Solid", "2"]);
        let volume = doc.state().objects[1]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 200.0).abs() < 1e-3);
    }
}
