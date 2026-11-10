# Spec: Milestone 1, control points

**Status:** Part of the approved F1 command list (architecture section 9, Edit) · 10 November 2026. The owner asked for development to continue without being asked.
**Scope comes from:** architecture section 10, Milestone 1, and the F1 Edit commands InsertKnot, RemoveKnot, RemoveControlPoint and Smooth.

## 1. What the owner will be able to do

- `PointsOn` (F10) shows the control points of the chosen curves; `PointsOff` (F11) hides them again.
- Click or drag a box around control points to select them, with Shift to add and Ctrl to remove, as with objects.
- `Move` and the gumball move selected control points, with the same live preview as moving objects. Each move is one undo step.
- `SetPt` lines selected control points up.
- `InsertKnot` and `InsertControlPoint` add a control point where the curve is picked; `RemoveKnot` and `RemoveControlPoint` take one away.
- `Smooth` evens out selected control points.

## 2. How each kind of curve shows its points

| Curve | Control points |
|---|---|
| Line, polyline | Its corners. It stays straight-sided when they move. |
| NURBS | Its control points. |
| Interpolated | The points it passes through, as with Rhino's edit points. |
| Circle, arc, compound | Shown as NURBS control points; moving one turns the curve into NURBS. |

## 3. Not in this piece of work

Weights, edit points on NURBS curves, and dragging control points directly with the mouse; for now they move with `Move` and the gumball. Taking a point away from a NURBS curve respaces its knots, so the curve moves more than Rhino's RemoveKnot, which keeps the shape as close as it can; a closer match can come later.

## 4. How it is checked

No Rhino tests (decision 0011). Automatic checks cover the points of each kind of curve, moving one point and undoing it, removing and inserting points, and Smooth keeping the ends of an open curve.

**The owner's check:** tries it by hand and says whether it behaves like Rhino.
