//! Rhino's `Bridge` for SubDs: two runs of open border edges joined by rows of quads. Measured in
//! Rhino 8 (the screen, 22 November 2026): it asks "Select first set of edges or faces to bridge
//! ( EdgeLoop )", the second set, then "Bridge options ( Segments=1  Join=Yes  Crease=No
//! Straightness=1 )". With those, two open 4 × 2 m strips 4 m apart, top edges bridged, become
//! one SubD (the second object goes) of 6 quads on their 12 points, the bridge joining smoothly
//! and the 10 outer border edges creased. `Join` off makes the bridge a SubD of its own; `Crease`
//! on keeps the joined edges creased. `Straightness` (1, straight, at first; 0 bows most) curves the
//! rows as Rhino does (measured 24 November 2026, see `curva_mesh`'s bridge rows). Curva offers the
//! options while the second set is picked; picking faces is not here yet.

use super::parts::{by_object, run};
use crate::Message;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_mesh::{Point, SubD};

const SEGMENTS: &str = "Segments";
const JOIN: &str = "Join";
const CREASE: &str = "Crease";
const STRAIGHTNESS: &str = "Straightness";

/// The recipe.
#[derive(Default)]
pub struct Bridge;

fn gap(a: Point, b: Point) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
}

/// `second` turned round when that brings its ends nearer to `first`'s, as Rhino lines a
/// bridge up.
fn aligned(first: &[u32], second: &[u32], at: &[Point]) -> Vec<u32> {
    let p = |v: u32| at[v as usize];
    let (f0, f1) = (p(first[0]), p(first[first.len() - 1]));
    let (s0, s1) = (p(second[0]), p(second[second.len() - 1]));
    let mut out = second.to_vec();
    if gap(f0, s1) + gap(f1, s0) < gap(f0, s0) + gap(f1, s1) {
        out.reverse();
    }
    out
}

/// The edges along a run of points.
fn edges_of(chain: &[u32]) -> Vec<[u32; 2]> {
    chain.windows(2).map(|w| [w[0], w[1]]).collect()
}

/// The SubD a picked object is, if it is one.
fn subd_of(cx: &Context<'_>, id: u64) -> Option<SubD> {
    let object = cx.doc.state().objects.iter().find(|o| o.id == id)?;
    object.shape.subd().cloned()
}

impl Recipe for Bridge {
    const NAME: &'static str = "bridge";
    // Rhino 8's options and their starting values, measured.
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole(SEGMENTS, 1, 1, 100),
        OptionDef::toggle(JOIN, true),
        OptionDef::toggle(CREASE, false),
        OptionDef::number(STRAIGHTNESS, 1.0),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::mesh_parts("prompt.bridge.first").offering(&[])];
        if !cx.values.is_empty() {
            steps.push(StepDef::mesh_parts("prompt.bridge.second").offering(&[
                SEGMENTS,
                JOIN,
                CREASE,
                STRAIGHTNESS,
            ]));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let bad = || Message::new("error.bridge_sets");
        let (first, second) = (by_object(cx, 0), by_object(cx, 1));
        let ([(a_id, a_parts)], [(b_id, b_parts)]) = (
            &first.into_iter().collect::<Vec<_>>()[..],
            &second.into_iter().collect::<Vec<_>>()[..],
        ) else {
            return Err(bad());
        };
        let a = subd_of(cx, *a_id).ok_or_else(bad)?;
        let (chain_a, chain_b) = (run(a_parts).ok_or_else(bad)?, run(b_parts).ok_or_else(bad)?);
        if chain_a.len() != chain_b.len() || chain_a.len() < 2 {
            return Err(Message::new("error.bridge_counts"));
        }
        let failed =
            |e: curva_mesh::MeshError| Message::with("error.geometry", vec![e.to_string()]);
        // Both runs on one cage: the other SubD joined on when they are apart.
        let (base, chain_b) = if a_id == b_id {
            (a, chain_b)
        } else {
            let b = subd_of(cx, *b_id).ok_or_else(bad)?;
            let start = u32::try_from(a.cage().vertex_count()).unwrap_or(u32::MAX);
            let joined = a.joined(&b).map_err(failed)?;
            (joined, chain_b.iter().map(|&v| v + start).collect())
        };
        let chain_b = aligned(&chain_a, &chain_b, base.cage().vertices());
        let before = base.cage().face_count();
        let segments = cx.number(SEGMENTS) as u32;
        let mut bridged = base
            .bridged(&chain_a, &chain_b, segments, cx.number(STRAIGHTNESS))
            .map_err(failed)?;
        // The joined edges are smooth unless Crease is on, as in Rhino.
        let mut joint = edges_of(&chain_a);
        joint.extend(edges_of(&chain_b));
        bridged = if cx.toggle(CREASE) {
            bridged.with_creases(&joint)
        } else {
            bridged.without_creases(&joint)
        };
        let mut made = Made::default();
        if cx.toggle(JOIN) {
            made.replace.push((*a_id, Shape::SubD(bridged)));
            if a_id != b_id {
                made.remove.push(*b_id);
            }
        } else {
            // The bridge alone, as a SubD of its own; the picked SubDs stay as they were.
            let old: Vec<usize> = (0..before).collect();
            let alone = bridged.without_faces(&old).map_err(failed)?;
            made.derived.push(Derived {
                shape: Shape::SubD(alone),
                from: *a_id,
                history: None,
            });
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Mesh, SubD};

    /// An open 4 × 2 m strip of two faces, upright at `y`.
    fn strip(y: f64) -> SubD {
        let points = vec![
            [300.0, y, 0.0],
            [302.0, y, 0.0],
            [304.0, y, 0.0],
            [300.0, y, 2.0],
            [302.0, y, 2.0],
            [304.0, y, 2.0],
        ];
        SubD::new(Mesh::new(points, &[vec![0, 1, 4, 3], vec![1, 2, 5, 4]]).unwrap())
    }

    /// The two top edges of a strip.
    fn top(id: u64) -> Vec<(u64, MeshPart)> {
        vec![(id, MeshPart::edge(3, 4)), (id, MeshPart::edge(4, 5))]
    }

    fn bridge(options: &[&str]) -> Document {
        let mut doc = Document::default();
        let a = doc.add_shape(Shape::SubD(strip(0.0)));
        let b = doc.add_shape(Shape::SubD(strip(4.0)));
        let mut processor = Processor::default();
        doc.select_mesh_parts(&top(a));
        processor.submit(&mut doc, "bridge");
        processor.submit(&mut doc, "");
        for word in options {
            processor.submit(&mut doc, word);
        }
        doc.select_mesh_parts(&top(b));
        processor.submit(&mut doc, "");
        doc
    }

    #[test]
    fn two_strips_join_into_one_as_rhino_bridges_them() {
        // Rhino 8 with Enter at every option: one SubD of 6 quads on the 12 points, the 10 outer
        // border edges creased and the bridge smooth.
        let doc = bridge(&[]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 1, "the second strip joins the first");
        let subd = objects[0].shape.subd().unwrap();
        assert_eq!(subd.cage().face_count(), 6);
        assert!(subd.cage().faces().all(|f| f.len() == 4));
        assert_eq!(subd.cage().vertex_count(), 12);
        assert_eq!(subd.creases().count(), 10);
    }

    #[test]
    fn more_segments_and_creases_when_asked() {
        let doc = bridge(&["Segments", "2", "Crease"]);
        let subd = doc.state().objects[0].shape.subd().unwrap().clone();
        assert_eq!(subd.cage().face_count(), 8, "two rows of two quads");
        assert_eq!(subd.cage().vertex_count(), 15, "a new row of three points");
        assert_eq!(
            subd.creases().count(),
            16,
            "the 12 border edges and the 4 joined edges, which stay creased"
        );
    }

    #[test]
    fn straightness_bows_the_rows_as_rhino_does() {
        // Rhino 8 (measured 24 November 2026): two upright strips 4 m apart, Segments=2 and
        // Straightness=0, put the middle row 2 m above their tops, 2/3 of the gap out along each.
        let doc = bridge(&["Segments", "2", "Straightness", "0"]);
        let subd = doc.state().objects[0].shape.subd().unwrap();
        for x in [300.0, 302.0, 304.0] {
            assert!(
                subd.cage()
                    .vertices()
                    .iter()
                    .any(|p| (p[0] - x).abs() < 1e-9
                        && (p[1] - 2.0).abs() < 1e-9
                        && (p[2] - 4.0).abs() < 1e-9),
                "a middle point at x = {x}, y = 2, z = 4"
            );
        }
    }

    #[test]
    fn without_join_the_bridge_stands_alone() {
        let doc = bridge(&["Join"]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 3, "both strips stay, and the bridge is new");
        let alone = objects[2].shape.subd().unwrap();
        assert_eq!(alone.cage().face_count(), 2);
        assert_eq!(alone.cage().vertex_count(), 6);
    }
}
