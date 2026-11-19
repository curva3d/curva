//! `Check` for meshes, and Rhino's mesh repairs: `MeshRepair`, `UnifyMeshNormals`,
//! `FillMeshHoles`, `CullDegenerateMeshFaces` and `RebuildMeshNormals` (spec
//! `docs/specs/mesh-and-subd.md` §10, step 8). What is wrong and each fix come from `curva-mesh`
//! (`Mesh::check` and `repair.rs`); these commands choose the meshes, report, and put the
//! repaired meshes back in place.

use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::{Document, Shape};
use curva_mesh::{Mesh, MeshCheck};

/// The meshes among the objects chosen at the first step, with their ids.
fn meshes<'a>(cx: &'a Context<'_>) -> Vec<(u64, &'a Mesh)> {
    let ids = objects(cx, 0);
    cx.doc
        .state()
        .objects
        .iter()
        .filter(|o| ids.contains(&o.id))
        .filter_map(|o| o.shape.mesh().map(|m| (o.id, m)))
        .collect()
}

/// The length under which an edge counts as short: a tenth of the model's tolerance, as Rhino 8.
fn short_edge(doc: &Document) -> f64 {
    doc.state().unit.tolerance() / 10.0
}

/// One mesh's report, as Rhino words it: what the mesh is, then "This is a good mesh." or what is
/// wrong with it.
#[must_use]
pub fn report(mesh: &Mesh, check: &MeshCheck) -> String {
    let mut text = format!(
        "{} mesh of {} points and {} faces",
        if mesh.is_closed() { "closed" } else { "open" },
        mesh.vertex_count(),
        mesh.face_count()
    );
    if check.pieces > 1 {
        text.push_str(&format!(" in {} pieces", check.pieces));
    }
    if check.naked_edges > 0 {
        text.push_str(&format!(", {} naked edges", check.naked_edges));
    }
    text.push('.');
    let problems: Vec<String> = [
        (check.degenerate_faces, "degenerate faces"),
        (check.short_edges, "very short edges"),
        (check.non_manifold_edges, "non-manifold edges"),
        (check.duplicate_faces, "duplicate faces"),
        (
            check.misdirected_faces.unwrap_or(0),
            "faces pointing the wrong way",
        ),
        (check.unused_vertices, "unused points"),
        (check.bad_normals, "bad normals"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .chain((check.crosses_itself == Some(true)).then(|| "faces crossing each other".to_owned()))
    .collect();
    if problems.is_empty() {
        text.push_str(" This is a good mesh.");
    } else {
        text.push_str(&format!(" Problems: {}.", problems.join("; ")));
    }
    text
}

/// `Check`: reports on each chosen mesh as Rhino does.
#[derive(Default)]
pub struct Check;

impl Recipe for Check {
    const NAME: &'static str = "check";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.check.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let found = meshes(cx);
        if found.is_empty() {
            return Err(Message::new("error.check_no_meshes"));
        }
        let short = short_edge(cx.doc);
        let lines: Vec<String> = found
            .iter()
            .map(|(_, mesh)| report(mesh, &mesh.check(short)))
            .collect();
        let others = objects(cx, 0).len() - found.len();
        let mut text = lines.join(" ");
        if others > 0 {
            text.push_str(&format!(" {others} other objects are not meshes."));
        }
        Ok(Made {
            message: Some(Message::with("done.check", vec![text])),
            ..Made::default()
        })
    }
}

/// The chosen meshes changed by `fix`, which returns the new mesh and how many things it changed;
/// the done message counts them.
fn repaired(
    cx: &Context<'_>,
    fix: impl Fn(&Mesh) -> (Mesh, usize),
    done: &'static str,
) -> Result<Made, Message> {
    let found = meshes(cx);
    if found.is_empty() {
        return Err(Message::new("error.check_no_meshes"));
    }
    let mut made = Made::default();
    let mut changed = 0;
    for (id, mesh) in found {
        let (fixed, count) = fix(mesh);
        if count > 0 {
            changed += count;
            made.replace.push((id, Shape::Mesh(fixed)));
        }
    }
    made.message = Some(Message::with(done, vec![changed.to_string()]));
    Ok(made)
}

/// One repair as a command of its own, as Rhino has: choose meshes, and the fix is made.
macro_rules! single_fix {
    ($(#[$doc:meta])* $name:ident, $key:literal, $prompt:literal, $done:literal, $fix:expr) => {
        $(#[$doc])*
        #[derive(Default)]
        pub struct $name;

        impl Recipe for $name {
            const NAME: &'static str = $key;

            fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
                vec![StepDef::objects($prompt)]
            }

            fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
                repaired(cx, $fix, $done)
            }
        }
    };
}

single_fix!(
    /// `UnifyMeshNormals`: turns faces round so each piece points one way, closed pieces out.
    UnifyMeshNormals,
    "unifymeshnormals",
    "prompt.meshrepair.objects",
    "done.unifymeshnormals",
    Mesh::unified
);
single_fix!(
    /// `FillMeshHoles`: closes every hole in the chosen meshes.
    FillMeshHoles,
    "fillmeshholes",
    "prompt.meshrepair.objects",
    "done.fillmeshholes",
    Mesh::with_holes_filled
);
single_fix!(
    /// `CullDegenerateMeshFaces`: takes out faces with no area.
    CullDegenerateMeshFaces,
    "culldegeneratemeshfaces",
    "prompt.meshrepair.objects",
    "done.culldegeneratemeshfaces",
    Mesh::without_degenerate_faces
);
single_fix!(
    /// `RebuildMeshNormals`: works the normals out again from the faces.
    RebuildMeshNormals,
    "rebuildmeshnormals",
    "prompt.meshrepair.objects",
    "done.rebuildmeshnormals",
    |m: &Mesh| (m.with_rebuilt_normals(), 1)
);

const CULL_DEGENERATE: &str = "CullDegenerate";
const CULL_DUPLICATES: &str = "CullDuplicates";
const UNUSED_POINTS: &str = "RemoveUnusedPoints";
const WELD: &str = "Weld";
const UNIFY: &str = "UnifyNormals";
const FILL_HOLES: &str = "FillHoles";
const REBUILD_NORMALS: &str = "RebuildNormals";

/// `MeshRepair`: the main fixes in one go, in Rhino's order, each an option that is remembered.
/// Welding joins points at exactly one place, so hard edges stay hard.
#[derive(Default)]
pub struct MeshRepair;

impl Recipe for MeshRepair {
    const NAME: &'static str = "meshrepair";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(CULL_DEGENERATE, true),
        OptionDef::toggle(CULL_DUPLICATES, true),
        OptionDef::toggle(UNUSED_POINTS, true),
        OptionDef::toggle(WELD, false),
        OptionDef::toggle(UNIFY, true),
        OptionDef::toggle(FILL_HOLES, false),
        OptionDef::toggle(REBUILD_NORMALS, false),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.meshrepair.objects")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let on = |name| cx.toggle(name);
        repaired(
            cx,
            |mesh| {
                let mut mesh = mesh.clone();
                let mut changed = 0;
                let mut step = |on: bool, fix: &dyn Fn(&Mesh) -> (Mesh, usize)| {
                    if on {
                        let (fixed, count) = fix(&mesh);
                        mesh = fixed;
                        changed += count;
                    }
                };
                step(on(CULL_DEGENERATE), &Mesh::without_degenerate_faces);
                step(on(CULL_DUPLICATES), &Mesh::without_duplicate_faces);
                step(on(WELD), &|m| m.welded(0.0));
                step(on(UNUSED_POINTS), &Mesh::without_unused_vertices);
                step(on(UNIFY), &Mesh::unified);
                step(on(FILL_HOLES), &Mesh::with_holes_filled);
                step(on(REBUILD_NORMALS), &|m| (m.with_rebuilt_normals(), 1));
                (mesh, changed)
            },
            "done.meshrepair",
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_mesh::{Frame, Mesh};

    fn cube() -> Mesh {
        Mesh::mesh_box(&Frame::world(), [1.0; 3], [1, 1, 1])
    }

    fn doc_with(mesh: Mesh) -> Document {
        let mut doc = Document::default();
        doc.add_shape(Shape::Mesh(mesh));
        doc
    }

    fn mesh(doc: &Document) -> &Mesh {
        doc.state().objects[0].shape.mesh().unwrap()
    }

    /// A cube with its first face turned round and its top missing.
    fn damaged() -> Mesh {
        let cube = cube();
        let mut faces: Vec<Vec<u32>> = cube.faces().map(<[u32]>::to_vec).collect();
        faces[0].reverse();
        faces.pop();
        Mesh::new(cube.vertices().to_vec(), &faces).unwrap()
    }

    #[test]
    fn a_good_mesh_is_reported_as_one() {
        let cube = cube();
        let text = super::report(&cube, &cube.check(0.001));
        assert_eq!(
            text,
            "closed mesh of 8 points and 6 faces. This is a good mesh."
        );
    }

    #[test]
    fn problems_are_listed() {
        let mesh = damaged();
        let text = super::report(&mesh, &mesh.check(0.001));
        assert!(
            text.starts_with("open mesh of 8 points and 5 faces, 4 naked edges."),
            "{text}"
        );
        assert!(text.contains("1 faces pointing the wrong way"), "{text}");
    }

    #[test]
    fn check_changes_nothing() {
        let mut doc = doc_with(damaged());
        let before = doc.state().clone();
        run(&mut doc, &["selall", "check"]);
        assert_eq!(doc.state(), &before);
    }

    #[test]
    fn mesh_repair_turns_faces_and_can_fill_holes() {
        let mut doc = doc_with(damaged());
        run(&mut doc, &["selall", "meshrepair"]);
        assert!(mesh(&doc).check(0.001).misdirected_faces == Some(0));
        assert!(!mesh(&doc).is_closed());
        let mut doc = doc_with(damaged());
        doc.remember("meshrepair.fillholes", 1.0);
        run(&mut doc, &["selall", "meshrepair"]);
        assert!(mesh(&doc).is_closed());
        assert!((mesh(&doc).volume().unwrap() - 1.0).abs() < 1e-9);
        assert!(mesh(&doc).check(0.001).is_good());
    }

    #[test]
    fn each_single_fix() {
        let mut doc = doc_with(damaged());
        run(&mut doc, &["selall", "unifymeshnormals"]);
        assert_eq!(mesh(&doc).check(0.001).misdirected_faces, Some(0));
        run(&mut doc, &["selall", "fillmeshholes"]);
        assert!(mesh(&doc).is_closed());
        run(&mut doc, &["selall", "rebuildmeshnormals"]);
        assert!(!mesh(&doc).normals().is_none());

        let cube = cube();
        let mut faces: Vec<Vec<u32>> = cube.faces().map(<[u32]>::to_vec).collect();
        let mut points = cube.vertices().to_vec();
        points.push(points[0]);
        faces.push(vec![0, 1, 8]);
        let mut doc = doc_with(Mesh::new(points, &faces).unwrap());
        run(&mut doc, &["selall", "culldegeneratemeshfaces"]);
        assert_eq!(mesh(&doc).face_count(), 6);
    }

    #[test]
    fn objects_that_are_not_meshes_are_left_alone() {
        let mut doc = Document::default();
        run(&mut doc, &["line", "0,0", "1,0", "selall", "meshrepair"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(doc.state().objects[0].shape.curve().is_some());
    }
}
