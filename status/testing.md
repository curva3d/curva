# Status: testing stream

*Written only by the testing stream, which tests Curva's commands and features against real Rhino 8
and turns every difference into a GitHub issue (label `rhino-parity` plus the owning stream's label:
`features`, `display` or `mesh`). It fixes nothing in code another stream owns; it may add tests that
pin Rhino-measured results. Newest first.*

## How the testing is done

- **Rhino 8** through the `rhino` MCP tools: each case is run with Rhino's own command (`_-Sweep1`,
  `_Box` and so on, with typed points) and the result measured with RhinoCommon (lengths, areas,
  volumes, face counts, bounding boxes).
- **Curva** through its command processor (the same code the command line drives), with a probe that
  types the same answers, clicks the same points and measures what was made, and times each step and
  each preview. Scenario files and results are kept in `E:\curva-scratch\testing\`.
- **Same\*** in the table means Curva's result equals the exact geometric answer for that case, which
  Rhino gives too (for example a box minus a cylinder), but the case has not yet been run in Rhino
  itself. These rows are rerun in Rhino as soon as it is available again.
- Interface tests (gumball, Ctrl+Shift picks, snaps, panels, layouts) need computer use on Curva's
  window; they are still to come (see Problems).

## Testing stream — 25 November 2026 (first session)

- Measured in Rhino 8: Sweep1 (6 cases), Sweep2 (3 cases) and 30 typed cases of curve and solid
  commands; in Curva the same cases, plus booleans, surfaces, curve editing, transforms and analysis
  against exact results.
- **Issues filed (features stream):** #665 and #666 (Sweep1), #667 (Sweep2), #668 (Polyline does not
  close on its start point), #669 (InterpCrv), #670 (Lines), #671 (round solids lag while their size
  follows the mouse), #672 (the first solid preview of a session freezes Curva for about 4 s; very
  likely the owner's "lag at the height step"), #673 (Area gives 0 for surfaces and solids), #674
  (Revolve's angle prompt shows `<{0}>`).
- **What matches:** every typed curve and primitive solid measured (Line, Polyline with Enter and
  Close, Curve, Circle, Arc, Ellipse, Rectangle, Box, Sphere, Cylinder, Cone, TruncatedCone, Torus,
  Plane, Point, Points): the same lengths, areas, volumes and boxes as Rhino. Sweep1 with one section
  and Sweep2 with one section at the start also match.

### Problems

- **Rhino:** after two scripted cases hung their Rhinos, new Rhino slots stopped starting ("did not
  bind port 10501 within 120 s", probably a dialog on the new window), and the adopted Rhino
  `armadillo` is waiting inside a scripted `_-Sweep1` (an Esc in its window frees it). Reported to
  the session that started this stream. Lesson: Rhino's `_-Polygon 0,0,0 5`, and `_-Sweep1` with
  only two Enters, wait for more input; never send them.
- **Computer use** on Curva's window was declined, so the gumball, Ctrl+Shift picks, snaps, panels and
  layouts are not tested yet.

### Next

1. When Rhino is back: rerun the **Same\*** rows in Rhino; measure Tube, Slot, Ellipsoid and Helix;
   check which objects Rhino leaves selected after Loft, ExtrudeCrv and Pipe (Curva keeps the input
   selected, so Enter repeats the command on it).
2. With computer use: the gumball (scale handles, circle, relocating by double-click), Ctrl+Shift
   sub-object picks in FilletEdge, Shell and MoveFace, snaps, layouts and detail views.
3. The remaining families: surfaces (Network, EdgeSrf, Patch, Blend), solids (FilletEdge, Shell),
   mesh, SubD, annotation, layouts, selection.

## Commands

State against Rhino 8: **same** (measured in both), **same\*** (matches the exact result; Rhino run
pending), **differs** (issue filed), **missing**, or **not yet** (not tested).

### Curves

| Command | State | Issue | Notes |
|---|---|---|---|
| Line | same |  | typed points, BothSides |
| Polyline | differs | #668 | Enter and Close match; picking the start point again does not close it |
| InterpCrv | differs | #669 | length 23.84 against Rhino's 22.09 (knot spacing) |
| Circle | same |  | centre and radius, 2Point, 3Point |
| Point | same |  |  |
| Divide | same* |  | circle into 6 points |
| Rebuild | not yet |  |  |
| PointsOn | not yet |  |  |
| PointsOff | not yet |  |  |
| InsertKnot | not yet |  |  |
| RemoveKnot | not yet |  |  |
| RemoveControlPoint | not yet |  |  |
| CloseCrv | not yet |  |  |
| SetPt | not yet |  |  |
| Offset | same* |  | circle (37.699) and rectangle with sharp corners (48) |
| Fillet | same* |  | two lines r=2 and two segments of a polyline |
| FilletCorners | same* |  | rectangle r=1, 38.283 |
| TweenCurves | not yet |  |  |
| Chamfer | same* |  |  |
| Ellipse | same |  |  |
| Arc | same |  | three points; centre, start, angle |
| Rectangle | same |  | corners, Center, typed length and width |
| Polygon | not yet |  |  |
| Helix | not yet |  |  |
| Spiral | not yet |  |  |
| Points | same |  |  |
| Lines | differs | #670 | Curva pairs points, Rhino chains them |
| Slot | not yet |  |  |
| Parabola | not yet |  |  |
| ChangeDegree | not yet |  |  |
| HandleCurve | not yet |  |  |
| Sketch | not yet |  |  |
| Conic | not yet |  |  |
| Hyperbola | not yet |  |  |
| BlendCrv | not yet |  |  |
| OffsetMultiple | not yet |  |  |
| Extend | not yet |  |  |
| Fair | not yet |  |  |
| CurveBoolean | not yet |  |  |
| SimplifyCrv | not yet |  |  |
| Reparameterize | not yet |  |  |
| MakeUniform | not yet |  |  |
| Weight | not yet |  |  |
| TextObject | not yet |  |  |
| Curve | same |  | control points, degree 3 |

### Surfaces

| Command | State | Issue | Notes |
|---|---|---|---|
| Plane | same |  |  |
| SrfPt | not yet |  |  |
| ExtrudeSrf | not yet |  |  |
| DupEdge | not yet |  |  |
| DupBorder | not yet |  |  |
| ExtrudeCrvTapered | not yet |  |  |
| ExtrudeCrvAlongCrv | not yet |  |  |
| ExtrudeCrvToPoint | not yet |  |  |
| RailRevolve | not yet |  |  |
| ExtractSrf | not yet |  |  |
| DupFaceBorder | not yet |  |  |
| EdgeSrf | not yet |  |  |
| NetworkSrf | not yet |  |  |
| Patch | not yet |  |  |
| ExtendSrf | not yet |  |  |
| Untrim | not yet |  |  |
| Project | not yet |  |  |
| RebuildUV | not yet |  |  |
| ExtractIsocurve | not yet |  |  |
| ShrinkTrimmedSrf | not yet |  |  |
| BlendSrf | not yet |  |  |
| MatchSrf | not yet |  |  |
| Sweep2 | differs | #667 | one section at the start matches; two sections impossible, section mid-rail fails |
| Pull | not yet |  |  |
| InterpCrvOnSrf | not yet |  |  |
| OffsetCrvOnSrf | not yet |  |  |
| UnrollSrf | not yet |  |  |
| FilletSrf | not yet |  |  |
| SplitSrf | not yet |  |  |
| ExtrudeCrv | same* |  | circle to a solid, volume 785.398 |
| PlanarSrf | same* |  |  |
| Revolve | differs | #674 | result the same (area 314.159); the angle prompt shows "<{0}>" |
| Loft | same* |  | two circles, area 256.30 |
| Sweep1 | differs | #665 #666 | one section matches (area within 0.001); several sections, a section away from the start, and polyline rails fail |
| OffsetSrf | same* |  | box offset 1 to 12×12×12 |

### Solids

| Command | State | Issue | Notes |
|---|---|---|---|
| Volume | same* |  | box 150 |
| Box | same |  | corners, typed sizes, Enter for the width; first preview of a session freezes (#672) |
| Sphere | same |  |  |
| Cylinder | differs | #671 #672 | result the same; preview 60 ms per pointer move, first one 4 s |
| Cone | differs | #671 | result the same; preview 64 ms per pointer move |
| TruncatedCone | same |  |  |
| Torus | differs | #671 | result the same; preview 320 ms per pointer move |
| Tube | differs | #671 | preview 120 ms per pointer move; result not yet measured in Rhino |
| Ellipsoid | not yet |  |  |
| BooleanSplit | not yet |  |  |
| Paraboloid | not yet |  |  |
| Pyramid | not yet |  |  |
| MergeAllFaces | not yet |  |  |
| Wirecut | not yet |  |  |
| EditBoolean | not yet |  |  |
| BreakBoolean | not yet |  |  |
| MoveFace | not yet |  |  |
| PushPull | not yet |  |  |
| Cap | not yet |  |  |
| ExtrudeFace | not yet |  |  |
| ChamferSrf | not yet |  |  |
| Pipe | same* |  | line, radius 1, capped (volume 31.416); Rhino's end radius step not asked |
| FilletEdge | not yet |  |  |
| ChamferEdge | not yet |  |  |
| Shell | not yet |  |  |
| BooleanUnion | same* |  | volume 1500 |
| BooleanDifference | same* |  | box minus cylinder, volume 874.336 |
| BooleanIntersection | same* |  | volume 500 |

### Transforms and editing

| Command | State | Issue | Notes |
|---|---|---|---|
| Move | same* |  |  |
| Copy | same* |  |  |
| Rotate | same* |  |  |
| Scale | same* |  |  |
| Mirror | same* |  |  |
| Array | same* |  | 3×2 with spacing |
| ArrayPolar | not yet |  |  |
| ArrayCrv | not yet |  |  |
| Orient | not yet |  |  |
| Smooth | not yet |  |  |
| History | not yet |  |  |
| Flip | not yet |  |  |
| ProjectToCPlane | not yet |  |  |
| Join | same* |  |  |
| Explode | same* |  |  |
| Split | same* |  |  |
| Trim | same* |  |  |
| Orient3Pt | not yet |  |  |
| Rotate3D | not yet |  |  |
| Scale1D | not yet |  |  |
| Scale2D | not yet |  |  |
| ScaleNU | not yet |  |  |
| ArrayLinear | not yet |  |  |
| Align | not yet |  |  |
| Distribute | not yet |  |  |
| Shear | not yet |  |  |
| Twist | not yet |  |  |
| Bend | not yet |  |  |
| Taper | not yet |  |  |
| Flow | not yet |  |  |
| CageEdit | not yet |  |  |
| Delete | not yet |  |  |
| Undo | not yet |  |  |
| Redo | not yet |  |  |

### Analysis

| Command | State | Issue | Notes |
|---|---|---|---|
| Dir | not yet |  |  |
| BoundingBox | not yet |  |  |
| Distance | not yet |  |  |
| Length | same* |  | circle 31.416 |
| Area | differs | #673 | surfaces and solids give 0 |
| Angle | not yet |  |  |
| EvaluatePt | not yet |  |  |
| Radius | not yet |  |  |
| What | not yet |  |  |
| AreaCentroid | not yet |  |  |
| VolumeCentroid | not yet |  |  |
| ShowEdges | not yet |  |  |
| Plan | not yet |  |  |
| Zebra | not yet |  |  |
| Count | not yet |  |  |
| Audit | not yet |  |  |
| DraftAnalysis | not yet |  |  |
| Section | not yet |  |  |
| Contour | not yet |  |  |
| Intersect | not yet |  |  |
| CurvatureGraph | not yet |  |  |
| Silhouette | not yet |  |  |

### Mesh

| Command | State | Issue | Notes |
|---|---|---|---|
| Mesh | not yet |  |  |
| MeshBox | not yet |  |  |
| MeshPlane | not yet |  |  |
| MeshSphere | not yet |  |  |
| MeshCylinder | not yet |  |  |
| MeshCone | not yet |  |  |
| MeshTorus | not yet |  |  |
| Check | not yet |  |  |
| MeshRepair | not yet |  |  |
| UnifyMeshNormals | not yet |  |  |
| FillMeshHoles | not yet |  |  |
| CullDegenerateMeshFaces | not yet |  |  |
| RebuildMeshNormals | not yet |  |  |
| ReduceMesh | not yet |  |  |
| TriangulateMesh | not yet |  |  |
| QuadrangulateMesh | not yet |  |  |
| ExtractMeshFaces | not yet |  |  |
| MeshToNURB | not yet |  |  |
| MeshBooleanUnion | not yet |  |  |
| MeshBooleanDifference | not yet |  |  |
| MeshBooleanIntersection | not yet |  |  |
| MeshBooleanSplit | not yet |  |  |
| QuadRemesh | not yet |  |  |

### SubD

| Command | State | Issue | Notes |
|---|---|---|---|
| SubDBox | not yet |  |  |
| SubDPlane | not yet |  |  |
| SubDSphere | not yet |  |  |
| SubDCylinder | not yet |  |  |
| SubDCone | not yet |  |  |
| SubDTorus | not yet |  |  |
| ToSubD | not yet |  |  |
| SubDDisplayToggle | not yet |  |  |
| ToNURBS | not yet |  |  |
| Crease | not yet |  |  |
| RemoveCrease | not yet |  |  |
| Fill | not yet |  |  |
| UnweldEdge | not yet |  |  |
| WeldEdge | not yet |  |  |
| Stitch | not yet |  |  |
| Bridge | not yet |  |  |
| InsertEdge | not yet |  |  |
| MergeFaces | not yet |  |  |
| InsertPoint | not yet |  |  |
| OffsetSubD | not yet |  |  |
| Bevel | not yet |  |  |
| SubDLoft | not yet |  |  |
| MultiPipe | not yet |  |  |

### Annotation

| Command | State | Issue | Notes |
|---|---|---|---|
| Text | not yet |  |  |
| Dim | not yet |  |  |
| DimAligned | not yet |  |  |
| DimRotated | not yet |  |  |
| DimRadius | not yet |  |  |
| DimDiameter | not yet |  |  |
| DimAngle | not yet |  |  |
| DimOrdinate | not yet |  |  |
| DimCurveLength | not yet |  |  |
| DimArea | not yet |  |  |
| DimVolume | not yet |  |  |
| DimArcLength | not yet |  |  |
| Leader | not yet |  |  |
| Dot | not yet |  |  |
| Hatch | not yet |  |  |
| SetHatchBasePoint | not yet |  |  |
| HatchPatterns | not yet |  |  |
| AnnotationStyle | not yet |  |  |
| SetCurrentAnnotationStyle | not yet |  |  |
| DupAnnotationStyle | not yet |  |  |
| Styles | not yet |  |  |

### Layouts

| Command | State | Issue | Notes |
|---|---|---|---|
| PrintWidth | not yet |  |  |
| SetLayoutUserText | not yet |  |  |
| Layout | not yet |  |  |
| CopyLayout | not yet |  |  |
| Layouts | not yet |  |  |
| Detail | not yet |  |  |
| Print | not yet |  |  |
| ExportPDF | not yet |  |  |
| Make2D | not yet |  |  |
| TitleBlock | not yet |  |  |
| ScaleBar | not yet |  |  |
| SheetIndex | not yet |  |  |
| Revision | not yet |  |  |
| ClippingPlane | not yet |  |  |
| SectionStyle | not yet |  |  |

### Selection

| Command | State | Issue | Notes |
|---|---|---|---|
| Hide | not yet |  |  |
| Show | not yet |  |  |
| SelAll | not yet |  |  |
| SelNone | not yet |  |  |
| SelCrv | not yet |  |  |
| SelSrf | not yet |  |  |
| SelSolid | not yet |  |  |
| SelPt | not yet |  |  |
| SelName | not yet |  |  |
| SelLayer | not yet |  |  |
| SelDup | not yet |  |  |
| SelLast | not yet |  |  |
| SelPrev | not yet |  |  |
| SelFilter | not yet |  |  |
| SelFilterToggle | not yet |  |  |
| Isolate | not yet |  |  |
| Invert | not yet |  |  |
| SelLine | not yet |  |  |
| SelPolyline | not yet |  |  |
| SelCircle | not yet |  |  |
| SelArc | not yet |  |  |
| SelClosedCrv | not yet |  |  |
| SelOpenCrv | not yet |  |  |
| HideSwap | not yet |  |  |
| SelColor | not yet |  |  |
| SelBlockInstanceNamed | not yet |  |  |

### Views

| Command | State | Issue | Notes |
|---|---|---|---|
| ZoomSelected | not yet |  |  |
| EnableClippingPlane | not yet |  |  |
| DisableClippingPlane | not yet |  |  |
| ZoomExtents | not yet |  |  |
| 4View | not yet |  |  |
| UndoView | not yet |  |  |
| RedoView | not yet |  |  |
| Top | not yet |  |  |
| Front | not yet |  |  |
| Right | not yet |  |  |
| Perspective | not yet |  |  |
| CPlane | not yet |  |  |
| Zoom | not yet |  |  |
| NamedView | not yet |  |  |
| NamedCPlane | not yet |  |  |
| SetDisplayMode | not yet |  |  |

### Organising

| Command | State | Issue | Notes |
|---|---|---|---|
| Rename | not yet |  |  |
| Match | not yet |  |  |
| Lock | not yet |  |  |
| Unlock | not yet |  |  |
| Group | not yet |  |  |
| Ungroup | not yet |  |  |
| Block | not yet |  |  |
| Insert | not yet |  |  |
| BlockManager | not yet |  |  |
| BlockEdit | not yet |  |  |
| ExplodeBlock | not yet |  |  |
| ReplaceBlock | not yet |  |  |
| ChangeLayer | not yet |  |  |
| MatchProperties | not yet |  |  |
| Linetype | not yet |  |  |
| ObjectColour | not yet |  |  |
| SetUserText | not yet |  |  |
| GetUserText | not yet |  |  |
| SetDocumentUserText | not yet |  |  |
| GetDocumentUserText | not yet |  |  |
| Notes | not yet |  |  |
| Purge | not yet |  |  |
| LockSwap | not yet |  |  |
| UnlockSelected | not yet |  |  |
| BringToFront | not yet |  |  |
| SendToBack | not yet |  |  |
| BringForward | not yet |  |  |
| SendBackward | not yet |  |  |

### Files

| Command | State | Issue | Notes |
|---|---|---|---|
| Save | not yet |  |  |
| SaveAs | not yet |  |  |
| Open | not yet |  |  |
| Import | not yet |  |  |
| Export | not yet |  |  |
| ExportSelected | not yet |  |  |
| New | not yet |  |  |

