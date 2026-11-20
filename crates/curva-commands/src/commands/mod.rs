//! The list of commands, and the steps several of them share. Commands are filed in folders by
//! kind of geometry and then by tab (spec `docs/specs/command-groups.md`): `nurbs`, `mesh`,
//! `subd`, then the tabs every kind shares. Each folder lists its own commands.

macro_rules! listed {
    ($name:literal, [$($alias:literal),*], $repeat:literal, $undo:literal, $kind:ty) => {
        $crate::commands::CommandDef {
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
        $crate::commands::CommandDef {
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

mod analyze;
mod annotate;
mod arguments;
mod edit;
mod files;
mod layouts;
pub mod mesh;
mod nurbs;
mod order;
mod organise;
mod select;
mod subd;
mod support;
mod view;

use crate::Command;
use curva_kernel::Point;
pub(crate) use support::{
    MOST_PREVIEW_OBJECTS, ask_objects, ask_point, ask_point_or_number, bodies_of, circle_through,
    curve_at, curves_of, cut_by, geometry_failed, ghost_results, ghosts, guide,
    objects_or_selection, plane_normal, unexpected, unexpected_step,
};

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

/// The buttons of each command tab, kept by the folder whose commands they are (spec
/// `docs/specs/command-groups.md` §2).
pub mod tabs {
    /// The Curves tab.
    pub const CURVES: &[&str] = super::nurbs::curves::TAB;
    /// The Surfaces tab.
    pub const SURFACES: &[&str] = super::nurbs::surfaces::TAB;
    /// The Solids tab.
    pub const SOLIDS: &[&str] = super::nurbs::solids::TAB;
    /// The Mesh tab.
    pub const MESH: &[&str] = super::mesh::TAB;
    /// The Subd tab.
    pub const SUBD: &[&str] = super::subd::TAB;
    /// The Edit tab.
    pub const EDIT: &[&str] = super::edit::TAB;
    /// The Organise tab.
    pub const ORGANISE: &[&str] = super::organise::TAB;
    /// The Annotate tab.
    pub const ANNOTATE: &[&str] = super::annotate::TAB;
    /// The Layouts tab.
    pub const LAYOUTS: &[&str] = super::layouts::TAB;
    /// The Analyze tab.
    pub const ANALYZE: &[&str] = super::analyze::TAB;
    /// The Select tab.
    pub const SELECT: &[&str] = super::select::TAB;
}

/// The command list: every command Curva has, in the order of `order.rs`.
pub struct CommandList(std::sync::LazyLock<Vec<&'static CommandDef>>);

impl CommandList {
    /// The commands, in order.
    pub fn iter(&self) -> std::iter::Copied<std::slice::Iter<'_, &'static CommandDef>> {
        self.0.iter().copied()
    }
}

impl<'a> IntoIterator for &'a CommandList {
    type Item = &'static CommandDef;
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, &'static CommandDef>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Every folder's list of commands.
const FOLDERS: [&[CommandDef]; 14] = [
    nurbs::curves::COMMANDS,
    nurbs::surfaces::COMMANDS,
    nurbs::solids::COMMANDS,
    nurbs::points::COMMANDS,
    mesh::COMMANDS,
    subd::COMMANDS,
    edit::COMMANDS,
    organise::COMMANDS,
    annotate::COMMANDS,
    layouts::COMMANDS,
    analyze::COMMANDS,
    select::COMMANDS,
    view::COMMANDS,
    files::COMMANDS,
];

/// Every command Curva has.
pub static COMMANDS: CommandList = CommandList(std::sync::LazyLock::new(|| {
    order::ORDER
        .iter()
        .map(|name| {
            FOLDERS
                .iter()
                .flat_map(|folder| folder.iter())
                .find(|def| def.name == *name)
                .unwrap_or_else(|| panic!("{name} is in order.rs but in no folder's list"))
        })
        .collect()
}));

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

/// Runs typed lines through a fresh processor, for the commands' tests.
#[cfg(test)]
fn run(doc: &mut curva_document::Document, lines: &[&str]) -> crate::Processor {
    // The formats the interface hands every model.
    doc.set_formats(curva_formats::all());
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
    fn no_command_has_a_button_in_two_tabs_and_every_mesh_and_subd_command_has_one() {
        let all = [
            tabs::CURVES,
            tabs::SURFACES,
            tabs::SOLIDS,
            tabs::MESH,
            tabs::SUBD,
            tabs::EDIT,
            tabs::ORGANISE,
            tabs::ANNOTATE,
            tabs::LAYOUTS,
            tabs::ANALYZE,
            tabs::SELECT,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for name in all.iter().flat_map(|tab| tab.iter()) {
            assert_eq!(resolve(name).map(|def| def.name), Some(*name), "{name}");
            assert!(seen.insert(*name), "{name} has a button in two tabs");
        }
        for def in mesh::COMMANDS {
            assert!(
                tabs::MESH.contains(&def.name),
                "{} has no Mesh button",
                def.name
            );
        }
        for def in subd::COMMANDS {
            assert!(
                tabs::SUBD.contains(&def.name),
                "{} has no SubD button",
                def.name
            );
        }
    }

    #[test]
    fn every_command_in_a_folder_has_one_place_in_the_order() {
        let in_folders: usize = FOLDERS.iter().map(|folder| folder.len()).sum();
        assert_eq!(
            in_folders,
            order::ORDER.len(),
            "a command missing from order.rs"
        );
        assert_eq!(COMMANDS.iter().count(), order::ORDER.len());
        let mut names = order::ORDER.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), order::ORDER.len(), "a name twice in order.rs");
    }

    #[test]
    fn a_model_in_feet_reads_and_shows_feet_and_inches() {
        let mut doc = curva_document::Document::default();
        doc.set_unit(curva_document::Unit::Feet, false);
        run(&mut doc, &["Circle", "0,0", "6in"]);
        let circle = doc.state().objects[0].curve().cloned().unwrap();
        let length = circle.length().unwrap();
        assert!((length - std::f64::consts::PI).abs() < 1e-9, "{length}");
        let processor = run(&mut doc, &["Distance", "0,0", "5'3\",0"]);
        assert!(
            processor.log().iter().any(|line| line.contains("5'-3\"")),
            "{:?}",
            processor.log()
        );
    }
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
    const PREVIEW_STEPS: [(&str, &[&str]); 74] = [
        ("SubDBox", &["0,0"]),
        ("SubDPlane", &["0,0"]),
        ("SubDSphere", &["0,0"]),
        ("SubDCylinder", &["0,0"]),
        ("SubDCone", &["0,0"]),
        ("SubDTorus", &["0,0"]),
        ("MeshBox", &["0,0"]),
        ("MeshPlane", &["0,0"]),
        ("MeshSphere", &["0,0"]),
        ("MeshCylinder", &["0,0"]),
        ("MeshCone", &["0,0"]),
        ("MeshTorus", &["0,0"]),
        ("HandleCurve", &["0,0", "1,1", "5,0"]),
        ("Sketch", &["0,0"]),
        ("Conic", &["0,0", "10,0"]),
        ("Hyperbola", &["0,0", "2,0"]),
        ("OffsetMultiple", &["1"]),
        ("Pyramid", &["0,0", "2,0"]),
        ("ExtrudeCrvToPoint", &[]),
        ("Parabola", &["0,0"]),
        ("Paraboloid", &["0,0"]),
        ("Points", &[]),
        ("Lines", &["0,0"]),
        ("Slot", &["0,0"]),
        ("Ellipsoid", &["0,0"]),
        ("Sphere", &["0,0"]),
        ("Cylinder", &["0,0"]),
        ("Cone", &["0,0"]),
        ("TruncatedCone", &["0,0"]),
        ("Torus", &["0,0"]),
        ("Tube", &["0,0"]),
        ("Polygon", &["0,0"]),
        ("Helix", &["0,0,0", "0,0,10"]),
        ("Spiral", &["0,0,0", "0,0,10", "1"]),
        ("Plane", &["0,0"]),
        ("SrfPt", &["0,0", "4,0"]),
        ("ArrayLinear", &["4", "0,0"]),
        ("Angle", &["0,0"]),
        ("Polygon", &["1,1"]),
        ("Line", &["0,0"]),
        ("Polyline", &["0,0", "9,0"]),
        ("InterpCrv", &["0,0", "9,0"]),
        ("Curve", &["0,0", "9,0"]),
        ("Dim", &["0,0", "10,0"]),
        ("DimAligned", &["0,0", "10,5"]),
        ("DimRotated", &["30", "0,0", "10,0"]),
        ("DimAngle", &["Points", "0,0", "10,0", "0,10"]),
        ("Leader", &["0,0"]),
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
    const NOTHING_TO_PREVIEW: [&str; 94] = [
        "QuadRemesh",
        "MeshBooleanUnion",
        "MeshBooleanDifference",
        "MeshBooleanIntersection",
        "MeshBooleanSplit",
        "TriangulateMesh",
        "QuadrangulateMesh",
        "ExtractMeshFaces",
        "MeshToNURB",
        "Check",
        "MeshRepair",
        "UnifyMeshNormals",
        "FillMeshHoles",
        "CullDegenerateMeshFaces",
        "RebuildMeshNormals",
        "SectionStyle",
        "Make2D",
        "Print",
        "ExportPDF",
        "Hatch",
        "HatchPatterns",
        "SetHatchBasePoint",
        "SetCurrentAnnotationStyle",
        "Dot",
        "DupAnnotationStyle",
        "Import",
        "Text",
        "ClippingPlane",
        "EnableClippingPlane",
        "DisableClippingPlane",
        "Detail",
        "Layout",
        "CopyLayout",
        "ChangeLayer",
        "MatchProperties",
        "Linetype",
        "PrintWidth",
        "ObjectColour",
        "SetUserText",
        "SetDocumentUserText",
        "SetLayoutUserText",
        "Revision",
        "Notes",
        "Purge",
        "LockSwap",
        "UnlockSelected",
        "BringToFront",
        "SendToBack",
        "BringForward",
        "SendBackward",
        "DupFaceBorder",
        "TextObject",
        "Block",
        "Insert",
        "BlockEdit",
        "ExplodeBlock",
        "ReplaceBlock",
        "EditBoolean",
        "BreakBoolean",
        "ShrinkTrimmedSrf",
        "Silhouette",
        "UnrollSrf",
        "EdgeSrf",
        "NetworkSrf",
        "Patch",
        "Cap",
        "DupBorder",
        "MergeAllFaces",
        "SimplifyCrv",
        "MakeUniform",
        "SelColor",
        "Plan",
        "Lock",
        "Unlock",
        "Group",
        "Ungroup",
        "Delete",
        "HideSwap",
        "EvaluatePt",
        "Radius",
        "What",
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
    const PREVIEW_TESTED_ELSEWHERE: [&str; 78] = [
        "ReduceMesh",
        "Mesh",
        "DimCurveLength",
        "DimArea",
        "DimVolume",
        "DimArcLength",
        "TitleBlock",
        "ScaleBar",
        "SheetIndex",
        "DimRadius",
        "DimDiameter",
        "DimOrdinate",
        "ExtractIsocurve",
        "BlendSrf",
        "MatchSrf",
        "Sweep2",
        "Twist",
        "Bend",
        "Taper",
        "Flow",
        "CageEdit",
        "Pull",
        "InterpCrvOnSrf",
        "OffsetCrvOnSrf",
        "ExtrudeFace",
        "FilletSrf",
        "ChamferSrf",
        "SplitSrf",
        "ExtendSrf",
        "Untrim",
        "Section",
        "Contour",
        "Intersect",
        "Project",
        "MoveFace",
        "PushPull",
        "Shear",
        "RebuildUV",
        "CurvatureGraph",
        "BlendCrv",
        "Match",
        "Extend",
        "Fair",
        "CurveBoolean",
        "Reparameterize",
        "Weight",
        "ExtrudeCrvTapered",
        "ExtrudeCrvAlongCrv",
        "RailRevolve",
        "ExtractSrf",
        "Wirecut",
        "Distribute",
        "ShowEdges",
        "Align",
        "ChangeDegree",
        "AreaCentroid",
        "VolumeCentroid",
        "BooleanSplit",
        "DupEdge",
        "ExtrudeSrf",
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

    /// Every `CUSTOM KERNEL OPERATION` mark in the source has a row in
    /// `docs/kernel/custom-operations.md`, and every row a mark (spec `engine-gaps.md` §1).
    #[test]
    fn custom_operations_match_their_list() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let list = std::fs::read_to_string(root.join("docs/kernel/custom-operations.md")).unwrap();
        let rows: std::collections::BTreeSet<String> = list
            .lines()
            .skip_while(|line| !line.starts_with("| Operation"))
            .skip(2)
            .take_while(|line| line.starts_with("| "))
            .map(|line| line.split('|').nth(1).unwrap().trim().to_owned())
            .collect();
        let mut marks = std::collections::BTreeSet::new();
        let mut stack = vec![root.join("crates")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for line in text.lines() {
                        if let Some(name) = line.trim().strip_prefix("// CUSTOM KERNEL OPERATION: ")
                        {
                            marks.insert(name.trim().to_owned());
                        }
                    }
                }
            }
        }
        assert_eq!(marks, rows, "marks in the code and rows in the list differ");
    }
    #[test]
    fn rhino_parity_lists_only_real_commands() {
        let table = include_str!("../../../../docs/commands/rhino-parity.md");
        let mut listed = 0;
        for line in table
            .lines()
            .filter(|l| l.starts_with("| ") && !l.starts_with("| Rhino command"))
        {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let (name, status) = (cells[1], cells[2]);
            // Every command that differs from Rhino says how (spec `rhino-parity-0.4.1.md` §1).
            assert!(
                status != "partly" || !cells[3].is_empty(),
                "{name} is partly done but its note does not say how it differs"
            );
            assert!(
                ["done", "partly", "missing", "blocked"].contains(&status),
                "{name}: unknown status {status}"
            );
            if status == "done" || status == "partly" {
                assert!(
                    resolve(name).is_some(),
                    "{name} is marked {status} but is not in Curva"
                );
            }
            listed += 1;
        }
        assert!(listed > 150, "the parity list looks short: {listed}");
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
            [
                "Polyline",
                "Point",
                "PointsOn",
                "PointsOff",
                "Polygon",
                "Points"
            ]
        );
        assert!(completions("").is_empty());
    }

    #[test]
    fn completion_finds_aliases_and_puts_an_exact_match_first() {
        assert_eq!(
            completions("pl"),
            ["Polyline", "Plane", "Plan", "PlanarSrf", "Print"]
        );
        assert_eq!(
            completions("c"),
            [
                "Circle",
                "Copy",
                "CloseCrv",
                "Chamfer",
                "Cylinder",
                "Cone",
                "ChangeDegree",
                "Conic",
                "CurveBoolean",
                "Count",
                "ChangeLayer",
                "Contour",
                "CurvatureGraph",
                "Cap",
                "Check",
                "CullDegenerateMeshFaces",
                "CageEdit",
                "ChamferSrf",
                "ChamferEdge",
                "CopyLayout",
                "ClippingPlane",
                "CPlane",
                "Curve"
            ]
        );
        assert_eq!(
            completions("co"),
            ["Copy", "Cone", "Conic", "Count", "Contour", "CopyLayout"]
        );
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
        assert_ne!(autofilled("n", 0), "New");
        assert_ne!(autofilled("ne", 0), "New");
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
        .map(|o| &**o)
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
                // A live boolean's inputs move with it, so it stays live.
                if object.live.is_some() && !cx.previewing {
                    for input in cx
                        .doc
                        .state()
                        .objects
                        .iter()
                        .filter(|o| o.input_of == Some(object.id))
                    {
                        made.replace.push((input.id, change.shape(&input.shape)));
                    }
                }
            }
        }
    }
    made
}
