//! The list of commands, and the steps several of them share. One command per file.

mod arc;
mod array;
mod array_crv;
mod array_polar;
mod chamfer;
mod circle;
mod close_crv;
mod copy;
mod cplane;
mod delete;
mod distance;
mod divide;
mod ellipse;
mod explode;
mod fillet;
mod flip;
mod four_view;
mod front;
mod hide;
mod interpcrv;
mod invert;
mod join;
mod length;
mod line;
mod mirror;
mod move_objects;
mod new;
mod offset;
mod open;
mod orient;
mod perspective;
mod point;
mod polyline;
mod rectangle;
mod redo;
mod right;
mod rotate;
mod rotate3d;
mod save;
mod scale;
mod scale1d;
mod scale2d;
mod sel_all;
mod sel_none;
mod show;
mod split;
mod top;
mod trim;
mod undo;
mod zoom_extents;

use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_kernel::{KernelError, Point};

/// How a command is listed.
pub struct CommandDef {
    /// Its Rhino name.
    pub name: &'static str,
    /// Its short names.
    pub aliases: &'static [&'static str],
    /// Whether Enter on an empty line runs it again.
    pub repeatable: bool,
    /// Whether what it changes becomes one undo step.
    pub records_undo: bool,
    /// Makes a fresh run of it.
    pub make: fn() -> Box<dyn Command>,
}

macro_rules! listed {
    ($name:literal, [$($alias:literal),*], $repeat:literal, $undo:literal, $kind:ty) => {
        CommandDef {
            name: $name,
            aliases: &[$($alias),*],
            repeatable: $repeat,
            records_undo: $undo,
            make: || Box::<$kind>::default(),
        }
    };
}

/// Every command Curva has.
pub static COMMANDS: [CommandDef; 50] = [
    listed!("Line", ["l"], true, true, line::Line),
    listed!("Polyline", ["pl", "pline"], true, true, polyline::Polyline),
    listed!(
        "InterpCrv",
        ["interpcurve"],
        true,
        true,
        interpcrv::InterpCrv
    ),
    listed!("Circle", ["c", "ci"], true, true, circle::Circle),
    listed!("Move", ["m"], true, true, move_objects::MoveObjects),
    listed!("Copy", ["co", "cp"], true, true, copy::CopyObjects),
    listed!("Rotate", ["ro"], true, true, rotate::Rotate),
    listed!("Scale", ["sc"], true, true, scale::Scale),
    listed!("Mirror", ["mi"], true, true, mirror::Mirror),
    listed!("Array", ["ar"], true, true, array::Array),
    listed!("ArrayPolar", ["arp"], true, true, array_polar::ArrayPolar),
    listed!("ArrayCrv", [], true, true, array_crv::ArrayCrv),
    listed!("Point", ["pt"], true, true, point::PointObject),
    listed!("Divide", [], true, true, divide::Divide),
    listed!("Orient", [], true, true, orient::Orient),
    listed!("Flip", [], true, true, flip::Flip),
    listed!("CloseCrv", [], true, true, close_crv::CloseCrv),
    listed!("Hide", ["h"], true, true, hide::Hide),
    listed!("Show", [], true, true, show::Show),
    listed!("SelAll", ["sa"], false, false, sel_all::SelAll),
    listed!("SelNone", ["sn"], false, false, sel_none::SelNone),
    listed!("Invert", [], false, false, invert::Invert),
    listed!("Distance", ["dist"], true, false, distance::Distance),
    listed!("Length", ["len"], true, false, length::Length),
    listed!("Join", ["j"], true, true, join::Join),
    listed!("Explode", ["x"], true, true, explode::Explode),
    listed!("Split", [], true, true, split::Split),
    listed!("Trim", ["tr"], true, true, trim::Trim),
    listed!("Offset", [], true, true, offset::Offset),
    listed!("Fillet", [], true, true, fillet::Fillet),
    listed!("Chamfer", [], true, true, chamfer::Chamfer),
    listed!("Ellipse", ["el"], true, true, ellipse::Ellipse),
    listed!("Rotate3D", [], true, true, rotate3d::Rotate3D),
    listed!("Scale1D", [], true, true, scale1d::Scale1D),
    listed!("Scale2D", [], true, true, scale2d::Scale2D),
    listed!("Arc", ["a"], true, true, arc::Arc),
    listed!("Rectangle", ["rec"], true, true, rectangle::Rectangle),
    listed!("Delete", ["del", "erase"], false, true, delete::Delete),
    listed!("Undo", [], false, false, undo::Undo),
    listed!("Redo", [], false, false, redo::Redo),
    listed!("Save", [], false, false, save::Save),
    listed!("Open", [], false, false, open::Open),
    listed!("New", [], false, false, new::New),
    listed!(
        "ZoomExtents",
        ["ze", "zea"],
        false,
        false,
        zoom_extents::ZoomExtents
    ),
    listed!("4View", [], false, false, four_view::FourView),
    listed!("Top", [], false, false, top::Top),
    listed!("Front", [], false, false, front::Front),
    listed!("Right", [], false, false, right::Right),
    listed!("Perspective", [], false, false, perspective::Perspective),
    listed!("CPlane", [], false, false, cplane::SetCPlane),
];

/// Finds a command by name or alias, ignoring case.
#[must_use]
pub fn resolve(typed: &str) -> Option<&'static CommandDef> {
    COMMANDS.iter().find(|def| {
        def.name.eq_ignore_ascii_case(typed)
            || def.aliases.iter().any(|a| a.eq_ignore_ascii_case(typed))
    })
}

/// Commands whose name or alias starts with the typed text, ignoring case.
///
/// An exact name or alias comes first, so `c` offers `Circle` ahead of `Copy`.
#[must_use]
pub fn completions(prefix: &str) -> Vec<&'static str> {
    let lowered = prefix.to_lowercase();
    if lowered.is_empty() {
        return Vec::new();
    }
    let exact = resolve(prefix).map(|def| def.name);
    let others = COMMANDS
        .iter()
        .filter(|def| {
            Some(def.name) != exact
                && std::iter::once(&def.name)
                    .chain(def.aliases)
                    .any(|name| name.to_lowercase().starts_with(&lowered))
        })
        .map(|def| def.name);
    exact.into_iter().chain(others).collect()
}

/// What Enter or Space should run for the typed text and the highlighted completion.
///
/// `New` discards the model and cannot be undone, so a partial name never reaches it.
#[must_use]
pub fn autofilled(typed: &str, highlighted: usize) -> String {
    match completions(typed).get(highlighted) {
        Some(&name) if name != "New" || name.eq_ignore_ascii_case(typed.trim()) => name.to_owned(),
        _ => typed.to_owned(),
    }
}

fn unexpected() -> Step {
    Step::Failed(Message::new("error.unexpected"))
}

fn geometry_failed(error: &KernelError) -> Step {
    Step::Failed(Message::with("error.geometry", vec![error.to_string()]))
}

fn ask_point(key: &'static str, preview: Vec<Point>) -> Step {
    Step::Ask(Request {
        preview,
        ..Request::new(key, Kind::Point)
    })
}

fn ask_point_or_number(key: &'static str, preview: Vec<Point>) -> Step {
    Step::Ask(Request {
        preview,
        ..Request::new(key, Kind::PointOrNumber)
    })
}

/// The direction straight out of the plane being drawn on.
fn plane_normal(doc: &curva_document::Document) -> Point {
    let [x, y] = doc.work_plane();
    curva_kernel::cross(x, y)
}

/// Most objects shown moving at once, so that a huge selection does not stall the window.
const MOST_PREVIEW_OBJECTS: usize = 2000;

/// A straight construction line for a preview, or nothing if its ends are in the same place.
fn guide(a: Point, b: Point) -> Vec<curva_kernel::Curve> {
    curva_kernel::Curve::line(a, b).into_iter().collect()
}

/// How chosen objects would look after a change, for a preview.
fn ghosts(
    doc: &curva_document::Document,
    ids: &[u64],
    change: impl Fn(&curva_kernel::Curve) -> curva_kernel::Curve,
) -> Vec<curva_kernel::Curve> {
    let chosen: std::collections::HashSet<u64> = ids.iter().copied().collect();
    doc.state()
        .objects
        .iter()
        .filter(|object| chosen.contains(&object.id))
        .take(MOST_PREVIEW_OBJECTS)
        .map(|object| change(&object.curve))
        .collect()
}

/// The curves of chosen objects, with their ids.
fn curves_of(doc: &curva_document::Document, ids: &[u64]) -> Vec<(u64, curva_kernel::Curve)> {
    doc.state()
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .map(|object| (object.id, object.curve.clone()))
        .collect()
}

/// A curve in pieces, cut wherever it crosses the cutters.
fn cut_by(
    curve: &curva_kernel::Curve,
    cutters: &[curva_kernel::Curve],
) -> Result<Vec<curva_kernel::Curve>, KernelError> {
    let mut points = Vec::new();
    for cutter in cutters {
        points.extend(curva_kernel::crossings(curve, cutter)?);
    }
    curve.cut_at(&points)
}

/// Curves worked out for a preview, without the ids they would replace.
fn ghost_results(made: Vec<(u64, curva_kernel::Curve)>) -> Vec<curva_kernel::Curve> {
    made.into_iter()
        .take(MOST_PREVIEW_OBJECTS)
        .map(|(_, curve)| curve)
        .collect()
}

fn ask_objects(key: &'static str) -> Step {
    Step::Ask(Request::new(key, Kind::Objects))
}

/// Collects points until Enter or `Close`, for commands that draw through many points.
#[derive(Default)]
struct PointRun {
    points: Vec<Point>,
}

/// What [`PointRun`] wants next.
enum RunState {
    Asking(Step),
    Finished { closed: bool },
}

impl PointRun {
    /// The points placed so far, then the cursor.
    fn through(&self, cursor: Option<Point>) -> Vec<Point> {
        self.points.iter().copied().chain(cursor).collect()
    }

    fn feed(
        &mut self,
        answer: Option<Answer>,
        start_key: &'static str,
        next_key: &'static str,
    ) -> RunState {
        match answer {
            None => {}
            Some(Answer::Point(point)) => self.points.push(point),
            Some(Answer::Empty) => return RunState::Finished { closed: false },
            Some(Answer::Option(_)) => return RunState::Finished { closed: true },
            Some(_) => return RunState::Asking(unexpected()),
        }
        let count = self.points.len();
        RunState::Asking(Step::Ask(Request {
            allow_empty: count >= 2,
            options: if count >= 3 {
                vec!["Close"]
            } else {
                Vec::new()
            },
            preview: self.points.clone(),
            ..Request::new(if count == 0 { start_key } else { next_key }, Kind::Point)
        }))
    }
}

/// Runs typed lines through a fresh processor, for the commands' tests.
#[cfg(test)]
fn run(doc: &mut curva_document::Document, lines: &[&str]) -> crate::Processor {
    let mut processor = crate::Processor::default();
    for line in lines {
        processor.submit(doc, line);
    }
    processor
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::Document;
    use curva_kernel::Curve;

    #[test]
    fn names_and_aliases_ignore_case() {
        assert_eq!(resolve("LINE").map(|d| d.name), Some("Line"));
        assert_eq!(resolve("pl").map(|d| d.name), Some("Polyline"));
        assert!(resolve("nothing").is_none());
    }

    #[test]
    fn no_two_commands_share_a_name_or_alias() {
        let mut seen = std::collections::BTreeSet::new();
        for def in &COMMANDS {
            for name in std::iter::once(&def.name).chain(def.aliases) {
                assert!(seen.insert(name.to_lowercase()), "{name} is used twice");
            }
        }
    }

    /// For each command that changes geometry, the typed steps that reach its live preview.
    /// A new command must be added here, which is how the preview rule in AGENTS.md is enforced.
    const PREVIEW_STEPS: [(&str, &[&str]); 20] = [
        ("Line", &["0,0"]),
        ("Polyline", &["0,0", "9,0"]),
        ("InterpCrv", &["0,0", "9,0"]),
        ("Circle", &["0,0"]),
        ("Move", &["0,0"]),
        ("Copy", &["0,0"]),
        ("Rotate", &["0,0", "5,0"]),
        ("Scale", &["0,0", "5,0"]),
        ("Mirror", &["0,0"]),
        ("Arc", &["0,0", "5,0"]),
        ("Rectangle", &["0,0"]),
        ("Array", &["3", "1", "1"]),
        ("Offset", &[]),
        ("ArrayPolar", &["0,0", "4"]),
        ("Ellipse", &["0,0", "5,0"]),
        ("Rotate3D", &["0,0,0"]),
        ("Scale1D", &["0,0", "2"]),
        ("Scale2D", &["0,0", "5,0"]),
        ("Point", &[]),
        ("Orient", &["0,0"]),
    ];

    /// These finish at once when objects are selected, so there is no moment to show a preview.
    const NOTHING_TO_PREVIEW: [&str; 8] = [
        "Delete", "Join", "Explode", "Split", "Flip", "CloseCrv", "Hide", "Show",
    ];

    /// Commands whose preview needs its own setup, tested in their own files.
    const PREVIEW_TESTED_ELSEWHERE: [&str; 5] = ["Trim", "Fillet", "Chamfer", "Divide", "ArrayCrv"];

    #[test]
    fn every_command_that_changes_geometry_is_listed_for_a_preview() {
        for def in COMMANDS.iter().filter(|def| def.records_undo) {
            assert!(
                NOTHING_TO_PREVIEW.contains(&def.name)
                    || PREVIEW_TESTED_ELSEWHERE.contains(&def.name)
                    || PREVIEW_STEPS.iter().any(|(name, _)| *name == def.name),
                "{} changes geometry but has no preview listed",
                def.name
            );
        }
    }

    #[test]
    fn every_command_that_changes_geometry_shows_a_preview() {
        for (name, steps) in PREVIEW_STEPS {
            let mut doc = curva_document::Document::default();
            let id = doc.add(curva_kernel::Curve::line([1.0, 1.0, 0.0], [4.0, 1.0, 0.0]).unwrap());
            doc.select(&[id]);
            let mut processor = crate::Processor::default();
            processor.submit(&mut doc, name);
            for step in steps {
                processor.submit(&mut doc, step);
            }
            let shown = processor.preview(&doc, Some([3.0, 7.0, 0.0]), "");
            assert!(!shown.is_empty(), "{name} shows no preview");
            assert_eq!(doc.state().objects.len(), 1, "{name} changed the model");
        }
    }

    fn shown_points(
        processor: &crate::Processor,
        doc: &curva_document::Document,
        typed: &str,
    ) -> Vec<Point> {
        processor
            .preview(doc, Some([0.0, 5.0, 0.0]), typed)
            .iter()
            .flat_map(|curve| curve.sample().unwrap())
            .collect()
    }

    fn with_selected_line() -> curva_document::Document {
        let mut doc = curva_document::Document::default();
        let id = doc.add(curva_kernel::Curve::line([2.0, 0.0, 0.0], [4.0, 0.0, 0.0]).unwrap());
        doc.select(&[id]);
        doc
    }

    fn shows(points: &[Point], wanted: Point) -> bool {
        points
            .iter()
            .any(|p| (0..3).all(|axis| (p[axis] - wanted[axis]).abs() < 1e-9))
    }

    #[test]
    fn rotate_previews_the_result_of_a_typed_angle() {
        let mut doc = with_selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["rotate", "0,0"] {
            processor.submit(&mut doc, typed);
        }
        assert!(shows(
            &shown_points(&processor, &doc, "90"),
            [0.0, 2.0, 0.0]
        ));
        assert_eq!(
            doc.state().objects[0].curve.sample().unwrap()[0],
            [2.0, 0.0, 0.0]
        );
    }

    #[test]
    fn rotate_previews_the_turn_to_the_cursor() {
        let mut doc = with_selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["rotate", "0,0", "9,0"] {
            processor.submit(&mut doc, typed);
        }
        assert!(shows(&shown_points(&processor, &doc, ""), [0.0, 4.0, 0.0]));
    }

    #[test]
    fn scale_previews_the_result_of_a_typed_factor() {
        let mut doc = with_selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["scale", "0,0"] {
            processor.submit(&mut doc, typed);
        }
        assert!(shows(
            &shown_points(&processor, &doc, "3"),
            [12.0, 0.0, 0.0]
        ));
    }

    #[test]
    fn scale_previews_the_ratio_to_the_cursor() {
        let mut doc = with_selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["scale", "0,0", "1,0"] {
            processor.submit(&mut doc, typed);
        }
        assert!(shows(&shown_points(&processor, &doc, ""), [20.0, 0.0, 0.0]));
    }

    #[test]
    fn a_bad_scale_factor_previews_no_result() {
        let mut doc = with_selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["scale", "0,0"] {
            processor.submit(&mut doc, typed);
        }
        assert!(!shows(
            &shown_points(&processor, &doc, "-2"),
            [-4.0, 0.0, 0.0]
        ));
    }

    #[test]
    fn move_previews_where_objects_would_land() {
        let mut doc = with_selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["move", "2,0"] {
            processor.submit(&mut doc, typed);
        }
        assert!(shows(&shown_points(&processor, &doc, ""), [0.0, 5.0, 0.0]));
    }

    #[test]
    fn mirror_previews_the_mirror_image() {
        let mut doc = with_selected_line();
        let mut processor = crate::Processor::default();
        for typed in ["mirror", "0,0"] {
            processor.submit(&mut doc, typed);
        }
        assert!(shows(&shown_points(&processor, &doc, ""), [-2.0, 0.0, 0.0]));
    }

    #[test]
    fn with_no_command_running_there_is_no_preview() {
        let doc = with_selected_line();
        assert!(
            crate::Processor::default()
                .preview(&doc, Some([1.0; 3]), "")
                .is_empty()
        );
    }

    /// A selected line on a layer that is not the current one.
    fn line_on_other_layer() -> (Document, u64, u64) {
        let mut doc = Document::default();
        let layer = doc.add_layer("Other".to_owned());
        let id = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.move_to_layer(&[id], layer);
        doc.select(&[id]);
        (doc, id, layer)
    }

    fn all_on(doc: &Document, layer: u64) -> bool {
        doc.state()
            .objects
            .iter()
            .all(|object| object.layer == layer)
    }

    #[test]
    fn copies_stay_on_the_layer_of_the_original() {
        let (mut doc, _, layer) = line_on_other_layer();
        run(&mut doc, &["copy", "0,0", "0,5", ""]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(all_on(&doc, layer));
    }

    #[test]
    fn mirrored_and_arrayed_copies_stay_on_the_layer_of_the_original() {
        let (mut doc, _, layer) = line_on_other_layer();
        run(&mut doc, &["mirror", "0,0", "10,0"]);
        assert!(all_on(&doc, layer));
        let (mut doc, _, layer) = line_on_other_layer();
        run(&mut doc, &["array", "2", "1", "1", "20"]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(all_on(&doc, layer));
    }

    #[test]
    fn offsets_exploded_parts_and_joins_stay_on_the_layer_of_the_original() {
        let (mut doc, _, layer) = line_on_other_layer();
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "offset");
        processor.click(&mut doc, [5.0, 5.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(all_on(&doc, layer));

        let mut doc = Document::default();
        let layer = doc.add_layer("Other".to_owned());
        let path = vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
        let id = doc.add(Curve::polyline(path, false).unwrap());
        doc.move_to_layer(&[id], layer);
        doc.select(&[id]);
        run(&mut doc, &["explode"]);
        assert_eq!(doc.state().objects.len(), 2);
        assert!(all_on(&doc, layer));
        let ids: Vec<u64> = doc.state().objects.iter().map(|o| o.id).collect();
        doc.select(&ids);
        run(&mut doc, &["join"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(all_on(&doc, layer));
    }

    #[test]
    fn cut_pieces_stay_on_the_layer_of_the_original() {
        let (mut doc, id, layer) = line_on_other_layer();
        let cutter = doc.add(Curve::line([3.0, -5.0, 0.0], [3.0, 5.0, 0.0]).unwrap());
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "split");
        doc.select(&[cutter]);
        processor.submit(&mut doc, "");
        let pieces = doc.state().objects.iter().filter(|o| o.id != cutter);
        assert_eq!(pieces.clone().count(), 2);
        assert!(pieces.into_iter().all(|o| o.id != id && o.layer == layer));
    }

    #[test]
    fn a_fillet_stays_on_the_layer_of_the_first_curve() {
        let mut doc = Document::default();
        let layer = doc.add_layer("Other".to_owned());
        let a = doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([10.0, 0.0, 0.0], [10.0, 10.0, 0.0]).unwrap());
        doc.move_to_layer(&[a, b], layer);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "fillet");
        processor.click(&mut doc, [1.0, 0.0, 0.0]);
        processor.click(&mut doc, [10.0, 9.0, 0.0]);
        assert_eq!(doc.state().objects.len(), 3);
        assert!(all_on(&doc, layer));
    }

    fn three_lines() -> (Document, Vec<u64>) {
        let mut doc = Document::default();
        let ids = (0..3)
            .map(|i| {
                let y = f64::from(i);
                doc.add(Curve::line([0.0, y, 0.0], [1.0, y, 0.0]).unwrap())
            })
            .collect();
        (doc, ids)
    }

    #[test]
    fn sel_all_takes_everything_that_can_be_selected() {
        let (mut doc, ids) = three_lines();
        run(&mut doc, &["selall"]);
        assert_eq!(doc.selection(), ids);
    }

    #[test]
    fn sel_none_clears_the_selection() {
        let (mut doc, ids) = three_lines();
        doc.select(&ids);
        run(&mut doc, &["selnone"]);
        assert!(doc.selection().is_empty());
    }

    #[test]
    fn invert_swaps_what_is_selected() {
        let (mut doc, ids) = three_lines();
        doc.select(&ids[..1]);
        run(&mut doc, &["invert"]);
        assert_eq!(doc.selection(), ids[1..]);
    }

    #[test]
    fn sel_all_leaves_out_hidden_objects_and_locked_layers() {
        let (mut doc, ids) = three_lines();
        doc.hide(&ids[..1]);
        let layer = doc.add_layer("Locked".to_owned());
        doc.move_to_layer(&ids[1..2], layer);
        doc.set_layer_locked(layer, true);
        run(&mut doc, &["selall"]);
        assert_eq!(doc.selection(), ids[2..]);
    }

    #[test]
    fn hidden_objects_come_back_with_show() {
        let (mut doc, ids) = three_lines();
        doc.select(&ids[..2]);
        run(&mut doc, &["hide"]);
        assert!(doc.selection().is_empty());
        assert_eq!(doc.selectable(), ids[2..]);
        run(&mut doc, &["show"]);
        assert_eq!(doc.selectable(), ids);
    }

    #[test]
    fn hiding_can_be_undone() {
        let (mut doc, ids) = three_lines();
        doc.select(&ids);
        run(&mut doc, &["hide", "undo"]);
        assert_eq!(doc.selectable(), ids);
    }
    #[test]
    fn completion_offers_matching_names() {
        assert_eq!(completions("po"), ["Polyline", "Point"]);
        assert!(completions("").is_empty());
    }

    #[test]
    fn completion_finds_aliases_and_puts_an_exact_match_first() {
        assert_eq!(completions("pl"), ["Polyline"]);
        assert_eq!(
            completions("c"),
            ["Circle", "Copy", "CloseCrv", "Chamfer", "CPlane"]
        );
        assert_eq!(completions("co"), ["Copy"]);
    }

    #[test]
    fn autofill_runs_the_highlighted_command() {
        assert_eq!(autofilled("pol", 0), "Polyline");
        assert_eq!(autofilled("c", 0), "Circle");
        assert_eq!(autofilled("c", 1), "Copy");
    }

    #[test]
    fn autofill_leaves_text_with_no_match_alone() {
        assert_eq!(autofilled("zzz", 0), "zzz");
        assert_eq!(autofilled("pol", 5), "pol");
    }

    #[test]
    fn a_partial_name_never_runs_new() {
        assert_eq!(autofilled("n", 0), "n");
        assert_eq!(autofilled("ne", 0), "ne");
        assert_eq!(autofilled("NEW", 0), "New");
    }
}
