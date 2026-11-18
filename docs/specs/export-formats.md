# Spec: exporting to OBJ, STL, FBX, STEP, USD and SKP

**Status:** Approved by the owner in chat · 18 November 2026, with the decisions in §9. All code added to Curva is Rust.
**Covers:** `Export` and `ExportSelected` (today: `.3dm`, `.dxf`, `.dwg`, `.svg`) gaining six formats,
and the one mesh panel they share with the `Mesh` command (mesh spec §1, Stage A). Written by the
mesh stream, which owns mesh making and the mesh file formats (`AGENTS.md`).
**Target:** Rhino 8's export of the same formats: its options measured in Rhino 8 (RhinoCommon's
`File*WriteOptions`, and the owner's screenshots of its dialogs), and its results opened in Rhino 8.
**Rules:** pure Rust. One outside library per format at most, behind one Curva interface, pinned, its
results checked by tests (as decision 0032).

## 0. In short

| What you will notice | How |
|---|---|
| `Export` and `ExportSelected` write OBJ, STL, FBX, STEP, USD and SKP | One writer per format in Curva's file crates; the existing commands gain the formats |
| STEP keeps NURBS exact: surfaces and solids arrive in other CAD programs as real solids | solidcore's own STEP writer, from the engine Curva already uses |
| The mesh formats mesh NURBS objects first, with one panel, the same panel as the `Mesh` command | One mesh panel (§2), used by `Mesh`, STL, OBJ, FBX, USD and SKP |
| Every panel is simple first: one slider and the few options most people change | A **simple** view, and an **Advanced** switch for everything Rhino offers (§2, §3) |
| A live preview of the mesh over your objects, with its size and how far it strays, while you move the slider | Preview button: mesh edges drawn over the objects, face count and largest distance shown (§2.3) |

## 1. What you get

- **`Export` and `ExportSelected`** both write all six formats. `ExportSelected` writes only the
  chosen objects, as it does for `.3dm` and `.dxf` today. The format comes from the file type picked
  in the save dialog (or the typed extension).
- **What each format holds:**

| Format | NURBS surfaces and solids | Meshes and SubDs | Curves | Notes |
|---|---|---|---|---|
| **OBJ** | Meshed (§2) | As they are (SubD: meshed at a level, or its cage) | Polylines or degree-1 curves | Names and layers as OBJ groups or objects; materials (`.mtl`) from layer colours |
| **STL** | Meshed (§2) | As they are | Left out | Binary by default, text on request; open objects can be left out |
| **FBX** | Meshed (§2) | As they are | Left out | Binary FBX 7.4; Z up or Y up; layer colours as Phong or Lambert materials |
| **STEP** | **Exact** (AP203, AP214, AP242) | Faceted solids (`FACETED_BREP`) | Left out | The format CAD programs (SolidWorks, Fusion, Inventor) read best |
| **USD** | Meshed (§2) | As they are | Left out | `.usda` (text), `.usdc` (binary) or `.usdz` (one packed file, for Apple's AR viewers) |
| **SKP** | Meshed (§2) | As they are | Left out | SketchUp 2020 format, which every SketchUp since 2020 opens (§4) |

- **Units:** every format is written in the model's units, with the unit named in the file where the
  format has a place for it (STEP, FBX, USD); OBJ and STL have none, so the panel says which unit the
  numbers are in.
- **Repeatable:** the same model and options write the same file, byte for byte (no dates, random
  names or map order in the output), so files can be compared and tested.

## 2. The mesh panel (shared with `Mesh`)

Rhino uses one meshing panel for `Mesh`, STL, OBJ, FBX, USD and SKP (its "Simple Mesh Options" and
"Detailed Controls", and STL's own simple tolerance box). Curva does the same with one panel, in one
new file, so there is no second copy of the meshing code or the panel.

### 2.1 Simple view (what opens first)

- **Presets:** Coarse, Default, Smooth, and the last settings used (Rhino's list).
- **One slider:** *Fewer polygons ↔ More polygons* (Rhino's density slider, which Curva already maps to
  tolerances: `curva_mesh::density_tolerances`).
- **SubD level slider** when the export holds SubDs.
- **Preview** button (§2.3). OK and Cancel.
- **Better than Rhino:** under the slider, the result in plain numbers, updated as the slider moves:
  *about 12,400 faces · strays at most 0.04 mm · about 0.6 MB*. Rhino shows none of these.

### 2.2 Advanced view (the Advanced switch)

Everything Rhino's detailed controls offer (RhinoCommon `MeshingParameters`, measured 18 November
2026), in Rhino's order, each with a one-line explanation:
- Density; maximum angle; maximum aspect ratio; minimum and maximum edge length; maximum distance,
  edge to surface; minimum initial grid quads;
- Refine mesh; jagged seams; simple planes; pack textures;
- SubD: absolute subdivision level.

The switch is remembered. The simple view's numbers stay visible in the advanced view.

### 2.3 Live preview

- **Preview** draws the mesh's edges over the objects being exported, in the viewports, and keeps it
  updated while the slider or any field changes (meshing runs in the background; the newest settings
  win; nothing is added to the model or the undo list).
- The objects keep their own display under the preview, so both can be compared.
- **Better than Rhino:** an optional **deviation colouring** (green where the mesh is within the
  tolerance, red where it strays most), so you see where more faces are needed.
- Drawing the preview is a display issue (§5).

## 3. Each format's panel

Each format has a small panel: its **simple** options first, the rest under **Advanced**. The mesh
panel (§2) is a section of it for formats that mesh. Rhino's "Always use these settings, do not show
this dialog again" is kept, as a checkbox in Advanced, with a way back in the export options.

| Format | Simple view | Advanced (Rhino's full list, measured) |
|---|---|---|
| **OBJ** | Mesh slider; Z up or Y up; export materials | NURBS as meshes or NURBS; trim curves as polylines or curves; line endings; significant digits; display colour for objects with no material; underscores for spaces in material names; wrap long lines; object names (none, OBJ groups, OBJ objects); layer or group names (none, layers, groups); merge nested names; sort by groups; polylines as degree-1 curves, one line or two-point lines; vertex welding (unmodified, welded, unwelded); SubD control net or level; n-gons (none, preserve, create, minimum face count); texture coordinates; vertex normals; vertex colours (0–1 or 0–255); open meshes; as triangles; use render meshes |
| **STL** | Mesh slider; binary or text | Open objects exported or left out; the mesh panel's advanced view |
| **FBX** | Mesh slider; Z up or Y up | Materials as Phong or Lambert; vertex normals; views and lights (Curva writes views; lights when Curva has them); file version (7 binary or text; 6 is left out, see §7) |
| **STEP** | Schema: AP214 (Curva's default), AP203 or AP242 | Parameter space curves; let the importing program colour black objects; split closed surfaces |
| **USD** | Mesh slider; `.usda`, `.usdc` or `.usdz` | Z up or Y up; vertex normals; layer colours as materials |
| **SKP** | Mesh slider; SketchUp version | Planar regions as polygons (§4) |

## 4. The libraries, and the owner's decision on SKP

| Format | How | Licence | State |
|---|---|---|---|
| OBJ, STL | Curva's own writers in `curva-meshfiles` (already there, checked against Rhino 8) | Curva's | Done for meshes; NURBS need the mesh panel |
| STEP | **solidcore's `step-export`**, the engine Curva already uses: exact B-rep (`MANIFOLD_SOLID_BREP`, `BREP_WITH_VOIDS`), faceted bodies, the AP203, AP214 and AP242 schemas, colours, product structure and units | Apache-2.0 (already allowed) | Writes; rejects geometry it cannot write rather than approximating it |
| FBX | **fbxcel** 0.9 with its `writer` feature: the FBX 7.4 binary node writer. Curva builds the scene (meshes, materials, Z up or Y up) on top | MIT or Apache-2.0 | Last release 2022; the FBX 7.4 format has not changed. Curva writes the scene itself, so fbxcel is only the low-level writer |
| USD | **openusd** (pure Rust): writes `.usda`, `.usdc` and `.usdz` | MIT | Active (0.8.0, October 2026), not yet 1.0: pinned exactly |
| SKP | **No Rust writer exists.** Curva's own, ported from OpenSKP (below) | MIT (OpenSKP's notice kept) | Last step |

**SKP.** SketchUp's own format is closed, and no Rust library writes it. Researched on 18 November
2026, at the owner's request, including how the open-source SketchUp-like modeller IngeTrazo does it:
- **IngeTrazo** (Python) reads and writes `.skp` through **OpenSKP**: an MIT-licensed, clean-room,
  reverse-engineered SKP library (github.com/iamahsanmehmood/openskp, active, in TypeScript, Python,
  .NET, Dart and C++). It reads both SKP generations: the 2021+ VFF container and the 2013–2020 MFC
  `CArchive` one. It **writes the 2013–2020 format**: faces with holes, edges, arcs and circles,
  materials and textures, layers, components and groups, attributes. Each feature is checked against
  Trimble's own SDK, and its reference `.skp` files are published with the code. Current SketchUp
  versions open files saved in the 2013–2020 format.
- **Trimble's SketchUp SDK** is a closed C library with its own licence, shipped as binaries per
  system. It is not Rust, and it is likely incompatible with Curva's GPL application licence
  (decision 0025). Not used.

**Recommended route: Curva's own SKP writer in Rust, ported from OpenSKP's writer** (about 3,500
lines of C++; MIT, so the port keeps OpenSKP's copyright notice). Curva meshes NURBS with the mesh
panel and writes the faces, layers and colours, with blocks as SketchUp components. It is tested
byte for byte against OpenSKP's published reference files, and opened in Rhino 8, which reads SKP.
It writes the SketchUp 2020 format, which SketchUp 2020 and every later version open. If SketchUp
or OpenSKP's checks reject the files, SKP is left out and SketchUp users take DWG or DXF, which Curva
already writes and SketchUp Pro opens.

## 5. What it needs from the other streams

- **Meshing NURBS at chosen settings** (features, #319): the kernel meshes at its display settings
  today. The mesh panel needs Rhino's parameters (§2.2) and an estimate of face count and deviation
  (§2.1). Without #319, the first step uses the density slider only, through the tolerances Curva
  already has.
- **The panels** (features, `panel-system.md`): the export panels are new files in `curva-app`
  (allowed by `AGENTS.md`), registered in one line each, built with the existing panel widgets. A
  features issue for any widget the panels lack (a slider with marks, a collapsible Advanced section).
- **The live preview** (display issue): mesh edges and deviation colours drawn over chosen objects,
  updated without adding to the model.
- **Meshes and SubDs in the document** (features, #266): needed only to export mesh and SubD objects.
  NURBS objects can be exported before it.

## 6. Tests and checks

- **Each format opened in Rhino 8** (through the Rhino connection, as for STL, OBJ, PLY and `.3dm`):
  object counts, face counts, volumes and areas match what Curva wrote; STEP solids come back as
  closed solids with the same volume to the model's tolerance.
- **Each format read back by its library** (or Curva's reader) in the tests, so a change that breaks a
  file fails before it is merged.
- **Byte-for-byte repeatable:** writing twice gives the same file (§1).
- **The mesh panel** against Rhino: the same object meshed at the same density and settings gives
  face counts and largest distances close to Rhino's (within 10%), shown side by side for you.
- **ExportSelected** writes only the chosen objects, for every format.

## 7. Left out on purpose

- FBX 6 and FBX NURBS output (Rhino offers both; few programs read either well). FBX lights until
  Curva has lights.
- IGES, 3MF, glTF and the other formats Rhino exports are later
  requests.

## 8. Steps

Small pull requests, in order:
1. **STEP export** of NURBS surfaces, polysurfaces and solids through solidcore, with its panel's
   simple view; opened in Rhino 8. (Needs nothing from the other streams.)
2. **The mesh panel's simple view and the NURBS-to-mesh path** (density slider, presets), and **STL
   and OBJ** export of NURBS objects through it, with their panels.
3. **FBX** export (fbxcel) and **USD** export (openusd), with their panels.
4. **Advanced views** of all panels, once #319 gives the kernel Rhino's meshing parameters.
5. **Live preview and deviation colouring**, once its display issue is done.
6. Meshes and SubDs in every format, once #266 is done.
7. **SKP**: Curva's own Rust port of OpenSKP's writer (§4).

## 9. The owner's decisions (18 November 2026)

1. The spec is approved as written ("continue with your recommendation"). All code added to Curva is
   Rust.
2. SKP: researched (§4). The recommended route is Curva's own Rust port of OpenSKP's writer, as the
   last step (§8, step 7), unless the owner says otherwise.
3. STEP's default schema is AP214 (it carries colours and layers; Rhino's default is AP203).
