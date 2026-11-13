# Spec: the engine's gaps, built in Curva and flagged

**Status:** Approved by the owner in chat · 13 November 2026.
**Covers:** `milestone-2-in-0.4.2.md`, points 4 and 5; decision 0019 (4).

## 1. The rule

solidcore is Curva's engine (decision 0010). Where it cannot do an operation yet, Curva builds the
operation in its own kernel (`crates/curva-kernel`), on the engine's lower-level pieces. Every such
operation is **custom** and:

- is marked in the code with a `// CUSTOM KERNEL OPERATION: <name>` comment on the function that does
  it, so a search finds them all;
- has a row in `docs/kernel/custom-operations.md`: the operation, the commands that use it, the
  solidcore function that would replace it, and the solidcore revision last checked;
- is checked before every release (a new step in `docs/releasing.md`, before tagging): update the
  pinned solidcore to its latest revision on a branch, and for each row see whether solidcore now
  covers the operation fully (same results on that operation's tests). If it does, switch to
  solidcore's, delete Curva's own code and its row. The release notes say what moved.

A test fails if a function carries the custom mark without a row in the list, or a row has no marked
function.

## 2. First: the latest solidcore

Curva pins solidcore at `c440e5f`. Work starts by moving to solidcore's latest revision (with the
whole test suite passing) so nothing is built that the engine already has.

## 3. The 34 commands, and how each is made

Three kinds: **engine** (solidcore has it; Curva only wraps it, not custom), **custom** (built in
Curva, flagged), and **custom on engine** (mostly the engine's pieces, assembled by Curva, flagged).

| Command | How | Kind |
|---|---|---|
| ExtendSrf | solidcore `extend_sheet` | engine |
| Untrim | solidcore `untrim_face` | engine |
| SplitSrf | solidcore `split_faces` with curves and surfaces as tools | engine |
| Intersect | solidcore-ssi `intersect` for surfaces, curve crossings for curves | engine |
| Section, Contour | solidcore `face_section` with planes (Contour: a plane every interval) | engine |
| MoveFace | solidcore `offset_faces`, or moving the face and re-sewing its neighbours | custom on engine |
| ExtrudeFace | the face's outline extruded and joined to the solid (`assemble.rs`) | custom on engine |
| Cap | planar faces filling each open loop, sewn with `Body::assembled` (which already closes solids) | custom on engine |
| Shear | an affine map on the body (`transform` takes any matrix) | custom on engine |
| ShrinkTrimmedSrf | the face's surface cut to the box of its trims, record rebuilt | custom |
| EdgeSrf | a Coons patch from 2–4 edge curves, made compatible, as one NURBS face | custom |
| Sweep2 | profiles moved along two rails, scaled to span them, skinned (`sweep_stations`) | custom on engine |
| NetworkSrf | a Gordon surface through the two families of curves | custom |
| Patch | a NURBS surface fitted through the curves and points, trimmed to the outer boundary | custom |
| BlendSrf | a surface between two edges, matching position, tangency or curvature across each | custom |
| MatchSrf | the surface's edge rows of control points moved to match another edge | custom |
| FilletSrf, ChamferSrf | the rolling-ball (or chamfer) strip between two surfaces, trimmed back | custom |
| RebuildUV | the surface refitted with a new number of control points | custom |
| ExtractIsocurve | the surface's curve at constant u or v, exact from its control net | custom |
| InterpCrvOnSrf | a curve through points in the surface's (u, v), lifted onto it | custom |
| OffsetCrvOnSrf | the curve moved along the surface by a distance, kept on it | custom |
| Project | curves projected along the view direction onto surfaces | custom on engine |
| Pull | curves pulled to the closest points of surfaces, refitted | custom |
| Silhouette | the outline of surfaces seen from the view, from where normals turn | custom |
| UnrollSrf | developable surfaces laid flat (ruled ones exactly; others refused) | custom |
| Twist, Bend, Taper, Flow, CageEdit | every face's control net deformed, trims kept in (u, v), edges refitted | custom |
| CurvatureGraph | curvature combs drawn from the exact derivatives (`Spline`) | custom |
| Zebra | stripes drawn on shaded faces by the viewport | custom |

The new kernel pieces these share, each tested on its own: a NURBS surface type with evaluation,
knot insertion, isocurves and closest point (`surface.rs`, the surface sibling of `spline.rs`);
building a face record from a NURBS surface and its trims, so any of the above becomes a body; making
curves compatible (one degree, one knot vector); fitting a surface through points.

## 4. How it is checked

- Each command: tests of its options against known shapes (areas, volumes, points on the result),
  its preview, an icon, its row in `rhino-reference.md`, its status moved from `blocked` in
  `rhino-parity.md` (with a note where it differs from Rhino).
- The custom list test (§1).
- In the app: each command run on a sample model.
