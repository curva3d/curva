# Custom mesh operations

Mesh work Curva does itself because its mesh libraries (manifold-rust, subdiv-kernels, quadrs) do not
do it yet (spec `docs/specs/mesh-and-subd.md` §3.4). Each row has a `// CUSTOM MESH OPERATION: <name>`
mark on the code that does it; a test (`custom_operations_match_their_list` in `curva-mesh`) fails if
a mark has no row or a row no mark.

**At each upgrade of a mesh library** (once per Curva release, spec §3.4): check each row against the
library's new version. Where the library now does the work as well (the row's tests pass on it),
switch to it, delete Curva's code and the row, and say so in the release notes. Then update
"Checked at".

| Operation | Code | Used by | The libraries today | Checked at |
|---|---|---|---|---|
| unify-normals | `curva-mesh/src/check.rs` (`Topology::orientation`) | `Check`, MeshRepair, UnifyMeshNormals | manifold-rust's `repair_orientation` turns whole closed shells, which Curva uses for that step; making the faces of a piece agree, and open meshes, are Curva's | manifold-rust 0.19.2 |
| cull-degenerate-faces | `curva-mesh/src/repair.rs` (`Mesh::without_degenerate_faces`) | MeshRepair, CullDegenerateMeshFaces | manifold-rust drops degenerate triangles only inside its own closed solids | manifold-rust 0.19.2 |
| cull-duplicate-faces | `curva-mesh/src/repair.rs` (`Mesh::without_duplicate_faces`) | MeshRepair | none | manifold-rust 0.19.2 |
| cull-unused-vertices | `curva-mesh/src/repair.rs` (`Mesh::without_unused_vertices`) | MeshRepair | none for Curva's meshes (polygons, data per corner) | manifold-rust 0.19.2 |
| weld-vertices | `curva-mesh/src/repair.rs` (`Mesh::welded`) | MeshRepair, Weld | `MeshGLP::merge` joins only the points on open edges of a triangle mesh, at its own tolerance, and does not take a distance | manifold-rust 0.19.2 |
| rebuild-normals | `curva-mesh/src/repair.rs` (`Mesh::with_rebuilt_normals`) | MeshRepair, RebuildMeshNormals | manifold-rust works out normals only for its own smoothing, by angle, as extra properties | manifold-rust 0.19.2 |
| fill-holes | `curva-mesh/src/repair.rs` (`Mesh::with_holes_filled`) | MeshRepair, FillMeshHoles | none (it only takes closed meshes) | manifold-rust 0.19.2 |
| triangles-to-quads | `curva-mesh/src/convert.rs` (`Mesh::quadrangulated`) | QuadrangulateMesh | none | manifold-rust 0.19.2 |
| reduce-mesh | `curva-mesh/src/reduce.rs` (`Mesh::reduced`) | ReduceMesh | manifold-rust's `simplify` merges edges within a distance, on closed triangle meshes only, and takes no face count | manifold-rust 0.19.2 |
| subd-limit-fit | `curva-mesh/src/smooth.rs` (`SubD::fitted`) | SubDSphere, SubDTorus | subdiv-kernels' `InverseStencilMap` says which outputs a cage point moves, not where to put cage points so their limit points land on given places | subdiv-kernels 0.1.0 |
| subd-feature-patches | `curva-mesh/src/smooth.rs` (`SubD::bezier_patches`) | ToNURBS | subdiv-kernels gives exact bicubic patches only on regular quads; around star points, creases and borders it evaluates the surface but makes no patches | subdiv-kernels 0.1.0 |
