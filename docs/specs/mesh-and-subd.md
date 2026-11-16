# Spec: mesh and SubD objects

**Status:** Draft for the owner's approval (issue #246). No code before it is approved.
**Covers:** decision 0032; `architecture.md` §10d. The mesh stream's plan (`AGENTS.md`, Streams).
**Target:** Rhino 8's mesh and SubD objects and commands. Each command's exact steps and options are
taken from Rhino itself into `docs/commands/rhino-reference.md` before it is written, as for every
command; the command names below are the plan, checked against Rhino at that point.
**Rules:** pure Rust. One outside library per job, each behind one Curva interface in
`crates/curva-mesh`, pinned, upgraded once per release, results checked by tests (decision 0032).

## 0. In short

| What you will notice | How |
|---|---|
| Your Rhino files' meshes and SubDs open in Curva, look right, and save back | Curva's own mesh and SubD objects; `.3dm` reading and writing; OBJ, STL and PLY too |
| SubD modelling as in Rhino: primitives, push and pull vertices, edges and faces, creases, Tab for flat or smooth | Curva keeps the cage (the coarse control net you edit); subdiv-kernels works out the smooth surface |
| SubD turns into NURBS as Rhino does: exact where the net is regular, close-fitting near star points | subdiv-kernels' exact patches become solidcore surfaces through Curva's kernel |
| Mesh booleans that do not crack, even on messy downloaded or scanned meshes | manifold-rust, with its robust engine for broken meshes |
| Mesh repair, reduce, weld, join and explode | manifold-rust's repair, plus Curva's own small tools |
| Not yet: quad remeshing (Rhino's QuadRemesh) | Held back until someone needs it (decision 0032) |

## 1. What you get, in stages

Each stage ends with something you can open and try. The order is your decision (§9, decision 2);
this is the recommended one.

### Stage A: Rhino's meshes and SubDs come in (first slice)

- Meshes and SubDs are objects like any other: on layers, with colour, name, locking, hiding,
  groups, copy, move, rotate, scale, mirror, array, undo.
- **Files:** `.3dm` meshes and SubDs read and written (Rhino 8 opens Curva's file with the same
  shape). OBJ, STL and PLY open, import and export.
- **Display:** meshes shaded with their edges; SubDs shaded smooth with their crease edges and
  outline; **Tab** switches a SubD between smooth and flat, as in Rhino.
- **Commands:** `What` and `Properties` describe them (vertex, face and edge counts, closed or not);
  `Explode` and `Join` for meshes.

### Stage B: SubD modelling

- **Primitives:** SubDBox, SubDSphere, SubDCylinder, SubDCone, SubDTorus, SubDPlane, with Rhino's
  options (faces in each direction, corner or smooth edges).
- **Editing the cage:** pick vertices, edges and faces (Ctrl+Shift-click, as in Rhino) and move,
  turn and scale them with the gumball, with the live preview every command has. Extrude faces with
  the gumball (Ctrl-drag, as in Rhino).
- **ToSubD** (from a mesh), **ToNURBS** (from a SubD, §3.3).
- **Crease** and **RemoveCrease**, **InsertEdge**, **Bridge**, **Fill**, **Stitch**, **Weld** and
  **Unweld** on SubD edges, **DeleteSubDFaces**.

### Stage C: mesh tools

- **Mesh** (a NURBS surface or solid to a mesh, with Rhino's simple density slider first; the
  detailed controls later) and the mesh primitives (MeshBox, MeshSphere, MeshCylinder, MeshCone,
  MeshPlane, MeshTorus).
- **MeshBooleanUnion, MeshBooleanDifference, MeshBooleanIntersection, MeshBooleanSplit.**
- **Repair:** `Check` on meshes, MeshRepair's main fixes (unify normals, fill holes, remove
  degenerate faces, weld, rebuild normals), **ReduceMesh**, **TriangulateMesh**,
  **QuadrangulateMesh**, **ExtractMeshFaces**, **MeshToNURB**.

### Later stages (not planned in detail yet)

Symmetry (Rhino's reflected SubD), MultiPipe, SubD offset and Slide, SubD from curves, loft and sweep
to SubD, live booleans on meshes (as `live-booleans.md` does for solids), Make2D of meshes and SubDs,
mesh analysis colours (curvature, draft angle), meshes in DXF and DWG (the features stream is waiting
for mesh objects, `status/features.md`), textures shown on meshes.

**Held back:** quad remeshing (QuadRemesh). When it is needed, quadrs is the candidate
(decision 0032), with its own decision then.

## 2. The objects

Curva's own small structure, in `crates/curva-mesh`. Two object kinds, as in Rhino:

### 2.1 Mesh

- **Vertices** at full (double) precision, as every other Curva object, so meshes at map
  coordinates do not wobble.
- **Faces** with three or more corners. Rhino stores triangles and quads, and groups them into
  "n-gons" (faces of more corners) on top; Curva stores n-gons directly and splits them into
  triangles only for display and for booleans, remembering which triangles came from which face.
- **Optional per-vertex or per-corner data:** normals, texture coordinates (UVs) and colours, kept
  through edits and files. Per-corner data lets a hard edge or a texture seam have two values at one
  vertex, as Rhino's unwelded edges do.
- **Welded or unwelded edges**, as in Rhino (an unwelded edge shows as a hard crease in shading).

### 2.2 SubD

- **The cage:** vertices at double precision, n-gon faces, and the edges between them.
- **Edge tags:** smooth or crease (Rhino 8's two kinds). Each edge also stores a sharpness number,
  0 for smooth and "infinite" for a crease, so softer creases can come later without changing files.
- **Vertex tags:** smooth, crease, corner and dart, as Rhino tags them; corners keep a SubD's box
  corners sharp.
- **Optional data:** texture coordinates and per-face colours, carried through subdivision.
- **The smooth surface is worked out, never stored:** subdiv-kernels turns the cage into triangles
  for display and into exact patches for ToNURBS. When a cage edit only moves points, Curva reuses
  the last subdivision and only recomputes the positions, which is what keeps dragging smooth.

### 2.3 Object memory

Meshes and SubDs sit in the document as two new kinds of shape, next to curves, bodies, instances
and annotation. Everything an object carries (layer, colour, name, locking, groups, user text,
recipe, live boolean) then comes with them. Where Rhino records history for a command (for example
`Mesh` following its NURBS source), Curva records a recipe, so the result follows its source. The
`.curva` file stores the cage and its tags, never the smooth surface.

## 3. The libraries, each behind one interface

All three sit inside `crates/curva-mesh`; no other crate calls them. An upgrade, or a swap if a
library dies, touches only this crate.

### 3.1 manifold-rust: mesh booleans and repair

Version **0.19.2**, pinned exactly. It has two engines:

- **The exact engine** (the port of Manifold): fast, for clean closed meshes, such as meshes Curva
  made itself.
- **The robust engine:** slower, built on exact arithmetic, for meshes that are not closed, overlap
  themselves, have doubled faces or face inside out: typical of downloads and scans. It was tested
  on the 10,000 meshes of the Thingi10K collection without a failure.

**Curva uses its `Auto` choice:** the exact engine only when both meshes are provably clean, the
robust one otherwise. You never choose. Repair (inside-out bodies, rebuilding a solid from a
messy one, finding self-crossings) uses the same library. Meshes go in and come out at double
precision, and the `parallel` option gives the same results as running on one core, so tests are
repeatable.

### 3.2 subdiv-kernels: SubD smoothing and the limit surface

Version **0.1.0**, pinned exactly. Catmull–Clark (Rhino's SubD rule) with creases, corners,
boundaries and texture coordinates, built and tested to match OpenSubdiv (Pixar's reference).

- It computes **weights**, not shapes: "this smooth point is 25% of that cage point plus ...". Curva
  applies the weights to its own points, so positions stay in Curva's hands.
- **Precision risk:** its weights are single precision (about 7 digits). Applied to points measured
  from the SubD's own centre, a 10 m SubD is off by well under a hundredth of a millimetre. Step 2
  measures this; if it is not good enough for ToNURBS, Curva asks the library's author for
  double-precision weights (and offers the change).
- Its optional Bevy and graphics-card parts are **not used**: they target an older Bevy (0.19;
  Curva is on 0.20.1), and Curva hands its own triangles to the display.

### 3.3 SubD to NURBS (ToNURBS), through Curva's kernel

As Rhino does it:

- Where the cage is a regular grid (every vertex meets four faces, no crease nearby), the smooth
  surface **is** a bicubic NURBS patch, exactly. subdiv-kernels hands over each patch's 16 control
  points; Curva makes a solidcore surface from them with `curva-kernel`'s existing public surface
  functions, and joins the patches into one polysurface or solid.
- **Around star points** (vertices meeting three, five or more faces), creases and borders, the
  surface is not a NURBS patch. Curva fits patches close to the true surface there, as Rhino does,
  within the document's tolerance, meeting their neighbours without gaps.
- This fitting is Curva's own work, so it is a **listed custom operation**, marked in the code and
  in `docs/mesh/custom-operations.md` (§3.4), to drop if solidcore or subdiv-kernels later offer it.
- If ToNURBS needs something `curva-kernel` does not offer (joining many patches quickly, for
  example), the mesh stream asks the features stream with an issue (§6).

### 3.4 Upgrades

- Both libraries pinned to an exact version. Upgraded once per Curva release, on a branch, with
  the whole test suite (§7); any changed result is explained before it merges.
- Anything Curva adds on top of a library is marked in the code (`// CUSTOM MESH OPERATION: <name>`)
  and listed in `docs/mesh/custom-operations.md`, as the kernel and Bevy lists do. A test fails if
  the marks and the list disagree. At each upgrade, rows the library now covers are removed.

## 4. Display

Bevy draws from triangles (decisions 0028 to 0030); the display stream owns the viewport. The mesh
stream gives the viewport triangles, normals, edges and points; the viewport draws them.

| What you see | Who | Issue for the display stream? |
|---|---|---|
| Meshes shaded, with smooth or hard edges from their normals | Display, from the mesh stream's triangles | **Yes:** the viewport learns the two new kinds of object (today it only knows bodies' triangles) |
| Mesh edges (wires) drawn as lines | Display's line drawer | **Yes:** a mesh can have millions of edges; drawing them fast is display work |
| Vertex colours on meshes | Display | **Yes**, Stage A |
| SubD smooth surface; Tab for flat | Mesh stream makes the triangles; display draws | Included in the first issue |
| SubD crease edges, borders and outline | Display's line drawer, from curves the mesh stream gives | Included in the first issue |
| The cage, shown while a SubD is being edited, and picked vertices, edges and faces highlighted | Display | **Yes**, Stage B |
| Picking vertices, edges and faces under the cursor | Display (picking is the viewport's) | **Yes**, Stage B |
| Live redraw while dragging a cage point (the surface changes every frame) | Mesh stream recomputes positions only; display updates them in place | **Yes**, Stage B. Bevy redrew a changing 524,000-triangle SubD in 4 ms in the display stream's test |
| Textures on meshes | Display | Later |

The display stream is midway through moving to Bevy (`display-on-bevy.md`). The issues will say
which stage needs each item, so it can fold them into its order; Stage A needs only the first three.

## 5. Files

Each file format's mesh and SubD reading and writing is the mesh stream's own new file in that
format's crate.

| Format | Meshes | SubD | Where |
|---|---|---|---|
| `.3dm` (Rhino) | Read and write, with n-gons, normals, UVs, colours | Read and write, with creases, corners and tags | New `mesh` and `subd` files in `curva-3dm` (today it skips both with a message) |
| OBJ | Read and write, with n-gons, UVs, normals, groups as objects | Written as its cage (as Rhino does), or as a mesh, by option | New crate `curva-meshfiles` |
| STL | Read and write (text and binary) | Written as a mesh | `curva-meshfiles` |
| PLY | Read and write, with vertex colours | Written as a mesh | `curva-meshfiles` |
| `.curva` | The mesh as stored | The cage and its tags | `curva-document`'s mesh module |

OBJ, STL and PLY are simple, so Curva reads and writes them itself rather than adding libraries.
Meshes in DXF and DWG are a later stage, with the features stream.

## 6. What it needs from the other streams

The mesh stream may add its own new files in shared crates and one-line registrations
(`AGENTS.md`). Everything else goes through an issue.

**The mesh stream adds itself:**
- `crates/curva-mesh` (new crate: the structure, the three library interfaces).
- In `curva-document`: a new `mesh` module (the two shapes as document objects, their `.curva`
  storage, their recipes).
- In `curva-commands`: `commands/mesh/` and `commands/subd/`, one file per command; one line per
  command in the registry and in `PREVIEW_STEPS`.
- In `curva-3dm`: new `mesh` and `subd` files, plus one line each where the reader and writer list
  object kinds.
- `crates/curva-meshfiles` (new crate: OBJ, STL, PLY).
- Menu entries and translation lines for the commands.

**Needs a features issue (shared code):**
1. **Two new kinds of shape in the document.** Not a one-line change: about 30 places across the
   document, commands, interface, `.3dm` and DXF code check every kind of shape, and each needs
   to say what a mesh or SubD does there (most say "nothing" or "not supported"). Asked of the
   features stream first, before Stage A.
2. **Sub-object picking in commands** (picking vertices, edges and faces of a SubD for the gumball
   and for commands), in the command framework, if the framework cannot express it today. Before
   Stage B.
3. **Recipes for meshes** (history kinds such as "a mesh of that surface") in the document's history,
   if they cannot be added as one-line entries. Stage C.
4. **Anything ToNURBS needs from `curva-kernel`** that its public functions lack. Found in Stage B.

**Needs display issues:** the items marked in §4.

## 7. Tests and checks

- **Known answers:** volumes, areas and counts of primitives (a mesh box of 1 m has 8 vertices,
  6 faces, volume 1 m³; a sphere's volume approaches 4/3 πr³ as it gets finer), boolean volumes
  (two overlapping cubes), repair results (an inside-out cube comes out with positive volume).
- **SubD exactness:** on a regular grid, the smooth surface must equal the bicubic NURBS patch
  exactly (a mathematical fact, so a firm test); ToNURBS of a regular SubD must sit on the true
  surface to round-off.
- **OpenSubdiv:** subdiv-kernels is tested against OpenSubdiv in its own project. OpenSubdiv is C++,
  so it cannot be part of Curva's build or tests (pure Rust). At each upgrade, the mesh stream runs
  the library's own OpenSubdiv comparison in the library's project, outside Curva, and notes the
  result in the upgrade's pull request.
- **Rhino as the reference:** for each primitive and command, Rhino 8 makes the same object
  (through the Rhino connection already used for other goldens), and Curva's result is compared:
  counts, volumes, the smooth surface's points, ToNURBS's patch count and distance from Rhino's.
  Near star points Rhino's exact rule may differ slightly from OpenSubdiv's; Stage B measures it
  and reports it to you with pictures if it shows.
- **Files:** round trips (Curva → file → Curva identical) for every format; Rhino 8 opens Curva's
  `.3dm`, OBJ, STL and PLY files and reads the same counts and shape; Rhino 8's own files open in
  Curva the same.
- **By eye:** side-by-side pictures of Curva and Rhino for each stage (the same SubD, smooth and
  flat; a boolean; a ToNURBS result with its patch edges), for you to judge.
- **Upgrade guard:** every library call has tests whose results are pinned, so an upgrade that
  changes a result fails until the change is explained (§3.4).

## 8. Licences

Checked with `cargo-deny` on a branch with both libraries added (16 November 2026):

| Library and what it brings | Licence | On Curva's allowed list? |
|---|---|---|
| manifold-rust | Apache-2.0 | Yes |
| dashu (exact arithmetic, for the robust engine) | MIT or Apache-2.0 | Yes |
| **clipper2-rust** (2D polygon clipping, used by manifold-rust) | **BSL-1.0** (the Boost licence) | **No** |
| subdiv-kernels | MIT, Apache-2.0, Zlib or BSD-3 | Yes |
| Their smaller helpers (rayon, smallvec, thiserror, serde and others) | MIT or Apache-2.0 | Yes, most already in Curva |

**The Boost licence** is a short permissive licence, like MIT: free use, change and sale, and it
asks for its notice in source copies only, not in the shipped program. It is common in C++ and
compatible with both of Curva's licences. Your decision 1, §9.

No other problems: no copyleft, nothing unmaintained flagged. subdiv-kernels' optional Bevy and
graphics-card parts are switched off, so no second Bevy is built.

## 9. Your decisions

**1. Allow the Boost licence (BSL-1.0) for clipper2-rust, which manifold-rust needs.**
- **(a) Allow it for clipper2-rust only (recommended).** Mesh booleans as planned. Same as the
  exception you gave acadrust's licence.
- (b) Allow BSL-1.0 generally. Same now; fewer questions later, less control.
- (c) Refuse it. manifold-rust cannot be used; mesh booleans would need another library (the ones
  decision 0032 dropped crack more often) or years of Curva's own work.

**2. The order of the stages.**
- **(a) Files first, then SubD modelling, then mesh tools (recommended).** Your own Rhino meshes and
  SubDs show up in Curva first, which tests the objects, the display and the files early on real
  models; SubD modelling, the bigger design tool, comes next.
- (b) SubD modelling first. You can model SubDs sooner, but cannot bring in your existing ones
  until after.
- (c) Mesh tools first (Mesh from NURBS, booleans, repair). Useful for 3D printing and scans
  sooner; SubD later.

**3. Approve the plan** (stages, objects, libraries, tests, steps), or tell me what to change.

## 10. Steps

Small pull requests, in order, a few hundred lines each. Stage A is planned in detail; B and C are
broken into issues of the same size when A ends.

**Stage A**
1. `crates/curva-mesh`: the mesh structure (vertices, n-gon faces, normals, UVs, colours, welded
   and unwelded edges), splitting n-gons into triangles, counts, volume, area, bounds. Tests.
2. The SubD structure (cage, edge and vertex tags), and the subdiv-kernels interface: smooth
   triangles at a chosen density, the precision measurement (§3.2). Tests on known answers and on
   the regular-grid exactness.
3. Features issue (§6.1) for the two shape kinds; once merged, `curva-document`'s mesh module:
   mesh and SubD objects, `.curva` storage, transforms. Tests.
4. Display issues (§4, Stage A rows). Meanwhile, triangles, edges and outlines handed to the
   viewport in the form it asks for.
5. `.3dm` meshes read and written, checked against Rhino 8 both ways.
6. `.3dm` SubDs read and written, checked against Rhino 8 both ways.
7. `curva-meshfiles`: OBJ read and write.
8. STL and PLY read and write.
9. `What`, `Properties`, `Join` and `Explode` for meshes; Tab for SubD flat and smooth (once the
   display can show it). Pictures beside Rhino's for you.

**Stage B** (first issues): the six SubD primitives (one pull request for the shared part, one for
the rest); ToSubD; sub-object picking (after its features and display issues); gumball editing of
the cage with live redraw; Crease and RemoveCrease; InsertEdge; Bridge; Fill; Stitch, Weld, Unweld,
DeleteSubDFaces; ToNURBS regular patches; ToNURBS around star points and creases.

**Stage C** (first issues): the manifold-rust interface and its licence entry; MeshBoolean commands;
Mesh from NURBS; mesh primitives; Check and MeshRepair; ReduceMesh; Triangulate, Quadrangulate,
ExtractMeshFaces, MeshToNURB.
