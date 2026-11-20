# Status: features stream

*Written only by the features stream (commands, geometry, documents, and the interface outside the
viewport). Updated at the end of every work session. Newest first. The display stream keeps its
own page, `status/display.md`.*

*Until 15 November 2026 this was the whole of `STATUS.md`, so it also holds the display history up
to then (display modes, glass, line widths, speed measurements).*

**Contributor Licence Agreement (decision 0026, owner's decision in chat, 15 November 2026):**
`CLA.md`, `CONTRIBUTING.md` and the `CLA` check (`.github/workflows/cla.yml`). Outside contributors sign
once by commenting on their first pull request; signatures go to the `cla-signatures` branch.

**Tidy-up first (owner's rule, 19 November 2026, `AGENTS.md`):** every stream starts each session
by completing one open `tidy-up` issue in code it owns, before its next feature; with none open, it
goes straight to features. The open issues are listed in `docs/reviews/code-health-2026-11-19.md`.
*(Added to every status page at the owner's request.)*

## Features stream — 20 November 2026

- **File format registry built (#391, code health fix 6)**, to the spec the owner approved today
  (`docs/specs/file-formats.md`, #405, both recommendations). Each format now describes itself once
  in its own crate and joins one list, gathered by the new crate `curva-formats`. The model carries
  the list; the file commands ask it and name no format. STEP export moved into its own crate,
  `curva-step`, and the shared meshing helper into `curva-document` (agreed with the mesh stream).
  The steps were #417 (pinned dialog lists), #418, #420, #423 and the last one, which takes the
  dialogs' lists from the registry and adds a check that `curva-commands` and `curva-app` depend on
  no format crate. Every dialog lists the same types in the same order. **New file formats can now
  be added** (FBX, USD, SketchUp export): each is one crate that joins the list.
- **Commands grouped by kind of geometry** (`docs/specs/command-groups.md`, approved by the owner
  today, #429). The code is filed as `nurbs/{curves,surfaces,solids,points}`, `mesh`, `subd`,
  `edit`, `organise`, `annotate`, `layouts`, `analyze`, `select`, `view`, `files`, each listing its
  own commands and its tab's buttons (#430, closes #422; completions keep their order through
  `commands/order.rs`). The top bar now shows **NURBS** (Curves, Surfaces, Solids), **Mesh**, then
  **Edit**, **Organise**, **Annotate**, **Layouts**, Analyze and Select; Modify is gone. SubD has a
  folder and a tab that stays hidden until it has commands (there are none yet).
- **SketchUp files in Import's dialog** (#427, the owner's choice), and **SketchUp Live Components**
  added to the smart-object references in `docs/architecture.md` (#428, the owner's note).
- **Size check at work:** it stopped `Export` and the app's start-up function from growing. `Export`
  was split, and `Open` shrank, so both left the exceptions list.
- **Tidy-up #392 done** (#434): `curva-dxf`'s 1,354-line `lib.rs` split into `export`, `import`,
  `ocs` and `units`, its six long functions in named steps; seven entries left the size list.
- **A command step can open an interface panel** (#436, closes #387 for the mesh stream): `Mesh`
  and STL/OBJ/PLY export ask for the mesh settings in the mesh panel (the mesh stream hooks it up in
  #440); scripts, agents and the dashed commands never see the panel. `StepDef::meshable()` takes
  only surfaces, solids and SubDs.
- **SketchUp import options like Rhino 8's** (#432, the owner's request through the mesh stream):
  faces can come in as **trimmed planes**, built straight from each face's loops and joined into
  solids where groups close (#437); the **SketchUp Import Options** dialog shows before a `.skp` is
  opened or imported, its choices kept in the settings (#443). Weld angle, Import curves, textures
  and Rhino's measured numbers follow in #444.

## Features stream — 19 November 2026

- **Code health review** (`docs/reviews/code-health-2026-11-19.md`, #402) and the owner's
  **tidy-up first** rule (#404). Tidy-up issues #391 to #401 opened. **Size check in CI** (#406):
  files over 800 lines and functions over 80 now fail; today's 5 files and 65 functions are in
  `crates/curva-size-check/known.txt`, which may only shrink. #401 (helpers out of
  `commands/mod.rs`) in #410.
- **More of the owner's `.3dm` files come in:** surfaces of revolution whose angle ended a rounding
  short (#386, 72 solids); plane faces only a rounding wide (#389, 35 of 37 "status 1007" solids);
  solids where three faces meet at an edge (#403, 4 solids); zero-length lines named as such
  (#385). Left: 2 solids with status 1007, 1 "parts do not meet end to end", 1 "tolerance not
  finite", and the two big files' single huge solids that one core builds alone.
- **View picker complete** (#390): Back, Left, Bottom and Two-point, and the lens in mm or degrees.
  Saved views keep a lens field (#384); the display stream fills it.
- **Waiting on the features stream:** #387 (the mesh stream asks for a command step that opens the
  mesh panel).
- **Big drawings open 2 to 8 times faster** (#379, #317 part 4; the owner's choice, decision 0037).
  Curva carries a patched copy of solidcore's `solidcore-ops` that gives each thread its own engine,
  so opened solids, plan cuts and hidden lines are worked on every core. The owner's largest test
  file opens in 377 s instead of 902 s, the 626 MB one in 22 s instead of 189 s, with exactly the same
  objects. Offered upstream as tangency-labs/solidcore#3358; the copy is deleted once solidcore has
  the fix and the tests and timings pass on it (#381). Timings in `docs/performance.md`.
- **Meshes and SubDs stretch, deform and sit in stretched blocks** (#376): ScaleNU, cage edits,
  the gumball's stretch, the deforming commands and block placements now move their points.
- **SketchUp files open** (the owner's choice: OpenSKP's pure-Rust reader, MIT, taken from its
  repository at a fixed commit; SketchUp's own toolkit may not ship with Curva). New crate
  `curva-skp`; `Open` and `Import` read `.skp`. Faces come in as meshes (one per tag and material in
  each component or group), components as blocks, top-level groups as grouped objects, tags as layers,
  materials as colours, loose edges as polylines; SketchUp's inches in the file's unit. The owner's
  22 MB SketchUp 2014 test file opens in 1.6 s (54 blocks placed 5,364 times, 161 groups, 168 meshes,
  7 layers; 7 hidden faces left out). Faces' own tags in files from before SketchUp 2021 are not named
  by OpenSKP; they take their group's tag, and the report says so. The mesh stream adds SketchUp
  export to the same crate later.

## Features stream — 18 November 2026

- **Meshes and SubDs are shapes in the document** (#266, for the mesh stream). `Shape` has two new
  kinds, `Mesh` and `SubD`, holding `curva-mesh`'s types; `.curva` files save and open them, and older
  files open unchanged. Every place that handles each kind of shape now says what a mesh or SubD does:
  they move, turn, count, show in `What` and have their box; a SubD's cage gives its edges.
  - **`.3dm`:** meshes and SubDs now come in and go out instead of being left out. A mesh with hard
    edges is written with its points split along them, as Rhino stores its own box (Rhino keeps one
    normal per point); welded back, it is the same mesh. New tests on both sides.
  - **Not yet:** stretching (ScaleNU, cage edits, placements that stretch) refuses or skips them until
    the mesh crate can deform; Properties shows nothing for them yet (#270 part 2). Drawing and
    printing them are the display stream's (#267, #324). Until then a mesh read from `.3dm` is kept in
    the document, and saved again, but not drawn.

## Features stream — 17 November 2026

**Later on 17 November:**

- **Dimensions that follow their geometry: the owner chose option A** (decision 0035, #294). A dimension
  remembers the object, and the place on it, that each of its points snapped to.
  - Merged #295 (the tie) and #300 (dimensions keep their ties and follow). If the object moves or changes,
    the dimension follows, and moves whole when both points moved the same way. If the dimension is moved
    by itself, it lets go, as in Rhino. If the object is deleted, the dimension stays. Ties are saved in
    `.curva`.
  - **Waiting on the display stream (#296):** snaps must report which object and place they landed on.
    Until then, no dimension drawn in the app gets a tie. When #296 lands, I add the small hand-over in the
    app.
  - Later (in #294): radius, diameter, arc length and angle dimensions; copies keeping ties; dimensions
    through details.
- **Rhino files never crash Curva:** #179 is closed. Merged #291 (long functions in named steps, checked
  identical) and #292 (a test that opens damaged copies of every example file). A longer run, every file at
  40 places, found no crash.
- **Interface tidy-up (#178):**
  - Merged #298 (part 1). The toolbar icons are drawn in 31 short functions in ten files by subject, and
    every icon was checked to draw the same shapes as before. The Layers table and Properties panel are in
    named parts, and `layout.rs` is split.
  - Part 2 in review: panel frames, title row, resize handles, Annotation Styles, Layouts, General
    settings, assistant panel, Print panel, viewport mouse handling, gumball.
  - Left until the display stream's open pull requests land: `viewports.rs`, `main.rs`, `sheets.rs`,
    `top_bar.rs`, `benchmark.rs`.
  - No before-and-after screenshots: the owner declined screen control. The icon comparison and the tests
    stand in.
- **Tidy-ups finished:**
  - #175 closed (#302): one shared "objects given, or the selection" step for 31 commands.
  - #176 closed (#304): the kernel's main file split by subject; `making2` and `surface_ops2` regrouped by
    subject; `hidden_lines` in named steps, its output checked identical.
  - #178: part 2 merged (#301). The rest, in files the display stream changes often (`viewports.rs`,
    `sheets.rs`, `main.rs`, `benchmark.rs`), is handed to the display stream as #303.
- **Live plans (spec `live-plans.md`):**
  - Step 2 merged (#307, closes #202): every mark on a page names the object it came from.
  - Fixed on the way (#309, closes #308): plans drew solids lying wholly above the clipping plane.
  - Step 3 (#315, #203): the test house's plan is down from 0.32 s to about 0.2 s, its drawing
    identical. *Large* does not finish (edges 60–78 s, crossings over 12 minutes).
  - The owner's decision: *Large* first, then step 4 (#317). Part 1 merged (#341): crossings 700 s → 2 s,
    edges 78 s → 7 s. Part 2 merged (#347): *Large* finishes, 122 s. The owner (18 November): keep on
    *Large*, drawings open all the time in architecture work. Part 3 (in review): 122 s → 26 s, the house
    and hidden-line scenes identical. Explored: a solidcore patch letting each thread run on an engine
    of its own (written and tested locally, not published), for cutting on every core.
- **Next (features):** *Large* part 4 (the engine patch, once the owner agrees to publish the fork and
  the upstream issue); then Live plans step 4 (#204); the tie hand-over once #296 lands; Make2D fills
  behind other objects.


**The repository moved** from `V:` to `E:` (an external SSD, a ReFS Dev Drive) on 16 November: it is at
`E:\curva`, worktrees at `E:\curva-<issue>`.

**Milestone 3, point 7 (dimensions):** merged (#264): DimArcLength's `Partial` (two points on the arc) and
`Leader` (a radial arrow from the dimension to the arc, offered for a turn over a quarter, as AutoCAD), both
ways in DXF and DWG; the command is on the framework. Annotation made by any framework command now previews as
drawn.

**Section styles (`layouts.md` §3):**
- Merged (#280): section styles on objects, layers and clipping planes (a fill, none or a colour, and an
  optional hatch pattern with scale and rotation), worked out in Rhino 8's order (object, layer, plane) and
  saved in `.curva`. `SectionStyle` (alias SectionHatch) sets them: `Fill`, `Hatch`, `Pattern`, `Scale`,
  `Rotation`, `Layers`, `ByLayer`. Rhino 8 has no such command (it sets them in Properties); checked in Rhino 8.
  The framework gained *name* options (typed, checked against the command's list, remembered).
- In review (#283): Make2D fills cut faces seen face-on with the section style, as hatches under the lines.
  Not yet: a fill partly behind other objects is drawn whole; a cut another chosen plane crosses is not filled.
- In review (#286): the views' solid fills follow each solid's style (its own, its layer's, the plane's).
- Asked of the display stream (#284): hatch patterns on cut faces in the views.

**Tidy-ups:** merged #277 (#177: the document's main file split into object, linetype, layer and state files)
and, inside #264 and #280, the annotation file split by kind and the command framework's file split. Make2D is
now a folder with `make()` in named steps (in #283).

**Owner's decision (17 November):** dimensions follow their geometry by option A, attach to the object
snapped to (see above).

**Next (features):** `.3dm` section styles both ways (Rhino 8 keeps them on objects and layers); Make2D fills
behind other objects; DimArcLength done, so the remaining dimension work is the decision above. Tidy-up #178
(icons and long panels): the icon file is 2,100 lines.

**Machine note:** builds share one folder, `E:\curva-target-features`, outside the worktrees, so removing a
worktree never deletes it; link steps run `-j 4` (more ran out of memory with other streams building).

## Features stream — 16 November 2026

**Owner's instruction (15 November 2026):** work that relies on the display is paused while a new display
architecture is weighed: drawing objects' own colours and the document's linetypes (#170), Technical and Hidden
views, shaded details, and PNG output from the viewport. Work on files, commands, documents and the kernel goes on.

**Interoperability gaps (`drawing-output.md` §7, decision 0023):**
- Merged (#209): DXF and DWG ellipses both ways, exact (ELLIPSE entities, whole or in part). The kernel gains
  `Curve::elliptical_arc` and `Curve::as_ellipse`, which also recognises Rhino's ellipses, written as rational
  splines. Rhino 8 reads Curva's ellipses with the same centre, radii and ends.
- Merged (#211): DXF and DWG MLEADERs come in as leaders with their note. Rhino 8 skips MLEADERs, so it cannot
  check this.
- Merged (#210): a detail's hidden layers as the DXF and DWG viewport's frozen layers, both ways; a Rhino 8
  file reads with the same layer hidden. Rhino reading Curva's layouts could not be checked: Rhino only brings in
  layouts when it opens a DXF, which closes a scripted Rhino slot.
- Merged (#212): `.3dm` Rhino 5 annotation (notes, linear, aligned, radius, diameter and angle dimensions,
  leaders), matched to what Rhino 8 reads back from a Rhino 5 file. Its style table came in #229.
- Merged (#214): DXF and DWG objects' user text both ways (Rhino's XDATA), and block attributes read as Rhino
  reads them (user text on the instance, notes where shown). Rhino 8 reads Curva's user text.
- Merged (#216): ordinate dimensions (`DimOrdinate`, X or Y from a base point, with Rhino's kinks) in `.3dm`,
  DXF and DWG; Rhino 8's own files read with the same values. AutoCAD draws its own jog, so DXF does not carry
  the kinks.
- Merged (#229): `.3dm` Rhino 5 annotation styles (the style and font tables): dimensions and leaders on their
  styles, notes on the first style, as Rhino 8 reads them.
- Merged (#231): `.3dm` a note's own height and alignment go out as Rhino's override of its style (no more
  "Plans 7 Centre" styles); Rhino 8 reads Curva's note on its style with the override.
- Merged (#232): DXF and DWG MLEADER block content comes in as an instance of the block where the MLEADER puts it.
- Merged (#233): DXF and DWG angles by three points come in as angle dimensions (as Rhino 8 reads them), jogged
  radii as plain radius dimensions (Rhino 8 skips them). Arc length dimensions came in #247.
- Merged (#238): DXF and DWG splines given only by fit points come in as Rhino 8 reads AutoCAD's: a cubic
  spline with knots at the points' spacing (chord, square root, uniform), the end tangents as derivatives and
  natural ends otherwise. New kernel `Curve::fitted` on solidcore's `cubic_spline_interp`; only the natural end
  derivatives are Curva's own (solidcore has none). Seven Rhino 8 imports agree to about 1e-12. A closed one
  stays on Curva's own interpolation: Rhino 8 reads it open and AutoCAD's shape is unchecked.
- Merged (#247): arc length dimensions both ways in DXF and DWG (ARC_DIMENSION; a partial one's leader is not
  carried). Rhino 8 skips them and has none, so `.3dm` takes them as their plain drawing, grouped.

**Milestone 3, point 7 (dimensions):**
- Merged (#241): DimCurveLength, DimArea and DimVolume as Rhino makes them: text or a leader holding the
  `CurveLength`, `Area` or `Volume` field, so the value follows the object. In `.3dm` those fields now name the
  object by its Rhino id (Rhino 8 works them out) and come back naming Curva's number; before, they went out as
  fixed numbers.
- Merged (#247): DimArcLength and Curva's arc length dimension, drawn as AutoCAD's DIMARC (Rhino 8 has no such
  command): parallel extension lines under a quarter turn, from the centre otherwise, the arc symbol over the
  value. AutoCAD's `Partial` and `Leader` options are not there yet.

**Milestone 3, point 9:** merged (#213): `HatchPatterns` loads a `.pat` file's patterns into the model
(`;%UNITS=INCH` files turned into millimetres). Merged (#219): `SetHatchBasePoint` moves where hatches' patterns
start.

**Milestone 3, point 11 (Make2D):**
- Merged (#254): four-view drawings, `Projection=ThirdAngle` and `FirstAngle`: top, front, right and perspective
  lined up by drafting convention, gaps a tenth of the largest view, lower left at the origin. The perspective is
  Rhino 8's standard Perspective direction drawn flat (the document keeps no camera per viewport). Rhino's own
  spacing is unchecked: its four-view comes only from its dialog.
- Merged (#257): `GroupOutput` and `MaintainSourceLayers` (`Make2D::Visible::Curves::Walls`).
- Merged (#258): annotation lying flat to a view is copied into that view's drawing on
  `Make2D::Visible::Annotations`; annotation standing across a view is left out for now.
- Merged (#261): clipping planes chosen with the objects cut the drawing (solids by the engine's split, curves
  where they cross); `ClippingPlaneIntersections` draws the cuts on their own layer. No section fill yet.
- Merged (#260): DimArea and the `Area` field measure hatches (what they fill).

**Next (features):** dimensions that follow their geometry (needs a decision: Rhino links a dimension to what it
snapped to, but the viewport's snaps carry no object, and changing them is display work; the alternative is to
link points that coincide with an object's snap points); DimArcLength's `Partial` and `Leader`; Make2D's section
fills. Meshes in DXF and DWG need the owner's decision first: Curva has
no mesh objects.

## Features stream — 15 November 2026

**Licence (decision 0025, owner's decision in chat):** the application crates (`curva-app`,
`curva-commands`, `curva-viewport`, `curva-mcp`) are now GPL-3.0-or-later with a plugin exception
(`LICENSE`, `PLUGIN-EXCEPTION.md`); the libraries stay MIT OR Apache-2.0. A permissive crate must never
depend on a GPL one. Release packages now ship `LICENSE` and `PLUGIN-EXCEPTION.md`.

**Milestone 3, point 5 (title blocks, scale bars, sheet index, revisions):**
- Merged: document and layout user text and notes (#128): `SetDocumentUserText`, `GetDocumentUserText`,
  `SetLayoutUserText`, `Notes`, and the `DocumentText`, `LayoutUserText` and `Notes` text fields.
- Merged (#133): `TitleBlock`, `ScaleBar`, `SheetIndex`, `Revision`. The title block is grouped lines and text
  with fields (ISO 7200-style in an ISO 5457 border, or ANSI); scale bars, the sheet index and revision
  tables are annotation worked out from the document, so they follow detail scales, layouts and new
  revisions. Checked on a printed PDF.

**Known gaps:** scale bars and tables have nothing to edit in
Properties yet; built-in title blocks are one design per standard, not one per sheet size.

**Milestone 3, points 14 to 16 (export):**
- Merged (#134): SVG export of layouts through Export (`.svg`), one file per layout.
- Merged (#142): DXF and DWG (new crate `curva-dxf` on `acadrust`, MPL-2.0 exception added to `deny.toml` and
  `about.toml` as decision 0021 approved). Export, ExportSelected, Open and Import take `.dxf` and `.dwg`:
  curves, points and layers (colour, print width, on, locked, sublayers as `Parent$Child`), the unit.
  Text, dimensions, hatches, blocks, solids and layouts were left out and reported.
- Merged (#145): DXF and DWG text (TEXT out, TEXT and MTEXT in, fields worked out), hatches (outlines and
  pattern lines; unknown patterns come in with the drawing) and blocks (definitions, nested, and INSERTs).
- Merged (#159): DXF and DWG dimensions (DIMENSION entities with their `*D` drawing blocks) and leaders, both
  ways; Rhino 8 reads them with the same values.
- In review: DXF and DWG layouts with their viewports, both ways (Rhino's layout DXF reads in place).

**Milestone 3, `.3dm` round trip of drawing objects** (`drawing-output.md` §5):
- Merged: dots (#146), hatches with the hatch pattern table (#147), Rhino's notes read (#149), Curva's notes
  written with annotation styles (#150). Each checked by opening Curva's file in Rhino 8.
- Merged (#152): linear, radius, diameter and angle dimensions and leaders, both ways.
- Merged (#156): layouts and details, both ways.
- Merged (#169): document user text, layout user text and notes, both ways; the `DocumentText`,
  `LayoutUserText` and `Notes` fields stay fields for Rhino (a layout named by number goes out as its
  page id and comes back as a number). Checked in Rhino 8 both ways. Rhino writes layout user text
  only when plug-in data is saved (`SaveSmall` with `SavePlugInData=No` drops it).
- Merged (#171): linetypes and objects' own colours both ways in `.3dm`, DXF and DWG. The document gains
  object colours (`ObjectColour`, a Colour row in Properties) and linetypes of its own that files bring
  (with their patterns, offered in the Linetype command and drop-downs). Linetype lengths are paper
  millimetres, as in Rhino and printing; DXF lengths convert to drawing units. DXF and DWG import also
  takes layers' on, locked and line weight. Checked in Rhino 8 both ways. Drawing the colours and the
  document's linetypes, and dashes at paper size, is the display stream's (#170).
- Merged (#181): `.3dm` annotation styles by name with their font, text height, arrow size, text gap,
  extension lines and decimals (`ON_DimStyle` 1.2 with an `ON_Font` record); notes and dimensions read
  keep their style. Checked in Rhino 8 both ways.
- Merged (#188): `.3dm` details keep their display mode (Rhino's standard modes) and lock.
- In review (#191): layers turned off in a detail stay off in it (`ON__LayerExtensions`; Rhino names a
  detail's viewport by the detail object's id). Checked in Rhino 8 both ways.
- In review (#184): DXF and DWG annotation styles as text styles (TrueType family) and dimension styles
  (sizes, decimals, text style by handle). Checked in Rhino 8; an AutoCAD check is still to come.
- Notes for the next session: Rhino slots sometimes fail to start or close on the first script; a
  retry works. A slot keeps styles between imports, so check each import in a fresh slot.
- Merged (#158): scale bars and tables to `.3dm` as their plain drawing; fields only Curva can work out written as
  they read.

**Owner's decision (0023, 15 November 2026):** finish all of Milestone 3 before 0.4.3, and close every
interoperability gap for `.3dm`, DXF and DWG (`drawing-output.md` §7). Technical and Hidden views are asked of the
display stream (#164).

**Next (features):** the interoperability gaps; arc length and curve length dimensions and dimensions
that follow their geometry; `.pat` hatch patterns, gradients, base point; printing the model or a view and to PNG;
Make2D four-view drawings, clipping cuts, annotations and source layers. Earlier list: Properties for scale bars and
tables. Known: Rhino's own DXF export writes a detail's view height as paper height ÷ scale, against AutoCAD's
reference; Curva follows the reference, so a Rhino DXF's detail scale reads inverted.

**Note:** the website's Cloudflare build (`Workers Builds`, from #155) fails on pull request branches. It is not a
required check, and pull requests that touch no website files merge without it.

## Current state — 8 November 2026

**Milestone:** M0, groundwork and kernel evaluation.

**Done**
- Architecture draft v0.9 written by the owner (`docs/architecture.md`).
- Agents work through the owner's GitHub account and merge their own work once checks pass (decisions 0005 and 0007).
- Starter repository prepared: rules, decision notes, M0 kernel evaluation spec, issue list.
- One-time GitHub setup finished: https://github.com/curva3d/curva, with `main` protected and the
  17 M0 tasks in https://github.com/curva3d/curva/milestone/1.
- The kernel test list grew from 25 to 31 operations, chosen by the owner after comparing it with
  Serpentine3D's commands (#19).
- The owner confirmed the 31-operation list is final and approved; #8 is released.
- Dev Drive and Rust toolchain set up (#2). The repository was at `V:\curva` (at `E:\curva` since 16 November 2026).
- Rust workspace skeleton (#3).
- Automated checks on Linux, Windows and macOS, required on `main` (#4).
- Claude Code hooks: format and lint after edits, tests before finishing, risky commands blocked (#5).
- The 31 kernel test cases as neutral files, and a harness to run them (#8), in `evaluation/`.
- A check that flags weakened tests on every pull request (#6). It is deliberately not a required
  check, so a change the owner has agreed to can still be merged.

**In progress**
- Milestone 1, first slice (`docs/specs/m1-first-slice.md`, approved by the owner): 12 commands, viewport and window built. The owner is trying it by hand.

- First hand-testing round (9 November 2026): command autofill, quick clicks no longer lost, window and crossing box selection, Shift adds and Ctrl removes. Orbit no longer waits for the display (31 ms to 2 ms between frames); it may still feel laggy over Remote Desktop, whose display runs at 32 Hz. Whether that is the connection is not settled: the owner can compare with Rhino over the same link or use Parsec.

- Object snaps (End, Mid, Cen, Int, Perp, Near), ortho and grid snap (`docs/specs/m1-snaps-and-ortho.md`). `New` now asks before discarding a model. With 5,000 circles a frame takes about 2.4 ms and snapping about 1 ms (`--orbit-benchmark --curves 5000`). The owner is trying it by hand.
- Four viewports with Top, Front, Right and Perspective views, each on its own plane (`docs/specs/m1-four-viewports.md`). The owner is trying it by hand.
- Rotate, Scale, Mirror, Arc and Rectangle (`docs/specs/m1-transforms-arc-rectangle.md`). A curve can snap to itself while being drawn, circles sit on any plane, and Curva opens on Perspective.
- Live previews for every command that changes geometry, drawn one way (rule in `AGENTS.md`).
- NURBS curve storage (decision 0012).
- Array and Join (`docs/specs/m1-array-join.md`).
- Mixed curves (decision 0013), Explode, and 0 allowed as an Array count.
- Split and Trim: cutters, then click the part to remove, as Serpentine3D and Rhino do.
- Offset and Fillet (`docs/specs/m1-split-trim-offset-fillet.md`).
- `CPlane` for custom construction planes; typed coordinates now follow the plane, and `w` means world.
- Fillet now works on arcs and polyline segments as well as lines.
- Gumball with move and turn handles (`docs/specs/m1-gumball.md`).
- Layers and properties panels (`docs/specs/m1-layers-properties.md`).
- Selection, hide and show, measuring, Flip, CloseCrv and ArrayPolar.
- Display modes: Wireframe and Shaded for each viewport, from `SetDisplayMode` or by clicking the viewport's title. Perspective starts shaded, as in Rhino.
- Ellipse, Chamfer, Rotate3D, Scale1D and Scale2D.
- Point objects, Divide, ArrayCrv and Orient.
- Rebuild.
- History on Mirror and Offset (`docs/specs/m1-history.md`).
- MCP server, first part (`docs/specs/m1-mcp-server.md`): `curva --mcp`.
- Rhai scripting is finished but held on the local branch `m1-rhai-scripting` (decision 0014), because Rhai 1.27.0 still requires `smartstring`.
- MCP connected to the open window, behind the Agent access checkbox, with approval of destructive actions.
- SelCrv, SelPt and BoundingBox.
- Names, SelName, SelLayer, SelDup, Isolate and ZoomSelected. Agent access starts on.
- SelLast, SelPrev, Area, SetPt, ProjectToCPlane, Zoom (window, in, out, target, extents, selected) and
  NamedView (saved in the file, outside undo). Agent pictures now show the window's last-used viewport.
- Agent approval errs on the safe side: an answer that reads like a guarded command (`save` in NamedView)
  also asks the user.
- FilletCorners, TweenCurves (closed curves are lined up first so tweens do not twist) and Orient3Pt.
  Option words can be shortened to any start no other option shares, as in Rhino (`c` for Close).
- Control points (`docs/specs/m1-control-points.md`): PointsOn (F10) and PointsOff (F11), clicking and boxing points, Move, the gumball and SetPt on picked points, InsertKnot (also InsertControlPoint), RemoveKnot, RemoveControlPoint (and Delete with points picked) and Smooth. The owner is trying it by hand. Known gap: pictures made with `--picture` do not show the point squares yet.
- Dir: arrows show which way the chosen curves run; click a curve to flip it, or type Flip for all.
- The built-in assistant panel is postponed by the owner (10 November 2026); its draft is in `docs/specs/m1-assistant-panel.md`.

**Milestone 2 started early, at the owner's choice (10 November 2026):** first surfaces, `docs/specs/m2-first-surfaces.md`, approved with Sweep1 and the booleans added.
- Surfaces and solids in the model and in files (older files still open), drawn by their edges, and moved, turned, scaled, mirrored, copied, hidden and put on layers like curves. `Box` makes the first solids. The properties panel shows a solid's volume or a surface's area.
- Shaded display: faces lit from the camera with a little light from above, dark edges on solids, depth in every view (flat views included). Pictures made with `--picture` and for agents are shaded too. Solids are meshed once and kept until they change.
- `ExtrudeCrv` (options Solid and BothSides, distance remembered) and `PlanarSrf`. As in Rhino, an outline and the closed curves inside it become one solid or surface with holes, and separate outlines make separate objects.
- `Revolve` (axis by two points, any angle up to a full turn; a closed curve turned part way is closed at its ends), `Loft` (through the curves in the order they were made, ends left open) and `Sweep1` (rail first, then cross sections; one curve selected beforehand is taken as the rail).
- `BooleanUnion`, `BooleanDifference` (solids to cut from, then solids to cut with) and `BooleanIntersection` (two sets). As in Rhino, the solids used are replaced by the result. Combining turned and moved solids works, and the result can be moved and combined again.
- The commands of `docs/specs/m2-first-surfaces.md` are all in. The owner is trying them by hand.
- Speed with solids (`--orbit-benchmark --solids 500`, RTX 3060): orbiting 500 cylinders takes 3.5 ms a frame (4.2 ms at worst); with 5,000 circles as well, 5.3 ms (7.3 ms). Meshing happens on all cores; opening a model of 500 cylinders spends about 3.5 seconds meshing them once.
- Surface tools (`docs/specs/m2-surface-tools.md`, approved 11 November 2026): `Pipe` (Cap and Thick options) and `OffsetSrf` (Solid option). **Cap is blocked** (#67): the engine sews caps on but leaves the space inside empty; three approaches tried, written up in the issue.
- Bodies are now stored in the engine's native format, which reads back every body tried, where the portable record failed for some (a thickened sheet; a box turned then moved).
- Measuring a solid's volume aims for six significant figures, and the properties panel measures once per change, as a curved solid takes about half a second.
- Picking edges: while a command asks for edges, clicking near an edge picks it (Shift adds, Ctrl removes, a window or crossing box picks many), and picked edges show thick and yellow. `FilletEdge` and `ChamferEdge` round or bevel them, with a live preview and the size remembered; the solid keeps its id, layer and name.
- Picking faces works the same way (a picked face is tinted yellow). `Shell` hollows a solid to a wall thickness, opening the picked faces, with a live preview.
- The surface tools spec is done apart from Cap (#67). The owner is trying them by hand.

**Closing Milestone 1** (`docs/specs/m1-closing.md`, approved 12 November 2026):
- `Curve` (control points, `Degree` remembered, `Close` makes a smooth closed curve), `ScaleNU` (three factors along the construction plane, `Copy`; solids too, round faces become NURBS), `SelSrf` and `SelSolid` (they add to the selection, as in Rhino; `SelCrv` and `SelPt` still replace it), and `Volume` (six significant figures). Telling a solid from a surface now reads whether every edge is shared by two faces instead of measuring the volume.
- Selection filter: `SelFilter` shows a row in the status bar, Points · Curves · Surfaces · Solids · Disable; clicking and boxing pick only the kinds ticked. `SelFilterToggle` (or Disable) turns it off and on and keeps the ticks. The `Sel…` commands ignore it, as in Rhino. Whether a body is a closed solid is worked out once, when it is meshed.
- Ghosted display: a third mode beside Wireframe and Shaded, from `SetDisplayMode` or the viewport title. Faces are drawn at half strength over the lines, so edges behind them show dimmed and edges in front stay sharp. Pictures made with `--picture` are ghosted when the script runs `SetDisplayMode Ghosted`; agent pictures follow the viewport.
- Autosave: every 5 minutes, if the model changed, a copy goes to `Curva/recovery` in the user's app-data folder (`%LOCALAPPDATA%` on Windows, `~/Library/Application Support` on macOS, `~/.local/share` on Linux), named by process so two Curvas do not clash, with the model's own file noted beside it. A normal close deletes it. At the next start a copy left behind is offered back; restored, it saves to its original file. Saving a file now writes beside it and renames, so a failed save never leaves a half-written file.
- File format measured (`docs/decisions/0015-curva-file-container.md`): on the test scene, a zip package is as fast as today's JSON (about 25 ms to save or read) and 13 times smaller; redb was slower and larger. The owner chose the zip package, still named `.curva`: files are now a zip holding `model.json`, and the older JSON files still open.
- Both solidcore bugs reported, with the owner's approval and without mentioning Curva: tangency-labs/solidcore#3265 (a turned box's record is refused) and #3266 (stitch with caps leaves no solid, which blocks Cap). Both checked at solidcore's latest code; texts in `docs/upstream/`.
- The closing spec is done. The owner is trying it by hand.

**Floating panels** (`docs/specs/ui-floating-panels.md`, approved 12 November 2026): the viewports fill the window under floating panels with rounded corners: command tabs at the top (Curves, Surfaces, Solids, Operations), Layers and Properties on the right, and a frosted command bar at the bottom with the drawing aids and, on the right, the agent connection. A focus mode slides them away, and users can move and dock panels. The blur is Curva's own, following SolidCraft's method.
- Steps 1 and 2: the panels float over the viewports with rounded corners and shadows, laid out by a layout model (`crates/curva-app/src/layout.rs`) that already allows any panel to dock to any edge or float; the arrangement is kept in `Curva/layout.json` in the user's app data. The command tabs run their commands (a running one is ended first). Focus mode: the arrow tab at the top centre or F12 slides the panels to their edges, leaving a strip; hovering a strip peeks, clicking it brings that panel back; typing brings the command bar out. The viewports sit in the area the panels leave free; drawing them under the panels comes with the glass (step 4).
- Step 3: each panel has a grip at its top-left. Dragging it moves the panel, the place it would land lights up, and dropping it near an edge docks it there (innermost on that edge), elsewhere it floats. Floating panels resize from their bottom-right corner. Right-clicking a grip offers Reset layout. Every change is saved at once.
- Step 4: the command bar is frosted glass. The viewports now draw into a window-sized texture during egui's prepare step and are copied to the screen; the glass blurs its part of that texture (dual Kawase, two levels, Curva's own code in `crates/curva-viewport/src/glass.rs`) and draws it under its tint, rounded to its corners. Viewports reach under the glass bar so the model shows through it. On a software graphics adapter the blur is off and the tint alone shows. Orbit benchmark (5,000 circles, 500 solids, RTX 3060): median frame gap unchanged at about 6.7 ms; work per frame up about 0.5 ms.
- The floating panels spec is done. The owner is trying it by hand.

**Released 0.2.0** (12 November 2026, the owner asked for it): Milestone 1 complete and the floating panels.

**Second round of interface work** (the owner's feedback on 0.2.0, `docs/specs/feedback-2026-11-12.md`; choices in decision 0016). Order: the command framework (`docs/specs/command-framework.md`), picking, snaps and undo (`picking-snaps-undo.md`), the look and layout (`panel-system.md`, then `ui-look-and-layout.md`), a Rhino-style gumball (`gumball-rhino.md`), then logo variations (`logo-variations.md`). **Specs approved by the owner on 12 November 2026**, with every panel frosted glass by default (Glass or Solid in Settings, kept light on the machine). Building the command framework first.
- Command framework, parts 1 and 2: typed `25`, `<30` and `25<30` for every point measured from a base; heights and extrusions follow the mouse along their direction (`Request::along`); a crosshair and a cursor tag for point steps; previews in the accent orange. Commands can now be written as recipes (`crates/curva-commands/src/recipe.rs`): options in Rhino's style, remembered between runs, steps that can change with a mode, one `make` for preview and result. Line (`BothSides`), Arc (centre, typed end angle, `StartPoint` three-point) and Box (`Center`, `BothSides`, Enter uses the width, height following the mouse) are on it. `docs/commands/rhino-reference.md` starts with them, and a test keeps it and the code in step. `AGENTS.md` says how to write a command.
- Command framework, part 3: ghosted solid previews. Commands can hand the surfaces and solids they would make to `Command::preview_bodies`; the viewport draws them see-through in the accent orange in every display mode (`curva_viewport::ghost_faces`), meshed once while they stay the same. Recipes do it from their `make`, and FilletEdge, ChamferEdge and Shell show the rounded, bevelled or hollowed solid.
- Rhino audit (framework step 3): `docs/commands/rhino-reference.md` now has a row for all 100 commands, written from the Rhino 8 help pages: Rhino's steps, its options, which Curva has, and the status. SelName, SelLayer, SelDup, SelFilter and SetDisplayMode could not be read there and are marked to complete from Rhino. A test checks that every command has a row.
- Curves tab on the framework (step 4): Line, Polyline (`Close`, `Undo`), Curve (`Degree`, `Close`, `Undo`), InterpCrv (`Close`, `Undo`), Circle (`2Point`, `3Point`, `Diameter`), Arc, Ellipse (`Diameter`, `Corner`), Rectangle (`Center`, `3Point`), Point, Offset (`Distance`, `ThroughPoint`, `BothSides`), Fillet (`Radius`) and Chamfer (`Distances`). A number typed at Offset's side prompt or at Fillet's and Chamfer's picks sets the distance or radius, as in Rhino. The framework gained finishing options (`Close`), `Undo`, whole-number and length options that refuse bad values, picks on curves, and results that keep their source's layer and history.
- Operations tab on the framework: Move (`Vertical`; picked control points still move instead of objects), Copy (`Vertical`, `InPlace`; each copy is placed at once, as in Rhino), Rotate, Scale and Mirror (`Copy`; Mirror also `XAxis` and `YAxis`), Array, Join, Explode, Split, and the three Booleans (`DeleteInput`). Their previews now show solids ghosted (outlines only past eight solids, to stay smooth). Trim stays as it was for now: it changes the model pick by pick, which the framework cannot state yet.
- Surfaces and Solids tabs on the framework: PlanarSrf, ExtrudeCrv (`Solid`, `BothSides`, `DeleteInput`; Enter uses the last distance), Revolve (`FullCircle`, `DeleteInput`), Loft, Sweep1, OffsetSrf (`Solid`, `DeleteInput`), Pipe (`Cap`, `Thick`, `Diameter`), FilletEdge, ChamferEdge and Shell (`DeleteInput`). The framework gained remembered defaults that Enter gives back, edge and face picks, a single-object step, and answers a recipe can send back to be asked again. Volume, a measurement, stays as it was. The remaining commands (off the tabs) move later.

**Picking, snaps, undo and navigation** (`docs/specs/picking-snaps-undo.md`): a click anywhere on a face selects its surface or solid, the nearest winning, with curves and points still winning within their few pixels; a crossing box also takes surfaces and solids whose faces it touches. In shaded and ghosted views a snap point hidden behind a face is not offered. Resting on a snap point for half a second holds it (up to eight): it sends out dashed guidelines along the plane's X and Y and at 45° (not with ortho), and the pointer snaps where guidelines cross, where they cross objects, and midway between held points, marked in ArchiCAD's symbols. Resting again lets go; the end of the command lets all go. Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z undo and redo even while the command line has focus (unless it holds typed text), and an undo brings back the selection as it was. `UndoView` and `RedoView` (Alt+Z, Alt+Shift+Z) step through each viewport's recent views. Ctrl + right-drag zooms about where the drag began.

**Panel system** (`docs/specs/panel-system.md`): the interface is now made of pieces (the command tabs, Layers, Properties, Command, Agent), each in a panel alone or with others as tabs. Every panel shows its name centred along its top, or a tab per piece with the showing one in the accent; the name or tab is the grip. A V at the end of the title hides the panel, leaving a tab on its edge with its name along it (running upwards on a side); hovering peeks, clicking brings it back. Dragging a panel over another lights it orange and merges them on drop; dragging one tab out makes that piece a panel of its own. Floating panels settle on a 24-point grid, line up with nearby edges within 12 points and never overlap. Panels sit 24 points apart; the command bar is compact and centred at the bottom, and the agent connection has its own panel in the bottom-right corner. Older layout files are read and brought up to date.

**Look and layout** (`docs/specs/ui-look-and-layout.md`): two themes in warm greys, chosen by the system or forced in Settings, covering the panels, text, controls, the 3D background and the grid; the accent orange for selected objects, picked control points, previews, held snaps, the active tab, focused fields and selected text; Inter for the interface (`licenses/fonts/Inter-OFL.txt`). Every panel is frosted glass, or solid if chosen in Settings. The grid follows Serpentine3D's: a faint line every unit and a stronger one every 10, out to 100, with softened axes. A File tab comes first: New, Open, Save, Save As (a new command), Import and Export (waiting for Rhino files in Milestone 2), Settings and About, with Ctrl+N, Ctrl+O, Ctrl+S and Ctrl+Shift+S. Settings holds the theme, glass or solid panels, the unit and the grid. A model is in millimetres or metres, kept in its file; changing the unit asks whether to scale the model or keep the numbers, and the cursor tag shows the unit. Panels resize from their edges and corners (a floating one from all, a docked one from its inner edge and, if shorter than its edge, its ends), the edge pulsing in the accent under a resize arrow; picking objects shows a small box beside the pointer. The command buttons show Serpentine3D's toolbar icons, redrawn in Rust, with names on hover. The list of matching commands is as wide as its longest name, ten rows before it scrolls. The 3D view shades from top to bottom, #262624 to #1f1f1d in dark and #f5f4ef to #ecebe5 in light.

**Gumball like Rhino's** (`docs/specs/gumball-rhino.md`): on the selection's centre, in red, green and blue: arrows to move along an axis, circles midway to extrude (curves become surfaces, closed ones solids, flat surfaces solids; Ctrl-drag on an arrow does the same), quarter arcs to turn, small squares to scale along an axis (Shift scales evenly), plane squares tinted by their plane to move in it, and the centre dot to move on the plane. Handles light in the accent under a grab hand; while dragging, the distance, angle or factor shows beside the pointer. Clicking a handle without dragging opens a box for an exact value. Each drag is one undo step. Handles along an axis pointing at the viewer, and plane squares seen edge on, are hidden. Object snaps apply to the point dragged: along an axis the centre lines up with the snapped point, in a plane it moves to it (Alt turns snaps off).

**Logo variations** (`docs/specs/logo-variations.md`): eight variations in two directions (in the spirit of Serpentine3D, and construction lines behind a thicker C), each with the C centred and the points and polygon in the accent orange, shown large and at 128, 32 and 16 pixels on dark and light, in `docs/design/logo-variations.html`. Waiting for the owner to choose; the current icon stays until then.

**After 0.3.0** (`docs/specs/feedback-0.3.0.md`, decision 0017): fixes (typing reaches the command line again, clickable options, Ghosted snaps, Escape keeps placed copies and clears the selection, steady previews while orbiting, two-pixel lines, clipboard keys, width × depth and typed values in the pointer tag, Fillet and Chamfer previews visible); one panel system (the fixed Curva top bar with File, Settings and the specializations dial; Settings, Display and Commands as panels; closing with a question and bringing back from Settings; reorderable tabs; Vs pointing to their edges; the version at the bottom right; glass a quarter clearer); the gumball circle extruding or stretching, every action snapping; an icon for every command; updates checked on opening and installed with a click, reopening Curva; the welcome line; the AI Assistant chatting in its panel with a ChatGPT account, LM Studio or an API key, plus MCP instructions; and Rhino parity: 36 more commands (primitives, Polygon, Slot, Helix, Spiral, Parabola, Points, Lines, Plane, SrfPt, ExtrudeSrf, ArrayLinear, Align, BooleanSplit, DupEdge, ChangeDegree, measuring and selection commands, HideSwap), with `docs/commands/rhino-parity.md` listing every in-scope Rhino command: 132 in Curva, 31 still to write, 34 waiting on the geometry engine.

**After 0.4.0** (`docs/specs/feedback-0.4.0.md`, decision 0018): released as 0.4.1, as each release now adds 0.0.1. No console window on Windows; lines drawn once as strips with their own width (`docs/performance.md`), and print widths in millimetres on layers and objects, shown in every view; exact DupEdge, ChangeDegree, centroids, ExtrudeSrf (with a NURBS toolkit and face joining in the kernel); the last 31 missing Rhino commands, so `docs/commands/rhino-parity.md` lists 163 in Curva and 34 waiting on the geometry engine; the refined Drafting logo with its variations (`docs/design/logo-drafting-variations.html`), waiting for the owner's pick.

**First release, 0.1.0 (12 November 2026, the owner asked for it):** a preview to show where Curva is. Pushing a tag `v*` runs `.github/workflows/release.yml`, which builds Windows, Linux and macOS (Apple silicon), adds `THIRD-PARTY-LICENSES.html` (made by `cargo-about` from `about.toml` and `about.hbs`) and the notices in `licenses/`, and publishes a GitHub pre-release with the notes in `docs/releases/<tag>.md`. Builds are not signed; on Windows a console window opens beside Curva.

**0.1.1 (12 November 2026):** at the owner's request, releases come as a Windows installer (`.exe`, Inno Setup, per-user, no administrator rights), a Linux AppImage (built on Ubuntu 22.04 for older systems) and a macOS `.dmg` holding `Curva.app` (ad-hoc signed only). Packaging files are in `packaging/`. Pull requests that change the packaging build and check all three without publishing. The icon is a C drawn as an arc, with a thin yellow line joining its three yellow control points, chosen by the owner on 12 November 2026. Its source and the files made from it are in `packaging/icons/` (`curva.svg`, PNGs at 256 and 1024 px, `curva.ico`); it is in the Windows program and installer, the AppImage, the macOS app (an `.icns` made by the release workflow), the window's title bar and task bar, and the README.

**Icon (13 November 2026):** the owner chose the Drafting mark with its architectural construction (`docs/design/curva-logo-drafting.svg`, centred on its circle). `packaging/icons/` holds it as `curva.svg`, PNGs at 256 and 1024 px, and `curva.ico` (16 to 256 px; the sizes up to 48 from `curva-logo-drafting-small.svg`, which leaves the construction out). The PNGs are rendered from the SVGs with headless Edge, and the `.ico` packs them as PNG entries.

**Icon (15 November 2026):** the owner chose a new logo, replacing the Drafting mark (decision 0022, `docs/specs/logo-final.md`): a C drawn as an arc in plan and extruded into a solid, in greyscale, lit from the top left, with no orange. `packaging/icons/` holds it under the same names (`curva.svg`, PNGs at 256 and 1024 px, `curva.ico` at 16 to 256 px, `hicolor/` at 16 to 512 px); 48, 32 and 16–24 px come from versions drawn for those sizes. All logo sources are in `docs/design/brand/`; the Drafting files moved to `docs/design/archive/` and `docs/specs/archive/`. The brand guidelines live in the Curva design system on claude.ai.

**Waiting for the owner**
- Nothing yet. The engine is solidcore, with Serpentine3D's commands as the base (decision 0010). SolidCraft
  reuse is parked (0009).

**Testing**
- Agents do not run tests through Rhino (decision 0011). Curva is verified by its own checks.
- solidcore ran 28 of the 31 cases; results and known gaps are in 0011 and `evaluation/reports/solidcore.tsv`.

**Handoff notes**
- `V:` is a file-based Dev Drive (`C:\DevDrives\curva.vhdx`), reconnected at startup by the
  scheduled task "Attach Curva Dev Drive". Reconnecting after a restart has not been tested yet.
- The old copy in `C:\Projects\curva` is out of date and no longer used.
- Agents merge their own pull requests once checks pass; the owner does not click merge.

**Known problems / risks**
- A pipe along a rail with a sharp corner (a polyline) is refused by the engine; Rhino mitres the corner. Smooth rails work.
- solidcore runs one operation at a time behind a single lock. Until solidcore fixes it, Curva carries a patched copy of `solidcore-ops` with a kernel per thread (decision 0037, `crates/curva-kernel/vendor/solidcore-ops`, offered upstream as tangency-labs/solidcore#3358); delete it once solidcore has the fix and Curva's tests and timings pass on it.
- solidcore cannot read back its own record of some turned bodies (status 1007): a box away from the origin, turned about z by certain angles (17°, 29°, 37° and others), is refused; at the origin every angle works. Moving is not the cause, as first thought. Curva avoids it by keeping each body as made plus one placement (`crates/curva-kernel/src/body.rs`). Reported as tangency-labs/solidcore#3265.

- Serpentine3D's geometry is done by OpenCascade (C++), so its geometric logic cannot be ported
  directly; only commands, prompts, options and interaction flow port. See decision 0003.
- The kernel choice is untested. M0 exists to test it before M1 starts. See decision 0004.
