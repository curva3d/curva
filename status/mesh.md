# Status: mesh stream

*Written only by the mesh stream, which owns the inside of `crates/curva-mesh` and its own new
mesh and SubD files elsewhere (see `AGENTS.md`). Updated at the end of every work session. Newest
first.*

**Tidy-up first (owner's rule, 19 November 2026, `AGENTS.md`):** every stream starts each session
by completing one open `tidy-up` issue in code it owns, before its next feature; with none open, it
goes straight to features. The open issues are listed in `docs/reviews/code-health-2026-11-19.md`.
*(Added to every status page at the owner's request.)*

## 21 November 2026

**ToSubD for surfaces and polysurfaces (owner's choice: extrusions too):** `SubD::from_body` fits a SubD as
Rhino 8's ToSubD does (measured with RhinoCommon): one point per control point, curved faces fitted
through the surface at the Greville points, flat straight-sided faces on their corners, borders creased.
Rhino refuses polysurfaces but takes extrusions; Curva has no extrusions, so a prism creases its cap rims
as Rhino's extrusions do and other polysurfaces their sharp edges. A capped extruded circle, an open tube
and a flat square match Rhino's counts and sizes within 0.2 %. Not yet: trimmed curved faces, holed
flat faces, and Rhino's 48-face sphere (Curva gives 32).

**ToNURBS packs faces as Rhino 8 does (#494):** `PackFaces` (on) joins the patches into one surface per region,
measured with RhinoCommon: quads join unless a crease, a border or a line running out from a point where
other than four quads meet comes between them; only quads from four-sided cage faces join. Boxes give 6
faces, an open plane or tube 1, a cylinder with triangle caps 49, as in Rhino. A torus gives 2 where Rhino
gives 1: the kernel cannot make a solid of one face closed both ways. Patches lying flat in one plane still
cannot be sewn by the kernel (status 1007, reported to features), so the grid tests stay bent.

**Mesh wires for display (#472):** measured in Rhino 8, every mesh wire is drawn (1 px, object colour) in
Wireframe, Shaded, Ghosted and Technical, none in Rendered and Arctic; naked, crease and non-manifold
edges are off everywhere. `Mesh::wire_edges()` gives every edge once, joined by position as Rhino
joins them (an unwelded box of 24 points shows 12 wires, as in Rhino). Display draws them.

**SubD face colours on the smooth surface (for display, #324 follow-up):** `SubD::surface_coloured(levels, fallback)`
gives each quad of the smooth surface its cage face's colour at every corner (`Attribute::PerCorner`),
using the library's record of which cage face each quad comes from; a face with no colour takes
`fallback`, and a SubD without face colours comes back uncoloured. `surface` is unchanged.

## 20 November 2026 (later still)

**ToNURBS (SubDs):** measured in Rhino 8, `-ToNURBS` keeps the SubD and makes a closed polysurface;
the volumes match Rhino's to 0.02 % (20.9673 against 20.963 m³ for a 4 m cube cage of one face a
side, 50.3594 against 50.3587 for 2 × 2). Rhino packs the patches into 6 larger faces; Curva keeps
one patch per quad (24) for now.

**SubDDisplayToggle:** measured in Rhino 8, it switches every SubD in the model whatever is selected
("SubD display changed from smooth to flat. 2 SubDs changed."). Curva does the same, undoably.
The Tab shortcut is not there yet.

**ToSubD (meshes):** measured in Rhino 8, `-ToSubD` makes the mesh's points the cage, leaves
unwelded edges smooth, creases an open border with no corners, and keeps the mesh. Curva's command
sets those defaults (`UseMesh` ControlPoints, `MeshCreases` and `MeshCorners` off); `curva-mesh`'s
own `ToSubdOptions::default` keeps creases and corners on for its callers.

**The SubD tab's first commands: SubDBox, SubDPlane, SubDSphere, SubDCylinder, SubDCone and SubDTorus.**
They are picked as the mesh primitives are (the same recipes, made generic) and build their cages
with `curva-mesh`'s SubD primitives. Rhino 8's SubD defaults, measured with Enter at every option,
are far coarser than its mesh ones: a box of 2 × 2 × 2 (24 cage faces), a plane of 2 × 2, a sphere
of Quads at 1 subdivision (24), a cylinder and a cone 8 around and 4 up (48 and 40, triangle caps),
and a torus 10 by 6 (60). The test holds those counts. Measuring changed Rhino's saved options for
these commands (a spawned Rhino saves the last values used when it closes); the owner was told.

**Weld by angle (`Mesh::welded_by_angle`, for SketchUp import's Weld angle, #444):** points at the
same place join where the faces along an edge there turn by no more than the angle, as Rhino 8's
`Weld`. It matches Rhino exactly on a hexagonal prism with every face apart: 60 points become 38
up to 59°, 26 from 61° and 14 from 91°.

**QuadRemesh `Symmetry` (#329):** measured in Rhino 8, its `X` mirrors y, `Y` mirrors x and `Z` mirrors
z, about the middle of the bounding box (not the world origin). Curva cuts the mesh along each
plane, remeshes one part, puts its border exactly on the plane, mirrors it back and welds the
seam, so the halves match exactly and the result stays closed (516 quads where Rhino makes 412 at
500).

**QuadRemesh `TargetEdgeLength` (#329):** measured in Rhino 8 on a 40 × 40 sphere of radius 5: 1,064,
244 and 59 quads at 0.5, 1 and 2, so its quads come out about 1.12 times the length asked for.
quadrs' own edge length made them about 0.85 times as long (434 quads at 1), so Curva now turns
the length into Rhino's quad count over the surface's area. Rhino's adaptive size makes no
difference on a sphere, so it does not explain Curva's 1,990 quads against Rhino's 1,750 at 2,000.

**`SubD::edge_curves` (asked by display for #324):** every cage edge as it lies on the smooth
surface, joined into runs between cage points where other than two edges meet. Rhino draws a
smooth SubD's wireframe this way; `crease_curves` gave only creases and borders, so a smooth
closed SubD had nothing to draw. Found on the way: subdiv-kernels 0.1.0's own `edge_polylines`
option loses points (runs of 6 and 3 where 9 are right at three levels), so Curva tells the edges
apart by which cage face each refined face comes from.

## 20 November 2026 (later)

**The mesh panel (export-formats spec §2, on features' #387 step):** `panels/mesh.rs` opens when
`Mesh` or a mesh export asks for it. It has Rhino's presets (Coarse, Default, Smooth), the
*Fewer polygons ↔ More polygons* slider, the SubD level when SubDs are meshed, and the faces, the
farthest stray and the size in plain numbers under the slider. Advanced adds the most a facet may
turn and stand off the surface. OK keeps the settings and answers the step; Cancel ends the command.
Still to come: the rest of Rhino's detailed controls (they need fields in `MeshSettings`), the
Preview button and live preview (#407, display), and meshing for the numbers in the background
(it runs each time the settings change, which is slow on large models).

**MeshBooleanUnion, MeshBooleanDifference, MeshBooleanIntersection and MeshBooleanSplit (#272
part 2):** commands now, with icons and reference rows, on the manifold-rust booleans of part 1.
They leave what Rhino 8 leaves, measured on the same boxes:
- Union replaces the meshes with one; Difference and Intersection replace each mesh of the first
  set on its own and delete the second set; Split replaces each mesh with its outside part, adds
  its inside part and keeps the second set.
- Difference's `DeleteInput` (on at first, asked at the second step in Rhino) off keeps the cutters.
- Meshes that do not overlap are left alone, and then nothing changes.

Where Curva differs on purpose: Rhino cuts with an open mesh and leaves an open result, and its
Split leaves open parts; Curva refuses open meshes and its parts are closed.

**QuadRemesh (#329):** a command now, with an icon and a reference row, on the quadrs library of
part 1. Measured in Rhino 8: `-QuadRemesh` takes no options on the command line (it uses the
dialog's choices: 2,000 quads, adaptive size 50, hard edges kept); it keeps the input, makes a
mesh, and meshes SubDs first. Curva does the same, with `TargetQuadCount`, `DetectHardEdges` and
`ConvertToSubD`. On a 40 × 40 sphere Rhino makes 1,750 quads and Curva 1,990; Rhino's adaptive
size, `TargetEdgeLength`, symmetry and guide curves are still to come.

**SketchUp import options asked for (#432, features):** the owner wants Rhino's "SketchUp Import
Options", above all faces brought in as joined trimmed planes. Rhino's own Trimmed planes import
is not measured yet: `-Import` uses the choice saved in its dialog, which a script cannot change.
The numbers go on #432 once the choice is saved in Rhino.

## 20 November 2026

**#274's commands measured in Rhino 8 (owner: Rhino is the minimum standard).** Rhino's script
connection had hung because an earlier `-Mesh` call was waiting for input. A freshly spawned Rhino
works. The measurements changed Curva:
- QuadrangulateMesh's defaults are 5° and 2 (were 2° and 1.143 from the help pages).
- ReduceMesh joins its triangles back into quads afterwards, as Rhino does, and reaches Rhino's
  counts exactly (a 40 × 40 sphere: 1,560 at 50 %, 312 at 90 %).
- MeshToNURB makes triangles trimmed flat surfaces, as Rhino does with `TrimTriangularFaces` on.

ExtractMeshFaces already matched. The tests now hold Rhino's numbers. Rhino's `-ReduceMesh` starts
at 0 % (Enter does nothing); Curva starts at 50 %.

**ReduceMesh, TriangulateMesh, QuadrangulateMesh, ExtractMeshFaces and MeshToNURB (#274):**
commands now, with icons and reference rows. Their options come from Rhino 8's help pages; the
Rhino connection would not return script output this session, so nothing was measured.
`ReduceMesh` takes away a percentage (Enter keeps the last) or reduces to `PolygonCount`, with a
preview; Rhino's "Planar only", locked points and its quadrangulating again afterwards are not
there yet. `ExtractMeshFaces` is the first command on the mesh-parts step. `MeshToNURB`'s
`TrimTriangularFaces` and `UseNgons` are not there yet. No mesh-stream tidy-up issue was open.

**Check and MeshRepair (#273):** `Check` reports on meshes as Rhino 8 counts (or "This is a good
mesh."); other kinds of object are counted and left for the features stream. `MeshRepair` makes
the main fixes in one go, each a remembered option; `UnifyMeshNormals`, `FillMeshHoles`,
`CullDegenerateMeshFaces` and `RebuildMeshNormals` are commands of their own, as in Rhino.

**Join and Explode for meshes (#270, second part):** `Mesh::joined` combines meshes into one
without welding, as Rhino's Join does, and `Mesh::pieces` splits a mesh into the parts that share
no point, as Rhino's Explode does (both listed custom operations). Hooking them into `Join`,
`Explode` and the Properties panel is the features stream's (#414). Also, `.3dm` mesh and SubD
reading and writing were split into steps under 80 lines (#395, the tidy-up first rule).

## 19 November 2026 (evening)

**Mesh primitives (#270, first part):** `MeshBox`, `MeshPlane`, `MeshSphere`, `MeshCylinder`,
`MeshCone` and `MeshTorus` are commands, with previews, icons and reference rows. Each is picked
like its solid counterpart (Box, Sphere and so on), and Rhino's face-count options are there:
`XCount`/`YCount`/`ZCount`, `VerticalFaces`/`AroundFaces`, the sphere's `Style` and
`Subdivisions`, `Solid` and `CapFaceStyle`. Rhino 8's defaults were measured by making each with
Enter at every option, and Curva makes the same faces: 600 quads for a box; 100 for a plane; 80
quads and 20 triangles for a sphere; 120 faces for a cylinder; 110 for a cone; 100 for a torus.
The sphere's subdivision default could not be measured (2 for now). Rhino's other ways of
picking (3Point, Diagonal, AroundCurve and so on) come later. `What`, `Properties`, `Join` and
`Explode` for meshes are the rest of #270. `Mesh` (#271) is closed: done in #388.

## 19 November 2026 (later)

**Mesh panel, part 1: the settings, and the `Mesh` command** (owner: mesh panel first):
- **One set of mesh settings** (`curva_mesh::MeshSettings`) for `Mesh` and STL/OBJ/PLY export. It
  holds the density slider, Rhino's Coarse/Default/Smooth presets (Smooth is Rhino's density 0.8
  and 20°, measured in Rhino 8), a maximum angle and a maximum distance, and the SubD level.
  The last settings used are remembered, and export uses them.
- **`Mesh`** (new): choose surfaces, solids or SubDs, then type a density (Enter keeps the last).
  `MaxAngle`, `MaxDistance` and `SubDLevel` are options. The preview shows the mesh's edges, and
  meshes of surfaces and solids follow their source with History.
- **The panel itself** needs a step in the command framework that opens it: asked of the features
  stream in #387. The numbers under the slider (faces, how far facets stray, file size) are
  already worked out by `mesh_objects` and `file_bytes`. Rhino's other detailed controls wait on
  the kernel's mesher.

## 19 November 2026

**Meshes and SubDs are in the document** (features stream, #266, PR #352), so the commands and
file hook-ups that waited on it have started:
- **STL, OBJ and PLY in `Import`, `Export` and `ExportSelected`** (#269 part 2, #275). Each mesh
  in a file comes in as a named mesh object. Going out, meshes are written as they are, SubDs as
  their smooth surface (Rhino's default level), and surfaces and solids meshed at the viewport's
  density until the mesh panel comes (`export-formats.md` §2). STL and PLY are written binary.
  `Open` reads them too: a mesh file opens as a new unsaved model named after the file, as
  `.3dm` files do.
- **Any affine map and deformations for meshes and SubDs** (#354, asked for by the features
  stream): `transformed` and `deformed`, for stretched blocks, ScaleNU, the gumball's stretch and
  the deformation commands.
- **SKP:** the owner chose OpenSKP's own Rust package (no port, no separate repository; spec
  updated). The features stream is adding SketchUp import as the crate `curva-skp`; SKP export
  goes into that crate as its own file, so Curva pins OpenSKP once.

## 18 November 2026

**Export spec approved, and STEP export (#361):** the owner asked the mesh stream for export to
OBJ, STL, FBX, STEP, USD and SKP, with simple panels and an Advanced switch, and a live preview
of meshing (`docs/specs/export-formats.md`, approved the same day: all new code Rust, AP214 by
default, SKP by a Rust port of OpenSKP's writer as the last step). Step 1 is in: `Export` and
`ExportSelected` write `.stp` and `.step` files through solidcore's own STEP writer, exact,
AP214, in millimetres (models in other units are scaled), with object names. Rhino 8 opens
Curva's box, sphere and box-with-a-hole as valid closed solids with the same volumes (8000.000000,
4188.790205 and 6927.669708 against Curva's 8000, 4188.790227 and 6927.669708). Curves, block
copies and annotation are left out and counted; the schema panel comes with the panels.

**QuadRemesh hard edges (#329 part 2, first half):** QuadRemesh keeps hard edges by default, as
Rhino's `DetectHardEdges`: edges where the faces meet at more than 30° (Rhino 8 smooths a 30°
fold and keeps a 31° one). The quadrs copy gained a `crease_angle` option that holds such edges
like borders, and Curva snaps the nearest points onto the edges and corners (a listed custom
operation). A 20 mm box remeshed to 600 quads comes out exact, no face inside it and volume
8000.0, as Rhino's does; without hard edges quadrs leaves faces 0.68 mm inside. The option is
not offered to quadrs' author yet: that needs the owner's go-ahead. Adaptive size (the second half
of part 2) is next; it needs a change to quadrs' solver.

**SubD Weld and Unweld (#345):** UnweldEdge cuts a SubD open along inner edges into creased
borders at the same place, and WeldEdge joins them back, smooth, as Rhino 8 does. Curva's cut
box matches Rhino's own in its counts, its tags and its limit area (17.150913, to 11 decimal
places), and Rhino 8 reads Curva's as valid. A SubD can now hold such a cut: `SubD::from_cage`
keeps points at the same place apart, and the `.3dm` reader uses it.

**QuadRemesh, part 1 (#329):** the owner chose a patched copy of quadrs (decision 0036). Curva
carries quadrs 1.0.0 in `crates/curva-mesh/vendor/quadrs`, with ordered maps in four places and its
maths functions from `libm`, so the same mesh and seed give the same quads on every run and
every system (Linux and Windows gave 188 and 210 quads before the second part). The fix was offered to the author
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
- quadrs works in double precision, but Instant Meshes is sensitive to its input's last digits: a
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
