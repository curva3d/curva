//! Rhino's `InsertPoint` for SubDs: points put on cage edges, each two in a row that share a face
//! joined by a new edge across it. Measured through the screen (22–23 November 2026): Rhino asks
//! "Select mesh or SubD", then "Select first point on mesh edge" and "Select next point on mesh
//! edge ( Undo )" until Enter; one point at the middle of a box's bottom edge splits it (24 faces
//! on 27 points), and two points on opposite edges of a face split that face in two. Each picked
//! point goes on the cage edge nearest it. Meshes are not taken here yet.

use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_mesh::{Point, SubD};

/// The recipe.
#[derive(Default)]
pub struct InsertPoint;

fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

/// The cage edge nearest `p`, and how far along it from its first point the foot of `p` falls.
fn nearest_edge(subd: &SubD, p: Point) -> Option<([u32; 2], f64)> {
    let at = subd.cage().vertices();
    let mut best: Option<(f64, [u32; 2], f64)> = None;
    for face in subd.cage().faces() {
        let n = face.len();
        for k in 0..n {
            let (a, b) = (face[k], face[(k + 1) % n]);
            let (pa, pb) = (at[a as usize], at[b as usize]);
            let ab = sub(pb, pa);
            let t = (dot(sub(p, pa), ab) / dot(ab, ab).max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
            let foot: Point = std::array::from_fn(|i| pa[i] + t * ab[i]);
            let gap = dot(sub(p, foot), sub(p, foot));
            if best.is_none_or(|(g, _, _)| gap < g) {
                best = Some((gap, [a, b], t));
            }
        }
    }
    best.map(|(_, edge, t)| (edge, t))
}

impl Recipe for InsertPoint {
    const NAME: &'static str = "insertpoint";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.insertpoint.object").one()];
        let picked = cx.values.len().saturating_sub(1);
        for k in 0..picked {
            steps.push(if k == 0 {
                StepDef::point("prompt.insertpoint.first")
            } else {
                StepDef::point("prompt.insertpoint.next").optional()
            });
        }
        if !cx.values.is_empty() && cx.values.last() != Some(&Value::Empty) {
            steps.push(if picked == 0 {
                StepDef::point("prompt.insertpoint.first")
            } else {
                StepDef::point("prompt.insertpoint.next").optional()
            });
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(&id) = objects(cx, 0).first() else {
            return Ok(Made::default());
        };
        let Some(subd) = cx
            .doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == id)
            .and_then(|o| o.shape.subd())
        else {
            return Err(Message::new("error.insertpoint_none"));
        };
        // Each point goes on the cage edge nearest it, all in one go so that each two in a row
        // sharing a face are joined across it.
        let places: Vec<([u32; 2], f64)> = cx.values[1..]
            .iter()
            .filter_map(Value::point)
            .filter_map(|p| nearest_edge(subd, p))
            .collect();
        let out = subd
            .with_points_inserted(&places)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        if out.cage().vertex_count() == subd.cage().vertex_count() {
            return Ok(Made::default());
        }
        Ok(Made {
            replace: vec![(id, Shape::SubD(out))],
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_mesh::{Frame, SubD};

    #[test]
    fn the_new_points_show_while_picking() {
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::SubD(SubD::subd_box(
            &Frame::world(),
            [4.0; 3],
            [2, 2, 2],
        )));
        doc.select(&[id]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "insertpoint");
        processor.submit(&mut doc, "1,0,0");
        assert!(!processor.preview(&doc, None, "3,0,0").is_empty());
    }

    #[test]
    fn points_on_edges_split_them_and_the_face_between_as_in_rhino() {
        // Rhino 8 on the measured box: (201, 0, 0) in one run, then (203, 0, 0) and (203, 2, 0) in
        // another, give 24 faces on 27 points and then 25 faces on 29.
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::SubD(SubD::subd_box(&frame, [4.0; 3], [2, 2, 2])));
        doc.select(&[id]);
        run(&mut doc, &["insertpoint", "201,0,0", ""]);
        let one = doc.state().objects[0].shape.subd().unwrap().clone();
        assert_eq!(
            (one.cage().face_count(), one.cage().vertex_count()),
            (24, 27)
        );
        doc.select(&[id]);
        run(&mut doc, &["insertpoint", "203,0,0", "203,2,0", ""]);
        let two = doc.state().objects[0].shape.subd().unwrap().clone();
        assert_eq!(
            (two.cage().face_count(), two.cage().vertex_count()),
            (25, 29)
        );
    }
}
