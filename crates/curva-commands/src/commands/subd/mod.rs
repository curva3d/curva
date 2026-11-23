//! SubD commands (the mesh stream's folder, `AGENTS.md`; spec `docs/specs/command-groups.md`).
//! The SubD primitives share their picking with the mesh primitives (`commands/mesh/primitives.rs`).

mod bevel;
mod bridge;
mod crease;
mod display_toggle;
mod fill;
mod insert_edge;
mod insert_point;
mod merge_faces;
mod offset_subd;
mod parts;
mod stitch;
mod to_nurbs;
mod to_subd;
mod weld;

use crate::commands::mesh::primitives::{
    SubDBox, SubDCone, SubDCylinder, SubDPlane, SubDSphere, SubDTorus,
};
use bevel::Bevel;
use bridge::Bridge;
use crease::{Crease, RemoveCrease};
use display_toggle::SubDDisplayToggle;
use fill::Fill;
use insert_edge::InsertEdge;
use insert_point::InsertPoint;
use merge_faces::MergeFaces;
use offset_subd::OffsetSubD;
use stitch::Stitch;
use to_nurbs::ToNurbs;
use to_subd::ToSubD;
use weld::{UnweldEdge, WeldEdge};

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    recipe!("SubDBox", [], true, SubDBox),
    recipe!("SubDPlane", [], true, SubDPlane),
    recipe!("SubDSphere", [], true, SubDSphere),
    recipe!("SubDCylinder", [], true, SubDCylinder),
    recipe!("SubDCone", [], true, SubDCone),
    recipe!("SubDTorus", [], true, SubDTorus),
    recipe!("ToSubD", [], true, ToSubD),
    recipe!("SubDDisplayToggle", [], true, SubDDisplayToggle),
    recipe!("ToNURBS", [], true, ToNurbs),
    recipe!("Crease", [], true, Crease),
    recipe!("RemoveCrease", [], true, RemoveCrease),
    recipe!("Fill", [], true, Fill),
    recipe!("UnweldEdge", [], true, UnweldEdge),
    recipe!("WeldEdge", [], true, WeldEdge),
    recipe!("Stitch", [], true, Stitch),
    recipe!("Bridge", [], true, Bridge),
    recipe!("InsertEdge", [], true, InsertEdge),
    recipe!("MergeFaces", [], true, MergeFaces),
    recipe!("InsertPoint", [], true, InsertPoint),
    recipe!("OffsetSubD", [], true, OffsetSubD),
    recipe!("Bevel", [], true, Bevel),
];

/// The buttons of the SubD tab, in order (spec `docs/specs/command-groups.md` §2).
pub(crate) const TAB: &[&str] = &[
    "SubDBox",
    "SubDPlane",
    "SubDSphere",
    "SubDCylinder",
    "SubDCone",
    "SubDTorus",
    "ToSubD",
    "SubDDisplayToggle",
    "ToNURBS",
    "Crease",
    "RemoveCrease",
    "Fill",
    "UnweldEdge",
    "WeldEdge",
    "Stitch",
    "Bridge",
    "InsertEdge",
    "MergeFaces",
    "InsertPoint",
    "OffsetSubD",
    "Bevel",
];
