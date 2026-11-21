//! SubD commands (the mesh stream's folder, `AGENTS.md`; spec `docs/specs/command-groups.md`).
//! The SubD primitives share their picking with the mesh primitives (`commands/mesh/primitives.rs`).

mod crease;
mod display_toggle;
mod fill;
mod parts;
mod stitch;
mod to_nurbs;
mod to_subd;
mod weld;

use crate::commands::mesh::primitives::{
    SubDBox, SubDCone, SubDCylinder, SubDPlane, SubDSphere, SubDTorus,
};
use crease::{Crease, RemoveCrease};
use display_toggle::SubDDisplayToggle;
use fill::Fill;
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
];
