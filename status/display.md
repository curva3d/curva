# Status: display stream

*Written only by the display stream, which owns the inside of `crates/curva-viewport` (see
`AGENTS.md`). Updated at the end of every work session. Newest first.*

## 15 November 2026: stream started

**Done**
- Status split into `status/display.md` and `status/features.md`; `STATUS.md` points to both
  (#124).
- `AGENTS.md` now says the display stream owns the inside of `crates/curva-viewport`, other streams
  ask for viewport changes with a `display` issue, and only the display stream runs speed
  benchmarks (#124, merged as #125). The `display` label exists.
- Spec for the new display pipeline (`docs/specs/display-pipeline.md`, #126), approved by the owner
  on 15 November 2026: detail that follows the zoom, anti-aliased edges, display modes as editable
  files, and speed on large scenes, in 15 steps (§7). Decisions: no Intel test machine, so the
  baseline is kept by design and by margins on the RTX 3060 (§5); the display stream makes the
  few changes outside the viewport that §8 lists, announcing each here first; display modes live in
  the user folder, later to be tied to a Curva online account.

**In progress**
- Step 1: the *Large* and *Far* benchmark scenes, the graphics card's own time per frame, a
  baseline on the RTX 3060, and a test drawing with every optional extra turned off.

**Heads-up for the features stream** (changes outside the viewport, decision B)
- None started yet. Step 1 touches only the benchmark code; where that lives outside the viewport
  it will be listed here first.

**Open `display` issues**
- None yet.

**Where the display stands today** (history before this date is in `status/features.md`)
- Wireframe, Shaded and Ghosted modes, fixed in code. Solids are meshed once, to a tolerance set
  by their size, and kept until they change. Curves are cut into a fixed number of segments
  whatever the zoom (a circle always 128), so arcs look like polygons when zoomed in. Edges are not
  anti-aliased.
- Lines are drawn once as strips with their own width (`docs/performance.md`).
- Frosted glass panels blur the viewport texture (`crates/curva-viewport/src/glass.rs`).
- Last measured speed (RTX 3060): orbiting 5,000 circles and 500 cylinders, about 6.7 ms between
  frames. Nothing has been measured on Intel integrated graphics yet.

**Known problems / risks**
- No measurements on the target low-end machine (2020 Intel integrated graphics) yet.
- Benchmarks are noisy when another session builds on the same machine: rerun any result that
  looks off.
