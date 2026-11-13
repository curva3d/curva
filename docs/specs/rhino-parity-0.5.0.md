# Spec: the last 31 missing Rhino commands, and fixing the approximations

**Status:** Draft, for the owner's approval · 13 November 2026.
**Covers:** feedback on 0.4.0, points 3 and 4. Follows `rhino-parity.md` (how each command is built,
how it is checked), which still applies.

## 1. The approximations, noted and fixed

`docs/commands/rhino-parity.md` gains a *Notes* column. Every command that differs from Rhino says how,
in one line, until it is fixed; a test fails if a `partly` command has no note.

| Command | 0.4.0 | Fix in 0.5.0 |
|---|---|---|
| ExtrudeSrf | Thickens along the surface normal | As Rhino: straight along a direction (default the construction plane's normal; `Direction` option), sides and caps added; `BothSides`, `Solid`. Planar faces exactly; curved surfaces too, through their boundary and a moved copy, where the engine can join them (otherwise `blocked`, noted). |
| DupEdge | Assumes edges come in the engine's order | Uses the document's edge identities (the same as edge picking), so the copied curve is always the edge picked. |
| ChangeDegree | Closed curves come out open | Closed curves stay closed (periodic knots kept); `Deformable` option as Rhino. |
| AreaCentroid, VolumeCentroid | From a display mesh | From the engine's exact geometry where it gives it; otherwise from a mesh refined until the result is stable to the document's tolerance, and the answer says which. |

## 2. The 31 commands

Rhino's steps and options from its help; each gets a recipe, tests for every option, an icon, its row in
`rhino-reference.md`, and a place in a Commands tab.

**Curves (drawing)**

| Command | What it does in Curva |
|---|---|
| HandleCurve | Draws a curve by points with tangent handles (as Rhino's HandleCurve / Bézier pen): click a point, drag its handle. |
| Sketch | Draws a freehand curve following the pointer while the button is held, fitted to a smooth curve. |
| Conic | Conic by start, end, apex and a rho value (0–1); `Rho` option, or a point the curve passes through. Exact (rational). |
| Hyperbola | By centre, vertex and focus (or `Vertices`, `Coefficients`), as Rhino; exact rational curve. |
| BlendCrv | A curve joining the ends of two curves, matching position, tangency or curvature (`Continuity`); the end picked is the end joined. |
| OffsetMultiple | Offsets several curves at once by one distance to one side. |
| DupBorder | Copies the open border of a surface or polysurface as curves. |

**Curves (editing)**

| Command | What it does in Curva |
|---|---|
| Extend | Extends a curve to a boundary, or by a length (`Type` = Line, Arc, Smooth). |
| Fair | Smooths a curve's curvature within a tolerance, keeping its ends. |
| CurveBoolean | Picks regions inside overlapping closed planar curves; makes their outlines. |
| Match | Changes the end of a curve to match another curve's end, at position, tangency or curvature. |
| SimplifyCrv | Replaces near-line and near-arc parts of a curve with exact lines and arcs. |
| Reparameterize | Sets a curve's domain (start and end parameters). |
| MakeUniform | Makes a curve's knots uniform, keeping its control points. |
| Weight | Sets the weight of picked control points (rational curves). |

**Surfaces and solids**

| Command | What it does in Curva |
|---|---|
| ExtrudeCrvTapered | Extrudes a curve with a draft angle (`DraftAngle`, `Corners`, `Solid`). |
| ExtrudeCrvAlongCrv | Extrudes a curve along a path curve, without turning (a translational sweep). |
| ExtrudeCrvToPoint | Extrudes a curve to a point (a cone or pyramid from any curve). |
| RailRevolve | Revolves a profile around an axis while following a rail curve. |
| Pyramid | A pyramid: a polygon base (`NumSides`, `Circumscribed`, `Star`), then the apex height. |
| ExtractSrf | Separates picked faces of a polysurface into their own surfaces (`Copy` option). |
| MergeAllFaces | Merges the coplanar faces of a polysurface that share an edge. |
| Wirecut | Cuts a solid with a curve extruded through it (both depths, `KeepAll`). |

**Transform, select, organise**

| Command | What it does in Curva |
|---|---|
| Distribute | Spaces objects evenly along X, Y or Z (`Mode` = Centres, Gap). |
| SelColor | Selects every object with the same displayed colour as the one picked. |
| Lock / Unlock | Locks objects so they show but cannot be picked or changed; Unlock frees them all. |
| Group / Ungroup | Groups objects so one pick selects all; Ungroup breaks the group. |

**Views and analysis**

| Command | What it does in Curva |
|---|---|
| ShowEdges | Highlights a polysurface's edges, naked (open) edges in another colour, and counts them. |
| Plan | Turns the view to look straight down on the construction plane. |

Groups and locks need two new fields on objects (`group`, `locked`), saved in files; older files open
unchanged. Picking honours both. A command the engine cannot do is marked `blocked` with a report sent
upstream, as before.

## 3. Checks

- The parity test: no `missing` row left (only `done`, `partly` with a note, or `blocked` with a report).
- Every command's options typed and clicked, previews where they have one, an icon each.
- In the app, the owner's check against Rhino, group by group.
