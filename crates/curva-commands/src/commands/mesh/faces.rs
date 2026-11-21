//! Rhino's commands that change a mesh's faces: `ReduceMesh`, `TriangulateMesh`,
//! `QuadrangulateMesh`, `ExtractMeshFaces` and `MeshToNURB` (spec `docs/specs/mesh-and-subd.md`
//! §10, step 9). The geometry comes from `curva-mesh` (`reduce.rs`, `convert.rs` and
//! `nurbs.rs`); these commands choose the meshes and put the results back.

use super::check::{meshes, repaired};
use crate::Message;
use crate::commands::arguments::{failed, number};
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::{MeshPart, Shape};
use curva_mesh::{Mesh, MeshToNurbOptions};
use std::collections::BTreeMap;

/// QuadrangulateMesh's defaults, measured in Rhino 8: triangles leaning less than 5° apart join,
/// into quads whose longer diagonal is at most twice the shorter. ReduceMesh joins its triangles
/// back into quads with them.
const PLANARITY: f64 = 5.0;
const RECTANGULARITY: f64 = 2.0;

/// How many triangles a mesh counts as, as Rhino counts polygons for ReduceMesh: a quad is two.
fn triangle_count(mesh: &Mesh) -> usize {
    mesh.faces().map(|f| f.len().saturating_sub(2)).sum()
}

/// `ReduceMesh`: fewer faces keeping the shape. The meshes, then the percentage to take away
/// (Enter keeps the last); with `PolygonCount` above 0, each mesh is reduced to that many
/// triangles instead, as the dialog's "Reduce to" does. The meshes change in place.
#[derive(Default)]
pub struct ReduceMesh;

impl Recipe for ReduceMesh {
    const NAME: &'static str = "reducemesh";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::whole("PolygonCount", 0, 0, u32::MAX)];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.reducemesh.objects"),
            StepDef::number("prompt.reducemesh.percent").remembered("ReductionPercentage", 50.0),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let percent = number(cx, 1)
            .or_else(|| cx.doc.remembered("reducemesh.reductionpercentage"))
            .unwrap_or(50.0)
            .clamp(0.0, 100.0);
        let count = cx.number("PolygonCount") as usize;
        repaired(
            cx,
            |mesh| {
                let target = if count > 0 {
                    count
                } else {
                    (triangle_count(mesh) as f64 * (1.0 - percent / 100.0)).round() as usize
                };
                // As Rhino 8 does (measured): reduced to the target in triangles, then pairs
                // joined back into quads with QuadrangulateMesh's defaults.
                let (reduced, _) = mesh.reduced(target);
                let (reduced, _) =
                    reduced.quadrangulated(PLANARITY.to_radians(), 1.0 / RECTANGULARITY);
                let removed = triangle_count(mesh).saturating_sub(triangle_count(&reduced));
                (reduced, removed)
            },
            "done.reducemesh",
        )
    }
}

/// `TriangulateMesh`: every face of four or more corners split into triangles, in place.
#[derive(Default)]
pub struct TriangulateMesh;

impl Recipe for TriangulateMesh {
    const NAME: &'static str = "triangulatemesh";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.triangulatemesh.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        repaired(cx, Mesh::triangulated, "done.triangulatemesh")
    }
}

/// `QuadrangulateMesh`: pairs of triangles joined into quads, in place. `Planarity` is the most
/// the two triangles may lean apart, in degrees; `Rectangularity` the most the quad's longer
/// diagonal may be over its shorter one (1 or more). The defaults are Rhino 8's, measured.
#[derive(Default)]
pub struct QuadrangulateMesh;

impl Recipe for QuadrangulateMesh {
    const NAME: &'static str = "quadrangulatemesh";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::number("Planarity", PLANARITY),
        OptionDef::number("Rectangularity", RECTANGULARITY),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.quadrangulatemesh.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let angle = cx.number("Planarity").clamp(0.0, 90.0).to_radians();
        let ratio = 1.0 / cx.number("Rectangularity").max(1.0);
        repaired(
            cx,
            |mesh| mesh.quadrangulated(angle, ratio),
            "done.quadrangulatemesh",
        )
    }
}

/// `ExtractMeshFaces`: the picked faces taken out of their mesh into a mesh of their own, on the
/// same layer. A mesh left with no faces goes.
#[derive(Default)]
pub struct ExtractMeshFaces;

impl Recipe for ExtractMeshFaces {
    const NAME: &'static str = "extractmeshfaces";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::mesh_parts("prompt.extractmeshfaces.faces")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let mut picked: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
        if let Some(Value::MeshParts(parts)) = cx.values.first() {
            for &(id, part) in parts {
                if let MeshPart::Face(f) = part {
                    picked.entry(id).or_default().push(f as usize);
                }
            }
        }
        let mut made = Made::default();
        let mut faces = 0;
        for (id, chosen) in picked {
            let Some(mesh) = cx
                .doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.shape.mesh())
            else {
                continue;
            };
            let (taken, rest) = mesh.split_off(&chosen);
            if taken.face_count() == 0 {
                continue;
            }
            faces += taken.face_count();
            made.derived.push(Derived {
                shape: Shape::Mesh(taken),
                from: id,
                history: None,
            });
            if rest.face_count() == 0 {
                made.remove.push(id);
            } else {
                made.replace.push((id, Shape::Mesh(rest)));
            }
        }
        if faces == 0 {
            return Err(Message::new("error.extractmeshfaces_none"));
        }
        made.message = Some(Message::with(
            "done.extractmeshfaces",
            vec![faces.to_string()],
        ));
        Ok(made)
    }
}

/// `MeshToNURB`: each chosen mesh as a polysurface of one surface per face, sewn where faces
/// meet (a closed mesh gives a solid). The meshes stay, as in Rhino.
#[derive(Default)]
pub struct MeshToNurb;

impl Recipe for MeshToNurb {
    const NAME: &'static str = "meshtonurb";
    // Rhino 8's options, both on at first (measured).
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle("TrimTriangularFaces", true),
        OptionDef::toggle("UseNgons", true),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.meshtonurb.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let found = meshes(cx);
        if found.is_empty() {
            return Err(Message::new("error.check_no_meshes"));
        }
        let mut made = Made::default();
        for (id, mesh) in found {
            let options = MeshToNurbOptions {
                trim_triangles: cx.toggle("TrimTriangularFaces"),
                ngons: cx.toggle("UseNgons"),
            };
            let body = mesh.to_nurbs_with(options).map_err(|e| failed(&e))?;
            made.derived.push(Derived {
                shape: Shape::Body(body),
                from: id,
                history: None,
            });
        }
        made.message = Some(Message::with(
            "done.meshtonurb",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, MeshPart, Shape};
    use curva_mesh::{Frame, Mesh};

    fn doc_with(mesh: Mesh) -> Document {
        let mut doc = Document::default();
        doc.add_shape(Shape::Mesh(mesh));
        doc
    }

    fn mesh(doc: &Document, k: usize) -> &Mesh {
        doc.state().objects[k].shape.mesh().unwrap()
    }

    /// A box of 4 × 4 × 4 quads on each side: 96 quads, 192 triangles as Rhino counts them.
    fn grid_box() -> Mesh {
        Mesh::mesh_box(&Frame::world(), [1.0; 3], [4, 4, 4])
    }

    #[test]
    fn reduce_mesh_by_a_percentage_and_to_a_count() {
        let mut doc = doc_with(grid_box());
        run(&mut doc, &["selall", "reducemesh", "75"]);
        let faces = mesh(&doc, 0).face_count();
        assert!(faces <= 48 && faces > 12, "{faces}");
        assert!((mesh(&doc, 0).volume().unwrap() - 1.0).abs() < 1e-6);
        assert_eq!(doc.state().objects.len(), 1);

        let mut doc = doc_with(grid_box());
        doc.remember("reducemesh.polygoncount", 100.0);
        run(&mut doc, &["selall", "reducemesh", ""]);
        assert!(mesh(&doc, 0).face_count() <= 100);
    }

    /// A radius-5 UV sphere of 40 × 40 faces: 1,520 quads and 80 triangles, 3,120 triangles as
    /// Rhino counts them, as Rhino 8's `Mesh.CreateFromSphere(sphere, 40, 40)` makes it.
    fn sphere_40() -> Mesh {
        use curva_mesh::SphereStyle;
        let style = SphereStyle::Uv {
            vertical: 40,
            around: 40,
        };
        Mesh::mesh_sphere(&Frame::world(), 5.0, style)
    }

    #[test]
    fn reduce_mesh_reaches_rhinos_counts_and_joins_quads_back() {
        // Measured in Rhino 8 with `-ReduceMesh ReductionPercentage=…`: the sphere's 3,120
        // triangles go to exactly 1,560 at 50 % (443 quads and 674 triangles) and 312 at 90 %;
        // the box's 192 go to 96 (19 quads and 58 triangles). All stay closed.
        for (start, percent, left) in [
            (sphere_40(), "50", 1560),
            (sphere_40(), "90", 312),
            (grid_box(), "50", 96),
        ] {
            assert_eq!(
                super::triangle_count(&start),
                if left == 96 { 192 } else { 3120 }
            );
            let mut doc = doc_with(start);
            run(&mut doc, &["selall", "reducemesh", percent]);
            let reduced = mesh(&doc, 0);
            assert_eq!(super::triangle_count(reduced), left, "{percent} %");
            assert!(reduced.is_closed());
            assert!(reduced.faces().any(|f| f.len() == 4), "quads joined back");
        }
    }

    /// Two triangles over the unit square, sharing the diagonal from corner 0 to corner 2.
    fn pair(points: [[f64; 3]; 4]) -> Document {
        doc_with(Mesh::new(points.to_vec(), &[vec![0, 1, 2], vec![0, 2, 3]]).unwrap())
    }

    #[test]
    fn quadrangulate_mesh_joins_as_rhinos_defaults_do() {
        // Measured in Rhino 8: lifting the fourth corner by 0.061 (faces 4.93° apart) still
        // joins, by 0.062 (5.01°) does not; a rhombus whose diagonals are 2 : 1 joins, one at
        // 2.04 : 1 does not.
        let joined = |points| {
            let mut doc = pair(points);
            run(&mut doc, &["selall", "quadrangulatemesh"]);
            mesh(&doc, 0).face_count() == 1
        };
        let fold = |z: f64| {
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, z],
            ]
        };
        assert!(joined(fold(0.061)));
        assert!(!joined(fold(0.062)));
        let rhombus = |r: f64| {
            [
                [-1.0, 0.0, 0.0],
                [0.0, -r, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, r, 0.0],
            ]
        };
        assert!(joined(rhombus(0.5)));
        assert!(!joined(rhombus(0.49)));
    }

    #[test]
    fn triangulate_then_quadrangulate_gives_quads_back() {
        let mut doc = doc_with(grid_box());
        run(&mut doc, &["selall", "triangulatemesh"]);
        assert_eq!(mesh(&doc, 0).face_count(), 192);
        assert!(mesh(&doc, 0).faces().all(|f| f.len() == 3));
        run(&mut doc, &["selall", "quadrangulatemesh"]);
        assert_eq!(mesh(&doc, 0).face_count(), 96);
        assert!(mesh(&doc, 0).faces().all(|f| f.len() == 4));
    }

    #[test]
    fn extract_mesh_faces_makes_a_mesh_of_its_own() {
        let mut doc = doc_with(Mesh::mesh_box(&Frame::world(), [1.0; 3], [1, 1, 1]));
        let id = doc.state().objects[0].id;
        doc.select_mesh_parts(&[(id, MeshPart::Face(0)), (id, MeshPart::Face(1))]);
        run(&mut doc, &["extractmeshfaces", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        assert_eq!(mesh(&doc, 0).face_count(), 4);
        assert_eq!(mesh(&doc, 1).face_count(), 2);
    }

    #[test]
    fn mesh_to_nurb_splits_ngons_when_use_ngons_is_off() {
        // Rhino 8's UseNgons is on at first: a hexagon gives one surface; off, its triangles.
        let points = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 1.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let hexagon = Mesh::new(points, &[vec![0, 1, 2, 3, 4, 5]]).unwrap();
        let mut doc = doc_with(hexagon.clone());
        run(&mut doc, &["selall", "meshtonurb"]);
        let one = doc.state().objects[1].shape.body().unwrap();
        assert_eq!(one.face_count().unwrap(), 1);
        let mut doc = Document::default();
        let id = doc.add_shape(Shape::Mesh(hexagon));
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "meshtonurb");
        processor.submit(&mut doc, "UseNgons");
        doc.select(&[id]);
        processor.submit(&mut doc, "");
        let split = doc.state().objects[1].shape.body().unwrap();
        assert_eq!(split.face_count().unwrap(), 4);
    }

    #[test]
    fn mesh_to_nurb_keeps_the_mesh_and_makes_a_solid() {
        let mut doc = doc_with(Mesh::mesh_box(&Frame::world(), [2.0; 3], [1, 1, 1]));
        run(&mut doc, &["selall", "meshtonurb"]);
        assert_eq!(doc.state().objects.len(), 2);
        let body = doc.state().objects[1].shape.body().unwrap();
        assert!(body.is_solid());
        assert!((body.volume().unwrap() - 8.0).abs() < 1e-6);
    }

    #[test]
    fn the_reduce_mesh_preview_follows_the_typed_percentage() {
        let mut doc = doc_with(grid_box());
        let ids: Vec<u64> = doc.state().objects.iter().map(|o| o.id).collect();
        doc.select(&ids);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "reducemesh");
        let few = processor.preview(&doc, None, "90").len();
        let many = processor.preview(&doc, None, "10").len();
        assert!(few > 0 && few < many, "{few} {many}");
        assert_eq!(mesh(&doc, 0).face_count(), 96);
    }
}
