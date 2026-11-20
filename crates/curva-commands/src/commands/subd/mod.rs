//! SubD commands (the mesh stream's folder, `AGENTS.md`; spec `docs/specs/command-groups.md`).
//! The SubD primitives share their picking with the mesh primitives (`commands/mesh/primitives.rs`).

mod display_toggle;
mod to_subd;

use crate::commands::mesh::primitives::{
    SubDBox, SubDCone, SubDCylinder, SubDPlane, SubDSphere, SubDTorus,
};
use display_toggle::SubDDisplayToggle;
use to_subd::ToSubD;

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
];
