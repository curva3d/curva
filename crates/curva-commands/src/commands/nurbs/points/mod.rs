//! Editing NURBS control points and knots: rebuilding, degree, knots and points (spec
//! `docs/specs/command-groups.md`).

pub(crate) mod change_degree;
pub(crate) mod insert_knot;
pub(crate) mod points_off;
pub(crate) mod points_on;
pub(crate) mod rebuild;
pub(crate) mod remove_control_point;
pub(crate) mod remove_knot;
pub(crate) mod set_pt;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
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
    listed!("SetPt", [], true, true, set_pt::SetPt),
    recipe!("ChangeDegree", [], true, change_degree::ChangeDegree),
];
