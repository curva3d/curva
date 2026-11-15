# Status: display stream

*Written only by the display stream, which owns the inside of `crates/curva-viewport` (see
`AGENTS.md`). Updated at the end of every work session. Newest first.*

## 15 November 2026

**Done**
- Step 1b, the graphics card's own time (#132): the benchmark reports it, read from the card's
  clock. *Medium* 1.5 ms, reduced *Large* 7.1 ms per frame; the processor is the bigger cost on both
  (`docs/performance.md`). The window now asks the card for its device through
  `curva_viewport::device_descriptor`: the baseline limits, plus optional extras only when present.
  Found on the way: building *Medium*'s 500 cylinders takes about 35 s, about 60 ms per extrusion in
  the engine (#136, for the features stream).
- Step 1a, benchmark scenes (#129, merged as #131): `--orbit-benchmark --scene medium|large|far`, plus `--splines`
  and `--trees`; the report says what the scene holds, how long it took to build and when the first
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
- Step 1c: a test that draws with every optional graphics extra turned off.
- Then **step 4 before steps 2 and 3**: *Large* crashing matters more to users than shimmer or
  jagged edges, so objects kept on the graphics card (which removes the 256 MB ceiling for lines
  and faces) comes first.

**Heads-up for the features stream** (changes outside the viewport, decision B)
- Done: `crates/curva-app/src/benchmark.rs` and the `--orbit-benchmark` arguments in
  `crates/curva-app/src/main.rs` (#131); the window's graphics set-up in `main.rs` now uses
  `curva_viewport::device_descriptor` (#132).
- Next: step 4 changes what the window hands the viewport each frame (`Drawing`, built in
  `crates/curva-app/src/viewports.rs` and `sheets.rs`), so selection no longer rebuilds the scene.

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
- *Large* scenes crash on opening (see above); a model of that size made by a user would crash too.
- No measurements on the target low-end machine (2020 Intel integrated graphics); there will be
  none (owner's decision A).
- Benchmarks are noisy when another session builds on the same machine: rerun any result that
  looks off.
