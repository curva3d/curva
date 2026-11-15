# Status: display stream

*Written only by the display stream, which owns the inside of `crates/curva-viewport` (see
`AGENTS.md`). Updated at the end of every work session. Newest first.*

## 15 November 2026

**Done**
- Step 4b, **selecting no longer freezes Curva** (#144): one selection change took 117 ms on
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

**In progress / next**
- Order, by what users notice most: **#130 with the object index of step 12** (snapping is most
  of each frame on every large scene), then **step 11** (instancing: *Large*'s 58 s first frame and
  2.1 GB), then **4c** (an edit sends only the changed object), then step 13 (memory limit), then
  steps 2 and 3.

**Heads-up for the features stream** (changes outside the viewport, decision B)
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
- #130: snapping takes most of each frame on large scenes; to use the object index of step 12.
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
