# Spec: the new display pipeline

**Status:** Draft for the owner's approval · 15 November 2026 (display stream, #126).
**Covers:** the owner's brief of 15 November 2026: detail that follows the zoom, smooth edges,
display modes as editable files, and speed on large scenes.
**Rules:** 100% pure Rust, no C++ libraries. Runs well on 2020 Intel integrated graphics (Intel UHD
620 and Iris Xe) and up. Anything newer, such as mesh shaders, is an optional extra, never required.

## 0. In short

| What you will notice | How |
|---|---|
| Circles and arcs stay perfectly round however far you zoom in; spline curves and surface edges stay within half a pixel of the true shape | Detail is worked out from the zoom, every frame, instead of once |
| Smooth, non-jagged edges on lines and on the outlines of shaded objects | Anti-aliasing (edge smoothing), on every machine |
| Display modes are settings you can edit, copy and share as files, like Rhino's | A few shared drawing programs on the graphics card, steered by each mode's settings |
| Large models stay smooth: tens of thousands of objects, thousands of repeated trees or chairs | Only what changed is sent to the graphics card; repeated objects are drawn once and placed many times; hidden and off-screen objects are skipped; distant ones are simplified |
| A model too large for the graphics card shows with less detail instead of crashing | A memory limit that lowers detail first |
| Models far from the origin (real map coordinates) no longer shimmer | Drawing measured from the camera, not from the origin |

Nothing changes in how commands, picking or snapping behave. The work is split into about fifteen
small pull requests (section 7), each merged on its own, so the features stream always works on a
current `main`.

## 1. Where the display stands today

Checked in the code on 15 November 2026:

- **Curves are cut into a fixed number of pieces**, whatever the zoom: a circle is always 128 straight
  segments, a spline 32 per control point. Zoomed in, arcs show as polygons.
- **Solids and surfaces are meshed once**, to a tolerance of a thousandth of their size, and kept until
  they change. Zoomed in on a large solid, its curved faces and edges show facets.
- **No anti-aliasing.** The line-weights spec asked for smooth line edges, but the line shader draws
  solid pixels; outlines of shaded faces are jagged too. This spec fixes both.
- **The whole scene is one block of data on the graphics card**, rebuilt and sent again whenever
  anything changes, *including the selection*, because colours are baked into it. Selecting one
  object in a large model rebuilds everything.
- **No culling:** everything is drawn every frame, on or off screen, hidden or not.
- **Copies are drawn as separate objects**, although the engine already shares one mesh between
  copies of a solid.
- **Positions are single precision measured from the origin.** Far from the origin they shimmer
  (`architecture.md` §20 asks for drawing measured from the camera).
- **Depth is 24-bit**, which makes distant faces flicker through each other in large scenes.
- **Display modes are three switches in code** (Wireframe, Shaded, Ghosted, plus Zebra and Draft
  angle as overlays).
- **Speed has only been measured on an RTX 3060.** Nothing is known about Intel integrated graphics.

## 2. Detail that follows the zoom, and smooth edges

### 2.1 The promise

At any zoom, a drawn curve, edge or surface outline is **within half a pixel** of the true shape.
"Pixel" means a real screen pixel, so a high-resolution laptop screen gets proportionally finer
detail. Straight lines and polyline corners are exact, as now.

### 2.2 Circles, arcs and ellipses: exact at any zoom

Circles, arcs and ellipses are sent to the graphics card as what they are (centre, axes, radius,
angles), not as points. Each frame, the graphics card cuts each one into just enough segments for
its size on screen: a segment's sag (its distance from the true arc) stays under half a pixel. A
circle 20 pixels across gets about 10 segments; one filling the screen gets a few hundred. Zoomed
so far in that the circle is far larger than the screen, only the part on screen is cut up. This
costs almost nothing, needs no work on the main processor, and is exact at every zoom.

### 2.3 Spline curves and edges of solids

Free-form curves (NURBS) and the edges of solids are cut up on the main processor, at a ladder of
**detail levels**, each twice as fine as the one before. Each frame, each object uses the coarsest
level whose error on screen is under half a pixel. Levels are made in the background on all cores,
largest-on-screen first; until a finer level is ready the object shows the level it has, so a frame
never waits. Zoomed far in, only the spans of the curve that are on screen are refined. The cutting
method (a flatness test on the curve's control polygon, after splitting it into Bézier pieces) is
cited from *The NURBS Book* in the code.

### 2.4 Curved faces

The engine can mesh a face to any tolerance. Faces use the same ladder of levels. Two points matter
for how it feels:

- **Outlines and edges keep the half-pixel promise.** The inside of a face already looks smooth,
  because its lighting is smoothed between mesh points.
- **Finer face meshes arrive a moment after the zoom stops** (aimed at under half a second for a
  typical object), because meshing is slow: 500 cylinders take about 3.5 seconds to mesh today,
  and the engine meshes one body at a time. Only faces that are on screen and large are refined.

### 2.5 Smooth edges (anti-aliasing)

- **Lines** (curves, edges, the grid) get soft edges worked out in their own shader: each line's
  outer half pixel fades out. This is cheap and works on every machine, with or without the next
  point.
- **Outlines of shaded faces** are smoothed by multisampling (the graphics card samples each edge
  pixel four times). Every graphics card Curva supports can do 4× multisampling, including 2020
  Intel ones.
- **Setting:** Settings › Display › Anti-aliasing: *Automatic* (default), *Off*, *2×*, *4×*, *8×*
  where the card offers it. *Automatic* starts at 4×, and drops to 2× if frames take longer than
  16 ms on that machine; it never turns line smoothing off.

## 3. Display modes as editable files

### 3.1 What the user sees

As in Rhino's *Display Modes* options page:

- The **Display** panel lists the modes. Built-in modes are read-only; **Copy** makes an editable
  copy, **New**, **Rename**, **Delete**, **Import** and **Export** do what they say.
- Editing a setting changes every viewport using that mode **at once**, with no reloading.
- Each mode is **one file**, so it can be emailed, kept with a project, or shared with a team.
  Files dropped into the modes folder appear in the list.
- `SetDisplayMode` and the viewport title menu offer every mode by name, built-in and user-made.

### 3.2 Built-in modes

| Mode | Looks like |
|---|---|
| Wireframe | As now: curves and edges only |
| Shaded | As now: lit faces, dark edges on solids |
| Ghosted | As now: faces at half strength, edges behind dimmed |
| X-Ray | Faces faint, every edge shown at full strength, hidden ones included |
| Technical | White faces, outlines (silhouettes), edges and creases in black; hidden edges hidden |
| Pen | Like Technical on a white background, lines only, for a sketch-like look |

Zebra and Draft angle stay as analysis overlays on top of any mode, as now. A rendered mode with
materials comes with materials, later.

### 3.3 What a mode can set

Grouped as Rhino groups them; anything not set uses the built-in Shaded value.

- **Background:** the theme's background, one colour, or a top-to-bottom gradient of two colours.
  Grid and axes on or off.
- **Faces:** off, or shaded; colour from the object, the layer, or one colour for all; transparency
  0–100%; back faces the same, a separate colour, or hidden; lighting: from the camera (as now),
  three studio lights, or flat (no lighting).
- **Edges:** surface and solid edges on or off; width (a multiple of each object's own width);
  colour from the object or one colour; silhouettes (outlines) on or off, with width and colour;
  creases on or off; edges hidden behind faces: shown, dimmed or hidden.
- **Curves:** on or off; width multiple; colour from the object or one colour.
- **Points and control points:** size and shape (square, round, cross).
- **Selection:** highlight colour (the accent by default) and whether highlighted lines are wider.

Isocurves (lines across a surface, as in Rhino) are listed in the file format from the start but
wait on the engine giving them.

### 3.4 The files

- One file per mode, named `<mode name>.curvadisplay`, in plain readable text (TOML, a simple
  settings format), in `Curva/display-modes/` in the user's app-data folder (the same place as
  `layout.json`). Built-in modes live inside the program, so a broken file can never lose them.
- Each file starts with a format version. Unknown settings are kept when saving, so a file made by
  a newer Curva survives a round trip through an older one. A setting with a bad value falls back
  to its default and the Display panel says which line is wrong; a bad file never stops Curva
  starting.
- A `.curva` model remembers each view's mode by name. If that mode is missing on another machine,
  the view shows Shaded and the status line says which mode was missing. Older files' Wireframe,
  Shaded and Ghosted map to the built-ins.

### 3.5 A few shared shaders

A shader is a small program the graphics card runs to draw. Instead of one per mode, there are
four, and each mode only changes the values they are given:

1. **Lines:** curves, edges, silhouettes and the grid, with width and edge smoothing; one variant
   cuts circles, arcs and ellipses itself (section 2.2).
2. **Faces:** colour source, transparency, lighting, back faces, and the Zebra and Draft overlays.
3. **Points:** markers of every shape.
4. **Screen:** the background, outlines found from the depth picture (silhouettes and creases for
   Technical and Pen), and the hand-over to the glass panels.

Settings that cannot be a value (transparent or not, edges hidden or shown through faces) choose
among a handful of prepared variants, kept ready, so switching modes never stutters.

## 4. Speed on large scenes

### 4.1 Objects kept on the graphics card

Each object's drawing data is stored once on the graphics card, in a few large shared blocks, with
a small record per object (where it is, its colour, width, highlight). Changing one object sends
only that object. **Selecting or hiding sends a few bytes**, not the whole scene. The view draws
from the records, so moving the camera sends almost nothing.

### 4.2 Repeated objects (instancing)

Objects with the same shape are stored once and drawn many times, each with its own placement:
copies, arrays, block instances (`docs/specs/blocks.md`) and copies of solids that the engine
already shares. 2,000 trees from one block cost about as much memory as one tree, plus a few bytes
each.

### 4.3 Skipping what cannot be seen (culling)

- **Off screen (frustum culling):** objects are kept in a tree of boxes, so each view finds what is
  inside it without testing every object. Done for all four viewports on all cores.
- **Hidden behind other objects (occlusion culling):** in shaded modes only (in Wireframe, Ghosted
  and X-Ray everything shows through). Each frame first draws what was visible last frame, builds a
  coarse depth picture from it, then tests the rest against that picture on the graphics card and
  draws the ones that turn out visible. This two-pass method (after Greene, Kass and Miller,
  *Hierarchical Z-buffer visibility*, 1993) means objects never pop in late when the camera turns.
  On a card without the needed features, a simpler version runs one frame behind on the main
  processor.
- **Tiny objects never vanish:** an object under two pixels on screen draws at its simplest level,
  but it is still drawn.

### 4.4 Simplified clusters for distant geometry

- **Curved objects from the engine** simply use a coarser level of the ladder in section 2 when far
  away; no extra work.
- **Heavy meshes** (imported meshes, render meshes from Rhino files, very detailed solids) are split
  into clusters of about 128 triangles, with simplified versions of groups of clusters, each chosen
  by its size on screen, so a distant part of a large mesh uses few triangles and a near part all of
  them. Simplification follows Garland and Heckbert, *Surface simplification using quadric error
  metrics* (1997); the cluster arrangement follows Karis, *Nanite: a deep dive* (2021). Written in
  Curva's own Rust; no C++ mesh library.
- **Many small distant objects** (a forest of block instances seen from far away) are merged into
  one simplified stand-in per group.
- On cards that offer mesh shaders, clusters may be drawn through them as an optional faster path;
  the normal path draws the same result without them.

### 4.5 A memory limit that lowers detail

- A limit on the graphics memory the viewport may use: *Automatic* (1 GB on integrated graphics,
  which share the computer's memory, 3 GB on others) or a number, in Settings › Display.
- When the limit is reached, Curva frees the **finest levels of the least visible objects first**,
  then the data of objects off screen (kept only as boxes, reloaded when needed). It never refuses
  to draw a visible object; at worst it draws it at its simplest level.
- If the graphics card still reports running out of memory, the limit is lowered and the frame is
  drawn again, with a note in the status line. Curva does not crash.
- The status line can show the memory in use and the share of objects at full detail (Settings ›
  Display › Show display statistics).

### 4.6 Precision far from the origin

Positions are kept in double precision and drawn measured from a point near the camera, moved when
the camera travels far. Depth uses 32-bit floating point arranged so precision is greatest far
away, which stops distant faces flickering through each other. A model at real map coordinates
draws as steadily as one at the origin.

## 5. Machines

- **Baseline:** 2020 Intel integrated graphics (UHD 620, Iris Xe) through Vulkan or DirectX 12 on
  Windows and Linux, and Apple's M1 and later through Metal. Everything in sections 2 to 4 works
  there.
- **Optional extras, used only when the card offers them:** drawing many objects in one call (on
  most cards; without it, a loop of calls does the same), mesh shaders, 8× multisampling. Each has
  a normal path giving the same picture.
- CI has no graphics card; its tests draw with software graphics (Mesa on Linux, WARP on Windows),
  which has every feature of the baseline.

## 6. How it is checked

- **Half a pixel, measured:** tests set up a camera at many zooms (up to a million times) and
  measure the largest distance on screen between the drawn segments and the exact curve or edge;
  it must be under half a pixel. Same for face outlines at each detail level.
- **Pictures:** reference pictures drawn with software graphics in CI: a circle zoomed 1,000 times
  shows no corners; each built-in mode; smooth edges. The three current modes must look as they do
  today, apart from the smoother edges, which the pull request will show side by side.
- **Display mode files:** saved and read back unchanged; unknown settings kept; bad values fall
  back with a message; a missing mode falls back to Shaded.
- **Culling changes nothing visible:** with culling on and off, the shaded picture is identical.
- **Memory limit:** a scene far over a tiny limit draws, at lower detail, without crashing; a
  simulated out-of-memory error lowers the limit and draws again.
- **Speed** (run by the display stream only, results that look off rerun): standard scenes added
  to `--orbit-benchmark`: *Medium* (5,000 curves, 500 solids, as now), *Large* (10,000 solids,
  50,000 curves, 2,000 instances of a 5,000-triangle tree) and *Far* (*Large* placed 500 km from the
  origin). Recorded in `docs/performance.md` on the RTX 3060 and on Intel integrated graphics.
  **Targets on Intel integrated graphics at 1920×1080:** *Medium* orbits at 60 frames a second,
  *Large* at 30 or better; selecting an object in *Large* costs under 1 ms. These targets are set
  after the first measurement (step 1), and the owner is told if they look unreachable.

## 7. Steps, each a small pull request

1. **Measure first:** the *Large* and *Far* scenes, and a baseline on the RTX 3060 and Intel
   integrated graphics.
2. **Precision:** drawing from the camera, 32-bit depth. *Far* stops shimmering.
3. **Smooth edges:** line edge smoothing, multisampling and its setting.
4. **Objects kept on the graphics card**, records per object; selection no longer rebuilds the scene.
5. **Circles, arcs and ellipses exact at any zoom** (section 2.2).
6. **Spline curves and edges** at levels (section 2.3).
7. **Curved faces** at levels (section 2.4).
8. **Display mode settings and shared shaders**, with the three current modes rebuilt on them and
   looking as before.
9. **Mode files** and the Display panel editor; X-Ray.
10. **Screen pass:** silhouettes and creases; Technical and Pen.
11. **Instancing.**
12. **Off-screen culling.**
13. **Memory limit.**
14. **Occlusion culling.**
15. **Clusters and stand-ins for distant geometry**; mesh shaders as an optional path.

Each step is measured before and after, and the result goes in `docs/performance.md` and
`status/display.md`. Steps 1 to 4 come first because everything else builds on them.

## 8. Changes outside the viewport crate

The display stream owns `crates/curva-viewport`. A few steps need small changes elsewhere:

- `curva-kernel`: two new functions, cutting a curve and meshing a body to a given tolerance
  (steps 6 and 7). Additions only; nothing existing changes.
- `curva-commands`: `SetDisplayMode` takes any mode name instead of three fixed ones (step 9).
- `curva-app`: where the window hands the viewport its views (steps 2, 4 and 8), the Display panel
  editor (step 9), and the new Display settings (steps 3, 13).

See decision B below for how.

## 9. Decisions for the owner

**A. A machine with 2020 Intel integrated graphics.** The speed rule can only be checked on one.
Today only the RTX 3060 is measured.
- *Option 1 (recommended):* a used laptop with Intel Iris Xe or UHD 620 graphics (roughly €250–400),
  set up once with exact steps I write for you, which agents use for benchmarks over the network.
  Real numbers on the real target.
- *Option 2:* limit the RTX 3060 machine to imitate a weaker card. Free, but the numbers can mislead
  in both directions: integrated graphics are slow in different ways.
- *Option 3:* no low-end machine; rely on users' reports after release. Risky: slowness would be
  found late.

**B. Who edits the code outside the viewport that the display needs** (section 8).
- *Option 1 (recommended):* the display stream makes those changes itself, each in its own small
  pull request touching only what it names in section 8, and says so in `status/display.md`, so the
  features stream sees them coming. Fastest, and keeps the two streams from waiting on each other.
- *Option 2:* the display stream opens an issue for each and the features stream does it. Cleaner
  separation, but each step waits for the other stream.

**C. Display modes in a model file.**
- *Option 1 (recommended):* modes live in the user's folder; a model remembers only the mode
  names, and Export makes a file to share. As Rhino does.
- *Option 2:* a model also carries a copy of every user-made mode it uses, so it looks the same on
  any machine. Convenient for teams, but the same mode can then exist in two versions, which
  confuses.

## 10. What this spec does not cover

Rendered mode with materials and textures, shadows, the Blender / Cycles bridge, SubD and mesh
modelling objects, printing (already in `layouts.md`), and the look of the interface panels.
