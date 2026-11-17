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

### The graphics card's own time (step 1b)

`--orbit-benchmark` now also reports the graphics card's time per frame for drawing the viewports,
read from the card's own clock (an optional extra; a card without it says "not measured"). Glass
blur and egui's panels are not included. Same machine, display and build as above, each run twice:

| Scene | Graphics card per frame | Work per frame (processor) |
|---|---|---|
| `--scene medium` | 1.51–1.52 ms | 5.6–5.7 ms |
| Large reduced | 7.08–7.09 ms | 22.8–22.9 ms |

The processor, not the card, limits both scenes; on reduced Large most of the processor's time is
snapping (#130). Against the spec's margins (§6: under 1.6 ms for *Medium*, 3.3 ms for *Large*, at
1920×1080), *Medium* is just inside at 1280×800, and reduced Large, with less than half of *Large*,
is twice over.

"First frame" in the report counts from the program starting, so it includes building the scene:
*Medium*'s 500 cylinders, each made its own by extruding a circle, take about 35 of its 39 seconds
to build, about 60 ms each in the engine (#136, outside the viewport).

## Large opens: uploads in blocks, one copy for all views (step 4a)

Uploads of lines and faces are split into blocks of at most 64 MB (the baseline cards refuse one
block over 256 MB), and views showing the same lines share one copy instead of each sending its
own. Same machine, display and build as above, nothing else building, each run twice. "Work per
frame" now leaves out the opening frames, like the frame gaps; before, the first upload of a
large scene swamped it (276 ms reported for *Large*, 48 ms without the opening frames).

| Scene | First frame | Frame gap (median) | Work per frame | Snapping | Graphics card |
|---|---|---|---|---|---|
| `--scene medium` | 38.3–38.4 s | 4.9–5.0 ms | 4.5–4.6 ms | 3.2 ms | 1.5–1.6 ms |
| Large reduced (as above) | 3.6 s | 20.0–20.1 ms | 19.5–19.6 ms | 17.4 ms | 7.0 ms |
| `--scene large --trees 0` | 3.8–3.9 s | 43.4 ms | 42.4 ms | 39.4–40.0 ms | 15.0–15.1 ms |
| `--scene large` | 58.4–58.5 s | 47.0–47.5 ms | 48.1–48.7 ms | 44.5–45.1 ms | 18.7 ms |

*Medium* and reduced Large take the same time as before, so splitting costs nothing. **Large
opens** (before: crashed). Its first frame takes 58 s, nearly all of it writing out each of the
2,000 trees' 5,344 triangles one by one (instancing, step 11, removes that), and by the block sizes measured above the card's memory
holds about 2.1 GB for it, too much for integrated graphics (memory limit, step 13). Snapping is
still most of each frame (#130). (*Medium*'s first frame is mostly building its cylinders, #136.)

## Selecting without rebuilding the scene (step 4b)

`--orbit-benchmark --select` selects and deselects one object every 10 frames once the window
has settled, and reports the median of the slowest frame gap in the few frames after each change.
Before, the window baked the selection colour into every vertex, so each change rebuilt and sent
all lines and faces again. Now each vertex carries its object's number, and the graphics card
looks it up in a small selection table (one byte per object) that is all a change sends. Same
machine, display and build as above, nothing else building, each run twice:

| Scene | One selection change, before | After | Ordinary frame gap after |
|---|---|---|---|
| `--scene medium` | 116.7–118.3 ms | 5.6–5.7 ms | 4.8 ms |
| `--scene large --trees 0` | 1,113.8–1,121.2 ms | 52.6–52.8 ms | 43.6–43.9 ms |
| `--scene large` | 2,264.6–2,301.7 ms | 56.7–57.9 ms | 47.8–48.7 ms |

(The second "before" run of `--scene large` overlapped another session's build; it agreed with
the first, so it was kept.) A change on *Large* still costs about 9 ms more than an ordinary
frame, against the spec's aim of under 1 ms; the rest of the window's work on a change is next to
look at. The graphics card's time per frame rose from 1.52 to 1.65 ms on *Medium* and from 18.7 to
22.0 ms on *Large*: vertices are 4 bytes larger and each looks the table up.

## Snapping through an index of the objects (#130, first part)

Snapping projected the eight corners of every object's box, and every snap place, each time it
looked for a point. The objects' boxes and the snap places now sit in a tree of boxes (a bounding
volume hierarchy), made once per model change, and snapping looks only into the parts of the tree
whose box comes within reach of the cursor; a box inside one out of reach is out of reach too, so
the same snap points are found. Same machine, display and build as above, nothing else building,
each run twice:

| Scene | Snapping, before | After | Frame gap, before | After |
|---|---|---|---|---|
| `--scene medium` | 3.2 ms | 1.28–1.29 ms | 4.8 ms | 2.6 ms |
| `--scene large --trees 0` | 39.4–40.0 ms | 20.3–20.5 ms | 43.4 ms | 22.6–22.9 ms |
| `--scene large` | 44.5–45.1 ms | 20.5–20.6 ms | 47.0–47.5 ms | 23.5 ms |

What is left on *Large* is crowding, not searching: timed once, the benchmark's view shows the
whole scene, each circle under a pixel, so about 115,000 segments lie within the cursor's 12
pixels, and working out where they cross takes about 15 ms. Thinning out objects too small to see
on screen would remove that, but changes which snap points are offered; it stays open in #130.

## Rhino on the same scenes (16 November 2026, the owner's reference)

The owner asked for Rhino's speed on the benchmark scenes, as the reference Curva is held to.
*Medium* and *Large* were saved from Curva as `.3dm` files (`curva --picture … --scene medium
--run "SaveAs;medium.3dm"`) and opened in Rhino 8 (8.36) through its connection, in millimetres,
on the same machine (RTX 3060). Views as Curva's: Perspective Shaded, Top, Front and Right
Wireframe, each zoomed to the whole scene. Rhino's own `TestMaxSpeed` redraws the active view 100
times; each view was measured on its own (a script cannot time Rhino's redraws, which return before
they finish), so "all four" is the sum. Rhino's views were 431 × 410 pixels; Curva's window is
1280 × 800 with four views of about 640 × 400, so Curva draws about 1.4 times the pixels.

| Scene | Rhino Perspective, Shaded | Rhino Top | Rhino Front | Rhino Right | Rhino, all four | Curva, all four (frame gap) |
|---|---|---|---|---|---|---|
| *Medium* | 1.6–1.9 ms | 5.0 ms | 1.4 ms | 1.6 ms | about 10 ms | 2.9–3.4 ms (Bevy views: 5.7–5.8 ms) |
| *Large* | 33–35 ms | 101 ms | 18 ms | 15 ms | about 167 ms | 24.9–26.2 ms (Bevy views: 25.6–26.4 ms) |

Rhino's *Large* figures are from the second run, after Rhino had made its display meshes. Rhino's
Top view is its slowest: it draws all the circles face-on. Curva's *Large* frame gap is mostly
snapping (about 23 ms, #130); its graphics card's own time is 22 ms.

## Faces drawn by Bevy (16 November 2026, display-on-Bevy step 1, #223)

Every view drawn by Bevy (`--orbit-benchmark --bevy`), faces only (Curva's lines come in step 2),
against Curva's own drawing on the same runs, alternated, two each:

| Scene | Curva's own | Bevy |
|---|---|---|
| *Medium* | 2.9–3.4 ms between frames | 5.7–5.8 ms |
| *Large* | 24.9–26.2 ms | 22.4–28.2 ms |

- **Bevy's fixed cost:** about 2.5 ms of processor time per frame even with an empty scene (its
  update 0.5 ms, handing over 0.2 ms, preparing the drawing 1.9 ms), plus about 0.5 ms per 500
  solids. On *Medium* that fixed cost is the gap. Run on one thread instead of all cores, the empty
  scene costs 1.5 ms, but the gain shrinks with larger scenes; decided in step 5 on *Large*.
- **What did not help:** one Bevy update for all views instead of one per view; handing Bevy only
  what changed; the graphics card's full features (2.6 → 2.4 ms). **What did:** not preparing
  Curva's own copy of the scene for views Bevy draws (6.7 → 5.8 ms).
- *Large*: Bevy draws a 1200 × 800 picture of it in 1.8 ms; the frame gap is snapping (#130).

## Copies drawn once (17 November 2026, display-on-Bevy step 4, #153)

*Large* on the RTX 3060, every view drawn by Bevy with Curva's lines, release build, two runs each
on a quiet machine, before (step 3, #255) and after (#281). Graphics memory is the card's peak use
during the run less its use before, read with `nvidia-smi` every 2 s.

| | Before | After |
|---|---|---|
| First frame | 73.4–74.5 s | 4.4–4.6 s |
| Graphics memory | about 3.2 GB | about 1.3 GB |
| Between frames | 24.3–26.2 ms | 24.5–25.7 ms |

- **Why it was slow:** placing a block instance remade its body in the engine, so each of the
  2,000 trees was a new body, meshed again. Now an instance that only moves, turns, scales evenly
  or mirrors shares its body's faces, and Bevy holds one mesh per shape and draws every copy at
  its place.
- **Spec targets (§2):** first frame under 5 s met. Graphics memory under 1 GB not yet: what is
  left is mostly Curva's lines (see the next section). The time between frames is still
  snapping (#130).

## Circles drawn from their axes (17 November 2026, display-on-Bevy step 4, #153)

Where *Large*'s graphics memory went after copies were drawn once, read by printing each upload:
Curva's lines were 468 MB (14.6 million line ends of 32 bytes). Of those, the 40,000 circles took
328 MB (128 segments each), the 10,000 splines about 100 MB, and the edges of all solids and trees
only 38 MB. An empty scene already takes about 800 MB: the window, Bevy and its lighting.

Now a whole, undashed circle is one entry of 56 bytes, and the graphics card works out its 128
segments as it draws. *Large* on the RTX 3060, release build, two runs each, alternating, same
machine and hour; graphics memory is the card's use above its use before, read with `nvidia-smi`
every second, its peak and its steady level over the run's second half.

| | Before (#281) | After |
|---|---|---|
| Graphics memory, steady | 1312 MiB | 800 MiB |
| Graphics memory, peak | 1440 MiB | 800 MiB |
| Card time for Curva's lines | 12.85–12.91 ms | 11.49–11.58 ms |
| First frame | 4.5–5.6 s | 4.4–5.3 s |
| Between frames | 25.3–32.3 ms | 24.3–25.7 ms |

- **Spec target (§2):** graphics memory under 1 GB is met; the scene now costs about what an
  empty one does (the card's driver hands memory out in large blocks, so smaller changes do not
  show). Other builds were running during the first pair of runs, so its frame times are slower.
- Pictures for agents and sheets still draw circles as segments; only the window's views draw
  them from their axes.

## Far from the origin (17 November 2026, display-on-Bevy step 4, #225)

A window scene is now measured from a point near the model. *Far* is *Large* moved 500 km from the
origin; on the RTX 3060, release build, two runs each:

| | *Large* | *Far* |
|---|---|---|
| First frame | 4.5–4.9 s | 4.5 s |
| Between frames | 24.3–27.3 ms | 23.4–23.6 ms |
| Card time for Curva's lines | 11.50–11.56 ms | 11.50–11.54 ms |
| Graphics memory, steady | 799 MiB | 799 MiB |

*Far* costs the same as *Large*. Whether it shimmers is now a test, not a look by eye: a small model
500 km out draws the same picture as at the origin, through Curva's lines and through Bevy
(`far_from_the_origin_draws_as_at_the_origin`, `far_from_the_origin_bevy_draws_as_at_the_origin`).

## Building a plan (17 November 2026, Live plans step 3, #203)

A plan is Make2D from the top with a clipping plane at 1.2 m keeping what is below (spec
`live-plans.md` §6). Release build on the owner's machine, with other builds running, so times
vary by about 10%. Measured with `cargo test --release -p curva-app plan_build -- --nocapture`,
which names the scene in `PLAN_SCENE` (the test house by default).

**The test house** (26 solids, the plan 70 to 82 lines and fills):

| Step | Before | After |
|---|---|---|
| Cutting each solid at the plane, for fills and cut lines | 158 ms (each cut done twice) | 80 ms (once) |
| Clipping the solids at the plane | 103 ms | 58 ms |
| Hidden lines | 56 ms | 48 ms |
| Putting the drawing in the model | under 1 ms | under 1 ms |
| **Whole plan** | **319–321 ms** | **187–212 ms** |

- Each cut through a solid was worked out twice, once for its fill and once for its cut lines; now
  once.
- **A bug was found and fixed on the way (#308):** a clipping plane that misses a solid was taken
  as cutting it, so solids wholly above the cut, such as the upper floor's slab, were drawn in the
  plan. The fix also skips the geometry engine for those solids, which is most of the clipping
  step's saving. The plan changed with that fix (now 22 solids drawn, not 26); the speed changes
  here leave it identical, line for line.
- Where lines cross is now found through a grid of cells, so only lines near each other are
  compared, in the same order: identical results on five test scenes in four views.
- **Not faster: cutting the solids on all cores.** Run side by side, each cut took 3 to 15 times
  longer than alone, so the whole took as long. The geometry engine seems to do one cut at a time
  inside; worth finding out why.

**The *Large* scene** (10,000 solids, 50,000 curves) did not finish:

| Step | Time |
|---|---|
| Cutting and clipping the solids | 15–19 s |
| Each solid's edges and silhouettes | 60–78 s |
| Where lines cross | over 12 minutes; stopped |

Finding the edges takes 6 to 8 ms a solid, and the dense curves of *Large* overlap so much that
working out their crossings, not finding which might cross, is what takes the time.

**Against the targets (§6):** the test house opens in 0.2 s (target 1 s), and an edit to one wall,
with step 4's kept results, would redo about 7 ms of cutting plus the hidden lines near it, inside
the 50 ms target. *Large* is far from its targets (open under 5 s, edit under 0.1 s) with the
present methods; see the report on #203.

### *Large*, first fixes (17 November 2026, #317)

- **Where lines cross: 700 s → under 1 s.** Counted on *Large*: 210,000 pairs of lines overlap,
  and all but 5,000 take no time. Those 5,000 are the cylinders' top and bottom circles, which lie
  exactly on each other seen from above; each pair took about 140 ms, compared point by point.
  Lines lying on each other already did not cross when straight; now curves do the same. The
  five hidden-line test scenes come out byte for byte the same (and run in 53 s, not 89 s).
- **Edges: far fewer trips to the geometry engine.** A moved copy of a solid (all of *Large*'s
  solids are copies of 100 shapes) asked the engine to copy, place and describe it again each
  time its edges, face kinds or face directions were wanted: about 26 times for a box. It now
  keeps the description once read. Not timed on its own yet.
- **Why cutting does not use all cores:** the geometry engine (solidcore-ops) passes every call
  through one lock, so only one runs at a time. Changing that is upstream's work.
- **Still not finishing:** with both fixes *Large* ran past 40 minutes and was stopped. The time
  now goes to a later step, most likely deciding which pieces of line are hidden (a ray test for
  each piece). Next for #317.
- The test house's plan is unchanged, line for line (193 ms).

### *Large* finishes (18 November 2026, #317 part 2)

Timed step by step on *Large* with part 1's fixes in, the hidden lines took 43 s, but the next
step, leaving out lines drawn twice, ran past 48 minutes: it compared each of 100,000 drawn lines
with every line kept before it, curves point by point. It now compares only lines near each other
(a grid of cells), which gives the same result.

| Step on *Large* | Before #317 | Now |
|---|---|---|
| Each solid's edges and silhouettes | 60–78 s | 7 s |
| Where lines cross | 700 s | 2 s |
| Setting up the visibility tests | not reached | 10 s |
| Which pieces are hidden | not reached | 23 s |
| **Whole plan** (75,000 lines and fills) | **did not finish** | **122 s** |

The test house's plan and the five hidden-line test scenes come out byte for byte the same (the
house in 190 ms).

**Against the targets:** *Large* should open in under 5 s; it takes about 2 minutes, so it is
still some 25 times too slow, though it now finishes. What remains: cutting and clipping the
10,000 solids (15–19 s, one at a time because of the engine's lock), the visibility tests (33 s),
and about a minute not yet broken down (likely putting 75,000 objects into the model). Live plans
step 4 keeps results between edits, which is what the edit target (0.1 s) depends on.

## Snapping searches only what could win (17 November 2026, #130)

Snapping offers the nearest snap within reach of the cursor. A meeting point lies on both its
segments and a perpendicular foot on its own, so once the nearest place (an end, a middle, a centre)
is known, only segments nearer the cursor than it can give a nearer point; the rest are no longer
searched, and the tree of object boxes skips them too. A test compares the result with searching
everything at 4,900 cursor positions over a tangle of lines and circles.

Measured as the benchmark measures it (every snap on, cursor in the middle of the Perspective view),
RTX 3060, release build, two runs each:

| | Before | After |
|---|---|---|
| *Large*, snapping per frame | 20.6 ms | 3.6–4.2 ms |
| *Large*, between frames | 24.0–24.5 ms | 16.2 ms |
| *Large*, processor's work per frame | 25.2 ms | 8.9–9.6 ms |
| *Medium*, snapping per frame | 0.48 ms | 0.23 ms |

On *Large* the whole scene is in view, so the cursor still has thousands of ends, middles and
centres within reach; they are most of what remains. #130's aim of under 1 ms is not met yet.
16.2 ms between frames is the screen's own rate (60 a second), so frames now wait for the screen.

## Selecting under 1 ms (17 November 2026, display-on-Bevy step 5)

The spec asks that a frame after a selection change take under 1 ms more than an ordinary one.
`--select` selects and clears an object every 10 frames; the figure is the slowest gap between
frames in the few frames after each change, against the usual gap. RTX 3060, release build, two
runs each, with snapping already sped up (#321):

| | Usual gap | After a selection change |
|---|---|---|
| *Large*, a solid selected | 16.1 ms | 16.3 ms |
| *Medium*, a solid selected | 5.6–6.1 ms | 6.0–6.8 ms |

Before, on main without the snapping change, *Large* went from about 27–30 ms to 52–59 ms after a
change. Two causes: Bevy was handed a material for every part in every view at each change (12,000
parts on *Large*, 1–2 ms a view in Curva and more in Bevy, which redid each part handed to it),
now only for the parts of objects whose selection changed; and the benchmark itself listed every
selectable object at each change, 10 ms on *Large*, which a click never does, now once. The
benchmark now selects a solid when there is one, as that is the case that recolours Bevy's
surfaces.
