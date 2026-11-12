//! The list of commands, and the steps several of them share. One command per file.

mod arc;
mod area;
mod array;
mod array_crv;
mod array_polar;
mod boolean_difference;
mod boolean_intersection;
mod boolean_union;
mod bounding_box;
mod box_solid;
mod chamfer;
mod chamfer_edge;
mod circle;
mod close_crv;
mod copy;
mod cplane;
mod curve;
mod delete;
mod dir;
mod distance;
mod divide;
mod ellipse;
mod explode;
mod extrude_crv;
mod fillet;
mod fillet_corners;
mod fillet_edge;
mod flip;
mod four_view;
mod front;
mod hide;
mod history;
mod insert_knot;
mod interpcrv;
mod invert;
mod isolate;
mod join;
mod length;
mod line;
mod loft;
mod mirror;
mod move_objects;
mod named_view;
mod new;
mod offset;
mod offset_srf;
mod open;
mod orient;
mod orient3pt;
mod perspective;
mod pipe;
mod planar_srf;
mod point;
mod points_off;
mod points_on;
mod polyline;
mod project_to_cplane;
mod rebuild;
mod rectangle;
mod redo;
mod remove_control_point;
mod remove_knot;
mod rename;
mod revolve;
mod right;
mod rotate;
mod rotate3d;
mod save;
mod save_as;
mod scale;
mod scale1d;
mod scale2d;
mod scale_nu;
mod sel_all;
mod sel_body;
mod sel_crv;
mod sel_dup;
mod sel_filter;
mod sel_last;
mod sel_layer;
mod sel_name;
mod sel_none;
mod sel_prev;
mod sel_pt;
mod set_display_mode;
mod set_pt;
mod shell;
mod show;
mod smooth;
mod split;
mod sweep1;
mod top;
mod trim;
mod tween_curves;
mod undo;
mod view_history;
mod volume;
mod zoom;
mod zoom_extents;
mod zoom_selected;

use crate::{Command, Kind, Message, Request, Step};
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
    /// Its options, for commands written as a [`crate::recipe::Recipe`]; empty for the others,
    /// which are still to be moved onto the framework.
    pub options: &'static [crate::recipe::OptionDef],
    /// Whether it is written as a recipe on the command framework.
    pub recipe: bool,
}

macro_rules! listed {
    ($name:literal, [$($alias:literal),*], $repeat:literal, $undo:literal, $kind:ty) => {
        CommandDef {
            name: $name,
            aliases: &[$($alias),*],
            repeatable: $repeat,
            records_undo: $undo,
            make: || Box::<$kind>::default(),
            options: &[],
            recipe: false,
        }
    };
}

/// Lists a command written as a [`crate::recipe::Recipe`]; how every new command is listed.
macro_rules! recipe {
    ($name:literal, [$($alias:literal),*], $repeat:literal, $recipe:ty) => {
        CommandDef {
            name: $name,
            aliases: &[$($alias),*],
            repeatable: $repeat,
            records_undo: true,
            make: || Box::<crate::recipe::RecipeCommand<$recipe>>::default(),
            options: <$recipe as crate::recipe::Recipe>::OPTIONS,
            recipe: true,
        }
    };
}

/// Every command Curva has.
pub static COMMANDS: [CommandDef; 103] = [
    recipe!("Line", ["l"], true, line::Line),
    recipe!("Polyline", ["pl", "pline"], true, polyline::Polyline),
    recipe!("InterpCrv", ["interpcurve"], true, interpcrv::InterpCrv),
    recipe!("Circle", ["c", "ci"], true, circle::Circle),
    recipe!("Move", ["m"], true, move_objects::MoveObjects),
    recipe!("Copy", ["co", "cp"], true, copy::CopyObjects),
    recipe!("Rotate", ["ro"], true, rotate::Rotate),
    recipe!("Scale", ["sc"], true, scale::Scale),
    recipe!("Mirror", ["mi"], true, mirror::Mirror),
    recipe!("Array", ["ar"], true, array::Array),
    listed!("ArrayPolar", ["arp"], true, true, array_polar::ArrayPolar),
    listed!("ArrayCrv", [], true, true, array_crv::ArrayCrv),
    recipe!("Point", ["pt"], true, point::PointObject),
    listed!("Divide", [], true, true, divide::Divide),
    listed!("Orient", [], true, true, orient::Orient),
    listed!("Rebuild", [], true, true, rebuild::Rebuild),
    listed!("PointsOn", [], false, false, points_on::PointsOn),
    listed!("PointsOff", [], false, false, points_off::PointsOff),
    listed!(
        "InsertKnot",
        ["insertcontrolpoint"],
        true,
        true,
        insert_knot::InsertKnot
    ),
    listed!("RemoveKnot", [], true, true, remove_knot::RemoveKnot),
    listed!(
        "RemoveControlPoint",
        [],
        true,
        true,
        remove_control_point::RemoveControlPoint
    ),
    listed!("Smooth", [], true, true, smooth::Smooth),
    listed!("History", [], false, false, history::History),
    listed!("Flip", [], true, true, flip::Flip),
    listed!("Dir", [], true, true, dir::Dir),
    listed!("CloseCrv", [], true, true, close_crv::CloseCrv),
    listed!("Hide", ["h"], true, true, hide::Hide),
    listed!("Show", ["unisolate"], true, true, show::Show),
    listed!("SelAll", ["sa"], false, false, sel_all::SelAll),
    listed!("SelNone", ["sn"], false, false, sel_none::SelNone),
    listed!("SelCrv", [], false, false, sel_crv::SelCrv),
    listed!("SelSrf", [], false, false, sel_body::SelSrf),
    listed!("SelSolid", [], false, false, sel_body::SelSolid),
    listed!("SelPt", [], false, false, sel_pt::SelPt),
    listed!("SelName", [], false, false, sel_name::SelName),
    listed!("SelLayer", [], false, false, sel_layer::SelLayer),
    listed!("SelDup", [], false, false, sel_dup::SelDup),
    listed!("SelLast", [], false, false, sel_last::SelLast),
    listed!("SelPrev", [], false, false, sel_prev::SelPrev),
    listed!("SelFilter", [], false, false, sel_filter::SelFilter),
    listed!(
        "SelFilterToggle",
        [],
        false,
        false,
        sel_filter::SelFilterToggle
    ),
    listed!("Rename", [], true, true, rename::Rename),
    listed!("Isolate", [], true, true, isolate::Isolate),
    listed!(
        "ZoomSelected",
        ["zs"],
        false,
        false,
        zoom_selected::ZoomSelected
    ),
    listed!(
        "BoundingBox",
        ["bb"],
        true,
        false,
        bounding_box::BoundingBox
    ),
    listed!("Invert", [], false, false, invert::Invert),
    listed!("Distance", ["dist"], true, false, distance::Distance),
    listed!("Length", ["len"], true, false, length::Length),
    listed!("Area", [], true, false, area::Area),
    listed!("Volume", [], true, false, volume::Volume),
    listed!("SetPt", [], true, true, set_pt::SetPt),
    listed!(
        "ProjectToCPlane",
        [],
        true,
        true,
        project_to_cplane::ProjectToCPlane
    ),
    recipe!("Join", ["j"], true, join::Join),
    recipe!("Explode", ["x"], true, explode::Explode),
    recipe!("Split", [], true, split::Split),
    listed!("Trim", ["tr"], true, true, trim::Trim),
    recipe!("Offset", [], true, offset::Offset),
    recipe!("Fillet", [], true, fillet::Fillet),
    listed!(
        "FilletCorners",
        [],
        true,
        true,
        fillet_corners::FilletCorners
    ),
    listed!("TweenCurves", [], true, true, tween_curves::TweenCurves),
    listed!("Orient3Pt", [], true, true, orient3pt::Orient3Pt),
    recipe!("Chamfer", [], true, chamfer::Chamfer),
    recipe!("Ellipse", ["el"], true, ellipse::Ellipse),
    listed!("Rotate3D", [], true, true, rotate3d::Rotate3D),
    listed!("Scale1D", [], true, true, scale1d::Scale1D),
    listed!("Scale2D", [], true, true, scale2d::Scale2D),
    listed!("ScaleNU", [], true, true, scale_nu::ScaleNu),
    recipe!("Arc", ["a"], true, arc::Arc),
    recipe!("Rectangle", ["rec"], true, rectangle::Rectangle),
    recipe!("Box", [], true, box_solid::BoxSolid),
    recipe!("ExtrudeCrv", [], true, extrude_crv::ExtrudeCrv),
    recipe!("PlanarSrf", [], true, planar_srf::PlanarSrf),
    recipe!("Revolve", [], true, revolve::Revolve),
    recipe!("Loft", [], true, loft::Loft),
    recipe!("Sweep1", [], true, sweep1::Sweep1),
    recipe!("Pipe", [], true, pipe::Pipe),
    recipe!("OffsetSrf", [], true, offset_srf::OffsetSrf),
    recipe!("FilletEdge", [], true, fillet_edge::FilletEdge),
    recipe!("ChamferEdge", [], true, chamfer_edge::ChamferEdge),
    recipe!("Shell", [], true, shell::Shell),
    recipe!("BooleanUnion", [], true, boolean_union::BooleanUnion),
    recipe!(
        "BooleanDifference",
        [],
        true,
        boolean_difference::BooleanDifference
    ),
    recipe!(
        "BooleanIntersection",
        [],
        true,
        boolean_intersection::BooleanIntersection
    ),
    listed!("Delete", ["del", "erase"], false, true, delete::Delete),
    listed!("Undo", [], false, false, undo::Undo),
    listed!("Redo", [], false, false, redo::Redo),
    listed!("Save", [], false, false, save::Save),
    listed!("SaveAs", [], false, false, save_as::SaveAs),
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
    listed!("UndoView", [], false, false, view_history::UndoView),
    listed!("RedoView", [], false, false, view_history::RedoView),
    listed!("Top", [], false, false, top::Top),
    listed!("Front", [], false, false, front::Front),
    listed!("Right", [], false, false, right::Right),
    listed!("Perspective", [], false, false, perspective::Perspective),
    listed!("CPlane", [], false, false, cplane::SetCPlane),
    recipe!("Curve", [], true, curve::CurveCommand),
    listed!("Zoom", [], false, false, zoom::Zoom),
    listed!("NamedView", ["nv"], false, false, named_view::NamedView),
    listed!(
        "SetDisplayMode",
        [],
        false,
        false,
        set_display_mode::SetDisplayMode
    ),
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

/// An answer a step did not ask for, for the framework in `recipe.rs`.
pub(crate) fn unexpected_step() -> Step {
    unexpected()
}

/// The centre and radius of the circle through three points, on the plane with axes `x` and `y`
/// through the first point. `None` when the points lie in a line.
pub(crate) fn circle_through(
    a: Point,
    b: Point,
    c: Point,
    [x, y]: [Point; 2],
) -> Option<(Point, f64)> {
    let along = |p: Point| {
        let d = curva_kernel::sub(p, a);
        [curva_kernel::dot(d, x), curva_kernel::dot(d, y)]
    };
    let ([bx, by], [cx, cy]) = (along(b), along(c));
    let d = 2.0 * (bx * cy - by * cx);
    if d.abs() < 1e-12 {
        return None;
    }
    let (b2, c2) = (bx * bx + by * by, cx * cx + cy * cy);
    let (u, v) = ((cy * b2 - by * c2) / d, (bx * c2 - cx * b2) / d);
    let centre: Point = std::array::from_fn(|i| a[i] + u * x[i] + v * y[i]);
    Some((centre, u.hypot(v)))
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
        .flat_map(|object| object.shape.outlines())
        .map(|curve| change(&curve))
        .collect()
}

/// The surfaces and solids among chosen objects, with their ids.
fn bodies_of(doc: &curva_document::Document, ids: &[u64]) -> Vec<(u64, curva_kernel::Body)> {
    doc.state()
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .filter_map(|object| Some((object.id, object.shape.body()?.clone())))
        .collect()
}

/// The curves of chosen objects, with their ids; surfaces and solids are left out.
fn curves_of(doc: &curva_document::Document, ids: &[u64]) -> Vec<(u64, curva_kernel::Curve)> {
    doc.state()
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .filter_map(|object| Some((object.id, object.curve()?.clone())))
        .collect()
}

/// The curve passing within `reach` of `at`, if any, for commands that pick a curve.
pub(crate) fn curve_at(
    doc: &curva_document::Document,
    at: Point,
    reach: f64,
) -> Result<Option<(u64, curva_kernel::Curve)>, KernelError> {
    for object in &doc.state().objects {
        if let Some(curve) = object.curve()
            && curva_kernel::gap(curve.closest_point(at)?, at) <= reach
        {
            return Ok(Some((object.id, curve.clone())));
        }
    }
    Ok(None)
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
    const PREVIEW_STEPS: [(&str, &[&str]); 30] = [
        ("Line", &["0,0"]),
        ("Polyline", &["0,0", "9,0"]),
        ("InterpCrv", &["0,0", "9,0"]),
        ("Curve", &["0,0", "9,0"]),
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
        ("ScaleNU", &["0,0"]),
        ("Point", &[]),
        ("Orient", &["0,0"]),
        ("SetPt", &["SetX", "SetZ"]),
        ("Orient3Pt", &["0,0", "2,0", "0,2", "5,0,0", "5,2,0"]),
        ("Smooth", &[]),
        ("Dir", &[]),
        ("Box", &["0,0", "5,5"]),
        ("ExtrudeCrv", &[]),
        ("Revolve", &["0,0", "0,9"]),
        ("Pipe", &[]),
    ];

    /// These finish at once when objects are selected, so there is no moment to show a preview.
    const NOTHING_TO_PREVIEW: [&str; 14] = [
        "Delete",
        "Join",
        "Explode",
        "Split",
        "Flip",
        "CloseCrv",
        "Hide",
        "Show",
        "Rename",
        "Isolate",
        "ProjectToCPlane",
        "PlanarSrf",
        "Loft",
        "BooleanUnion",
    ];

    /// Commands whose preview needs its own setup, tested in their own files.
    const PREVIEW_TESTED_ELSEWHERE: [&str; 18] = [
        "Trim",
        "Fillet",
        "Chamfer",
        "Divide",
        "ArrayCrv",
        "Rebuild",
        "FilletCorners",
        "TweenCurves",
        "InsertKnot",
        "RemoveKnot",
        "RemoveControlPoint",
        "Sweep1",
        "BooleanDifference",
        "BooleanIntersection",
        "OffsetSrf",
        "FilletEdge",
        "ChamferEdge",
        "Shell",
    ];

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

    /// The option names written in backticks in a cell of the reference table.
    fn named_options(cell: &str) -> Vec<&str> {
        cell.split('`').skip(1).step_by(2).collect()
    }

    #[test]
    fn the_rhino_reference_matches_the_commands() {
        let table = include_str!("../../../../docs/commands/rhino-reference.md");
        let mut checked = 0;
        for row in table.lines().filter(|line| line.starts_with("| ")) {
            let cells: Vec<&str> = row.trim_matches('|').split('|').map(str::trim).collect();
            let [name, _steps, rhino, curva, status] = cells.as_slice() else {
                continue;
            };
            if *name == "Command" {
                continue;
            }
            let Some(def) = COMMANDS.iter().find(|def| def.name == *name) else {
                assert_eq!(*status, "missing", "{name} is listed but not a command");
                continue;
            };
            // Only commands written on the framework declare their options, so only they are
            // checked; the others' rows are the audit for moving them.
            if !def.recipe {
                assert_ne!(
                    *status, "done",
                    "{name} is marked done but is not on the framework"
                );
                continue;
            }
            let has = |option: &str| def.options.iter().any(|o| o.name == option);
            for option in named_options(curva) {
                assert!(
                    has(option),
                    "{name}: the table says Curva has {option}, but it does not"
                );
            }
            for option in def.options {
                assert!(
                    named_options(curva).contains(&option.name),
                    "{name}: {} is in Curva but not under In Curva in the table",
                    option.name
                );
            }
            if *status == "done" {
                for option in named_options(rhino) {
                    assert!(has(option), "{name} is marked done but lacks {option}");
                }
            }
            checked += 1;
        }
        assert!(checked >= 3, "the reference table was not read");
        // The audit covers every command Curva has.
        for def in &COMMANDS {
            assert!(
                table.contains(&format!("| {} |", def.name)),
                "{} has no row in docs/commands/rhino-reference.md",
                def.name
            );
        }
    }

    #[test]
    fn every_option_offered_can_be_typed() {
        let fresh = || {
            let mut doc = curva_document::Document::default();
            let id = doc.add(curva_kernel::Curve::line([0.0; 3], [4.0, 0.0, 0.0]).unwrap());
            doc.select(&[id]);
            doc
        };
        for def in COMMANDS.iter().filter(|def| def.recipe) {
            let mut probe = crate::Processor::default();
            probe.submit(&mut fresh(), def.name);
            let offered: Vec<&str> = probe.options().iter().map(|(name, _)| *name).collect();
            for option in offered {
                let mut doc = fresh();
                let mut processor = crate::Processor::default();
                processor.submit(&mut doc, def.name);
                let before = processor.log().len();
                processor.submit(&mut doc, option);
                let said: Vec<&String> = processor.log().iter().skip(before).collect();
                assert!(
                    said.iter()
                        .all(|line| !line.starts_with("Not a") && !line.contains("does not fit")),
                    "{} {option}: {said:?}",
                    def.name
                );
            }
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
            doc.state().objects[0].curve().unwrap().sample().unwrap()[0],
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
        assert_eq!(
            completions("po"),
            ["Polyline", "Point", "PointsOn", "PointsOff"]
        );
        assert!(completions("").is_empty());
    }

    #[test]
    fn completion_finds_aliases_and_puts_an_exact_match_first() {
        assert_eq!(completions("pl"), ["Polyline", "PlanarSrf"]);
        assert_eq!(
            completions("c"),
            [
                "Circle",
                "Copy",
                "CloseCrv",
                "Chamfer",
                "ChamferEdge",
                "CPlane",
                "Curve"
            ]
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

/// Solids and surfaces at most whose changed copies a preview draws ghosted; beyond that, only
/// their outlines, so moving many stays smooth.
const MOST_GHOSTED_BODIES: usize = 8;

/// A move or a turn, mirror or resize, applied alike to shapes and outlines.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Change {
    /// A move by a vector.
    Move(Point),
    /// A turn, mirror or resize.
    Map(curva_kernel::Similarity),
}

impl Change {
    fn shape(&self, shape: &curva_document::Shape) -> curva_document::Shape {
        match self {
            Self::Move(by) => shape.translated(*by),
            Self::Map(change) => shape.mapped(change),
        }
    }

    fn curve(&self, curve: &curva_kernel::Curve) -> curva_kernel::Curve {
        match self {
            Self::Move(by) => curve.translated(*by),
            Self::Map(change) => curve.mapped(change),
        }
    }
}

/// The objects `ids` changed by `change`: in place, or as copies on their layers when `copy`.
/// Each copy is passed to `history` with its source, for a copy that should follow it. For the
/// preview, many solids show as outlines only.
pub(crate) fn changed(
    cx: &crate::recipe::Context<'_>,
    ids: &[u64],
    changes: &[Change],
    copy: bool,
    history: impl Fn(u64, &Change) -> Option<curva_document::Recipe>,
) -> crate::recipe::Made {
    let mut made = crate::recipe::Made::default();
    let objects: Vec<&curva_document::Object> = cx
        .doc
        .state()
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .collect();
    let bodies = objects.iter().filter(|o| o.shape.body().is_some()).count() * changes.len();
    let light = cx.previewing && bodies > MOST_GHOSTED_BODIES;
    for change in changes {
        for object in objects.iter().take(if cx.previewing {
            MOST_PREVIEW_OBJECTS
        } else {
            usize::MAX
        }) {
            if light && object.shape.body().is_some() {
                made.guides
                    .extend(object.shape.outlines().iter().map(|c| change.curve(c)));
                continue;
            }
            let shape = change.shape(&object.shape);
            if copy {
                made.derived.push(crate::recipe::Derived {
                    shape,
                    from: object.id,
                    history: history(object.id, change),
                });
            } else {
                made.replace.push((object.id, shape));
            }
        }
    }
    made
}
