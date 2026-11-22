//! Rhino's `InsertEdge` for SubDs, `Type=Loop` and `Mode=Full`: edges inserted beside a run of
//! cage edges. Measured through the screen (22 November 2026): Rhino asks "Select edge from loop
//! ( Type=Loop  Mode=Full )" until Enter, then "Edge positioning. Select point or give amount
//! ( BothSides=No  OffsetMode=Absolute )"; a click picks one edge, which runs on through points
//! where four edges meet; the new run goes beside it at the amount along each crossing edge. A
//! clicked point picks the side nearer to it and the distance from the edge; a typed amount goes
//! on the side Rhino put it in the measured case. Rhino's `Type=Ring` and `Mode=Range` are not
//! here yet.

use super::parts::{by_object, edges};
use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_mesh::{InsertAmount, InsertSide, Point, SubD};

const BOTH_SIDES: &str = "BothSides";
const OFFSET_MODE: &str = "OffsetMode";
const MODES: [&str; 2] = ["Absolute", "Proportional"];

/// The side a typed amount goes on: the one that puts the run on the front of Rhino's box, as
/// Rhino did (see `curva_mesh::insert_edge`'s tests).
const TYPED_SIDE: InsertSide = InsertSide::Along;

/// The recipe.
#[derive(Default)]
pub struct InsertEdge;

fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

/// How far `p` is from the line through `a` and `b`.
fn off_line(p: Point, a: Point, b: Point) -> f64 {
    let (ab, ap) = (sub(b, a), sub(p, a));
    let t = dot(ap, ab) / dot(ab, ab).max(f64::MIN_POSITIVE);
    let foot: Point = std::array::from_fn(|i| a[i] + t * ab[i]);
    dot(sub(p, foot), sub(p, foot)).sqrt()
}

/// The side of edge `[a, b]` a clicked point is on: the one whose face's middle is nearer.
fn side_of(subd: &SubD, [a, b]: [u32; 2], p: Point) -> InsertSide {
    let at = subd.cage().vertices();
    let middle = |face: &[u32]| -> Point {
        let n = face.len().max(1) as f64;
        std::array::from_fn(|i| face.iter().map(|&v| at[v as usize][i]).sum::<f64>() / n)
    };
    let mut along = None;
    let mut against = None;
    for face in subd.cage().faces() {
        let n = face.len();
        for k in 0..n {
            let (p0, p1) = (face[k], face[(k + 1) % n]);
            if (p0, p1) == (a, b) {
                along = Some(middle(face));
            } else if (p0, p1) == (b, a) {
                against = Some(middle(face));
            }
        }
    }
    let far = |m: Option<Point>| m.map_or(f64::MAX, |m| dot(sub(p, m), sub(p, m)));
    if far(along) <= far(against) {
        InsertSide::Along
    } else {
        InsertSide::Against
    }
}

impl Recipe for InsertEdge {
    const NAME: &'static str = "insertedge";
    // Rhino 8's options and their starting values, measured.
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(BOTH_SIDES, false),
        OptionDef::name(OFFSET_MODE, "Absolute"),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == OFFSET_MODE).then(|| MODES.map(str::to_owned).to_vec())
    }

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::mesh_parts("prompt.insertedge.edges").offering(&[])];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::point_or_number("prompt.insertedge.position")
                    .offering(&[BOTH_SIDES, OFFSET_MODE]),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(answer) = cx.values.get(1) else {
            return Ok(Made::default());
        };
        let proportional = cx.name(OFFSET_MODE) == "Proportional";
        let mut made = Made::default();
        for (id, parts) in by_object(cx, 0) {
            let Some(mut subd) = cx
                .doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.shape.subd().cloned())
            else {
                continue;
            };
            for edge in edges(&parts) {
                let (size, side) = match answer {
                    Value::Number(n) => (*n, TYPED_SIDE),
                    other => {
                        let Some(p) = other.point() else { continue };
                        let at = subd.cage().vertices();
                        let line = (at[edge[0] as usize], at[edge[1] as usize]);
                        (off_line(p, line.0, line.1), side_of(&subd, edge, p))
                    }
                };
                let side = if cx.toggle(BOTH_SIDES) {
                    InsertSide::Both
                } else {
                    side
                };
                let amount = if proportional {
                    InsertAmount::Proportional(size)
                } else {
                    InsertAmount::Absolute(size)
                };
                subd = subd
                    .with_edges_inserted(edge, amount, side)
                    .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            }
            made.replace.push((id, Shape::SubD(subd)));
        }
        if made.replace.is_empty() {
            return Err(Message::new("error.insertedge_none"));
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Frame, SubD};

    #[test]
    fn a_typed_amount_inserts_as_rhino_did() {
        // Rhino 8: the corner edge of a 4 m SubD box of 2 × 2 a side and 0.5 give a run at
        // x = 203.5 on the front: 26 faces on 29 points.
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let mut doc = Document::default();
        let subd = SubD::subd_box(&frame, [4.0; 3], [2, 2, 2]);
        let at = |p: [f64; 3]| {
            u32::try_from(subd.cage().vertices().iter().position(|&q| q == p).unwrap()).unwrap()
        };
        let edge = MeshPart::edge(at([204.0, 0.0, 0.0]), at([204.0, 0.0, 2.0]));
        let id = doc.add_shape(Shape::SubD(subd));
        doc.select_mesh_parts(&[(id, edge)]);
        run(&mut doc, &["insertedge", "", "0.5"]);
        let out = doc.state().objects[0].shape.subd().unwrap().clone();
        assert_eq!(
            (out.cage().face_count(), out.cage().vertex_count()),
            (26, 29)
        );
        assert!(out.cage().vertices().contains(&[203.5, 0.0, 2.0]));
    }

    #[test]
    fn the_new_edges_show_while_the_amount_is_typed() {
        let mut doc = Document::default();
        let subd = SubD::subd_box(&Frame::world(), [4.0; 3], [2, 2, 2]);
        let face = subd.cage().faces().next().unwrap().to_vec();
        let (a, b) = (face[0], face[1]);
        let id = doc.add_shape(Shape::SubD(subd));
        doc.select_mesh_parts(&[(id, MeshPart::edge(a, b))]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "insertedge");
        processor.submit(&mut doc, "");
        assert!(!processor.preview(&doc, None, "0.5").is_empty());
    }
}
