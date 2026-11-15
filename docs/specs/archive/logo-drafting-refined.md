# Spec: the refined Drafting logo

**Status:** Approved by the owner in chat · 13 November 2026.
**Covers:** feedback on 0.4.0, points 6 to 8; decision 0018 (3).

## 1. The problem

In the Drafting variant the C is an arc of radius 78 about (139.4, 128), but the grey guides are a
generic sheet: circles about the tile's centre (128, 128), a square, a 32-unit grid and the tile's
diagonals. None of them passes through the C, its ends or its control points.

## 2. The guides become the construction

The owner's choice (13 November 2026): the C, the orange control polygon and the points stay exactly
as in the original Drafting mark; only the grey lines change. They are drawn as an architectural
drafting sheet sets out a part, and every one of them builds the C:

- centre lines (long dash, dot) through the C's centre, running past it, and the C's own centre
  line (r 78) dash-dot;
- the edges of its stroke (r 59 and r 97) as thin solid lines;
- the 45° rays from the centre through its ends, which set its opening;
- the control polygon's sides carried on past the points, as construction lines overshoot;
- the chord between the ends, dimensioned with extension lines, a dimension line and 45° ticks.

## 3. Deliverables

- `docs/design/curva-logo-drafting.svg`: the refined master, layers for Illustrator as before
  (Tile, Construction, Control polygon, C, Control points).
- `docs/design/curva-logo-drafting-small.svg`: for 16–48 px (favicon, taskbar): no guides, a heavier C,
  larger points, the polygon kept only if it reads at 32 px.
- `docs/design/logo-drafting-variations.html`: the refined master beside five or six new variations of
  Drafting (guide weights and density, dark tile, outline only, accent on the C instead of the points,
  dashed construction), shown at 256, 64, 32 and 16 px, on light and dark.
- When the owner picks one, it replaces Curva's icon everywhere (app, installer, README).
