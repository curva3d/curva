# Spec: gumball and icons, after 0.3.0

**Status:** Proposed · 12 November 2026. Third in the order of work (decision 0017).
**Covers:** feedback items 26 to 29 (`docs/specs/feedback-0.3.0.md`). Revises `gumball-rhino.md` §1
and extends `ui-look-and-layout.md` §5.

## 1. Gumball

- The **scale square goes.** The **circle** on each axis extrudes curves and open surfaces, and
  stretches closed solids along its axis. Shift-drag stretches anything instead; Shift+Ctrl scales
  evenly (decision 0017). A click without a drag asks for a distance, or a factor when it stretches.
- **Every action snaps:** moving along an arrow, in a plane or from the centre, extruding and
  stretching all take object snaps and held points: along an axis the result lines up with the snapped
  point; in a plane it goes to it.

## 2. Icons

- Every command on the Commands panel, and every command a button could hold, gets an icon in the
  current style (line drawings in the theme's text colour, the accent for points and what the command
  adds). Where Serpentine3D has one, it is redrawn from it; otherwise the icon takes Rhino's idea for
  that command, in Curva's style and colours, never Rhino's colour coding.
- **Curve and InterpCrv differ:** Curve shows the curve with its control polygon off the curve (points
  away from it); InterpCrv shows the curve passing through its points.
- A test checks every command has an icon.
