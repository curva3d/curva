# Status: display stream

*Written only by the display stream, which owns the inside of `crates/curva-viewport` (see
`AGENTS.md`). Updated at the end of every work session. Newest first.*

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

**In progress / next**
- **Correct drawing first** (owner, 15 November 2026), before styles are tuned: (1) seams no
  longer drawn in shaded views (this pull request); (2) outlines of curved shapes in every live
  view, Wireframe included, found from the depth picture; then wireframe and previews drop seams
  too; (3) edges that stay whole where a surface is seen at a steep angle; (4) edges that follow
  the true shape when zooming in (spec steps 5 to 7).
- **Spec: views and visualisation styles** (`docs/specs/views-and-styles.md`, #160), approved by
  the owner on 15 November 2026: modelling layers and per-view layers, the model carries its styles,
  the display stream builds the model and interface parts. Replaces display modes. Next: the
  prototype pictures (Diagram, Architectural, Textured), then step 11.
- **Prototype 1 of 3, Diagram**: `curva --picture out.png --scene building
  --style diagram` draws a small test building on white with white surfaces, thin grey
  inner edges and bold black outlines, soft shading and ambient occlusion, as the owner asked
  (outlines found from a hidden picture of object
  numbers, drawn at twice
  the size and shrunk). Pictures only; the live viewport comes after the owner judges the look.
  Known: a thin seam line shows across each sphere. Merged as #168.
- **Prototype 2 of 3, Architectural** (this pull request): `curva --picture out.png --scene house
  --style architectural [--view top]` draws a small house cut at 1.2 m: cut walls filled solid
  black (where the cut opens a solid, the back of its faces shows), lines thinner and lighter the
  further from the viewer, white surfaces and paper. Pictures for the owner. Known: cut tree
  trunks show outlined, not filled.
- Order, by what users notice most: **step 11** (instancing: *Large*'s 58 s first frame and
  2.1 GB), then **4c** (an edit sends only the changed object), then step 13 (memory limit), then
  steps 2 and 3. The object index made for snapping (`crates/curva-viewport/src/index.rs`) is the
  one step 12 (off-screen culling) will use.

**Handoff for the next display session**
- Start with step 11: findings and the plan in four pull requests are on #153. It needs two
  additions to `curva-kernel` (the shared unplaced mesh and the placement), announced here first.
- Measuring: build with `cargo build --release -p curva-app`, check nothing else is building
  (`tasklist | grep -ci "^rustc.exe\|^cargo.exe"` is 0), run each scene twice:
  `curva-app --orbit-benchmark --scene medium|large [--trees 0] [--select]`. The machine is
  reached over Parsec at 1280×800; note the size used.
- Repository habits that bit this session: files have Windows line endings, so multi-line `sed` or
  `perl` replacements silently do nothing (use the Edit tool); `git push --force`, `reset --hard`
  and `branch -D` are blocked by the project's hooks (merge `origin/main` into a branch instead of
  rebasing); after a pull request it builds on is squash-merged, merge `origin/main` and keep this
  branch's side of `status/display.md`.

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
