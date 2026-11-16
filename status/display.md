# Status: display stream

*Written only by the display stream, which owns the inside of `crates/curva-viewport` (see
`AGENTS.md`). Updated at the end of every work session. Newest first.*

## 16 November 2026

**Done**
- **Plan for the display on Bevy approved** (`docs/specs/display-on-bevy.md`, #242): seven steps
  (faces, lines, Bevy only, copies and far coordinates, speed, styles as settings, plans and
  sheets), targets including "faster than Rhino" on every scene. Owner's decisions: Bevy becomes
  the only 3D engine at step 3; Zebra and Draft angle dropped for now; machines without a graphics
  card run Bevy on software graphics.
- **Decisions 0029 and 0030 merged:** Bevy draws every view, 3D and 2D (Vello not adopted, after
  the plan accuracy test #235); use Bevy as it ships, custom parts marked, listed
  (`docs/bevy/custom-parts.md`) and checked at every upgrade; `architecture.md` §6b on how Curva,
  solidcore and Bevy fit together.
- **Step 1, faces in Bevy, in review** (#223): each Bevy view has its own copy of the objects on
  its own render layer, sharing meshes, with Bevy's own materials and lights. The owner chose the
  Studio look for base Shaded (a light from the viewer, slightly above and to the left, one from
  above, Bevy's gradient environment reflected, shine 0.6), after pictures beside Rhino's Shaded;
  Sunlit (a sun with shadows) for presentation. Curva's own background gradient stays, as a flat
  backdrop. Selection, Wireframe, Ghosted. `--bevy` on the benchmark.
- **Rhino measured on the same scenes** (the owner's reference, `docs/performance.md`): all four
  views about 10 ms on *Medium*, 167 ms on *Large*; Curva 2.9–3.4 and 24.9–26.2 ms; Bevy faces
  5.7–5.8 ms on *Medium* (Bevy's fixed cost per frame, about 2.5 ms, to work on in step 5).
- **Pilot step 1 merged** (#228): Bevy running inside Curva on its own device. A clean build takes
  about twice as long (306 → 601 s, debug). The owner allowed the MIT-0 licence (`encase`).
- **Disk full on V:**, builds failed. Cleaned the display stream's old build folders and the main
  folder's stale debug cache (about 60 GB). From now on worktrees and spike build folders are
  removed when done.
- **The owner chose Bevy for the 3D views** ("go ahead with your recommendation"): decision 0028
  (#217, merged). Bevy draws faces, meshes and SubD; Curva keeps its CAD layer. 0027 marked
  partly replaced (its 3D sentence).
- **Pilot spec approved by the owner** (`docs/specs/bevy-pilot.md`, #218): one view behind a
  setting, the pass targets as written; Bevy's own lines considered and not used (no
  paper-millimetre weights, exact arcs, outlines, cuts, snapping or precision far away). Its six
  steps are issues #222 to #227.
- **#180 tidy-up merged** (#220): `lib.rs` and `gpu.rs` split into modules named by what they
  do, the nine functions over 80 lines broken into named steps, tests unchanged (119 pass). Speed
  unchanged: *Medium* 2.6–3.0 → 2.7 ms between frames, *Large* 23.1–23.2 → 23.3–23.4 ms, card time
  the same, two runs each on a quiet machine.
- **#199 viewport-lib and #198 Bevy tried inside Curva, reported, decided (0028), issues closed.** Both on
  Curva's device, from Curva's camera, on the same scenes (`spikes/common/scenes.rs`, shared by all
  renderer spikes), each measured twice on the RTX 3060. Branches `199-viewport-lib-spike` and
  `198-bevy-spike`, sources only, not merged. *Large* at 1920×1080: viewport-lib 38 ms per frame
  with lines kept (540 ms its normal way), 1 GB; Bevy 13.4 ms (21 ms with SSAO, TAA, shadows), 374 MB,
  first frame under 1 s. Curva today at 1280×800: *Large* 22 ms on the card, first frame 62 s, about
  2.1 GB; the difference is mostly instancing (step 11).
  - viewport-lib: GPL-3.0-only (clashes with decision 0025), one author, a crash with 4-sample
    smoothing plus lines, no splitting of big line blocks. Recommendation: borrow ideas only.
  - Bevy: MIT/Apache; SSAO and TAA need more than Curva's baseline device (allowed on recommended
    cards by decision 0026); metres assumed; single precision far away; no cuts, selection outline
    or usable picking. Recommendation: adopt with conditions (updated for meshes and SubD).
  - Updated for meshes and SubD (owner, 16 November): two more tests. Big one-off meshes
    (15.7 million triangles): Bevy 3.8 ms per frame and 569 MB, viewport-lib 11.9 ms and 1.2 GB;
    Curva's face format would need about 1.9 GB (120 bytes per triangle, no shared corners). SubD
    surface of 524,000 triangles changing every frame: Bevy 4 ms, viewport-lib 11 ms. On Curva's
    own route, meshes bring back steps 13 to 15 and add shared corners, textures, mesh wires and
    live edits.
  - Comparison of the three routes: adopt Bevy for the 3D views on conditions, Curva keeping its
    CAD layer (lines, outlines, cuts, styles, picking, precision). First a pilot the owner approves:
    Bevy draws one view's faces behind a setting, Curva's lines and outlines as a Bevy pass.
- Status order updated (#200, merged).

## 15 November 2026

**Done**
- **Snapping about twice as fast** (#130, first part, merged as #151): the objects and snap places sit in a tree
  of boxes made once per model change, so snapping looks only near the cursor and finds the same
  points. Snapping per frame: *Medium* 3.2 → 1.3 ms, *Large* 45 → 20.5 ms; frame gaps *Medium*
  4.8 → 2.6 ms, *Large* 47 → 23.5 ms. What is left on *Large* is crowding (zoomed all the way out,
  115,000 segments within the cursor's reach); #130 stays open for it.
- Step 4b, **selecting no longer freezes Curva** (#144, merged as #148): one selection change took 117 ms on
  *Medium* and 2.3 s on *Large*; now 5.7 ms and 57 ms, about an ordinary frame. Scene vertices
  carry their object's number and the graphics card looks it up in a selection table (one byte
  per object), drawing selected objects in the selection colour and lines a pixel wider, exactly
  as before (a test compares the pictures pixel for pixel). The benchmark measures it with
  `--select`. Cost: the card's time per frame is up 8–17%. A change on *Large* still costs about
  9 ms more than an ordinary frame (the spec aims under 1 ms): to look at.
- Step 4a, **Large opens** (#141, merged as #143): uploads of lines and faces go in blocks of at most 64 MB, and
  views showing the same lines share one copy instead of each sending its own. *Large* orbits at
  47 ms between frames (it crashed before); *Medium* is unchanged. Its first frame takes 58 s and
  it holds about 2.1 GB on the card, both from the 2,000 trees written out one by one (step 11).
  The benchmark's work figure now leaves out the opening frames. Fixed on the way: the card's timer
  dropped passes shorter than one tick of its clock, which made its test fail now and then.
- Step 1c, the baseline kept (#138, merged as #139): pictures made without a window (for agents and `--picture`)
  ask for their device as the window does, and a test draws a shaded face and lines of three
  widths with every optional extra off and on and checks the pictures are identical. Step 1 is
  complete.
- Step 1b, the graphics card's own time (#132, merged as #137): the benchmark reports it, read from the card's
  clock. *Medium* 1.5 ms, reduced *Large* 7.1 ms per frame; the processor is the bigger cost on both
  (`docs/performance.md`). The window now asks the card for its device through
  `curva_viewport::device_descriptor`: the baseline limits, plus optional extras only when present.
  Found on the way: building *Medium*'s 500 cylinders takes about 35 s, about 60 ms per extrusion in
  the engine (#136, for the features stream).
- Step 1a, benchmark scenes (#129, merged as #131): `--orbit-benchmark --scene medium|large|far`,
  plus `--splines` and `--trees`; the report says what the scene holds, how long it took to build and when the first
  frame showed. Baseline on the RTX 3060 in `docs/performance.md`:
  - *Medium* orbits at 4.8–4.9 ms between frames.
  - ***Large* does not open: Curva crashes.** All lines, and all faces, go into one block of
    graphics memory, and the card refuses a block over 256 MB; each part of *Large* alone is too
    big (curves 376 MB, solid copies 580 MB, trees 1,154 MB). The largest scene that opens holds
    roughly 35,000 circles' worth of lines. Steps 4, 11 and 13 remove this ceiling.
  - Reduced *Large* (20,000 circles, 5,000 splines, 4,000 solids) orbits at 20 ms between frames,
    of which **snapping takes 17.6 ms** (#130).
  - Measured on a 1280×800 display (the machine was reached over Parsec); the spec's targets are
    for 1920×1080.
- Spec for the new display pipeline approved and merged (`docs/specs/display-pipeline.md`, #127):
  15 steps (§7). Decisions: no Intel test machine, so the baseline is kept by design and by margins
  on the RTX 3060 (§5); the display stream makes the few changes outside the viewport that §8
  lists, announcing each here first; display modes live in the user folder, later to be tied to a
  Curva online account.
- Status split per stream and the display stream's rules in `AGENTS.md` (#125).

**In progress / next** (owner, 15–16 November 2026: decisions 0024, 0026 and 0028)
- **Recommended graphics card** (decision 0026): an RTX 3060, or an AMD or Intel card of the same
  class, with fallbacks for weaker machines. Speed targets are set on the RTX 3060 itself
  (`docs/specs/display-pipeline.md` §5 and §6); 2020 Intel integrated graphics are the fallback
  case.
- **3D direction** (owner, 16 November 2026, decision 0028, replacing the 15 November direction):
  Bevy draws the 3D views' faces, meshes and SubD, with its shading, shadows, ambient occlusion,
  smoothing and instancing; Curva keeps its CAD layer (paper-millimetre lines, exact arcs, dashes,
  outlines, cuts and fills, styles, picking and snapping, precision far from the origin). A pilot
  comes first. re_renderer and viewport-lib are not used as renderers; re_renderer's line and
  outline technique may still be borrowed for the CAD layer.
- **2D direction** (decision 0029, replacing 0027): Bevy draws plans, sections and sheets too,
  from the drawing description; Vello not adopted.
- Order of work (owner, 16 November 2026):
  1. **The display on Bevy** (`docs/specs/display-on-bevy.md`, approved): step 1 faces (#244),
     step 2 lines and clipping planes (#248, #249) in review; step 3a, Bevy draws every view, in
     review; then 3b pictures for agents through Bevy, 3c Curva's own surface drawing deleted.
  2. **Closed:** #180 (merged), #186, #198, #199 (decided by 0028; spike branches kept unmerged for
     reference), #197 (superseded; its Diagram-look comparison is in pilot step 5).
  3. **Retired:** steps 5 to 15 of `display-pipeline.md` (Bevy has instancing and culling; exact
     curves move into the line drawer); #153 to close once step 4 lands.
- **Shelved** (decision 0024): steps 14 (occlusion culling) and 15 (clusters, mesh shaders), back
  only if a measured real model is still too slow after steps 11 to 13. The styles prototype's
  third look (Textured) and fine-tuning of the looks wait until the drawing is correct.
- **Correct drawing:** seams no longer drawn in shaded views (#183, merged); outlines of curved
  shapes in shaded views (#189); still to do: outlines in Ghosted and Wireframe (then seams go
  there too), edges that stay whole where a surface is seen at a steep angle, and edges that
  follow the true shape when zooming in (spec steps 5 to 7).
- **Styles prototypes merged:** Diagram (#168: white surfaces, thin grey inner lines, bold
  outlines, soft shading and ambient occlusion) and Architectural (#173: a plan cut at 1.2 m with
  solid black cut walls and lines that thin and lighten with distance).

**Heads-up for the features stream** (changes outside the viewport, decision B)
- **Bevy steps 1 to 3a (#244, #248, #249, and 3a):** `curva_viewport::Drawing` has a new field
  `camera` (the view's camera); `Engine::Bevy` draws every model view and every detail on a sheet
  (`viewports.rs`, `sheets.rs`). The "3D engine" choice, `Pane::engine`, its three words in
  `en.txt` and the benchmark's `--bevy` are gone again. `curva --look-pictures <folder>` makes
  pictures of Bevy's looks; `save_bevy_picture` draws a picture through the window's own path.
- **Bevy pilot step 1 (#228):** `Pane` in `crates/curva-app/src/viewports.rs` has a new field
  `engine`, and the view's menu a "3D engine" choice; `curva_viewport::Drawing` has a new field
  `engine` (`Engine::Curva` keeps today's drawing, as `sheets.rs` does); `main.rs` stores a
  `curva_viewport::PilotSeed`; three new entries in `crates/curva-commands/locales/en.txt`. The
  viewport crate now depends on Bevy 0.20.1, which lengthens clean builds.
- **`curva_kernel::BodyMesh` has a new field `seams`** (one flag per edge): edges where one face
  meets itself, which the engine does not mark (`seam_edges` in `crates/curva-kernel/src/body.rs`).
  Shaded views leave them out; wireframe, previews and prints keep them until outlines of curved
  shapes are drawn there.
- Done: `crates/curva-app/src/benchmark.rs` and the `--orbit-benchmark` arguments in
  `crates/curva-app/src/main.rs` (#131); the window's graphics set-up in `main.rs` now uses
  `curva_viewport::device_descriptor` (#132).
- Done in step 4b (#144): `refresh_scene` in `crates/curva-app/src/viewports.rs` builds the scene
  with `curva_viewport::SceneParts` once per model change and hands the selection to the viewport
  through `Drawing::objects` and `Drawing::selection`; `sheets.rs` and `clipping.rs` give the new
  fields. Pictures for agents still draw the selection into colours, unchanged.
- **`curva_viewport::Vertex` and `MeshVertex` have a new field `object`** (#144). Code that makes
  them by hand sets `object: curva_viewport::NO_OBJECT`, as `clipping.rs` now does.

**Open `display` issues**
- #130: snapping now goes through an index of the objects (about twice as fast); what is left on
  *Large* is crowding: zoomed out, 115,000 segments lie within the cursor's reach.
- #136 (not display, for the features stream): extruding a circle takes about 60 ms.

**Where the display stands today** (history before this date is in `status/features.md`)
- Wireframe, Shaded and Ghosted modes, fixed in code. Solids are meshed once, to a tolerance set
  by their size, and kept until they change. Curves are cut into a fixed number of segments
  whatever the zoom (a circle always 128), so arcs look like polygons when zoomed in. Edges are not
  anti-aliased.
- Lines are drawn once as strips with their own width (`docs/performance.md`).
- Frosted glass panels blur the viewport texture (`crates/curva-viewport/src/glass.rs`).

**Known problems / risks**
- *Large* opens but needs about 2.1 GB on the graphics card; on integrated graphics, which share
  the computer's memory, a model that size could still fail until steps 11 and 13.
- No measurements on the target low-end machine (2020 Intel integrated graphics); there will be
  none (owner's decision A).
- Benchmarks are noisy when another session builds on the same machine: rerun any result that
  looks off.
