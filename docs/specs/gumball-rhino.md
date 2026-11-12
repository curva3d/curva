# Spec: a gumball like Rhino's

**Status:** Approved by the owner in chat · 12 November 2026. Fourth in the order of work (decision 0016).
**Covers:** feedback items 2 and 3 (`docs/specs/feedback-2026-11-12.md`). Replaces the handles of
`m1-gumball.md`; how the gumball follows the selection and the construction plane stays as it is.

## 1. The handles

Drawn on the selection's centre, along the construction plane's axes, in red (X), green (Y) and blue (Z):

| Handle | Where | Drag it to |
|---|---|---|
| **Arrow** | the tip of each axis | move along that axis |
| **Circle** (extrude) | midway along each axis | extrude along that axis: curves become surfaces, flat surfaces and faces become solids, as `ExtrudeCrv` and `ExtrudeSrf` do. Ctrl-drag on an arrow does the same, as in Rhino |
| **Arc** | a quarter circle between each pair of axes | turn about the third axis |
| **Small square** | the end of each axis, just inside the arrow | scale along that axis; Shift-drag scales evenly |
| **Plane square** | a small filled square, tinted by its plane (XY blue, YZ red, XZ green), between each pair of axes | move freely in that plane |
| **Centre** | the dot at the centre | move freely on the construction plane |

## 2. Behaviour

- Hovering a handle lights it in the accent orange, and the pointer becomes a grab hand.
- While dragging, the cursor tag shows the distance, angle or factor, and the result is ghosted in orange
  (`command-framework.md` §4). Snaps and sticky snaps apply to the point dragged.
- **Typed values:** clicking a handle without dragging opens a small box at the cursor for an exact value
  (a distance, angle or factor), as in Rhino.
- Each drag is one undo step.

## 3. How it is checked

- Automatic:
  - each handle is found under the pointer at its place
  - dragging an arrow moves along its axis only, and a plane square moves in its plane only
  - the extrude circle makes a surface from a curve and a solid from a flat surface
  - a scale square scales along one axis, and an arc turns about the third
  - a typed value is exact, and each drag is one undo step
- **The owner's check:** tries it against Rhino's gumball by hand.
