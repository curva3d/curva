//! Mesh commands and the meshing they share (the mesh stream's folder, `AGENTS.md`).

pub(crate) mod boolean;
pub(crate) mod check;
pub(crate) mod faces;
pub(crate) mod mesh_command;
pub mod meshing;
pub(crate) mod primitives;

pub(crate) use boolean::{
    MeshBooleanDifference, MeshBooleanIntersection, MeshBooleanSplit, MeshBooleanUnion,
};
pub(crate) use check::{
    Check, CullDegenerateMeshFaces, FillMeshHoles, MeshRepair, RebuildMeshNormals, UnifyMeshNormals,
};
pub(crate) use faces::{
    ExtractMeshFaces, MeshToNurb, QuadrangulateMesh, ReduceMesh, TriangulateMesh,
};
pub(crate) use mesh_command::MeshCommand;
pub(crate) use primitives::{MeshBox, MeshCone, MeshCylinder, MeshPlane, MeshSphere, MeshTorus};

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    recipe!("Mesh", [], true, MeshCommand),
    recipe!("MeshBox", [], true, MeshBox),
    recipe!("MeshPlane", [], true, MeshPlane),
    recipe!("MeshSphere", [], true, MeshSphere),
    recipe!("MeshCylinder", [], true, MeshCylinder),
    recipe!("MeshCone", [], true, MeshCone),
    recipe!("MeshTorus", [], true, MeshTorus),
    recipe!("Check", [], true, Check),
    recipe!("MeshRepair", [], true, MeshRepair),
    recipe!("UnifyMeshNormals", [], true, UnifyMeshNormals),
    recipe!("FillMeshHoles", [], true, FillMeshHoles),
    recipe!("CullDegenerateMeshFaces", [], true, CullDegenerateMeshFaces),
    recipe!("RebuildMeshNormals", [], true, RebuildMeshNormals),
    recipe!("ReduceMesh", [], true, ReduceMesh),
    recipe!("TriangulateMesh", [], true, TriangulateMesh),
    recipe!("QuadrangulateMesh", [], true, QuadrangulateMesh),
    recipe!("ExtractMeshFaces", [], true, ExtractMeshFaces),
    recipe!("MeshToNURB", [], true, MeshToNurb),
    recipe!("MeshBooleanUnion", [], true, MeshBooleanUnion),
    recipe!("MeshBooleanDifference", [], true, MeshBooleanDifference),
    recipe!("MeshBooleanIntersection", [], true, MeshBooleanIntersection),
    recipe!("MeshBooleanSplit", [], true, MeshBooleanSplit),
];

/// The buttons of the Mesh tab, in order (spec `docs/specs/command-groups.md` §2).
pub(crate) const TAB: &[&str] = &[
    "Mesh",
    "MeshBox",
    "MeshSphere",
    "MeshCylinder",
    "MeshCone",
    "MeshTorus",
    "MeshPlane",
    "ReduceMesh",
    "TriangulateMesh",
    "QuadrangulateMesh",
    "ExtractMeshFaces",
    "MeshBooleanUnion",
    "MeshBooleanDifference",
    "MeshBooleanIntersection",
    "MeshBooleanSplit",
    "Check",
    "MeshRepair",
    "FillMeshHoles",
    "UnifyMeshNormals",
    "RebuildMeshNormals",
    "CullDegenerateMeshFaces",
    "MeshToNURB",
];
