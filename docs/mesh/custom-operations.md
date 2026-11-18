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
| quad-remesh-onto-surface | `curva-mesh/src/quad_remesh.rs` (`onto_triangles`) | QuadRemesh | quadrs places its points by averaging and leaves them off a curved surface; Rhino's QuadRemesh leaves them on the original | quadrs 1.0.0 |
| quad-remesh-count | `curva-mesh/src/quad_remesh.rs` (`Mesh::quad_remeshed`) | QuadRemesh | quadrs' face count is rough (a fifth over is common, against the spec's 10%), so Curva runs it again with the request scaled, up to three tries | quadrs 1.0.0 |

## Patched copies of libraries

A library Curva carries as a copy with a fix, because the released version gets something wrong
that Curva needs. The fix is offered to the library's author, and **when a release has it, Curva
goes back to the release and deletes its copy** (owner's decision 0036). Check each row at every
mesh library upgrade, like the operations above.

| Library | Copy | The fix | Offered | Go back when | Checked at |
|---|---|---|---|---|---|
| quadrs 1.0.0 | `crates/curva-mesh/vendor/quadrs` | ordered maps in four places, so the same mesh and seed give the same quads on every run (`graph.rs`, `extract.rs`, `hierarchy.rs`) | [vidstige/quadrs#1](https://github.com/vidstige/quadrs/issues/1) | a quadrs release passes `the_same_seed_gives_the_same_quads`; steps in #349 | quadrs 1.0.0, 18 November 2026 |
