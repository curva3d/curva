//! Mesh commands and the meshing they share (the mesh stream's folder, `AGENTS.md`).

pub(crate) mod check;
pub(crate) mod faces;
pub(crate) mod mesh_command;
pub mod meshing;
pub(crate) mod primitives;

pub(crate) use check::{
    Check, CullDegenerateMeshFaces, FillMeshHoles, MeshRepair, RebuildMeshNormals, UnifyMeshNormals,
};
pub(crate) use faces::{
    ExtractMeshFaces, MeshToNurb, QuadrangulateMesh, ReduceMesh, TriangulateMesh,
};
pub(crate) use mesh_command::MeshCommand;
pub(crate) use primitives::{MeshBox, MeshCone, MeshCylinder, MeshPlane, MeshSphere, MeshTorus};
