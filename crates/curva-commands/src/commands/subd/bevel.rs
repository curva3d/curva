//! Rhino's `Bevel` for SubD edges. Measured in Rhino 8 (23 November 2026, a RhinoCommon pick and
//! `-Bevel`): it asks "Select Mesh or SubD edges to bevel ( EdgeLoop )", then "Bevel positioning.
//! Select point or give amount ( Segments=3  OffsetMode=Absolute  Straightness=0  KeepCreases=Yes
//! )"; 0.5 on the middle edge of a 4 m SubD box's front gives a strip of 3 quads, two five-sided
//! caps and Rhino's rows of points (29 faces on 32 points). Edges joined end to end bevel as one
//! run: one row at each point, caps only at open ends (two in a row: 32 faces on 35 points; a
//! loop round the box: 48 quads on 50 points). A clicked point gives the distance from the
//! nearest picked edge.
//!
//! On meshes Rhino offers WeldAngle=30 and RetainShape=No instead of KeepCreases; the shape is the
//! same, each cap a fan of triangles, and the changed faces' sharp edges unwelded (see
//! `Mesh::bevelled_run`). Points that are not inner ones where four edges meet, branching edges,
//! RetainShape and EdgeLoop are not here yet; KeepCreases (on, as Rhino) carries creases round the
//! bevel (see `curva_mesh`'s `subd_bevel_creases`).

use super::parts::{by_object, edges, run};
use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::{MeshPart, Shape};
use curva_mesh::{BevelAmount, Mesh, Point, SubD};

const SEGMENTS: &str = "Segments";
const OFFSET_MODE: &str = "OffsetMode";
const STRAIGHTNESS: &str = "Straightness";
const WELD_ANGLE: &str = "WeldAngle";
const KEEP_CREASES: &str = "KeepCreases";
const MODES: [&str; 2] = ["Absolute", "Proportional"];

/// A SubD or mesh being bevelled.
enum Bevelling {
    SubD(SubD),
    Mesh(Mesh),
}

impl Bevelling {
    fn points(&self) -> &[Point] {
        match self {
            Self::SubD(s) => s.cage().vertices(),
            Self::Mesh(m) => m.vertices(),
        }
    }

    /// The run `points` bevelled by `size`, with the command's options.
    fn bevelled(
        self,
        cx: &Context<'_>,
        points: &[u32],
        size: f64,
    ) -> Result<Self, curva_mesh::MeshError> {
        let amount = if cx.name(OFFSET_MODE) == "Proportional" {
            BevelAmount::Proportional(size)
        } else {
            BevelAmount::Absolute(size)
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let segments = cx.number(SEGMENTS) as u32;
        let straightness = cx.number(STRAIGHTNESS);
        Ok(match self {
            Self::SubD(s) => Self::SubD(s.bevelled_run(
                points,
                amount,
                segments,
                straightness,
                cx.toggle(KEEP_CREASES),
            )?),
            Self::Mesh(m) => Self::Mesh(m.bevelled_run(
                points,
                amount,
                segments,
                straightness,
                cx.number(WELD_ANGLE),
            )?),
        })
    }

    fn shape(self) -> Shape {
        match self {
            Self::SubD(s) => Shape::SubD(s),
            Self::Mesh(m) => Shape::Mesh(m),
        }
    }
}

/// Whether any picked part is on a mesh.
fn on_a_mesh(cx: &Context<'_>) -> bool {
    by_object(cx, 0).keys().any(|id| {
        cx.doc
            .state()
            .objects
            .iter()
            .any(|o| o.id == *id && o.shape.mesh().is_some())
    })
}

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

/// The picked edges split into runs joined end to end, each as [`run`] reads it. `None` when
/// edges branch.
fn runs(picked: &[[u32; 2]]) -> Option<Vec<Vec<u32>>> {
    let mut left: Vec<[u32; 2]> = picked.to_vec();
    let mut out = Vec::new();
    while let Some(seed) = left.pop() {
        let mut group = vec![seed];
        loop {
            let ends: Vec<u32> = group.iter().flatten().copied().collect();
            let Some(k) = left.iter().position(|e| e.iter().any(|v| ends.contains(v))) else {
                break;
            };
            group.push(left.swap_remove(k));
        }
        let parts: Vec<MeshPart> = group.iter().map(|&[a, b]| MeshPart::edge(a, b)).collect();
        out.push(run(&parts)?);
    }
    Some(out)
}

impl Recipe for Bevel {
    const NAME: &'static str = "bevel";
    // Rhino 8's options and their starting values, measured.
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole(SEGMENTS, 3, 1, 100),
        OptionDef::name(OFFSET_MODE, "Absolute"),
        OptionDef::number(STRAIGHTNESS, 0.0),
        OptionDef::number(WELD_ANGLE, 30.0),
        OptionDef::toggle(KEEP_CREASES, true),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == OFFSET_MODE).then(|| MODES.map(str::to_owned).to_vec())
    }

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::mesh_parts("prompt.bevel.edges").offering(&[])];
        if !cx.values.is_empty() {
            let offered: &[&'static str] = if on_a_mesh(cx) {
                &[SEGMENTS, OFFSET_MODE, STRAIGHTNESS, WELD_ANGLE]
            } else {
                &[SEGMENTS, OFFSET_MODE, STRAIGHTNESS]
            };
            steps.push(StepDef::point_or_number("prompt.bevel.position").offering(offered));
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
            let Some(object) = cx.doc.state().objects.iter().find(|o| o.id == id) else {
                continue;
            };
            let mut target = match &object.shape {
                Shape::SubD(s) => Bevelling::SubD(s.clone()),
                Shape::Mesh(m) => Bevelling::Mesh(m.clone()),
                _ => continue,
            };
            // Vertices at one place count as one, so a mesh's unwelded edges still join.
            let first_at = |v: u32| {
                let at = target.points();
                at.iter()
                    .position(|&q| q == at[v as usize])
                    .map_or(v, |k| k as u32)
            };
            let picked: Vec<[u32; 2]> = edges(&parts).iter().map(|e| e.map(first_at)).collect();
            // Bevelling one run renumbers points, so each later run is found again by place.
            let Some(found) = runs(&picked) else {
                return Err(Message::new("error.bevel_apart"));
            };
            let places: Vec<Vec<Point>> = found
                .iter()
                .map(|r| r.iter().map(|&v| target.points()[v as usize]).collect())
                .collect();
            for place in places {
                let at = target.points();
                let find = |p: &Point| at.iter().position(|q| q == p).map(|k| k as u32);
                let Some(points) = place.iter().map(find).collect::<Option<Vec<u32>>>() else {
                    continue;
                };
                let size = match answer {
                    Value::Number(n) => *n,
                    other => match other.point() {
                        Some(p) => place
                            .windows(2)
                            .map(|w| off_segment(p, w[0], w[1]))
                            .fold(f64::INFINITY, f64::min),
                        None => continue,
                    },
                };
                target = target.bevelled(cx, &points, size).map_err(failed)?;
            }
            made.replace.push((id, target.shape()));
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

    #[test]
    fn two_edges_in_a_row_bevel_as_one_run() {
        let mut doc = Document::default();
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let subd = SubD::subd_box(&frame, [4.0; 3], [2, 2, 2]);
        let at = |p: [f64; 3]| {
            let k = subd.cage().vertices().iter().position(|&q| q == p).unwrap();
            u32::try_from(k).unwrap()
        };
        let (a, b, c) = (
            at([202.0, 0.0, 0.0]),
            at([202.0, 0.0, 2.0]),
            at([202.0, 0.0, 4.0]),
        );
        let id = doc.add_shape(Shape::SubD(subd));
        doc.select_mesh_parts(&[(id, MeshPart::edge(a, b)), (id, MeshPart::edge(b, c))]);
        run(&mut doc, &["bevel", "", "0.5"]);
        let out = doc.state().objects[0].shape.subd().unwrap();
        assert_eq!(
            (out.cage().face_count(), out.cage().vertex_count()),
            (32, 35)
        );
    }

    #[test]
    fn a_mesh_edge_bevels_as_in_rhino() {
        let mut doc = Document::default();
        let frame = Frame {
            origin: [200.0, 0.0, 0.0],
            ..Frame::world()
        };
        let mesh = SubD::subd_box(&frame, [4.0; 3], [2, 2, 2])
            .cage()
            .clone()
            .with_rebuilt_normals();
        let at = |p: [f64; 3]| {
            let k = mesh.vertices().iter().position(|&q| q == p).unwrap();
            u32::try_from(k).unwrap()
        };
        let edge = MeshPart::edge(at([202.0, 0.0, 0.0]), at([202.0, 0.0, 2.0]));
        let id = doc.add_shape(Shape::Mesh(mesh));
        doc.select_mesh_parts(&[(id, edge)]);
        run(&mut doc, &["bevel", "", "0.5"]);
        let out = doc.state().objects[0].shape.mesh().unwrap();
        assert_eq!(out.face_count(), 33, "Rhino's 27 quads and 6 triangles");
    }
}
