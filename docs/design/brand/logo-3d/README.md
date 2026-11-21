# Curva logo, 3D curve (work in progress)

Sources for the logo explored on 21 November 2026, which may replace the extruded C
(decision 0022). Nothing here is the published logo yet.

## The idea

One closed curve inside a 50 × 50 × 50 cube. Seen from the front it reads as a **C** (Curva);
seen from the top it reads as a **U** (cUrva). The logo shows the curve as a soft, thick tube in an
axonometric view, coloured with one two-tone gradient.

Current pick: **R4** view (turn −70°, tilt 35°), corners filleted at radius **15**, tube 20 wide.

## Files

`rhino/` holds the owner's Rhino models, in the order they were made:

| File | What it is |
|---|---|
| `logo_simple.3dm`, `logo_simple_fillet.3dm` | First block sketches |
| `logo_simple_curve_v1.3dm` | First freehand closed curve |
| `logo_simple_curve.3dm` | Clean curve: lines and arcs (radius 20) on the cube faces |
| `logo_simple_pipe.3dm` | That curve piped at radius 5 |
| `logo_simple_curve_sharp.3dm` | **Base for the current logo**: the same loop as 8 straight cube edges, filleted in the scripts |

`scripts/` makes the renders (Python 3, `numpy`, `pillow`, `rhino3dm`, `cairosvg`):

- `gen_fillet.py` reads the sharp curve and fillets its corners in 3D.
- `gen_duo.py` draws the tube: per-pixel depth, two-colour shading (lit side / shadow side) plus a
  colour run along the loop.
- `gen_final.py` makes the colourways, tiles, flat one-colour versions, lockups and small sizes into
  `renders/`.
- `gen_base.py`, `gen_2d.py`, `gen_tube2d.py`, `sdf.py` are helpers from earlier rounds that the
  others import.

`renders/` holds the latest outputs (`board.png` is the overview, `compare.png` compares fillet 25, 20 and 15, `axo.png` shows isometric, dimetric and planometric).

## Open points

- Orange is in the logo here; decision 0022 kept orange out of the logo. Publishing this needs a
  new decision.
- The top view reads as a U on its side, not upright.
- Renders are bitmaps; the final logo needs a vector or high-resolution master.
