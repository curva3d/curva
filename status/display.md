# Status: display stream

*Written only by the display stream, which owns the inside of `crates/curva-viewport` (see
`AGENTS.md`). Updated at the end of every work session. Newest first.*

## 16 November 2026

**Done**
- **#199 viewport-lib and #198 Bevy tried inside Curva, reported, waiting for the owner.** Both on
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

**In progress / next** (owner, 15 November 2026: decisions 0024 and 0026)
- **Recommended graphics card** (decision 0026): an RTX 3060, or an AMD or Intel card of the same
  class, with fallbacks for weaker machines. Speed targets are set on the RTX 3060 itself
  (`docs/specs/display-pipeline.md` §5 and §6); 2020 Intel integrated graphics are the fallback
  case.
- **3D direction** (owner, 15 November 2026): re_renderer for lines, points, outlines and picking
  (its latest version uses wgpu 30 and ecolor 0.37, as Curva does); Bevy's WGSL shaders (MIT/Apache)
  for ambient occlusion, edge smoothing, tonemapping and materials, copied and adapted with credit;
  our own code for the CAD basics (exact arcs, precision far from the origin, instancing, off-screen
  culling, the memory limit, the styles); Blender Workbench only as the picture to match for the
  cavity effect, with no Blender code except in GPL crates (decision 0025, #190).
- **2D direction** (decision 0027): Vello draws 2D views and sheets, vello_cpu the fallback. Its
  production follow-up is not started.
- Order of work (owner, 16 November 2026: test existing 3D pipelines before deciding):
  1. **#186, re_renderer: reported, decision on hold.** Borrowing its line and outline technique
     stands whichever way the 3D route goes. No decision recorded yet. Spike on branch
     `186-re-renderer-spike`, not merged.
  2. **#199 viewport-lib and #198 Bevy: reported 16 November, waiting for the owner's decision.**
     Spikes on branches `199-viewport-lib-spike` and `198-bevy-spike`, not merged. No decision
     recorded in `docs/decisions/` until the owner chooses.
  3. **Comparison of the three routes: reported, updated for meshes and SubD.** Recommendation:
     adopt Bevy for the 3D views on conditions, starting with a pilot; Curva keeps its CAD layer;
     viewport-lib stays out. Waiting for the owner.
  4. **Between the tests: #180,** splitting the viewport's large files, with no behaviour change
     and speed checked before and after.
  5. **#197 (shaded look from Bevy's shaders) waits** for the 3D route decision.
  6. **Then, adjusted to the decision:** step 11 (instancing, #153), 4c, step 12, step 13; any
     change to the remaining steps or the 3D part of `docs/specs/views-and-styles.md` proposed as
     choices first.
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
