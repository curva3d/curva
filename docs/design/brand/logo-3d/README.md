# Curva logo: the C/U curve

Sources for Curva's logo since 21 November 2026 (decision 0040, which replaces 0022). The app
icons in `packaging/icons/` are made from these.

## The idea

One closed curve inside a 50 × 50 × 50 cube. Seen from the front it reads as a **C** (Curva);
seen from the top it reads as a **U** (cUrva). The logo shows the curve as a soft, thick tube in an
axonometric view, coloured with one two-tone gradient.

Current pick: **R4** view (turn −70°, tilt 35°), corners filleted at radius **15**, tube 20 wide.

The app icon has **no tile**: the mark floats with a soft drop shadow. All three versions have a
fine **3.5% film grain**. They are in `renders/set/`, made by `scripts/gen_set.py` (owner's pick, 21 November 2026):

| Version | Use | Shading |
|---|---|---|
| Amber | Main icon | shadow side #E0603F, light side #F6C35B; the colour also flows along the loop |
| Black | On light backgrounds | #060606 → #E8E8E6, lit by the light only (no flow along the loop) |
| White | On dark backgrounds | #9E9C95 → #FFFFFF, lit by the light only |

Why the black and white have no flow along the loop: in colour, the amber's shift from vermilion to
yellow reads as two colours; in black and white, any change in brightness reads as shadow, so the
same flow made one loop look as if it sat in a dark hole.

Each version comes as the bare mark (`*-mark.png`, 1024 px, rendered from 2048 px) and as floating icons
from 1024 down to 16 px (`*-icon-<size>.png`). `board.png` shows them on light and dark, in the dock,
as lockups and at small sizes.

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
- `gen_set.py` makes the final amber / black / white set in `renders/set/`; `grain.py` adds the film grain.
- `gen_graphite.py` holds the floating-icon and dock helpers.
- `gen_final.py` makes the colourways, tiles, flat one-colour versions, lockups and small sizes into
  `renders/`.
- `gen_base.py`, `gen_2d.py`, `gen_tube2d.py`, `sdf.py` are helpers from earlier rounds that the
  others import.

`renders/set/` holds the three bare marks and `board.png`; `renders/animation/` the rotating logo
(WebM with transparency, animated WebP, MP4 on dark). The icons in every size, the other animations and
the exploration renders (colourways, angles, fillets) are on the branch `brand-logo-3d-sources`.

## Open points

- At 16 px the 3D mark blurs; the 16–48 px icons are drawn without grain to stay as crisp as possible.
  A flat favicon may still be needed for the website.
- The top view reads as a U on its side; pictures of the plan turn it 90°.
- Recent macOS versions may place an icon that isn't a rounded square on a grey plate. Check after release.
- These scripts are design tools, not part of anything Curva builds; they need Python with `numpy`,
  `pillow`, `scipy`, `rhino3dm` and `cairosvg`.
