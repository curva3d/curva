# Status: mesh stream

*Written only by the mesh stream, which owns the inside of `crates/curva-mesh` and its own new
mesh and SubD files elsewhere (see `AGENTS.md`). Updated at the end of every work session. Newest
first.*

## 18 November 2026

**SubD Weld and Unweld (#345):** UnweldEdge cuts a SubD open along inner edges into creased
borders at the same place, and WeldEdge joins them back, smooth, as Rhino 8 does. Curva's cut
box matches Rhino's own in its counts, its tags and its limit area (17.150913, to 11 decimal
places), and Rhino 8 reads Curva's as valid. A SubD can now hold such a cut: `SubD::from_cage`
keeps points at the same place apart, and the `.3dm` reader uses it.

**QuadRemesh, part 1 (#329):** the owner chose a patched copy of quadrs (decision 0036). Curva
carries quadrs 1.0.0 in `crates/curva-mesh/vendor/quadrs`, with ordered maps in four places, so the
same mesh and seed give the same quads on every run and machine. The fix was offered to the author
([vidstige/quadrs#1](https://github.com/vidstige/quadrs/issues/1)). When a release has it, Curva
goes back to the released crate and deletes the copy (steps in #349; checked at every mesh library
upgrade, `docs/mesh/custom-operations.md`). On top of quadrs, two listed custom operations: the
quads' points are moved back onto the original surface, as Rhino's are, and the count is brought
within the spec's 10% by running again with the request scaled (394 quads for 400 asked on a
sphere). A mesh or a SubD comes out. With this, every library part of Stages A, B and C is done
except QuadRemesh's parts 2 and 3 (hard edges and adaptive size; guide curves and symmetry) and
its comparison with Rhino on the test models.

**Found on the way**
- Rhino's WeldEdge welds only edges at exactly the same place. It refuses a gap of 0.0006 at a
  tolerance of 0.001, and it refuses inner creases; RemoveCrease is for those.
- Until now, reading a `.3dm` SubD joined points at the same place, so a cut SubD saved by Rhino
  came in sewn shut, and its edge sharpness could land on the wrong edges. Fixed with
  `SubD::from_cage`.
- quadrs 1.0.0 sizes its quads for the count asked, then splits each into four, so it gives about
  four times the faces asked and half the edge length; Curva asks for a quarter and twice. Its count
  is also rough (about a fifth over), and it leaves points up to 0.9 off a sphere of radius 10.
- quadrs works in single precision, and Instant Meshes is sensitive to its input's last digits: a
  mesh moved a million millimetres from the origin remeshes as well, but into different quads.

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
PLY read and written, text and binary (#275 PLY part, PR #311), checked both ways against Rhino 8.
`.3dm` meshes read and written (#275, PR #314): n-gons, normals, texture coordinates, colours
and double-precision points; Rhino 7 and 8 files read, and Rhino 8 reads Curva's as valid.
The mesh primitives' geometry (#270 part 1, PR #318): box, plane, sphere in Rhino's three styles,
cylinder and cone with both cap styles, and torus; face counts, areas and volumes match Rhino 8's.
Mesh from NURBS (#271 part 1, PR #322): the kernel's triangles as a mesh, and Rhino's density
slider mapped to tolerances (0.5 is the viewport's; other densities need #319 from the features
stream). Stage B's first step (#320, PR #325): the SubD structure (edge sharpness, Rhino's point
tags, per-face colours, flat or smooth display) and subdiv-kernels 0.1.0 in `smooth.rs`, the one
file that calls it: refined cages, the limit surface with normals, and crease and border curves
for the viewport; Rhino 8's limit surfaces of the same cages agree to 0.025%.

`.3dm` SubDs read and written (#323, PR #330): the cage with its tags, sharpness, face colours
and texture points; Rhino 8's SubDs read and smooth to Rhino's limit sizes, and Rhino 8 reads
Curva's as valid SubDs with the same volumes and areas (partly sharp edges come back smooth in
Rhino, as Curva writes Rhino 7 files). The SubD primitives' cages (#326, PR #331): box, plane,
sphere, cylinder, cone and torus placed as Rhino 8 places its own, matching Rhino's volumes, areas
and cage sizes.

ToNURBS (#327, PR #337): exact patches on regular quads and fitted ones around star points and
creases, sewn into a polysurface or solid; a torus cage gives Rhino's limit volume, a cube's
patches close up within 0.2% of its limit volume (flat patches wait on #328, features). SubD cage
edits (#332, PR #338): RemoveCrease, DeleteSubDFaces, Fill, Stitch, InsertEdge across a ring and
Bridge, and `LiveSurface`, the smooth surface worked out again for moved cage points (#334).
ToSubD from meshes (#336, PR #339): control points or through the points, mesh creases and
corners. The six finished library issues (#320, #323, #326, #327, #332, #336) are closed; their
commands are listed under "Waiting for #266".

The library parts of Stages A, B and C are done, except QuadRemesh (below). What is left needs
the other streams first (see "Asked of the other streams").

**Waiting on the owner (decided 18 November: decision 0036, above):** QuadRemesh (#329). quadrs, chosen in decision 0033 because it was said
to give the same quads every time, does not: it uses Rust's randomly seeded hash maps, so the
same mesh and seed give slightly different quads from one run to the next (504 to 524 faces for
one request). It also makes about four times the quads asked for, which Curva can allow for.
The library interface is written and parked on the local branch `329-quad-remesh` (worktree
`E:/curva-329`, not pushed) until the owner decides: a patched copy of quadrs offered upstream, its
results accepted as they are, or Curva's own QuadRemesh.

**Precision measured (spec §3.2):** subdiv-kernels' single-precision weights, applied level by
level to Curva's double-precision points measured from the centre, move a 10 m cube a million
millimetres from the origin by 0.00033 mm after three levels (the spec allows 0.01 mm). On a
regular grid the limit surface equals the bicubic B-spline to 3 × 10⁻⁸ (spec §7's exactness test).

**Waiting for #266:** every command and file hook-up: STL, OBJ, PLY and `.3dm` in Open, Import
and Export (#269 part 2, #275), MeshBoolean (#272 part 2), Check and MeshRepair's commands (#273
part 2), the commands of #274 (part 2), the mesh primitive commands with `What`, `Properties`,
`Join` and `Explode` (#270 part 2), `Mesh` (#271 part 2), and SubDs in the document with
`SubDDisplayToggle`, ToSubD, SubD export and the SubD commands.

**Asked of the other streams:** the two shape kinds in the document (#266, features; needed before
any command or file hook-up); picking parts of meshes and SubDs (#333, features); history
recipes from bodies to meshes (#335, features); meshing at a chosen density (#319, features); a
flat bicubic patch as a body (#328, features); drawing meshes (#267, display); drawing SubDs
(#324, display); SubD editing in the viewport (#334, display).

**Found on the way**
- subdiv-kernels' weights are single precision; measured as plenty (above), so no change is
  needed from its author for now.
- Rhino 8 makes a SubD from a mesh with its open border creased but no corners (an open grid's
  outer points are tagged crease, so its corners round off); Curva's `SubD::new` does the same.
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
- `curva-3dm`'s shared `Reader::compressed` and `Writer::compressed` (in `archive.rs`) put a
  buffer's size in 8 bytes, where openNURBS writes 4 (`WriteSize`) in every version. Nothing used
  them until meshes, so the mesh files carry their own correct copy; the shared ones are for the
  features stream to fix (#313).
- RhinoCommon's `Mesh.CreateFromCylinder` makes a cylinder with one cap facing in: Rhino calls it
  not oriented and gives its volume as 3.771 where it holds 5.657. Curva's face out.
- Rhino's quad sphere (MeshSphere `Quads`) places its points its own way, between an even grid on
  the cube and an equal-angle one; Curva uses equal angles, its sizes within 0.3% of Rhino's.

## 16 November 2026

**Started.** The owner started the mesh stream (decision 0032): mesh and SubD objects, built on
manifold-rust (mesh booleans and repair) and subdiv-kernels (SubD), with Curva's own small mesh and
SubD cage structure. Nothing built yet.

**Next:** the spec, `docs/specs/mesh-and-subd.md`, for the owner's approval (issue labelled `mesh`).
No code before it is approved.
