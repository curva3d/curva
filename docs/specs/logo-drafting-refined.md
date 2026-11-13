# Spec: the refined Drafting logo

**Status:** Approved by the owner in chat · 13 November 2026.
**Covers:** feedback on 0.4.0, points 6 to 8; decision 0018 (3).

## 1. The problem

In the Drafting variant the C is an arc of radius 78 about (139.4, 128), but the grey guides are a
generic sheet: circles about the tile's centre (128, 128), a square, a 32-unit grid and the tile's
diagonals. None of them passes through the C, its ends or its control points.

## 2. The guides become the construction

Only lines that build the C are drawn, in the same pale grey:

- the C's centre line: its circle (r 78) about its own centre, with a cross at the centre;
- the stroke's inner and outer circles (r 59 and r 97), where the thick C starts and ends;
- the two 45° rays from the centre through the C's ends, which set its opening;
- the control polygon's two sides, extended past the left point so the tangents read;
- one horizontal axis through the centre and the left control point.

The C is placed so the whole construction is centred optically in the tile, and its numbers are kept
round (radius, stroke, angle) so the file is easy to edit.

## 3. Deliverables

- `docs/design/curva-logo-drafting.svg`: the refined master, layers for Illustrator as before
  (Tile, Construction, Control polygon, C, Control points).
- `docs/design/curva-logo-drafting-small.svg`: for 16–48 px (favicon, taskbar): no guides, a heavier C,
  larger points, the polygon kept only if it reads at 32 px.
- `docs/design/logo-drafting-variations.html`: the refined master beside five or six new variations of
  Drafting (guide weights and density, dark tile, outline only, accent on the C instead of the points,
  dashed construction), shown at 256, 64, 32 and 16 px, on light and dark.
- When the owner picks one, it replaces Curva's icon everywhere (app, installer, README).
