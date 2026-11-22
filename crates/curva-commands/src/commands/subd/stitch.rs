//! Rhino's `Stitch` for SubDs: a first point or run of edges joined onto a second, point to point.
//! Rhino 8 asks "Select first vertex or set of edges to stitch together ( EdgeLoop )", then the
//! second set, then where the joined points go: `Average` (Enter), `First` or `Second` (Rhino's
//! help, 22 November 2026). Curva asks it after the second set, as Rhino does. Two runs of
//! edges must have as many points; the second is turned round when its ends lie nearer that way.

use super::parts::{by_object, run};
use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_mesh::{Point, StitchAt};

const LOCATION: &str = "Location";
const PLACES: [&str; 3] = ["Average", "First", "Second"];

/// The recipe.
#[derive(Default)]
pub struct Stitch;

fn gap(a: Point, b: Point) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
}

fn place(cx: &Context<'_>) -> StitchAt {
    match cx.name(LOCATION).as_str() {
        "First" => StitchAt::First,
        "Second" => StitchAt::Second,
        _ => StitchAt::Average,
    }
}

/// Each point of `first` paired with the point of `second` it joins, `second` turned round when
/// that brings their ends nearer.
fn pairs(first: &[u32], second: &[u32], at: &[Point]) -> Option<Vec<[u32; 2]>> {
    if first.len() != second.len() {
        return None;
    }
    let mut second = second.to_vec();
    let (f0, f1) = (first[0], first[first.len() - 1]);
    let (s0, s1) = (second[0], second[second.len() - 1]);
    let straight = gap(at[f0 as usize], at[s0 as usize]) + gap(at[f1 as usize], at[s1 as usize]);
    let turned = gap(at[f0 as usize], at[s1 as usize]) + gap(at[f1 as usize], at[s0 as usize]);
    if turned < straight {
        second.reverse();
    }
    let mut joined: Vec<[u32; 2]> = first
        .iter()
        .zip(&second)
        .filter(|(a, b)| a != b)
        .map(|(&a, &b)| [a, b])
        .collect();
    joined.dedup();
    Some(joined)
}

impl Recipe for Stitch {
    const NAME: &'static str = "stitch";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::name(LOCATION, "Average")];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == LOCATION).then(|| PLACES.map(str::to_owned).to_vec())
    }

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::mesh_parts("prompt.stitch.first")];
        if !cx.values.is_empty() {
            steps.push(StepDef::mesh_parts("prompt.stitch.second").offering(&[]));
        }
        if cx.values.len() >= 2 {
            // Where the joined points go: Average (Enter), First or Second, as Rhino asks.
            steps.push(StepDef::choice("prompt.stitch.location", LOCATION, &PLACES));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (first, second) = (by_object(cx, 0), by_object(cx, 1));
        let bad = || Message::new("error.stitch_sets");
        let ([(id, a)], [(other, b)]) = (
            &first.into_iter().collect::<Vec<_>>()[..],
            &second.into_iter().collect::<Vec<_>>()[..],
        ) else {
            return Err(bad());
        };
        let subd_of = |id: u64| {
            cx.doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.shape.subd().cloned())
        };
        let failed =
            |e: curva_mesh::MeshError| Message::with("error.geometry", vec![e.to_string()]);
        let (a, mut b) = (run(a).ok_or_else(bad)?, run(b).ok_or_else(bad)?);
        // Sets on two SubDs join them into one first, as Rhino combines them.
        let subd = if id == other {
            subd_of(*id).ok_or_else(bad)?
        } else {
            let (one, two) = (
                subd_of(*id).ok_or_else(bad)?,
                subd_of(*other).ok_or_else(bad)?,
            );
            let start = u32::try_from(one.cage().vertex_count()).unwrap_or(u32::MAX);
            b = b.iter().map(|&v| v + start).collect();
            one.joined(&two).map_err(failed)?
        };
        let joined = pairs(&a, &b, subd.cage().vertices()).ok_or_else(bad)?;
        let stitched = subd.stitched(&joined, place(cx)).map_err(failed)?;
        let mut made = Made {
            replace: vec![(*id, Shape::SubD(stitched))],
            ..Made::default()
        };
        if id != other {
            made.remove.push(*other);
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Frame, SubD};

    fn subd(doc: &Document, id: u64) -> SubD {
        let object = doc.state().objects.iter().find(|o| o.id == id).unwrap();
        object.shape.subd().unwrap().clone()
    }

    fn at(subd: &SubD, p: [f64; 3]) -> u32 {
        let k = subd.cage().vertices().iter().position(|&q| q == p).unwrap();
        u32::try_from(k).unwrap()
    }

    #[test]
    fn points_on_two_subds_join_them_into_one_as_in_rhino() {
        // Rhino 8 combines different SubDs into one when it stitches across them (its help).
        let mut doc = Document::default();
        let plane = |x: f64| {
            let frame = Frame {
                origin: [x, 0.0, 0.0],
                ..Frame::world()
            };
            SubD::subd_plane(&frame, [2.0, 2.0], [1, 1])
        };
        let (left, right) = (plane(0.0), plane(3.0));
        let a = at(&left, [2.0, 0.0, 0.0]);
        let b = at(&right, [3.0, 0.0, 0.0]);
        let left_id = doc.add_shape(Shape::SubD(left));
        let right_id = doc.add_shape(Shape::SubD(right));
        let mut processor = Processor::default();
        doc.select_mesh_parts(&[(left_id, MeshPart::Point(a))]);
        processor.submit(&mut doc, "stitch");
        processor.submit(&mut doc, "");
        doc.select_mesh_parts(&[(right_id, MeshPart::Point(b))]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "");
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 1, "the second SubD joins the first");
        let one = objects[0].shape.subd().unwrap();
        assert_eq!(one.cage().vertex_count(), 7);
        assert_eq!(one.cage().face_count(), 2);
        assert!(one.cage().vertices().contains(&[2.5, 0.0, 0.0]));
    }

    #[test]
    fn two_points_meet_halfway_as_rhinos_average() {
        // A flat 4 m SubD plane of 2 × 2 faces: two of its corners stitched meet halfway.
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::SubD(SubD::subd_plane(
            &Frame::world(),
            [4.0, 4.0],
            [2, 2],
        )));
        let before = subd(&doc, id);
        let (a, b) = (at(&before, [0.0, 0.0, 0.0]), at(&before, [4.0, 0.0, 0.0]));
        let mut processor = Processor::default();
        doc.select_mesh_parts(&[(id, MeshPart::Point(a))]);
        processor.submit(&mut doc, "stitch");
        processor.submit(&mut doc, "");
        doc.select_mesh_parts(&[(id, MeshPart::Point(b))]);
        processor.submit(&mut doc, "");
        processor.submit(&mut doc, "");
        let after = subd(&doc, id);
        assert_eq!(
            after.cage().vertex_count(),
            before.cage().vertex_count() - 1
        );
        let points = after.cage().vertices();
        assert!(
            !points.contains(&[4.0, 0.0, 0.0]),
            "the second point is gone"
        );
        let halfway = points.iter().filter(|&&p| p == [2.0, 0.0, 0.0]).count();
        assert_eq!(
            halfway, 2,
            "the first point moved halfway, beside the edge's middle point"
        );
    }
}
