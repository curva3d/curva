//! Curva's 3D viewport: what to draw, how to pick it, and drawing it on the graphics card.

mod camera;
mod circles;
mod fills;
mod glass;
mod gpu;
mod gumball;
mod index;
mod line_table;
mod lines;
mod meshes;
mod outline;
mod outlines;
mod palette;
mod pick;
mod pilot;
pub mod print;
mod simplify;
mod snap;
mod styles;
mod subd_live;
mod subd_parts;
mod subds;
mod timer;
mod view_style;
mod views;

#[cfg(test)]
use curva_document::Object;
#[cfg(test)]
use curva_kernel::{Curve, Point};

pub use camera::{CPlane, Camera};
pub use circles::{CIRCLE_SEGMENTS, PlacedCircle};
pub use glass::Glass;
pub use gpu::{
    DEPTH_BITS, DETAIL_SLOTS, Drawing, LineRenderer, MOST_CLIPS, Picture, VIEW_SLOTS, save_picture,
};
pub use gumball::{Amount, Gumball, GumballChange, Handle};
pub use line_table::{KindLook, LineTable};
pub use snap::{Guides, SnapHit, SnapKind, SnapSet, constrained};
pub use styles::{PictureStyle, save_styled_picture};
pub use subd_parts::{PartShape, subd_parts};
pub use timer::{OPTIONAL_FEATURES, baseline_limits, device_descriptor};
pub use view_style::{Light, OutlineLook, Style, Surfaces};
pub use views::StandardView;

pub use lines::*;
pub use outlines::*;
pub use palette::*;
pub use pick::*;
pub use pilot::{Lighting, Look, PilotSeed, bevy_picture, save_bevy_picture, save_png};

const GRID_HALF_SIZE: i32 = 100;
const GRID_STEP: i32 = 10;
/// Steps of the search along a segment for a picked point.
const PICK_SEARCH_STEPS: usize = 48;
/// How much of a body's size a point may lie behind its shaded mesh and still count as on it,
/// as the mesh only approximates curved faces.
const HIDING_ALLOWANCE: f64 = 0.002;
const GRID_COLOUR: [f32; 3] = [0.30, 0.31, 0.34];
const CURVE_COLOUR: [f32; 3] = [0.92, 0.92, 0.95];
pub(crate) const SELECTED_COLOUR: [f32; 3] = PREVIEW_COLOUR;
const LOCKED_COLOUR: [f32; 3] = [0.45, 0.45, 0.48];
/// Edges of surfaces and solids in shaded views, as the style colour `curva_document::style::Colour::EDGE`.
pub(crate) const EDGE_ON_FACES_COLOUR: [f32; 3] = [20.0 / 255.0, 20.0 / 255.0, 26.0 / 255.0];
/// The Hidden display mode (#164), as Rhino 8 draws it: surfaces flat in paper white, hiding
/// what is behind them, and every line and outline in black ink.
pub const PAPER_COLOUR: [f32; 3] = [1.0, 1.0, 1.0];
#[cfg(test)]
pub(crate) const INK_COLOUR: [f32; 3] = [0.0, 0.0, 0.0];
/// The colour every preview is drawn in: the accent orange, `#e0782f` (decision 0016).
pub const PREVIEW_COLOUR: [f32; 3] = [0.878, 0.471, 0.184];
const X_AXIS_COLOUR: [f32; 3] = [0.85, 0.3, 0.3];
const Y_AXIS_COLOUR: [f32; 3] = [0.35, 0.8, 0.4];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_space_draws_only_its_own_objects() {
        let mut doc = curva_document::Document::default();
        doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        let paper = curva_document::layouts::paper_size("A4").unwrap();
        let sheet = doc.add_layout("Sheet".to_owned(), paper, false);
        doc.set_space(Some(sheet));
        doc.add(Curve::circle([50.0, 50.0, 0.0], 10.0).unwrap());
        let mut cache = MeshCache::default();
        let model = Outlines::of_space(doc.state(), None, &mut cache);
        let on_sheet = Outlines::of_space(doc.state(), Some(sheet), &mut cache);
        assert_eq!(model.objects.len(), 1);
        assert_eq!(on_sheet.objects.len(), 1);
        assert_ne!(model.objects[0].0, on_sheet.objects[0].0);
    }

    /// Draws `doc` with `cache` and checks it matches drawing it from nothing.
    fn drawn_alike(doc: &curva_document::Document, cache: &mut MeshCache) {
        let fresh = Outlines::of(doc.state());
        let cached = Outlines::of_cached(doc.state(), cache);
        assert_eq!(cached, fresh);
    }

    #[test]
    fn drawing_again_reuses_unchanged_objects_and_stays_exact() {
        let mut doc = curva_document::Document::default();
        let a = doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::circle([0.0, 9.0, 0.0], 2.0).unwrap());
        doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        let mut cache = MeshCache::default();
        drawn_alike(&doc, &mut cache);
        let kept = std::sync::Arc::as_ptr(&cache.drawn[&b].0);
        doc.translate(&[a], [0.0, 0.0, 3.0]);
        drawn_alike(&doc, &mut cache);
        assert_eq!(std::sync::Arc::as_ptr(&cache.drawn[&b].0), kept);
        doc.hide(&[b]);
        drawn_alike(&doc, &mut cache);
        doc.remove(&[a]);
        drawn_alike(&doc, &mut cache);
        assert!(!cache.drawn.contains_key(&a));
    }

    #[test]
    fn a_layer_change_draws_its_objects_again() {
        let mut doc = curva_document::Document::default();
        let layer = doc.add_layer("Walls".to_owned());
        doc.add_on(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap(), Some(layer));
        let mut cache = MeshCache::default();
        drawn_alike(&doc, &mut cache);
        doc.set_layer_colour(layer, [1.0, 0.0, 0.0]);
        drawn_alike(&doc, &mut cache);
        doc.set_layer_visible(layer, false);
        drawn_alike(&doc, &mut cache);
        assert!(
            Outlines::of_cached(doc.state(), &mut cache)
                .objects
                .is_empty()
        );
    }

    #[test]
    fn an_instance_follows_its_definition() {
        let mut doc = curva_document::Document::default();
        let line = doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        let (definition, _) = doc.define_block("Post".to_owned(), [0.0; 3], &[line]);
        let mut cache = MeshCache::default();
        drawn_alike(&doc, &mut cache);
        let mut parts = Vec::new();
        let mut other = curva_document::Document::default();
        other.add(Curve::circle([0.0; 3], 4.0).unwrap());
        parts.extend(other.state().objects.iter().map(|o| Object::clone(o)));
        doc.set_definition_objects(definition, parts);
        drawn_alike(&doc, &mut cache);
    }
    #[test]
    fn block_instances_draw_their_definition_under_their_own_id() {
        let mut doc = curva_document::Document::default();
        let leg = doc.add(Curve::line([0.0; 3], [0.0, 0.0, 1.0]).unwrap());
        let seat = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [1.0, 1.0, 0.2]).unwrap());
        let (definition, first) = doc.define_block("Chair".to_owned(), [0.0; 3], &[leg, seat]);
        let second = doc.insert_block(
            definition,
            curva_document::blocks::moved_by([5.0, 0.0, 0.0]),
        );
        let outlines = Outlines::of(doc.state());
        let ids: std::collections::BTreeSet<u64> =
            outlines.objects.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, [first, second].into_iter().collect());
        // The second chair's lines are 5 along.
        let far = outlines
            .objects
            .iter()
            .filter(|(id, _)| *id == second)
            .flat_map(|(_, points)| points.iter())
            .map(|p| p[0])
            .fold(f64::INFINITY, f64::min);
        assert!((far - 5.0).abs() < 1e-9, "{far}");
    }

    use curva_document::Document;

    const SIZE: [f64; 2] = [800.0, 600.0];

    fn scene() -> (Outlines, u64, u64) {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::circle([0.0, 40.0, 0.0], 5.0).unwrap());
        (Outlines::of(doc.state()), a, b)
    }

    #[test]
    fn clicking_on_a_curve_picks_it() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let on_line = camera.project([5.0, 0.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, on_line, SIZE, 6.0), Some(line));
    }

    #[test]
    fn a_click_over_several_objects_finds_them_all_nearest_first() {
        let mut doc = Document::default();
        let near = doc.add(Curve::line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]).unwrap());
        let far = doc.add(Curve::line([-20.0, 0.2, 0.0], [20.0, 0.2, 0.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = Camera::default();
        let at = camera.project([5.0, 0.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick_all(&camera, at, SIZE, 6.0), vec![near, far]);
        let away = camera.project([5.0, 30.0, 0.0], SIZE).unwrap();
        assert!(outlines.pick_all(&camera, away, SIZE, 6.0).is_empty());
    }

    #[test]
    fn a_pick_finds_the_exact_point_on_the_curve() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let aim = [7.0, 0.0, 0.0];
        let at = camera.project(aim, SIZE).unwrap();
        let picked = outlines.pick_point(&camera, at, SIZE, 6.0).unwrap();
        assert!(
            picked[1].abs() < 1e-9 && (picked[0] - 7.0).abs() < 1e-3,
            "{picked:?}"
        );
    }

    #[test]
    fn a_pick_over_nothing_finds_nothing() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let at = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick_point(&camera, at, SIZE, 6.0), None);
    }

    #[test]
    fn hidden_layers_are_not_drawn_or_picked() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Hidden".to_owned());
        let line = doc.add(Curve::line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[line], layer);
        doc.set_layer_visible(layer, false);
        let outlines = Outlines::of(doc.state());
        assert!(outlines.objects.is_empty());
    }

    #[test]
    fn locked_layers_are_drawn_grey_and_cannot_be_picked() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Locked".to_owned());
        let line = doc.add(Curve::line([-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[line], layer);
        doc.set_layer_locked(layer, true);
        let outlines = Outlines::of(doc.state());
        let camera = Camera::default();
        let at = camera.project([5.0, 0.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, at, SIZE, 6.0), None);
        assert_eq!(outlines.pick_point(&camera, at, SIZE, 6.0), None);
        assert!(
            object_lines(&outlines, &[])
                .iter()
                .all(|v| v.colour == LOCKED_COLOUR)
        );
    }

    #[test]
    fn the_selection_filter_limits_boxing_to_the_kinds_ticked() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap());
        let line = doc.add(Curve::line([-5.0, 0.0, 0.0], [-3.0, 0.0, 0.0]).unwrap());
        let mut outlines = Outlines::of(doc.state());
        let camera = Camera::default();
        let everything = |o: &Outlines| {
            let mut found = o.in_box(&camera, [0.0, 0.0], SIZE, SIZE);
            found.sort_unstable();
            found.dedup();
            found
        };
        assert_eq!(everything(&outlines), [solid, line]);
        outlines.set_filter(PickFilter {
            solids: false,
            surfaces: false,
            points: false,
            ..PickFilter::default()
        });
        assert_eq!(everything(&outlines), [line]);
        // Turned off, the filter lets everything through again without losing its ticks.
        outlines.set_filter(PickFilter {
            on: false,
            solids: false,
            ..PickFilter::default()
        });
        assert_eq!(everything(&outlines), [solid, line]);
    }

    #[test]
    fn solids_give_faces_and_dark_edges_only_in_shaded_views() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.add(Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        // Twelve box edges and the line, each its own entry.
        assert_eq!(outlines.objects.len(), 13);
        let faces = face_vertices(&outlines, &[]);
        assert!(faces.len() >= 36 && faces.len().is_multiple_of(3));
        assert!(faces.iter().all(|v| v.colour == CURVE_COLOUR));
        assert!(
            object_lines(&outlines, &[])
                .iter()
                .all(|v| v.colour == CURVE_COLOUR)
        );
        let shaded = shaded_object_lines(&outlines, &[]);
        let dark = shaded
            .iter()
            .filter(|v| v.colour == EDGE_ON_FACES_COLOUR)
            .count();
        assert!(dark >= 24, "every piece of the box's twelve edges is dark");
        assert_eq!(shaded.len() - dark, 2, "the line keeps its colour");
        let picked = face_vertices(&outlines, &[solid]);
        assert!(picked.iter().all(|v| v.colour == SELECTED_COLOUR));
    }

    #[test]
    fn edges_are_picked_by_click_and_by_box() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 4.0, 4.0]).unwrap());
        doc.add(Curve::line([10.0, 0.0, 0.0], [12.0, 0.0, 0.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let at = |p: Point| camera.project(p, SIZE).unwrap();
        // A click on the box's edge along X at y = 0 picks a box edge; one on the line, nothing.
        let picked = outlines.pick_edge(&camera, at([2.0, 0.0, 0.0]), SIZE, 6.0);
        assert!(picked.is_some_and(|(id, _)| id == solid));
        let points = outlines.edge_points(picked.unwrap()).unwrap();
        assert!(points.iter().all(|p| p[1].abs() < 1e-9));
        assert_eq!(
            outlines.pick_edge(&camera, at([11.0, 0.0, 0.0]), SIZE, 6.0),
            None
        );
        // A window round the whole box, seen from above, takes all twelve edges.
        let (a, b) = (at([-1.0, -1.0, 0.0]), at([5.0, 5.0, 0.0]));
        let all = outlines.edges_in_box(&camera, [a[0], b[1]], [b[0], a[1]], SIZE);
        assert_eq!(all.len(), 12);
        assert!(all.iter().all(|(id, _)| *id == solid));
    }

    #[test]
    fn the_nearest_face_under_the_cursor_is_picked() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 4.0, 4.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let at = |p: Point| camera.project(p, SIZE).unwrap();
        // Seen from above, the middle of the box shows its top face, not its bottom.
        let (id, face) = outlines
            .pick_face(&camera, at([2.0, 2.0, 4.0]), SIZE)
            .unwrap();
        assert_eq!(id, solid);
        let triangles = outlines.face_triangles((id, face));
        assert!(!triangles.is_empty());
        assert!(
            triangles
                .iter()
                .flatten()
                .all(|p| (p[2] - 4.0).abs() < 1e-9)
        );
        assert_eq!(
            outlines.pick_face(&camera, at([20.0, 20.0, 0.0]), SIZE),
            None
        );
        // A window round the whole box takes all six faces.
        let (a, b) = (at([-1.0, -1.0, 0.0]), at([5.0, 5.0, 0.0]));
        assert_eq!(
            outlines
                .faces_in_box(&camera, [a[0], b[1]], [b[0], a[1]], SIZE)
                .len(),
            6
        );
    }

    #[test]
    fn a_window_takes_a_solid_only_with_all_its_edges() {
        let mut doc = Document::default();
        let solid = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [4.0, 4.0, 4.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let corner = |p: Point| camera.project(p, SIZE).unwrap();
        let (a, b) = (corner([-1.0, -1.0, 0.0]), corner([5.0, 5.0, 0.0]));
        let window = |from: [f64; 2], to: [f64; 2]| outlines.in_box(&camera, from, to, SIZE);
        assert_eq!(window([a[0], b[1]], [b[0], a[1]]), [solid]);
        // A window over half the box takes nothing; a crossing box over it takes the solid.
        let half = corner([2.0, 5.0, 0.0]);
        assert!(window([a[0], half[1]], [half[0], a[1]]).is_empty());
        assert_eq!(window([half[0], a[1]], [a[0], half[1]]), [solid]);
    }

    #[test]
    fn objects_take_the_colour_of_their_layer() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Red".to_owned());
        doc.set_layer_colour(layer, [1.0, 0.0, 0.0]);
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[line], layer);
        let lines = object_lines(&Outlines::of(doc.state()), &[]);
        assert!(lines.iter().all(|v| v.colour == [1.0, 0.0, 0.0]));
    }

    #[test]
    fn a_selected_object_is_highlighted_whatever_its_layer() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Red".to_owned());
        doc.set_layer_colour(layer, [1.0, 0.0, 0.0]);
        let line = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[line], layer);
        let lines = object_lines(&Outlines::of(doc.state()), &[line]);
        assert!(lines.iter().all(|v| v.colour == SELECTED_COLOUR));
    }

    #[test]
    fn a_click_inside_a_letter_picks_its_text() {
        use curva_document::annotation::{Annotation, Text, TextAlign};
        let mut doc = Document::default();
        let id = doc.add_annotation(Annotation::Text(Text {
            content: "I".to_owned(),
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            height: 40.0,
            font: None,
            align: TextAlign::Left,
            style: 0,
        }));
        let (low, high) = doc.state().objects[0].shape.bounds().unwrap();
        let middle = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0, 0.0];
        let outlines = Outlines::of(doc.state());
        let mut camera = StandardView::Top.camera();
        camera.frame([-20.0, -20.0, 0.0], [60.0, 60.0, 0.0]);
        let at = camera.project(middle, SIZE).unwrap();
        // Far from the letter's outline, but on it.
        assert_eq!(outlines.pick(&camera, at, SIZE, 1.0), Some(id));
    }

    fn one_point() -> (Outlines, u64) {
        let mut doc = Document::default();
        let id = doc.add(Curve::Point {
            at: [5.0, 5.0, 0.0],
        });
        (Outlines::of(doc.state()), id)
    }

    #[test]
    fn a_point_can_be_clicked() {
        let (outlines, id) = one_point();
        let camera = Camera::default();
        let at = camera.project([5.0, 5.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, at, SIZE, 6.0), Some(id));
    }

    #[test]
    fn a_point_is_caught_by_a_box_either_way() {
        let (outlines, id) = one_point();
        let camera = Camera::default();
        let at = camera.project([5.0, 5.0, 0.0], SIZE).unwrap();
        let (low, high) = ([at[0] - 5.0, at[1] - 5.0], [at[0] + 5.0, at[1] + 5.0]);
        assert_eq!(outlines.in_box(&camera, low, high, SIZE), [id]);
        assert_eq!(outlines.in_box(&camera, high, low, SIZE), [id]);
    }

    #[test]
    fn a_box_elsewhere_misses_a_point() {
        let (outlines, _) = one_point();
        let camera = Camera::default();
        let at = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        let (low, high) = ([at[0] - 5.0, at[1] - 5.0], [at[0] + 5.0, at[1] + 5.0]);
        assert!(outlines.in_box(&camera, high, low, SIZE).is_empty());
    }

    #[test]
    fn a_selected_point_is_marked_as_selected() {
        let (outlines, id) = one_point();
        assert_eq!(outlines.point_markers(&[id])[0].1, SELECTED_COLOUR);
        assert_eq!(outlines.point_markers(&[])[0].1, CURVE_COLOUR);
    }
    #[test]
    fn clicking_on_nothing_picks_nothing() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let empty = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, empty, SIZE, 6.0), None);
    }

    /// Screen corners a margin outside two model points, left corner first.
    fn around(camera: &Camera, a: Point, b: Point, margin: f64) -> ([f64; 2], [f64; 2]) {
        let (a, b) = (
            camera.project(a, SIZE).unwrap(),
            camera.project(b, SIZE).unwrap(),
        );
        (
            [a[0].min(b[0]) - margin, a[1].min(b[1]) - margin],
            [a[0].max(b[0]) + margin, a[1].max(b[1]) + margin],
        )
    }

    #[test]
    fn a_window_takes_an_object_wholly_inside() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let (left, right) = around(&camera, [-20.0, 0.0, 0.0], [20.0, 0.0, 0.0], 3.0);
        assert!(outlines.in_box(&camera, left, right, SIZE).contains(&line));
    }

    #[test]
    fn a_window_leaves_an_object_partly_outside() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let (left, right) = around(&camera, [-20.0, 0.0, 0.0], [0.0, 0.0, 0.0], 3.0);
        assert!(!outlines.in_box(&camera, left, right, SIZE).contains(&line));
    }

    #[test]
    fn the_same_box_dragged_leftwards_takes_the_partly_inside_object() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let (left, right) = around(&camera, [-20.0, 0.0, 0.0], [0.0, 0.0, 0.0], 3.0);
        assert!(outlines.in_box(&camera, right, left, SIZE).contains(&line));
    }

    #[test]
    fn a_crossing_box_catches_a_segment_with_no_sample_inside() {
        let (outlines, line, _) = scene();
        let camera = Camera::default();
        let middle = camera.project([0.0, 0.0, 0.0], SIZE).unwrap();
        let (low, high) = (
            [middle[0] - 2.0, middle[1] - 2.0],
            [middle[0] + 2.0, middle[1] + 2.0],
        );
        assert_eq!(outlines.in_box(&camera, high, low, SIZE), [line]);
    }

    #[test]
    fn a_box_over_empty_space_takes_nothing_in_either_direction() {
        let (outlines, _, _) = scene();
        let camera = Camera::default();
        let at = camera.project([60.0, -60.0, 0.0], SIZE).unwrap();
        let (low, high) = ([at[0] - 5.0, at[1] - 5.0], [at[0] + 5.0, at[1] + 5.0]);
        assert!(outlines.in_box(&camera, low, high, SIZE).is_empty());
        assert!(outlines.in_box(&camera, high, low, SIZE).is_empty());
    }

    #[test]
    fn selected_objects_change_colour() {
        let (outlines, line, _) = scene();
        let plain = scene_lines(&outlines, &[]);
        let marked = scene_lines(&outlines, &[line]);
        assert_eq!(plain.len(), marked.len());
        assert!(marked.iter().any(|v| v.colour == SELECTED_COLOUR));
        assert!(!plain.iter().any(|v| v.colour == SELECTED_COLOUR));
    }

    #[test]
    fn bounds_surround_everything() {
        let (outlines, _, _) = scene();
        let (low, high) = outlines.bounds().unwrap();
        assert!(low[0] <= -20.0 && high[0] >= 20.0 && high[1] >= 45.0 - 1e-6);
    }

    #[test]
    fn preview_runs_through_its_points_to_the_cursor() {
        let lines = preview_lines(&[[0.0; 3], [1.0, 0.0, 0.0]], Some([2.0, 0.0, 0.0]));
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3].position, [2.0, 0.0, 0.0]);
        assert!(preview_lines(&[], Some([2.0, 0.0, 0.0])).is_empty());
    }

    #[test]
    fn a_click_inside_a_face_picks_the_nearest_solid() {
        let mut doc = Document::default();
        let low = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [400.0, 400.0, 40.0]).unwrap());
        let high = doc
            .add_body(curva_kernel::Body::cuboid([0.0, 0.0, 100.0], [400.0, 400.0, 40.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let at = camera.project([200.0, 200.0, 0.0], SIZE).unwrap();
        // Away from every edge, the box nearer the camera is taken.
        assert_eq!(outlines.pick(&camera, at, SIZE, 6.0), Some(high));
        assert_ne!(outlines.pick(&camera, at, SIZE, 6.0), Some(low));
        // A crossing box wholly inside the faces takes both boxes.
        let (a, b) = (
            camera.project([150.0, 150.0, 0.0], SIZE).unwrap(),
            camera.project([250.0, 250.0, 0.0], SIZE).unwrap(),
        );
        let crossed = outlines.in_box(&camera, [b[0], a[1]], [a[0], b[1]], SIZE);
        assert!(crossed.contains(&low) && crossed.contains(&high));
    }

    #[test]
    fn a_snap_behind_a_face_is_offered_only_in_wireframe() {
        let mut doc = Document::default();
        doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [400.0, 400.0, 40.0]).unwrap());
        doc.add(Curve::line([150.0, 200.0, -10.0], [200.0, 200.0, -10.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let camera = StandardView::Top.camera();
        let at = camera.project([200.0, 200.0, -10.0], SIZE).unwrap();
        let shaded = crate::SnapSet {
            visible_only: true,
            ..crate::SnapSet::default()
        };
        assert!(outlines.snap(&camera, at, SIZE, &shaded, None).is_none());
        let wire = crate::SnapSet::default();
        let hit = outlines.snap(&camera, at, SIZE, &wire, None).unwrap();
        assert_eq!(hit.point, [200.0, 200.0, -10.0]);
    }

    /// A sphere's only edge is a seam: shaded views, where the shading shows the sphere, leave
    /// it out; wireframe keeps it, so the sphere does not vanish there.
    #[test]
    fn shaded_views_leave_seams_out_and_wireframe_keeps_them() {
        let mut doc = curva_document::Document::default();
        doc.add_body(curva_kernel::Body::sphere([0.0; 3], 10.0).unwrap());
        let outlines = Outlines::of(doc.state());
        assert!(shaded_object_lines(&outlines, &[]).is_empty());
        assert!(!object_lines(&outlines, &[]).is_empty());
        let parts = SceneParts::of(&outlines);
        assert!(parts.shaded_lines.is_empty());
        assert!(!parts.lines.is_empty());
    }

    /// After an edit elsewhere, an unchanged instance keeps what it drew last time instead of being
    /// worked out again; when its block changes, it is drawn again.
    #[test]
    fn an_unchanged_instance_is_not_drawn_again_until_its_block_changes() {
        let mut doc = Document::default();
        let line = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let (definition, instance) = doc.define_block("B".into(), [0.0; 3], &[line]);
        let mut cache = MeshCache::default();
        let points_of = |outlines: &Outlines| {
            let (_, points) = outlines
                .objects
                .iter()
                .find(|(id, _)| *id == instance)
                .unwrap();
            std::sync::Arc::clone(points)
        };
        let first = points_of(&Outlines::of_cached(doc.state(), &mut cache));
        doc.add(Curve::line([0.0, 5.0, 0.0], [1.0, 5.0, 0.0]).unwrap());
        let second = points_of(&Outlines::of_cached(doc.state(), &mut cache));
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        let mut objects = doc.definitions()[0].objects.clone();
        objects[0].shape =
            curva_document::Shape::Curve(Curve::line([0.0; 3], [20.0, 0.0, 0.0]).unwrap());
        doc.set_definition_objects(definition, objects);
        let third = points_of(&Outlines::of_cached(doc.state(), &mut cache));
        assert_eq!(third.last(), Some(&[20.0, 0.0, 0.0]));
    }

    /// For the window, two instances of a block and a moved copy of a solid come as copies of
    /// one shape each at its own place, with no triangles written out (#153); pictures still
    /// get every triangle.
    #[test]
    fn instances_and_copies_share_one_shape() {
        let mut doc = curva_document::Document::default();
        let seat = doc.add_body(curva_kernel::Body::cuboid([0.0; 3], [1.0, 1.0, 0.2]).unwrap());
        let (definition, _) = doc.define_block("Seat".to_owned(), [0.0; 3], &[seat]);
        doc.insert_block(
            definition,
            curva_document::blocks::moved_by([5.0, 0.0, 0.0]),
        );
        let outlines = Outlines::of(doc.state());
        let parts = SceneParts::shared(&outlines);
        assert!(parts.faces.is_empty());
        let [a, b] = &parts.shapes[..] else {
            panic!("two copies: {:?}", parts.shapes.len());
        };
        assert!(std::sync::Arc::ptr_eq(&a.mesh, &b.mesh), "one shape");
        assert_ne!(a.object, b.object);
        assert_eq!(b.placement[3][0] - a.placement[3][0], 5.0);
        assert!(!SceneParts::of(&outlines).faces.is_empty());
        assert!(SceneParts::of(&outlines).shapes.is_empty());
        let without = scene_shapes_without(&parts.shapes, &parts.objects, &parts.objects[..1]);
        assert_eq!(without.len(), 1, "a hidden instance leaves one copy");
    }

    /// After an edit elsewhere, a dimension keeps what it drew last time; when the model's unit
    /// or length display changes, it is drawn again with the value it shows.
    #[test]
    fn an_unchanged_dimension_is_not_drawn_again_until_its_settings_change() {
        use curva_document::annotation::{Annotation, Dimension, DimensionKind};
        let mut doc = Document::default();
        let dimension = doc.add_annotation(Annotation::Dimension(Dimension {
            kind: DimensionKind::Horizontal,
            first: [0.0; 3],
            second: [100.0, 0.0, 0.0],
            line: [50.0, 20.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: doc.current_annotation_style(),
            text: None,
        }));
        let mut cache = MeshCache::default();
        let points_of = |outlines: &Outlines| {
            let (_, points) = outlines
                .objects
                .iter()
                .find(|(id, _)| *id == dimension)
                .unwrap();
            std::sync::Arc::clone(points)
        };
        let first = points_of(&Outlines::of_cached(doc.state(), &mut cache));
        let line = doc.add(Curve::line([0.0, 50.0, 0.0], [1.0, 50.0, 0.0]).unwrap());
        let second = points_of(&Outlines::of_cached(doc.state(), &mut cache));
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        // Its value as drawn follows the unit and the length display.
        doc.set_unit(curva_document::Unit::Metres, false);
        drawn_alike(&doc, &mut cache);
        assert!(!std::sync::Arc::ptr_eq(
            &second,
            &points_of(&Outlines::of_cached(doc.state(), &mut cache))
        ));
        doc.set_length_display(curva_document::units::LengthDisplay::default());
        drawn_alike(&doc, &mut cache);
        doc.translate(&[line], [0.0, 1.0, 0.0]);
        drawn_alike(&doc, &mut cache);
    }
}
