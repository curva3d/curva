# Spec: the display on Bevy

**Status:** Draft, for the owner's approval.
**Covers:** the owner's direction of 16 November 2026: add Bevy fully to Curva, make the display
solid, reliable, quick and customisable, then return to the view styles. Decisions 0028 to 0030.
**Replaces:** steps 3 to 6 of `bevy-pilot.md`; steps 5 to 15 of `display-pipeline.md` (5 to 7 move
into the line drawer, 11 to 13 are Bevy's, 14 and 15 stay shelved); the drawing part of
`views-and-styles.md` §4.1. The rest of those specs (what styles, views, plans and sheets are and
how they behave) stands.
**Rules:** 100% pure Rust. Use Bevy as it ships, then maintained libraries, and only then Curva's
own code, marked and listed (decision 0030). Recommended machine: an RTX 3060-class card (decision
0026); weaker machines run with the effects off.

## 0. In short

| What you will notice | How |
|---|---|
| Every view drawn by Bevy, with its lighting, highlights and shadows | Bevy draws all surfaces; Curva's own surface drawing is deleted |
| Lines, edges and outlines exactly as today, with widths in paper millimetres | Curva's line drawer draws them into Bevy's picture, hidden behind faces by Bevy's depth |
| Large models open in seconds and stay smooth | Copies drawn once from one shared shape; only changes sent to the card |
| Models at real map coordinates draw without shimmer | Positions measured from near the camera |
| Each view can have its own look, saved as a style | Styles are settings over Bevy's parts and Curva's line drawer |
| Plans, sections and sheets on screen exactly as printed | Drawn by Bevy from the same drawing description the exports write |

## 1. How it is built

As `architecture.md` §6b: the model is the only copy; solidcore works out shapes; Bevy draws the
pictures; Curva hands Bevy each view's objects, camera, lights and look, and draws every line.

- **Per-view looks:** each view on screen has its own copy of the objects on its own render layer,
  sharing their meshes, with that view's materials and lights (measured in step 1 of the move).
- **Curva's line drawer** (custom part `cad-lines`) runs after Bevy's own passes, on the finished
  picture, mixed as printers mix colours, hidden by Bevy's depth. Grid, curves, edges, outlines,
  points, previews, the gumball and selection all go through it.
- **The drawing description** (a separate spec, for approval before plans move) is drawn by Bevy
  for plans, sections and sheets, through lyon and kurbo (custom part `page-drawing`).

## 2. Targets

On the RTX 3060 at 1920 × 1080, each scene twice, against today's Curva and Rhino 8 on the same
scenes (`docs/performance.md`):

| Scene | Target |
|---|---|
| *Medium* | No more than 10% slower than today's Curva between frames; faster than Rhino (about 10 ms) |
| *Large* | Graphics card's time at or below today's; first frame under 5 s; graphics memory under 1 GB; faster than Rhino (about 167 ms) |
| *Far* | No shimmer: the same picture as near the origin |
| *Meshes* (15.7 million triangles) | Under 16 ms per frame; under 1 GB |
| Selecting | Under 1 ms more than an ordinary frame |
| Lines | The same pictures as today's, pixel for pixel within edge smoothing, in Wireframe, Shaded and Ghosted |

## 3. Steps, each a small pull request

1. **Faces in Bevy** (#223, in review): the Studio and Sunlit looks, per-view copies, Curva's own
   background, selection, Wireframe and Ghosted.
2. **Lines on Bevy's pictures:** Curva's line drawer after Bevy's passes, with Bevy's depth;
   previews, the gumball, grid, points and selection; outlines of curved shapes; clipping planes
   with filled cut faces from the kernel's exact cut outlines (custom part `cut-caps`); pictures for
   agents drawn by Bevy.
3. **Bevy draws every 3D view:** the "3D engine" setting goes; Curva's own surface drawing, its
   outline pass and their tests' surface parts are deleted; Zebra and Draft as a small Curva
   material (custom part `analysis-looks`, decision B). Lines of code removed are counted.
4. **Copies drawn once, accuracy far away:** shared shapes from the kernel (one Bevy mesh, many
   placements, which Bevy draws together), the moving origin (custom part `far-origin`), Bevy's
   settings in millimetres. *Large* first frame and memory; *Far* without shimmer.
5. **Speed:** Bevy's fixed cost per frame (measured at about 2.5 ms on an empty scene: which of its
   passes run, one thread or all cores), selection, the *Meshes* scene; every target in §2,
   Rhino's figures beside Curva's.
6. **Styles as settings:** the style layers of `views-and-styles.md` §2 mapped onto Bevy and the
   line drawer: Surfaces (lighting, shine, colour, opacity), Shadows and Ambient occlusion (Bevy's,
   on recommended cards), Background and Finish, and the line layers. Then `views-and-styles.md`
   steps 1 to 9 resume on top: views as saved items, layers per view, style files, the Style panel.
7. **Plans, sections and sheets:** the drawing description spec, then plans drawn by Bevy from it
   and `live-plans.md` steps 5 to 7, with its four open checks (dashes, text, a large sheet, no
   graphics card).

Each step: tests first where behaviour changes; measured before and after on *Medium* and *Large*;
`docs/performance.md` and `status/display.md` updated; new custom parts listed in
`docs/bevy/custom-parts.md`.

## 4. Changes outside the viewport crate

Announced in `status/display.md` before each step, as before: the benchmark's `--bevy` switch
(step 1) and its removal (step 3); the kernel's shared shape and placement (step 4, additive); the
views, styles and Style panel of `views-and-styles.md` §7 (step 6).

## 5. For the owner's decision

- **A. Bevy becomes the only 3D engine at step 3,** as soon as lines are drawn on Bevy's pictures,
  rather than after all steps (recommended: less code carried twice, fewer places for bugs).
- **B. Zebra and Draft angle as a small Curva material** on Bevy (recommended): Bevy has no such
  analysis looks; listed as a custom part and checked at each upgrade.
- **C. Machines without a graphics card** run Bevy on the system's software graphics, slowly, with
  the effects off, rather than a separate engine (recommended: one engine). Measured in step 5.
