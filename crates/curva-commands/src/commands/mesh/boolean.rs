//! Rhino's mesh booleans: `MeshBooleanUnion`, `MeshBooleanDifference`,
//! `MeshBooleanIntersection` and `MeshBooleanSplit` (spec `docs/specs/mesh-and-subd.md` §10,
//! step 7). The geometry comes from `curva-mesh` (`boolean.rs`, on manifold-rust); these commands
//! choose the meshes and put the results back as Rhino 8 does, measured there:
//!
//! - Union replaces the meshes with the one result. Meshes that do not overlap are left alone.
//! - Difference replaces each mesh cut with what is left, and deletes the cutters
//!   (`DeleteInput`, on as in Rhino; off keeps them).
//! - Intersection replaces each mesh with its overlap with the second set, and deletes the second
//!   set.
//! - Split replaces each mesh with the part outside the second set, adds the part inside, and
//!   keeps the second set.
//!
//! Each mesh of the first set is worked on alone, as in Rhino. A mesh the second set does not
//! touch is left alone, and when nothing is touched nothing changes.

use super::check::meshes;
use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_mesh::Mesh;
use curva_mesh::boolean::{self, BooleanError};

const DELETE_INPUT: &str = "DeleteInput";

/// Meshes with their ids.
type Meshes = Vec<(u64, Mesh)>;

/// The meshes chosen at step `index`, with their ids.
fn chosen(cx: &Context<'_>, index: usize) -> Meshes {
    let ids = objects(cx, index);
    cx.doc
        .state()
        .objects
        .iter()
        .filter(|o| ids.contains(&o.id))
        .filter_map(|o| o.shape.mesh().map(|m| (o.id, m.clone())))
        .collect()
}

/// The command bar's words for a boolean that could not be made.
fn failed(error: &BooleanError) -> Message {
    match error {
        BooleanError::Open { .. } => Message::new("error.meshboolean_open"),
        BooleanError::Nothing => Message::new("error.check_no_meshes"),
        BooleanError::Failed(why) => Message::with("error.geometry", vec![why.clone()]),
    }
}

/// A mesh's volume, nothing for an open one.
fn volume(mesh: &Mesh) -> f64 {
    mesh.volume().unwrap_or(0.0)
}

/// Whether two volumes are the same, to the digits a boolean keeps.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// The two sets of a difference, intersection or split, or a message when one has no meshes.
fn sets(cx: &Context<'_>) -> Result<Option<(Meshes, Meshes)>, Message> {
    if cx.values.len() < 2 {
        return Ok(None);
    }
    let (first, second) = (chosen(cx, 0), chosen(cx, 1));
    if first.is_empty() || second.is_empty() {
        return Err(Message::new("error.check_no_meshes"));
    }
    Ok(Some((first, second)))
}

/// The steps of a two-set boolean: the meshes to work on, then the meshes to work with.
fn two_sets(cx: &Context<'_>, first: &'static str, second: &'static str) -> Vec<StepDef> {
    let mut steps = vec![StepDef::objects(first)];
    if !cx.values.is_empty() {
        steps.push(StepDef::objects(second).apart());
    }
    steps
}

/// The recipe.
#[derive(Default)]
pub struct MeshBooleanUnion;

impl Recipe for MeshBooleanUnion {
    const NAME: &'static str = "meshbooleanunion";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.meshbooleanunion.objects").at_least(2)]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let found = meshes(cx);
        if found.len() < 2 {
            return Err(Message::new("error.meshbooleanunion_two"));
        }
        let all: Vec<Mesh> = found.iter().map(|(_, m)| (*m).clone()).collect();
        let joined = boolean::union(&all).map_err(|e| failed(&e))?;
        let apart: f64 = all.iter().map(volume).sum();
        let mut made = Made::default();
        if same(volume(&joined), apart) {
            made.message = Some(Message::new("done.meshboolean_untouched"));
            return Ok(made);
        }
        let (first, rest) = found.split_first().expect("two meshes");
        made.replace.push((first.0, Shape::Mesh(joined)));
        made.remove.extend(rest.iter().map(|(id, _)| *id));
        made.message = Some(Message::with(
            "done.meshbooleanunion",
            vec![found.len().to_string()],
        ));
        Ok(made)
    }
}

/// What a two-set boolean does to one mesh of the first set: `None` when the second set does
/// not touch it, else the meshes that take its place (the first replaces it; none removes it).
type Work = fn(&Mesh, &[Mesh]) -> Result<Option<Vec<Mesh>>, BooleanError>;

/// Runs `work` on each mesh of the first set. The second set is deleted when `delete` and
/// something changed. `done` reports how many meshes changed.
fn each(cx: &Context<'_>, work: Work, delete: bool, done: &'static str) -> Result<Made, Message> {
    let Some((first, second)) = sets(cx)? else {
        return Ok(Made::default());
    };
    let cutters: Vec<Mesh> = second.iter().map(|(_, m)| m.clone()).collect();
    let mut made = Made::default();
    let mut changed = 0;
    for (id, mesh) in &first {
        let Some(results) = work(mesh, &cutters).map_err(|e| failed(&e))? else {
            continue;
        };
        changed += 1;
        let mut results = results.into_iter();
        match results.next() {
            Some(kept) => made.replace.push((*id, Shape::Mesh(kept))),
            None => made.remove.push(*id),
        }
        made.derived.extend(results.map(|m| Derived {
            shape: Shape::Mesh(m),
            from: *id,
            history: None,
        }));
    }
    if changed == 0 {
        made.message = Some(Message::new("done.meshboolean_untouched"));
        return Ok(made);
    }
    if delete {
        made.remove.extend(second.iter().map(|(id, _)| *id));
    }
    made.message = Some(Message::with(done, vec![changed.to_string()]));
    Ok(made)
}

/// What is left of `mesh` once `cutters` are taken away; `None` when they do not touch it.
fn cut(mesh: &Mesh, cutters: &[Mesh]) -> Result<Option<Vec<Mesh>>, BooleanError> {
    let left = boolean::difference(std::slice::from_ref(mesh), cutters)?;
    let kept: f64 = left.iter().map(volume).sum();
    Ok((!same(kept, volume(mesh))).then_some(left))
}

/// Where `mesh` overlaps `cutters`; `None` when they do not touch it.
fn overlap(mesh: &Mesh, cutters: &[Mesh]) -> Result<Option<Vec<Mesh>>, BooleanError> {
    let inside = boolean::intersection(std::slice::from_ref(mesh), cutters)?;
    Ok((!inside.is_empty()).then_some(inside))
}

/// The part of `mesh` outside `cutters`, then the part inside; `None` unless it has both.
fn halves(mesh: &Mesh, cutters: &[Mesh]) -> Result<Option<Vec<Mesh>>, BooleanError> {
    let one = std::slice::from_ref(mesh);
    let outside = boolean::difference(one, cutters)?;
    let inside = boolean::intersection(one, cutters)?;
    if outside.is_empty() || inside.is_empty() {
        return Ok(None);
    }
    Ok(Some(outside.into_iter().chain(inside).collect()))
}

/// The recipe.
#[derive(Default)]
pub struct MeshBooleanDifference;

impl Recipe for MeshBooleanDifference {
    const NAME: &'static str = "meshbooleandifference";
    const OPTIONS: &'static [OptionDef] = &[OptionDef::toggle(DELETE_INPUT, true)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        two_sets(
            cx,
            "prompt.meshbooleandifference.from",
            "prompt.meshbooleandifference.with",
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        each(
            cx,
            cut,
            cx.toggle(DELETE_INPUT),
            "done.meshbooleandifference",
        )
    }
}

/// The recipe.
#[derive(Default)]
pub struct MeshBooleanIntersection;

impl Recipe for MeshBooleanIntersection {
    const NAME: &'static str = "meshbooleanintersection";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        two_sets(
            cx,
            "prompt.meshbooleanintersection.first",
            "prompt.meshbooleanintersection.second",
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        each(cx, overlap, true, "done.meshbooleanintersection")
    }
}

/// The recipe.
#[derive(Default)]
pub struct MeshBooleanSplit;

impl Recipe for MeshBooleanSplit {
    const NAME: &'static str = "meshbooleansplit";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        two_sets(
            cx,
            "prompt.meshbooleansplit.objects",
            "prompt.meshbooleansplit.with",
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        each(cx, halves, false, "done.meshbooleansplit")
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::{Document, Shape};
    use curva_mesh::{Frame, Mesh};

    /// A closed mesh box of side `size` with its lowest corner at `at`.
    fn cube(at: [f64; 3], size: f64) -> Mesh {
        let mut frame = Frame::world();
        frame.origin = at;
        Mesh::mesh_box(&frame, [size; 3], [1, 1, 1])
    }

    /// Rhino's measuring pair: a 2 m box at the origin, and one moved by (1, 0.5, 0.3), which
    /// overlap by 1 × 1.5 × 1.7 = 2.55 m³.
    fn pair() -> (Document, u64, u64) {
        let mut doc = Document::default();
        let a = doc.add_shape(Shape::Mesh(cube([0.0; 3], 2.0)));
        let b = doc.add_shape(Shape::Mesh(cube([1.0, 0.5, 0.3], 2.0)));
        (doc, a, b)
    }

    /// The volumes of the meshes left, smallest first.
    fn volumes(doc: &Document) -> Vec<f64> {
        let mut all: Vec<f64> = doc
            .state()
            .objects
            .iter()
            .filter(|o| !o.hidden)
            .filter_map(|o| o.shape.mesh())
            .map(|m| (m.volume().unwrap_or(0.0) * 1e6).round() / 1e6)
            .collect();
        all.sort_by(f64::total_cmp);
        all
    }

    /// Runs a two-set boolean on `first` and `second`.
    fn two(doc: &mut Document, command: &str, first: &[u64], second: &[u64]) {
        let mut processor = Processor::default();
        doc.select(first);
        processor.submit(doc, command);
        doc.select(second);
        processor.submit(doc, "");
        assert!(!processor.is_running());
    }

    #[test]
    fn the_four_booleans_leave_what_rhino_8_leaves() {
        // Measured in Rhino 8 on the same two boxes.
        let (mut doc, a, b) = pair();
        doc.select(&[a, b]);
        Processor::default().submit(&mut doc, "meshbooleanunion");
        assert_eq!(volumes(&doc), vec![13.45]);

        let (mut doc, a, b) = pair();
        two(&mut doc, "meshbooleandifference", &[a], &[b]);
        assert_eq!(volumes(&doc), vec![5.45]);

        let (mut doc, a, b) = pair();
        two(&mut doc, "meshbooleanintersection", &[a], &[b]);
        assert_eq!(volumes(&doc), vec![2.55]);

        let (mut doc, a, b) = pair();
        two(&mut doc, "meshbooleansplit", &[a], &[b]);
        assert_eq!(volumes(&doc), vec![2.55, 5.45, 8.0]);
        let kept = doc.state().objects.iter().find(|o| o.id == a).unwrap();
        assert_eq!(kept.shape.mesh().unwrap().volume().unwrap().round(), 5.0);
    }

    #[test]
    fn difference_keeps_the_cutters_when_delete_input_is_off() {
        let (mut doc, a, b) = pair();
        let mut processor = Processor::default();
        doc.select(&[a]);
        processor.submit(&mut doc, "meshbooleandifference");
        processor.submit(&mut doc, "DeleteInput");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
        assert_eq!(volumes(&doc), vec![5.45, 8.0]);
    }

    #[test]
    fn meshes_that_do_not_touch_are_left_alone_as_in_rhino() {
        for command in [
            "meshbooleandifference",
            "meshbooleanintersection",
            "meshbooleansplit",
        ] {
            let mut doc = Document::default();
            let a = doc.add_shape(Shape::Mesh(cube([0.0; 3], 2.0)));
            let b = doc.add_shape(Shape::Mesh(cube([5.0, 0.0, 0.0], 2.0)));
            two(&mut doc, command, &[a], &[b]);
            assert_eq!(volumes(&doc), vec![8.0, 8.0], "{command}");
        }
        let mut doc = Document::default();
        let a = doc.add_shape(Shape::Mesh(cube([0.0; 3], 2.0)));
        let b = doc.add_shape(Shape::Mesh(cube([5.0, 0.0, 0.0], 2.0)));
        doc.select(&[a, b]);
        Processor::default().submit(&mut doc, "meshbooleanunion");
        assert_eq!(doc.state().objects.iter().filter(|o| !o.hidden).count(), 2);
    }

    #[test]
    fn each_mesh_of_the_first_set_is_cut_on_its_own() {
        // Rhino 8: two boxes cut by one larger box give two results of 5.45 m³ each.
        let mut doc = Document::default();
        let a = doc.add_shape(Shape::Mesh(cube([0.0; 3], 2.0)));
        let c = doc.add_shape(Shape::Mesh(cube([0.0, 3.0, 0.0], 2.0)));
        let b = doc.add_shape(Shape::Mesh(cube([1.0, 0.5, 0.3], 4.0)));
        two(&mut doc, "meshbooleandifference", &[a, c], &[b]);
        assert_eq!(volumes(&doc), vec![5.45, 5.45]);
    }

    #[test]
    fn an_open_mesh_is_refused_and_nothing_changes() {
        let (mut doc, a, _) = pair();
        let mut open = cube([1.0, 0.5, 0.3], 2.0);
        open = open.split_off(&[0]).1;
        let b = doc.add_shape(Shape::Mesh(open));
        two(&mut doc, "meshbooleandifference", &[a], &[b]);
        assert_eq!(volumes(&doc).len(), 3);
    }
}
