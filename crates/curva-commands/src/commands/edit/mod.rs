//! The Edit tab's commands: cutting, joining, moving, copying and deforming objects of every
//! kind (spec `docs/specs/command-groups.md`). Booleans moved to `nurbs/solids` and control
//! point and knot editing to `nurbs/points`.

pub(crate) mod align;
pub(crate) mod array;
pub(crate) mod array_crv;
pub(crate) mod array_linear;
pub(crate) mod array_polar;
pub(crate) mod bend;
pub(crate) mod cage_edit;
pub(crate) mod copy;
pub(crate) mod delete;
pub(crate) mod distribute;
pub(crate) mod explode;
pub(crate) mod flip;
pub(crate) mod flow;
pub(crate) mod history;
pub(crate) mod join;
pub(crate) mod mirror;
pub(crate) mod move_objects;
pub(crate) mod orient;
pub(crate) mod orient3pt;
pub(crate) mod project_to_cplane;
pub(crate) mod redo;
pub(crate) mod rotate;
pub(crate) mod rotate3d;
pub(crate) mod scale;
pub(crate) mod scale1d;
pub(crate) mod scale2d;
pub(crate) mod scale_nu;
pub(crate) mod shear;
pub(crate) mod smooth;
pub(crate) mod split;
mod support;
pub(crate) mod taper;
pub(crate) mod trim;
pub(crate) mod twist;
pub(crate) mod undo;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    recipe!("Move", ["m"], true, move_objects::MoveObjects),
    recipe!("Copy", ["co", "cp"], true, copy::CopyObjects),
    recipe!("Rotate", ["ro"], true, rotate::Rotate),
    recipe!("Scale", ["sc"], true, scale::Scale),
    recipe!("Mirror", ["mi"], true, mirror::Mirror),
    recipe!("Array", ["ar"], true, array::Array),
    listed!("ArrayPolar", ["arp"], true, true, array_polar::ArrayPolar),
    listed!("ArrayCrv", [], true, true, array_crv::ArrayCrv),
    listed!("Orient", [], true, true, orient::Orient),
    listed!("Smooth", [], true, true, smooth::Smooth),
    listed!("History", [], false, false, history::History),
    listed!("Flip", [], true, true, flip::Flip),
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
    recipe!("Trim", ["tr"], true, trim::Trim),
    listed!("Orient3Pt", [], true, true, orient3pt::Orient3Pt),
    listed!("Rotate3D", [], true, true, rotate3d::Rotate3D),
    listed!("Scale1D", [], true, true, scale1d::Scale1D),
    listed!("Scale2D", [], true, true, scale2d::Scale2D),
    listed!("ScaleNU", [], true, true, scale_nu::ScaleNu),
    recipe!("ArrayLinear", [], true, array_linear::ArrayLinear),
    recipe!("Align", [], true, align::Align),
    recipe!("Distribute", [], true, distribute::Distribute),
    recipe!("Shear", [], true, shear::Shear),
    recipe!("Twist", [], true, twist::Twist),
    recipe!("Bend", [], true, bend::Bend),
    recipe!("Taper", [], true, taper::Taper),
    recipe!("Flow", [], true, flow::Flow),
    recipe!("CageEdit", [], true, cage_edit::CageEdit),
    listed!("Delete", ["del", "erase"], false, true, delete::Delete),
    listed!("Undo", [], false, false, undo::Undo),
    listed!("Redo", [], false, false, redo::Redo),
];
