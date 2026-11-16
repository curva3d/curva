# 0029: Bevy is Curva's one drawing engine, for 3D views, plans, sections and sheets

**Status:** Decided by the owner in chat · 16 November 2026, after the plan accuracy test (#235).
Replaces 0027 (Vello for 2D views and sheets) and extends 0028 (Bevy for 3D views): the pilot's
"measure first, then decide" becomes "Bevy now, each step measured".

**What was decided.**
- **Bevy now, for 3D.** The pilot steps of `docs/specs/bevy-pilot.md` become the move itself. Each
  step must still meet the pass targets of that spec's §5 before it merges. Curva's own surface
  drawing and the remaining steps of `docs/specs/display-pipeline.md` (11 to 15) are retired once
  Bevy draws the 3D views.
- **Bevy also for plans, sections, elevations and sheets.** They are drawn by Bevy from the same
  vector page (`curva_document::drawing::Page`) the PDF and SVG writers use, so the screen still
  matches the print. Vello is not adopted.
- **Lines and hatches are written once.** One line drawer, Curva's own, serves 3D views and 2D
  drawings: widths in paper millimetres, dashes, hatches, cut fills, mixed at edges as printers mix
  them. The owner's reason: that code is needed in either engine, so write it once, and plans and
  sections become as interactive as the 3D views.
- **Printing and PDF stay Curva's own vector writer** (`curva-pdf`), as before.
- **Vello can come back** if plans on Bevy run into problems the tests did not show.

**What the test found** (#235, branch `235-bevy-plan-spike`; RTX 3060, A3 at 150 dpi): the same plan
page as Vello's test (the test house cut at 1.2 m, 0.5 mm cut walls filled black, lines 0.35 to
0.13 mm by depth, a hatch), drawn by Bevy as triangles with the true widths and 8 samples per pixel,
against the PDF rasterised by hayro: 55 pixels of 4.5 million off by more than a quarter of the
colour range (all in the tree trunks' wrong cut outlines, #193), none at 4x zoom; redraw while
panning and zooming 1.8 ms (Vello 1.4 ms). Two settings were needed: colours mixed at edges as
stored, as printers do, and both sides of triangles drawn.

**Before plans and sheets move to Bevy, still to check:** dashes, text on sheets, a very large sheet,
and machines without a graphics card (Vello had a processor-only version; Bevy has none).

**What the specs ask of Bevy, and how Curva answers:** `docs/reviews/bevy-drawing-review-2026-11-16.md`.

**Rejected.** Vello for 2D beside Bevy for 3D (two engines, lines and hatches written twice); Bevy's
own line drawing (no paper-millimetre widths, dashes, exact arcs or print-matching edges).

**Revisit if** plans on Bevy cannot keep the screen-against-PDF target of `docs/specs/live-plans.md`
§6, sheets on weaker machines are too slow, or the reasons in 0028's "revisit" apply.
