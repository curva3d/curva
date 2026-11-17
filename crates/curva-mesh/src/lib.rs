//! Curva's mesh and SubD objects (spec `docs/specs/mesh-and-subd.md`).
//!
//! This crate is the mesh stream's: it holds Curva's own polygon mesh and SubD cage, and it is the
//! only place that calls the outside mesh libraries (manifold-rust, subdiv-kernels, quadrs), so an
//! upgrade or a swap touches nothing else (decision 0032).

mod measure;
mod mesh;
mod subd;
mod transform;
mod triangulate;

pub use curva_kernel::Point;
pub use mesh::{Attribute, Mesh, MeshError};
pub use subd::SubD;
pub use triangulate::Triangles;
