# Spec: Bevy pilot for the 3D views

**Status:** Approved by the owner, 16 November 2026.
**Covers:** the first step of decision 0028: Bevy draws one 3D view's faces, Curva keeps its CAD
layer, and the result is measured against today's Curva before anything else moves.
**Rules:** 100% pure Rust. Recommended machine: an RTX 3060-class card (decision 0026); weaker
machines run with fallbacks. Owned by the display stream; everything lives inside
`crates/curva-viewport` except the few lines listed in §7.

## 0. In short

| What you will notice | How |
|---|---|
| A new setting, **3D engine: Curva / Bevy (pilot)**, per view, off by default | A view with the pilot on is drawn by Bevy; every other view is unchanged |
| With the pilot on, *Large* opens in about a second instead of a minute | Repeated shapes (the trees, blocks) are sent to the graphics card once and drawn many times |
| Lines, outlines, selection, snapping and the gumball look and behave exactly as today | They stay Curva's, drawn on top of Bevy's faces in the same picture |
| On a recommended card, the Diagram style can use Bevy's ambient occlusion, shadows and smoother edges | Bevy's effects, on only where the card has the features for them |
| If the pilot misses its targets, nothing is lost | The setting is removed and Curva's own pipeline continues with step 11 |

## 1. What Bevy draws, and what stays Curva's

| Bevy draws | Curva keeps |
|---|---|
| Faces of solids and surfaces, shaded | Lines and edges with widths in paper millimetres, dashes, exact arcs |
| Repeated shapes drawn once (instancing) | Outlines of curved shapes and the selection colours |
| Skipping what is off screen or hidden | Picking and snapping (its index of objects, #130) |
| Shadows, ambient occlusion, edge smoothing, colour handling | The display modes and styles, which tell Bevy what to draw |
| | Cuts and their solid fills (clipping planes), precision far from the origin |

Curva's document stays the only model. A small translation layer mirrors the scene into Bevy: each
object becomes one Bevy entity, each distinct shape one Bevy mesh shared by all its copies. When an
object changes, only its entity changes.

## 2. How the picture is made

1. Bevy runs inside Curva on **Curva's own graphics device** (no second device, no copying between
   them), the way the spike did on branch `198-bevy-spike`.
2. The pilot view draws into its own picture, colour and depth. Bevy fills it first. Curva's line,
   outline and preview pass then draws into the same picture and is hidden behind faces by Bevy's
   depth. The finished picture is shown in the view.
3. **Depth.** Bevy keeps depth reversed (near is 1, far is 0), which also gives better precision; Curva's
   line pass follows it in pilot views.
4. **Precision far from the origin.** Positions go to Bevy relative to an origin near the camera,
   moved when the camera moves far, so the *Far* scene draws exactly (Bevy uses single precision).
5. **Millimetres.** Ambient occlusion radius, shadow distances, near and far planes are set from the
   model's size in millimetres, not Bevy's metre defaults.

## 3. Machines

- **Recommended card:** Curva asks for the card's full features when the pilot is on, so Bevy's
  ambient occlusion and temporal edge smoothing work.
- **Fallback (baseline device):** Bevy's plain drawing only, effects off. The existing test that draws
  with every optional extra off and on and expects identical pictures is kept, and a pilot version of
  it is added.

## 4. Dependency

- `bevy = "=0.20.1"`, pinned, with only the parts used: rendering, meshes, lights, cameras, the core
  passes, ambient occlusion and edge smoothing. No windows, audio, user interface, scenes or Bevy's
  picking. MIT or Apache-2.0; checked with `cargo-deny` when added.
- Upgraded only when egui moves to the same wgpu version (decision 0028).
- **Cost:** a clean build of Curva gets several minutes longer, and CI with it. Measured and reported
  in the first pull request.

## 5. How it is checked

On the RTX 3060 at 1920 × 1080, each run twice, the same scenes as the renderer tests (`--scene medium`,
`large`, `far`, and a new `meshes` scene), Curva today against Curva with the pilot on.

| Scene | The pilot passes if |
|---|---|
| *Medium* | Time between frames no more than 10% above today's |
| *Large* | Graphics card's time per frame at or below today's; first frame under 5 s; graphics memory under 1 GB |
| *Far* | No shimmer: the same picture as the scene drawn near the origin |
| *Meshes* (16 terrain tiles, 15.7 million triangles) | Opens; under 16 ms per frame; under 1 GB |
| Selecting | A selection change costs under 1 ms more than an ordinary frame |

Plus pictures, side by side with today's: Shaded, Ghosted and Wireframe of the test house, and the
Diagram style of the building, with lines on top and hidden lines hidden as today. Bevy's effects are
measured separately, one by one, so their cost on a recommended card is known.

## 6. Steps, each a small pull request

1. **Bevy inside Curva:** the dependency, Bevy running on Curva's device, the setting, an empty pilot
   view drawn by Bevy. Build time measured.
2. **Faces:** solids mirrored into Bevy, shared meshes for repeated shapes, selection colours. Shaded
   and Ghosted.
3. **Curva's lines and outlines** drawn into Bevy's picture with Bevy's depth; previews and the gumball.
4. **Precision and millimetres:** the moving origin, the millimetre settings; the *Far* check.
5. **Effects on recommended cards:** ambient occlusion, shadows, smoothing in the Diagram style.
6. **Measure and report** (§5) with a recommendation: move every 3D view to Bevy, or stop the pilot.

Clipping planes and the Architectural style's cut fills keep working in pilot views from step 3 (they
are Curva's); if they cannot, the report says so.

## 7. Changes outside the viewport crate

Announced in `status/display.md` first, as before:
- The setting in the view's menu, with its words in the translation files.
- `curva_viewport::device_descriptor` gains the full-features choice used by pilot views.
- The `meshes` benchmark scene in `crates/curva-app/src/benchmark.rs`.

## 8. Not in the pilot

Meshes and SubD as Curva objects (features stream, after Milestone 4); textures; prints and sheets
(Vello, decision 0027); Bevy's own picking or user interface; moving all views to Bevy, which waits for
the report.

## 9. The owner's decisions (16 November 2026)

1. **Pilot in one view behind a setting.** Each 3D view gets "3D engine: Curva / Bevy (pilot)",
   Curva by default; the rest of Curva is unchanged until the report.
2. **Pass targets as in §5.** At least as good as Curva today everywhere, clearly better on *Large*
   and meshes.
3. Bevy's own lines were considered and not used: they lack paper-millimetre weights and dashes,
   exact arcs, outlines of curved shapes, cuts, snapping and precision far from the origin. Curva's
   lines are drawn into Bevy's picture with its depth (§2).
