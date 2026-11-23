//! Curva's mesh and SubD objects (spec `docs/specs/mesh-and-subd.md`).
//!
//! This crate is the mesh stream's: it holds Curva's own polygon mesh and SubD cage, and it is the
//! only place that calls the outside mesh libraries (manifold-rust, subdiv-kernels, quadrs), so an
//! upgrade or a swap touches nothing else (decision 0032).

pub mod boolean;
mod check;
mod convert;
mod fill;
mod from_body;
mod insert_edge;
mod insert_point;
mod join;
mod measure;
mod mesh;
mod mesh_bevel;
mod mesh_unweld;
mod nurbs;
mod pack;
mod primitives;
mod quad_remesh;
mod quad_symmetry;
mod reduce;
mod repair;
mod settings;
mod smooth;
mod subd;
mod subd_bevel;
mod subd_bevel_creases;
mod subd_edit;
mod subd_from_body;
mod subd_loft;
mod subd_merge;
mod subd_offset;
mod subd_parts;
mod subd_primitives;
mod to_nurbs;
mod to_subd;
mod transform;
mod triangulate;
mod weld_angle;

pub use check::MeshCheck;
pub use curva_kernel::Point;
pub use from_body::density_tolerances;
pub use insert_edge::{InsertAmount, InsertSide};
pub use mesh::{Attribute, Mesh, MeshError};
pub use nurbs::MeshToNurbOptions;
pub use primitives::{Frame, SphereStyle};
pub use quad_remesh::{HARD_EDGE_ANGLE, QuadRemeshError, QuadRemeshOptions, QuadTarget};
pub use settings::{DEFAULT_SUBD_LEVEL, MOST_SUBD_LEVEL, MeshPreset, MeshSettings};
pub use smooth::{LiveSurface, MAX_LEVELS, Patch, SmoothError};
pub use subd::{Sharpness, SubD, SubDDisplay, VertexTag};
pub use subd_bevel::BevelAmount;
pub use subd_edit::{FillStyle, StitchAt};
pub use subd_loft::loft_ring;
pub use to_nurbs::ToNurbsError;
pub use to_subd::ToSubdOptions;
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
