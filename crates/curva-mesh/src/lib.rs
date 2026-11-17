//! Curva's mesh and SubD objects (spec `docs/specs/mesh-and-subd.md`).
//!
//! This crate is the mesh stream's: it holds Curva's own polygon mesh and SubD cage, and it is the
//! only place that calls the outside mesh libraries (manifold-rust, subdiv-kernels, quadrs), so an
//! upgrade or a swap touches nothing else (decision 0032).

pub mod boolean;
mod check;
mod convert;
mod measure;
mod mesh;
mod nurbs;
mod primitives;
mod reduce;
mod repair;
mod smooth;
mod subd;
mod subd_primitives;
mod to_nurbs;
mod transform;
mod triangulate;

pub use check::MeshCheck;
pub use curva_kernel::Point;
pub use mesh::{Attribute, Mesh, MeshError};
pub use primitives::{Frame, SphereStyle};
pub use smooth::{MAX_LEVELS, Patch, SmoothError};
pub use subd::{Sharpness, SubD, SubDDisplay, VertexTag};
pub use to_nurbs::ToNurbsError;
pub use triangulate::Triangles;

#[cfg(test)]
mod tests {
    /// Every `CUSTOM MESH OPERATION` mark in this crate has a row in
    /// `docs/mesh/custom-operations.md`, and every row a mark (spec `mesh-and-subd.md` §3.4).
    #[test]
    fn custom_operations_match_their_list() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let list =
            std::fs::read_to_string(root.join("../../docs/mesh/custom-operations.md")).unwrap();
        let rows: std::collections::BTreeSet<String> = list
            .lines()
            .skip_while(|line| !line.starts_with("| Operation"))
            .skip(2)
            .take_while(|line| line.starts_with("| "))
            .map(|line| line.split('|').nth(1).unwrap().trim().to_owned())
            .collect();
        let mut marks = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(root.join("src")).unwrap() {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            for line in text.lines() {
                if let Some(name) = line.trim().strip_prefix("// CUSTOM MESH OPERATION: ") {
                    marks.insert(name.trim().to_owned());
                }
            }
        }
        assert_eq!(marks, rows, "marks in the code and rows in the list differ");
    }
}
