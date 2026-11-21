//! The Solids tab's commands.

pub(crate) mod boolean_difference;
pub(crate) mod boolean_intersection;
pub(crate) mod boolean_split;
pub(crate) mod boolean_union;
pub(crate) mod box_solid;
pub(crate) mod box_ways;
pub(crate) mod cap;
pub(crate) mod chamfer_edge;
pub(crate) mod ellipsoid;
pub(crate) mod extrude_face;
pub(crate) mod fillet_edge;
pub(crate) mod live;
pub(crate) mod merge_all_faces;
pub(crate) mod move_face;
pub(crate) mod paraboloid;
pub(crate) mod pipe;
pub(crate) mod primitives;
pub(crate) mod pyramid;
pub(crate) mod round_ways;
pub(crate) mod shell;
pub(crate) mod volume;
pub(crate) mod wirecut;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    listed!("Volume", [], true, false, volume::Volume),
    recipe!("Box", [], true, box_solid::BoxSolid),
    recipe!("Sphere", ["sph"], true, primitives::Sphere),
    recipe!("Cylinder", ["cyl"], true, primitives::Cylinder),
    recipe!("Cone", [], true, primitives::Cone),
    recipe!("TruncatedCone", [], true, primitives::TruncatedCone),
    recipe!("Torus", [], true, primitives::Torus),
    recipe!("Tube", [], true, primitives::Tube),
    recipe!("Ellipsoid", [], true, ellipsoid::Ellipsoid),
    recipe!("BooleanSplit", [], true, boolean_split::BooleanSplit),
    recipe!("Paraboloid", [], true, paraboloid::Paraboloid),
    recipe!("Pyramid", [], true, pyramid::Pyramid),
    recipe!("MergeAllFaces", [], true, merge_all_faces::MergeAllFaces),
    recipe!("Wirecut", [], true, wirecut::Wirecut),
    listed!("EditBoolean", [], true, true, live::EditBoolean),
    listed!("BreakBoolean", [], true, true, live::BreakBoolean),
    recipe!("MoveFace", [], true, move_face::MoveFace<false>),
    recipe!("PushPull", [], true, move_face::MoveFace<true>),
    recipe!("Cap", [], true, cap::Cap),
    recipe!("ExtrudeFace", [], true, extrude_face::ExtrudeFace),
    recipe!("Pipe", [], true, pipe::Pipe),
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
];

/// The buttons of the Solids tab, in order (spec `docs/specs/command-groups.md` §2).
pub(crate) const TAB: &[&str] = &[
    "Box",
    "Sphere",
    "Cylinder",
    "Cone",
    "TruncatedCone",
    "Torus",
    "Tube",
    "Ellipsoid",
    "Paraboloid",
    "Pyramid",
    "Pipe",
    "FilletEdge",
    "ChamferEdge",
    "Shell",
    "MergeAllFaces",
    "Wirecut",
    "MoveFace",
    "PushPull",
    "Cap",
    "ExtrudeFace",
    "BooleanUnion",
    "BooleanDifference",
    "BooleanIntersection",
    "BooleanSplit",
    "EditBoolean",
    "BreakBoolean",
];
