//! Rhino's `Bevel` for SubD edges. Measured in Rhino 8 (23 November 2026, a RhinoCommon pick and
//! `-Bevel`): it asks "Select Mesh or SubD edges to bevel ( EdgeLoop )", then "Bevel positioning.
//! Select point or give amount ( Segments=3  OffsetMode=Absolute  Straightness=0  KeepCreases=Yes
//! )"; 0.5 on the middle edge of a 4 m SubD box's front gives a strip of 3 quads, two five-sided
//! caps and Rhino's rows of points (29 faces on 32 points). A clicked point gives the distance
//! from the edge. Edges whose ends are not inner points where four edges meet, edges that share
//! an end, `KeepCreases`, meshes and EdgeLoop are not here yet.

use super::parts::{by_object, edges};
use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_mesh::{BevelAmount, Point};

const SEGMENTS: &str = "Segments";
const OFFSET_MODE: &str = "OffsetMode";
const STRAIGHTNESS: &str = "Straightness";
const MODES: [&str; 2] = ["Absolute", "Proportional"];

/// The recipe.
#[derive(Default)]
pub struct Bevel;

/// How far `p` is from the segment `a`–`b`.
fn off_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab: Point = std::array::from_fn(|i| b[i] - a[i]);
    let ap: Point = std::array::from_fn(|i| p[i] - a[i]);
    let dot = |u: Point, v: Point| (0..3).map(|i| u[i] * v[i]).sum::<f64>();
    let t = (dot(ap, ab) / dot(ab, ab).max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
    let foot: Point = std::array::from_fn(|i| a[i] + t * ab[i]);
    let d: Point = std::array::from_fn(|i| p[i] - foot[i]);
    dot(d, d).sqrt()
}

impl Recipe for Bevel {
    const NAME: &'static str = "bevel";
    // Rhino 8's options and their starting values, measured.
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole(SEGMENTS, 3, 1, 100),
        OptionDef::name(OFFSET_MODE, "Absolute"),
        OptionDef::number(STRAIGHTNESS, 0.0),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == OFFSET_MODE).then(|| MODES.map(str::to_owned).to_vec())
    }

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::mesh_parts("prompt.bevel.edges").offering(&[])];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::point_or_number("prompt.bevel.position").offering(&[
                    SEGMENTS,
                    OFFSET_MODE,
                    STRAIGHTNESS,
                ]),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(answer) = cx.values.get(1) else {
            return Ok(Made::default());
        };
        let failed =
            |e: curva_mesh::MeshError| Message::with("error.geometry", vec![e.to_string()]);
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
            let picked = edges(&parts);
            // Edges sharing an end would need the ends joined, which is not here yet.
            let ends: Vec<u32> = picked.iter().flatten().copied().collect();
            let mut unique = ends.clone();
            unique.sort_unstable();
            unique.dedup();
            if unique.len() != ends.len() {
                return Err(Message::new("error.bevel_apart"));
            }
            // Bevelling one edge renumbers points, so each later edge is found again by place.
            let places: Vec<[Point; 2]> = picked
                .iter()
                .map(|e| e.map(|v| subd.cage().vertices()[v as usize]))
                .collect();
            for [pa, pb] in places {
                let at = subd.cage().vertices();
                let find = |p: Point| at.iter().position(|&q| q == p).map(|k| k as u32);
                let (Some(a), Some(b)) = (find(pa), find(pb)) else {
                    continue;
                };
                let size = match answer {
                    Value::Number(n) => *n,
                    other => match other.point() {
                        Some(p) => off_segment(p, pa, pb),
                        None => continue,
                    },
                };
                let amount = if cx.name(OFFSET_MODE) == "Proportional" {
                    BevelAmount::Proportional(size)
                } else {
                    BevelAmount::Absolute(size)
                };
                let segments = cx.number(SEGMENTS) as u32;
                subd = subd
                    .bevelled([a, b], amount, segments, cx.number(STRAIGHTNESS))
                    .map_err(failed)?;
            }
            made.replace.push((id, Shape::SubD(subd)));
        }
        if made.replace.is_empty() {
            return Err(Message::new("error.bevel_none"));
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Frame, SubD};

    fn front_edge(doc: &mut Document) -> (u64, MeshPart) {
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let subd = SubD::subd_box(&frame, [4.0; 3], [2, 2, 2]);
        let at = |p: [f64; 3]| {
            let k = subd.cage().vertices().iter().position(|&q| q == p).unwrap();
            u32::try_from(k).unwrap()
        };
        let edge = MeshPart::edge(at([202.0, 0.0, 0.0]), at([202.0, 0.0, 2.0]));
        (doc.add_shape(Shape::SubD(subd)), edge)
    }

    #[test]
    fn a_front_edge_bevels_as_in_rhino() {
        let mut doc = Document::default();
        let (id, edge) = front_edge(&mut doc);
        doc.select_mesh_parts(&[(id, edge)]);
        run(&mut doc, &["bevel", "", "0.5"]);
        let out = doc.state().objects[0].shape.subd().unwrap();
        assert_eq!(
            (out.cage().face_count(), out.cage().vertex_count()),
            (29, 32)
        );
    }

    #[test]
    fn the_bevel_shows_while_the_amount_is_typed() {
        let mut doc = Document::default();
        let (id, edge) = front_edge(&mut doc);
        doc.select_mesh_parts(&[(id, edge)]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "bevel");
        processor.submit(&mut doc, "");
        assert!(!processor.preview(&doc, None, "0.5").is_empty());
    }
}
