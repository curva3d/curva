# Performance measurements

Measured with `curva --orbit-benchmark`, which opens the window on a scene, spins the view and prints
how long each frame took (on Windows, pipe it: `curva --orbit-benchmark --curves 5000 | more`).
Compare builds made the same way on the same machine: a local `cargo build --release` and the
release workflow's build give different absolute numbers.

## Lines drawn once at their own width (0.4.1, spec `line-weights.md` §2)

13 November 2026, NVIDIA GeForce RTX 3060 (Vulkan), both built locally with `cargo build --release`.
"Before" is 0.4.0, which drew each line three times, a pixel apart.

| Scene | Before: work per frame, frame gap | After: work per frame, frame gap |
|---|---|---|
| 5,000 circles and 200 cylinders | 9.29–9.46 ms, median 4.0–4.1 ms | 9.18–9.39 ms, median 3.9–4.1 ms |
| 20,000 circles | 8.30 ms, median 7.5 ms | 8.62 ms, median 7.6 ms |

The same within run-to-run noise: drawing lines as quads of any width costs no more than before.

## Solids held parsed (0.4.3, `docs/reviews/code-health-2026-11-14.md` fix 1)

14 November 2026, the Windows machine, `cargo run --release -p curva-kernel --example body_costs`: a
40 × 30 × 20 box with all twelve edges rounded (26 faces, 48 edges), moved. "Before" is 0.4.2, which
kept each solid as the engine's native text and read it again for every question; "after" holds the
engine's body once, shared between copies, and keeps what is worked out from it (record, mesh,
bounds, volume, area). Average of repeated calls on the same body:

| Question | Before | After: first time | After: again |
|---|---|---|---|
| is_solid | 4.29 ms | 1.54 ms | 0.000 ms |
| volume | 45.6 ms | 43.5 ms | 0.000 ms |
| bounds | 16.5 ms | 14.6 ms | 0.000 ms |
| mesh | 24.7 ms | 5.1 ms | 0.016 ms |
| edge_curves | 24.3 ms | 0.020 ms | 0.018 ms |
| a moved copy, then is_solid | 3.85 ms | 0.001 ms | 0.000 ms |

A copy of a body (what each undo step makes) now shares its faces instead of copying the text.
Bounds of a body that was moved, or scaled or mirrored along the axes, come from the kept box; a
turned body is still placed and measured exactly.

## Undo shares unchanged objects (0.4.3, `docs/reviews/code-health-2026-11-14.md` fix 2)

14 November 2026, the Windows machine, `cargo run --release -p curva-document --example undo_costs`:
5,000 circles and 200 rounded boxes (5,200 objects). Objects are now held behind an `Arc`, so an undo
step copies the list of objects and shares every object a change left alone; only changed objects
are copied (`Arc::make_mut`).

| | After fix 1 only | After fix 2 |
|---|---|---|
| Copy the state (one undo step) | 0.479 ms | 0.037 ms |
| Compare the state with a copy | 0.187 ms | 0.090 ms |
| 100 edits, each moving one object: undo holds | every object copied 100 times | about 4.2 MB |

Most of the 4.2 MB is the list of 5,200 pointers each step keeps. In 0.4.2 each step held a full copy
of every solid's text as well (the model's JSON is 40 MB), so the same 100 edits held several
gigabytes.

Undo now also has Rhino's two limits, in Settings → General: **Minimum undo steps** (10) are always
kept, and older steps are dropped once undo holds more than **Maximum memory used** (256 MB). A
step's size is estimated from what it does not share with the state next to it.

## Drawing only what changed (0.4.3, `docs/reviews/code-health-2026-11-14.md` fix 3)

14 November 2026, the Windows machine, `cargo run --release -p curva-viewport --example redraw_costs`,
the same 5,200-object model:

| | Before | After |
|---|---|---|
| Tell nothing changed (every frame) | 0.093 ms (compare the whole model) | 0.001 ms (`State::same_as`) |
| Draw again after moving one object | 57.8 ms (sample everything) | 11.6 ms (reuse unchanged objects) |
| Draw again after a selection change | the same as a model change | recolour only, no sampling |

`same_as` compares objects by their shared copy: since fix 2 any change to an object makes a new
copy while the drawn state holds the old one, so no change can be missed, and no change counter has
to be kept by every edit. The viewport keeps what each object drew as, with its layer, and samples an
object again only when it or its layer changed; an instance is always drawn again, since its
definition can change without it. The 11.6 ms left is putting the kept pieces together; the mesh
cache's clean-up of removed objects no longer slows down with the square of the object count.

## Display pipeline baseline (15 November 2026, spec `display-pipeline.md` step 1)

Before any of the new display pipeline. NVIDIA GeForce RTX 3060 (Vulkan), window maximized on a
1280×800 display (the machine was reached over Parsec at the time; the spec's targets are for
1920×1080, which has twice the pixels, so later runs note the size used), built locally
with `cargo build --release`, nothing else building on the machine. Each run done twice; the two
agreed within 0.2 ms (99th percentiles excepted). `--scene` names the standard scenes of the spec
(§6). "Work per frame" is the main processor's time, which includes snapping: the benchmark looks
for snap points every frame with every snap on, as moving the mouse does.

| Scene | Opens | Frame gap (median) | Work per frame | Of which snapping |
|---|---|---|---|---|
| `--scene medium` (5,000 circles, 500 cylinders) | yes | 4.8–4.9 ms | 5.5–5.7 ms | 3.1–3.2 ms |
| `--scene large` (40,000 circles, 10,000 splines, 10,000 solids as copies of 100 shapes, 2,000 trees of 5,344 triangles) | **no, crashes** | — | — | — |
| Large reduced: `--scene large --curves 20000 --splines 5000 --solids 4000 --trees 0` | yes, first frame after 3.4–3.5 s | 20.1–20.4 ms | 23.2 ms | 17.6–17.7 ms |
| Far reduced: the same, `--scene far` (500 km from the origin) | yes, first frame after 3.4–3.6 s | 20.1–20.2 ms | 22.8–22.9 ms | 17.4–17.5 ms |

**Large does not open.** The viewport puts all lines, and all faces, each in one block of graphics
memory, and the graphics card refuses a block over 256 MB. Each part of Large alone is already too
big:

| Part alone | One block would need |
|---|---|
| 40,000 circles and 10,000 splines (128 segments per circle, 32 per spline point) | 376 MB |
| 10,000 solids, copies of 100 shapes (each copy written out again) | 580 MB |
| 2,000 trees (each instance written out again, about 2 minutes of work before the crash) | 1,154 MB |

Steps 4 (objects kept on the graphics card), 11 (instancing) and 13 (memory limit) of the spec
remove this ceiling; until then the largest scene that opens holds roughly 35,000 circles' worth of
lines. **Snapping** takes three quarters of the processor's time on the reduced Large scene; it is
not in the spec, and is tracked in a `display` issue. **Far from the origin** costs no more time;
whether it shimmers is checked by eye in step 2. The graphics card's own time per frame is not
measured yet (step 1b).
