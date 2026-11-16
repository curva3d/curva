# Spec: Live plans and sections

**Status:** Approved by the owner in chat · 16 November 2026.
**Covers:** the owner's direction of 16 November 2026: model in 3D, switch to plan at once, edit the same elements there, and **the plan on screen must match the PDF that is sent out**. Builds on decision 0027 (Vello for 2D views) and `views-and-styles.md`; replaces the part of `views-and-styles.md` §4.1 that draws cut lines and fills of plans and sections on the graphics card.
**Rules:** 100% pure Rust. Recommended machine: an RTX 3060-class card (decision 0026); weaker machines run with fallbacks.

## 0. In short

| What you will notice | How |
|---|---|
| The plan on screen is exactly the plan in the PDF | Both are drawn from **one vector page**: the screen with Vello, the PDF by the PDF writer |
| Switching from 3D to plan is instant | Plans are kept up to date in the background, so there is nothing to wait for |
| Click a wall in plan and it's the same wall as in 3D | Every line on the page remembers the object it came from; selecting, snapping and the gumball are Curva's own and work the same in every view |
| An edit shows in plan almost at once | Only the changed objects and their neighbours are recalculated, on all cores |
| Edits look instant, with no flicker between 2D and 3D | The plan only ever shows the exact vector page; updates are fast enough to be invisible, and the edit's own preview stays on screen until the updated plan arrives |

This is how Archicad and Revit work: a computed vector drawing for plans and sheets, separate from the shaded 3D view. It avoids Rhino's problem, where the viewport and the vector print come from two methods and can differ. Unlike Archicad, Curva's plan is cut from the real 3D geometry, not drawn by separate 2D scripts, so plan and 3D cannot disagree.

## 1. Three things shared by every view

1. **One model.** Plans, sections, elevations and 3D views are live views of the same document. There is no separate 2D copy of an element.
2. **One interaction layer.** Picking, snapping, selection, highlighting, the gumball and command previews belong to Curva, not to any drawing engine. They behave the same in 3D and in plan. The 3D engine (whichever wins #198, #199 or #186) and Vello only draw.
3. **One style definition.** A style (`views-and-styles.md` §2) is defined once. In plans and sections, its line widths, colours, dashes and cut fills are applied to the vector page; in 3D views, to the 3D drawing.

## 2. Where each view comes from

| View | Drawn from | Drawn by | Matches the PDF |
|---|---|---|---|
| Plan, section, elevation | The vector page, computed by the kernel (cut, projection, hidden lines) | Vello on screen; `curva-pdf` / SVG for files | Yes, by construction |
| Hidden-line axonometric or perspective (Technical, Hidden) | The same vector page, from that camera | Same | Yes, by construction |
| Shaded 3D views (Shaded, Diagram, Textured…) | The 3D model | The 3D engine | Goes into a PDF as a picture, under any lines (`views-and-styles.md` §4.2) |

Anything that goes into a PDF as lines comes from the vector page. Nothing in a PDF is drawn by the 3D engine except pictures.

## 3. What the user sees

- **Switching:** choosing a plan (or the Plan view of a level) in a viewport shows its vector page at once, in the same viewport.
- **Editing in plan:** every command works in plan as in 3D, drawing on the plan's level and cut. Picking takes the nearest line on the page and selects its object; window and crossing selection work on the page's lines; snaps (end, mid, near, intersection, perpendicular…) work on the plan's lines, in paper or model units as now.
- **During a drag:** the usual ghost preview (AGENTS.md command rules) shows the moving objects over the plan.
- **After an edit (owner's decision B):** the plan **waits for the exact update**. There is no stand-in from the 3D view, because switching from the old 2D plan to a 3D picture and back to the new plan would be jarring. Instead:
  - the update is made fast enough not to be seen (targets in §6);
  - the edit's preview stays on screen over the old plan until the updated page arrives, so the edited object never jumps back to where it was;
  - only if an update takes longer than 0.3 s does a small marker in the status line say the plan is updating.
- Printing and export always use the exact page.

## 4. How it stays fast

The test house's plan took about 3 seconds to build in the Vello spike (decision 0027). Live plans need it under the targets in §6. In order of effect:

1. **Per-object results kept.** For each plan or section, each object's cut outline, its projected edges and its silhouettes are computed once and kept, until that object or the cut changes.
2. **Hidden lines only where needed.** When an object changes, only lines whose visibility it can affect are recomputed: objects whose screen footprint overlaps its old or new footprint, found with the object index already used for snapping (`crates/curva-viewport/src/index.rs`, or a copy in the kernel).
3. **In the background, on all cores.** Recalculation runs on worker threads, largest-on-screen first, and never blocks a frame.
4. **Plans kept current.** Plans and sections that are open in a viewport or placed on a sheet are updated after every edit. Others are updated when opened, from their kept results.
5. **Cheaper geometry steps.** Profile the 3-second build (cutting solids, `hidden_lines`, Make2D) and fix the slow parts. The kernel's existing hidden-line method keeps its results; only its speed changes.

## 5. Correctness first

- **#193 fixed before anything else:** `Body::section` gives wrong outlines for moved solids. Live plans are only as good as the cut.
- **Every mark knows its source.** Each line and fill on the vector page (`curva_document::drawing::Mark`) carries the number of the object it came from, or none for annotation drawn on the sheet. Writers ignore it; picking and snapping use it.
- **Kept results never go stale:** a test edits, moves, deletes and undoes objects and checks that the incremental page equals a page built from scratch, mark for mark.

## 6. Targets

Measured on the RTX 3060 at 1920×1080, test house and a larger plan (the *Large* scene cut at 1.2 m), each run twice:

| What | Target |
|---|---|
| Switch from 3D to an open plan | Under one frame (16 ms) |
| Edit one wall: exact plan updated | Test house under 50 ms (about three frames); *Large* under 0.1 s. Below about a tenth of a second, a change feels instant |
| Edit many objects at once (moving a whole floor) | Test house under 0.2 s; *Large* under 1 s, with the edit's preview shown meanwhile |
| Pan and zoom a plan | Under 16 ms per frame (spike: 1.4 ms) |
| Open a plan never opened before | Test house under 1 s; *Large* under 5 s, with a progress marker meanwhile |
| Screen against PDF | No pixel differs by more than a quarter of the colour range (as in the spike), on every reference plan |

If a target looks unreachable after the profiling step, the owner is told with numbers before the work goes on.

## 7. Steps, each a small pull request

1. **Features stream:** fix #193.
2. **Features stream:** marks carry their source object (`drawing::Mark`), filled in by everything that makes pages today.
3. **Features stream:** profile the plan build and speed up the slowest parts; numbers in `docs/performance.md`.
4. **Features stream:** per-object results kept, and incremental hidden lines (§4.1–4.2), with the from-scratch comparison test.
5. **Display stream:** plans and sections shown in viewports with Vello from the page, kept current in the background (§4.3–4.4), with the screen-against-PDF test.
6. **Display stream:** picking, selection and snapping on the page; command previews over plans; the gumball in plan.
7. **Display stream:** the edit's preview held over the plan until its update arrives, and the status-line marker for slow updates.

Steps 1 to 4 can start now. Steps 5 to 7 follow the Vello adoption work. None of them depend on the 3D engine decision (#186, #198, #199).

## 8. Changes outside each stream's own area

- `curva-kernel`: faster cutting and hidden lines; per-object results; incremental hidden lines. Additions; existing results unchanged.
- `curva-document`: `Mark` carries a source object; views know which plans are open or placed on sheets.
- `curva-viewport` (display stream): Vello plan views, picking and snapping on pages, previews held until the update arrives.
- `curva-app`: the status-line marker; viewport title menus open plans.

## 9. Decisions for the owner

- **A. Vector page as the single source for plans, sections and every line in a PDF** (§2). Agreed in chat on 16 November 2026.
- **B. No 3D stand-in** (§3). Decided in chat on 16 November 2026: the plan waits for the exact update, which must be fast enough not to be seen, with the edit's preview held on screen meanwhile.
- **C. Targets** (§6): approved with the spec on 16 November 2026.

## 10. Not covered

Symbolic plan elements that a pure cut doesn't produce (door swings, stair break lines, plan-only symbols): these are the "smart representations" moved to Milestone 4 by decision 0020, and they will draw onto the same vector page. Also levels from the architecture specialisation, and dimensions and text placed in plan views (already in `annotation.md`).

## 11. Shadows in drawings (amendment, owner, 16 November 2026)

Plans, sections and elevations can show sun shadows, in two kinds. Both match the PDF.

- **Crisp shadows (vector), the default for plans, sections and elevations.** The kernel projects each object along the view's sun direction onto the faces behind or below it (floors, ground, walls), and the shadow shapes go onto the vector page as fills, under the lines. They're sharp at any scale, come out in PDF, SVG and DWG as real shapes, and update live like the rest of the plan: an object's shadow is kept with its other results (§4.1), and an edit recomputes shadows only where the moved object's old or new shadow falls.
- **Soft shadows (picture), an option for presentation styles.** The 3D engine draws a shadow picture from the same sun, made once at print resolution. The screen shows that same picture under the plan's lines, and the PDF embeds it, so screen and PDF still match. They update a little less quickly than crisp shadows, and their speed isn't part of the targets in §6.
- **Settings:** the style's Shadows layer (`views-and-styles.md` §2.1) chooses crisp or soft, with strength and colour (and softness for soft). The sun is the view's own (`views-and-styles.md` §1.5). A view's shadows can be switched off.
- **Checked by:** crisp shadows of the test house against a reference (shapes compared within 0.01 mm at 1:100); the screen-against-PDF test of §6 with shadows on, in both kinds; the incremental-against-from-scratch test of §5 with shadows on.
- **When:** after steps 1 to 7. Crisp shadows are a kernel task for the features stream; soft shadows and the style setting go to the display stream.
