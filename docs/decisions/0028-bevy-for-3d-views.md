# 0028: Bevy draws the 3D views, on conditions, starting with a pilot

**Status:** Decided by the owner in chat · 16 November 2026, after the renderer tests (#186, #198,
#199). Partly replaces 0027 ("3D views keep Curva's own pipeline") and the 3D direction of
15 November in `status/display.md` (re_renderer plus Bevy's shaders copied into Curva's own pipeline).
Extended by 0029: Bevy now, without waiting for the pilot's report, and Bevy also for 2D views and
sheets.

**What was decided.**
- The 3D views move to **Bevy** (an open-source Rust engine, MIT/Apache, built on the same graphics
  library as Curva, wgpu 30) for drawing faces, meshes and SubD surfaces, shading, shadows, ambient
  occlusion, edge smoothing, repeated shapes drawn once (instancing) and skipping what is hidden.
- **Curva keeps its own CAD layer** on top: lines with widths in paper millimetres, exact arcs, dashes,
  outlines, cuts and their solid fills, the display styles, picking and snapping, and precision far from
  the origin.
- **First a pilot** (spec `docs/specs/bevy-pilot.md`, to be approved): Bevy draws one view's faces
  behind a setting, with Curva's lines and outlines drawn into the same picture. It is measured against
  today's Curva. If it holds, it replaces step 11 (instancing) of `docs/specs/display-pipeline.md`; if
  not, Curva keeps its own pipeline and this note is replaced.
- **Conditions:**
  - Bevy's richer effects (ambient occlusion, temporal smoothing) use the graphics card's full features
    on recommended cards (decision 0026); on the fallback they are off, and the plain drawing keeps
    working on the baseline device.
  - Curva's picking is extended to meshes and SubD parts; Bevy's own picking is not used.
  - Positions are sent to Bevy relative to an origin near the camera, so models far from the origin
    draw exactly (Bevy uses single precision).
  - Bevy's settings are given in millimetres (Bevy assumes metres).
  - Bevy is pinned to one version and upgraded only when egui uses the same wgpu version.
- **2D views and sheets stay with Vello** (decision 0027).

**What the tests found** (RTX 3060, 1920 × 1080, each run twice; pictures and numbers in the owner's
report of 16 November and on branches `186-re-renderer-spike`, `198-bevy-spike`,
`199-viewport-lib-spike`, not merged):

| Test | Bevy | viewport-lib | Curva today |
|---|---|---|---|
| *Large* (2,000 trees, 20,000 circles), time per frame | 13.4 ms (21 ms with all effects) | 38 ms | 22 ms on the card, at 1280 × 800 |
| *Large*, first frame | under 1 s | | 62 s |
| *Large*, graphics memory | 374 MB | 1 GB | about 2.1 GB |
| Big one-off meshes (15.7 million triangles) | 3.8 ms, 569 MB | 11.9 ms, 1.2 GB | not possible yet (about 1.9 GB) |
| SubD surface of 524,000 triangles changing every frame | 4 ms | 11 ms | not possible yet |

Meshes and SubD (architecture §10d, the Urban specialisation's terrain and city models) are what
changed the recommendation: on Curva's own route they would bring back the shelved steps 14 and 15,
the memory limit, shared corners, textures, mesh wires and live edits, all of which Bevy already has.

**Rejected.**
- **viewport-lib** as a renderer: GPL-3.0-only (clashes with decision 0025), one author, a crash with
  4-sample smoothing and lines, slowest at live edits.
- **re_renderer** as a renderer: made for Rerun's data views, not for shaded models; its line and
  outline technique may still be borrowed for Curva's CAD layer.
- **Curva's own pipeline for everything** (the 15 November direction): workable for solids and lines,
  but meshes and SubD would mean writing a mesh engine.

**The price.** Bevy releases a version with breaking changes every three to five months; each upgrade
is a piece of work and waits for egui. A clean build of Curva takes several minutes longer (the Bevy
spike alone took 9 minutes from scratch). Bevy has no cuts, selection outline or CAD picking: those
stay Curva's work.

**Revisit if** the pilot misses its measurements, Bevy stops following wgpu versions that egui uses,
or keeping Curva's CAD layer in step with Bevy's upgrades costs more than the drawing it saves.
