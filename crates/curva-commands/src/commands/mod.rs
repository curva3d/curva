//! The list of commands, and the steps several of them share. One command per file.

mod arc;
mod circle;
mod copy;
mod delete;
mod four_view;
mod front;
mod interpcrv;
mod line;
mod mirror;
mod move_objects;
mod new;
mod open;
mod perspective;
mod polyline;
mod rectangle;
mod redo;
mod right;
mod rotate;
mod save;
mod scale;
mod top;
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
pub static COMMANDS: [CommandDef; 23] = [
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
    const PREVIEW_STEPS: [(&str, &[&str]); 11] = [
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
    ];

    /// Delete has nothing to preview: what it will remove is already highlighted.
    const NOTHING_TO_PREVIEW: [&str; 1] = ["Delete"];

    #[test]
    fn every_command_that_changes_geometry_is_listed_for_a_preview() {
        for def in COMMANDS.iter().filter(|def| def.records_undo) {
            assert!(
                NOTHING_TO_PREVIEW.contains(&def.name)
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
    #[test]
    fn completion_offers_matching_names() {
        assert_eq!(completions("po"), ["Polyline"]);
        assert!(completions("").is_empty());
    }

    #[test]
    fn completion_finds_aliases_and_puts_an_exact_match_first() {
        assert_eq!(completions("pl"), ["Polyline"]);
        assert_eq!(completions("c"), ["Circle", "Copy"]);
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
