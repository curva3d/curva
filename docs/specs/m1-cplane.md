# Spec: Milestone 1, construction planes

**Status:** Approved by the owner in chat · 9 November 2026 (the plan this was part of, with "keep going")
**Scope comes from:** architecture section 10, Milestone 1.

## 1. What the owner will be able to do

- `CPlane`: pick an origin, a point along X and a point towards Y. The last-used viewport then draws, snaps and shows its grid on that plane.
- Type `World` at the first prompt to put that viewport back on its standard plane.
- Typed coordinates now follow the viewport plane, as in Rhino: in Front, `3,2` means X 3 and Z 2. Put `w` in front for world coordinates, such as `w3,2,0`.

## 2. Not in this piece of work

Named construction planes, and aligning a plane to an object.

## 3. How it is checked

No Rhino tests (decision 0011). Automatic checks cover the plane built from three points, resetting it, points in a line, and typed points on each plane.

**The owner's check:** he tries it by hand and says whether it behaves like Rhino.
