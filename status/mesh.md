# Status: mesh stream

*Written only by the mesh stream, which owns the inside of `crates/curva-mesh` and its own new
mesh and SubD files elsewhere (see `AGENTS.md`). Updated at the end of every work session. Newest
first.*

## 16 and 17 November 2026 (second session, after the move to E:)

**Spec approved by the owner** (17 November 2026, `docs/specs/mesh-and-subd.md`, #246), with
these decisions:
- Stage order: A meshes and mesh tools (with STL, OBJ, PLY and `.3dm` meshes), B SubDs come in,
  C SubD modelling with QuadRemesh.
- QuadRemesh now, on quadrs (decision 0033). No well-maintained pure-Rust quad remesher exists;
  the maintained ones are C++.
- Flat or smooth SubD display is a command like any other, `SubDDisplayToggle`, with Tab as its
  shortcut.
- The Boost licence (BSL-1.0) is allowed for all of Curva (decision 0034, in `deny.toml` and
  `about.toml`); manifold-rust needs it through clipper2-rust.

**Merged:** the spec (PR #265); Stage A step 1, the mesh structure in `crates/curva-mesh`
(#268, PR #276); STL and OBJ read and written, new crate `crates/curva-meshfiles` (#269 part 1,
PR #293), checked both ways against Rhino 8; mesh booleans on manifold-rust (#272 part 1, PR #297);
`Check` for meshes and MeshRepair's main fixes (#273 part 1, PR #306), with `Check`'s counts
compared with Rhino 8's. Meshes now join faces by where their points are, as Rhino does.
ReduceMesh, TriangulateMesh, QuadrangulateMesh, ExtractMeshFaces and MeshToNURB (#274 part 1,
PR #310). ReduceMesh is quadric error edge collapse (Garland and Heckbert, 1997), Curva's own.

**In review:** PLY read and written, text and binary, with normals, colours and texture
coordinates (#275, PLY part), in `crates/curva-meshfiles`; checked both ways against Rhino 8.

**Waiting for #266:** the hook-ups that need meshes in the document: STL, OBJ and PLY in Open,
Import and Export (#269 part 2, #275), the four MeshBoolean commands (#272 part 2), the `Check`,
MeshRepair, UnifyMeshNormals, FillMeshHoles, Weld, RebuildMeshNormals and CullDegenerateMeshFaces
commands (#273 part 2), and the five commands of #274 (part 2).

**Next:** `.3dm` meshes read and written (the rest of #275), then the library parts of #270 and
#271 (spec §10). Asked of the other streams: the two shape kinds in the
document (#266, features; needed before any command or file hook-up) and drawing meshes (#267,
display).

**Found on the way**
- subdiv-kernels' weights are single precision; step 2 measures whether that is enough for
  ToNURBS (spec §3.2).
- subdiv-kernels' optional Bevy feature targets Bevy 0.19 (Curva is on 0.20.1): never enable it.
- Adding the two shape kinds touches about 30 exhaustive checks in features-owned code, so it is a
  features issue, not a one-line registration (spec §6).
- The release build's licence list (`about.toml`) lacked MIT-0, which `deny.toml` has allowed
  since Bevy came in, so release builds on main failed. Fixed in PR #265.
- Rhino's OBJ export writes no object names, so several parts come back as one mesh, and its STL
  puts all parts in one solid. Rhino's own STL import splits parts that do not touch; Curva could
  offer the same once there is an import options dialog.
- Rhino 8's mesh booleans leave open results with missing faces when the two meshes have faces
  in the same plane (two overlapping cubes: union volume 1.333 and area 7, where 1.5 and 8 are
  right, with or without a tolerance). Curva's, on manifold-rust, come out closed and right.
- manifold-rust can test only closed meshes for faces crossing each other, so `Check` says "not
  known" for open ones, where Rhino counts the crossing pairs. An open-mesh test is for later.
- Where Curva differs from Rhino on purpose: its UnifyMeshNormals turns a closed mesh that points
  wholly inwards the right way out (Rhino leaves it), and on an open mesh it keeps the way most
  faces point (Rhino sometimes keeps the fewer).
- Rhino's `Check` calls an edge of 0.0001 "extremely short" and one of 0.00015 not, at a tolerance
  of 0.001 and any mesh size; a face is degenerate when its height is under about 1.5 × 10⁻⁸ of its
  longest edge.
- Rhino 8's `Mesh.Reduce` left a 512-face UV sphere unreduced (960 triangles, asked for 200),
  though it reduces its own `CreateFromSphere` meshes; Curva's ReduceMesh reduces that sphere.
- ReduceMesh does not carry texture coordinates or colours yet; normals are worked out again.
- Rhino 8 reads text PLY at single precision (1000000.1 came in as 1000000.125) but binary PLY at
  full precision, so PLY export should default to binary. Rhino treats PLY numbers as millimetres.

## 16 November 2026

**Started.** The owner started the mesh stream (decision 0032): mesh and SubD objects, built on
manifold-rust (mesh booleans and repair) and subdiv-kernels (SubD), with Curva's own small mesh and
SubD cage structure. Nothing built yet.

**Next:** the spec, `docs/specs/mesh-and-subd.md`, for the owner's approval (issue labelled `mesh`).
No code before it is approved.
