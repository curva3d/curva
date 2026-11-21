//! Rhino's `Stitch` for SubDs: a first point or run of edges joined onto a second, point to point.
//! Rhino 8 asks "Select first vertex or set of edges to stitch together ( EdgeLoop )", then the
//! second set, then where the joined points go: `Average` (Enter), `First` or `Second` (Rhino's
//! help, 22 November 2026). Curva offers the place while the second set is picked. Two runs of
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
            steps.push(StepDef::mesh_parts("prompt.stitch.second").offering(&[LOCATION]));
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
        if id != other {
            return Err(Message::new("error.stitch_one_subd"));
        }
        let subd = cx
            .doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == *id)
            .and_then(|o| o.shape.subd())
            .ok_or_else(bad)?;
        let (a, b) = (run(a).ok_or_else(bad)?, run(b).ok_or_else(bad)?);
        let joined = pairs(&a, &b, subd.cage().vertices()).ok_or_else(bad)?;
        let stitched = subd
            .stitched(&joined, place(cx))
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
        Ok(Made {
            replace: vec![(*id, Shape::SubD(stitched))],
            ..Made::default()
        })
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
