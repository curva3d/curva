# Status: testing stream

*Written only by the testing stream, which tests Curva's commands and features against real Rhino 8
and turns every difference into a GitHub issue (label `rhino-parity` plus the owning stream's label:
`features`, `display` or `mesh`). It fixes nothing in code another stream owns; it may add tests that
pin Rhino-measured results. Newest first.*

## How the testing is done

- **Rhino 8** through the `rhino` MCP tools: each case is run with Rhino's own command (`_-Sweep1`,
  `_Box` and so on, with typed points) and the result measured with RhinoCommon (lengths, areas,
  volumes, face counts, extents). A watchdog in the helper cancels any scripted command still waiting
  after a few seconds, so a wrong macro no longer hangs Rhino.
- **Curva** through its command processor (the same code the command line drives), with a probe that
  types the same answers, clicks the same points, picks edges and faces, measures what was made and
  times each step and preview; and in the app itself with computer use (clicks, Ctrl+Shift, the
  gumball, snaps, layouts), side by side with Rhino 8 doing the same by hand.
- Scenario files, results and the probe live in `E:\curva-scratch\testing\`.
- **Same\*** means Curva's result equals the exact geometric answer for that case (which Rhino gives
  too), but the case has not been run in Rhino itself yet.

## Testing stream — 25 November 2026 (evening)

- **The owner's decision:** "Yes, as in Rhino": the gumball gets Rhino 8's scale squares back
  (#693, with Rhino's measured behaviour: squares on the negative side at the selection's extent; a
  drag scales about the centre by the distance ratio; a click takes a typed factor). Sent to the
  display and features streams; #528 points to it.
- **New issues (features):** #694 (Angle takes three points where Rhino takes two lines; Distance
  reports no deltas), #695 (three-point Circle, Arc, Rectangle and Box flatten typed 3D points onto
  the construction plane and fail). #692 (Knot and Vertex snaps) went to the display stream.
- **Matching Rhino:** every transform tried (Scale1D, Scale2D, ScaleNU, Rotate3D, Orient, ArrayPolar,
  Array, Mirror, Shear), ExtrudeSrf, NetworkSrf, RailRevolve, SrfPt, Divide, Fillet, the measuring
  commands' values, Hide, Show, Isolate, HideSwap, Invert, SelPrev, SelDup, SelLast, and Text, Dot,
  Leader, Hatch and dimensions.
- **Not reported:** first-use delays of 1–8 s (FilletEdge, Sphere, OffsetSrf, Pipe) showed up in
  fresh processes, but the same run once took 0.5 s and once 23.9 s with 14 compiler processes from
  other sessions running, so they are the machine, not Curva. To rerun on a quiet machine.
- **In the app:** #697 (a right-click without dragging is not Enter; Space already is) and #698 (the
  Layers panel takes no clicks at all: New Layer, On, the Properties tab and Search do nothing, in a
  fresh start too; every other panel works). Both for the features stream; #698 flagged as urgent.
- **Files both ways match Rhino:** a .3dm written by Curva (solids, curves, mesh, SubD, dimension) reads
  back in Rhino with every value; a .3dm written by Rhino (extrusion, trimmed solid, polycurve, mesh,
  SubD, text, hatch, dimension, point) imports into Curva with every value, SubD surface included.
- Habit difference noted, not filed: Rhino's Array and ArrayPolar wait for an Enter to accept; Curva's
  finish at once, so a Rhino user's extra Enter repeats the command.

## Testing stream — 25 November 2026 (afternoon)

- **Rhino and computer use working again.** New Rhino windows had been stuck on Rhino's "Autosave
  Recovery" dialog (left by closed slots); recovered into `E:\curva-scratch\testing\recovered-autosave.3dm`.
- **Issues filed since the morning:** #679 (SelSrf/SelSolid), #680 (objects picked inside a command
  stay selected), #682 (Ellipsoid third axis), #683 (ChamferEdge corners), #684 (Patch faces), #685
  (gumball double-click does not relocate), #686 (Ctrl+Shift click read at the end of the frame), #687
  (Layout makes no detail), #688 (BoundingBox), all for the features stream; #689 (SubD extent is the
  cage's) and #690 (mesh primitives weld sharp edges) for the mesh stream.
- **Checked fixed:** #668 (Polyline closes on its start point, in the app) and #671 (round solids
  preview as curves, which is what Rhino 8 does, checked by hand). Closed #672: the 4-second first
  preview no longer happens on `main`.
- **Matching Rhino now measured in Rhino:** booleans, Split, OffsetSrf, EdgeSrf, FilletEdge (all edges
  of a box), Shell, Tube, Helix, mesh primitives' faces and volumes, SubD primitives' cages and
  surfaces.
- **For the owner (a decision):** in 0.3.0 he asked for the gumball's scale squares to be removed
  (feedback item 26: the circle extrudes, and stretches where it cannot extrude; Shift-drag stretches).
  His 0.1.6 notes ask for Rhino's scale handles, which Rhino 8 still has (squares at the end of dotted
  lines on the negative side of each axis). Raised with the session that started this stream.

## Testing stream — 25 November 2026 (morning)

- Measured in Rhino 8: Sweep1 (6 cases), Sweep2 (3 cases) and 30 typed cases of curve and solid
  commands; in Curva the same, plus booleans, surfaces, curve editing, transforms and analysis.
- Issues #665–#674 for the features stream; tests pinned in #676
  (`crates/curva-commands/tests/rhino_measured_sweeps.rs` and seven rows of `rhino_measured.rs`).

### Lessons

- Rhino macros that wait for input hang the slot: `_-Polygon 0,0,0 5`, `_-Sweep1` with only two Enters,
  `_Divide` with the count typed after the selection. Use the watchdog.
- The owner's Rhino remembers options (Circle in Diameter mode, Polygon 6 sides circumscribed): set
  options explicitly when they matter.
- `open_application` on Curva starts a new copy instead of switching to the running one; bring the
  window forward by its handle instead.

### Next

1. The gumball's other handles and typed values against Rhino's; snaps (each one, and Tab, Space and
   Enter during picks); the panels (Layers, Properties).
2. Annotation (dimensions, text, hatches), the remaining surfaces (NetworkSrf, BlendSrf, Sweep options
   once #665–#667 land), SubD editing commands, and printing a layout.
3. Rerun the **Same\*** rows in Rhino.

## Commands

State against Rhino 8: **same** (measured in both), **same\*** (matches the exact result; Rhino run
pending), **differs** (issue filed), **missing**, or **not yet** (not tested). Counts: 23 differs, 222 not yet, 76 same, 2 same*.

### Curves

| Command | State | Issue | Notes |
|---|---|---|---|
| Line | same |  | typed points, BothSides |
| Polyline | same |  | fixed in #668 (start point closes it); checked in the app |
| InterpCrv | differs | #669 | length 23.84 against Rhino's 22.09 (knot spacing) |
| Circle | differs | #695 | 3Point with 3D points is flattened |
| Point | same |  |  |
| Divide | same |  | circle into 6 |
| Rebuild | not yet |  |  |
| PointsOn | not yet |  |  |
| PointsOff | not yet |  |  |
| InsertKnot | not yet |  |  |
| RemoveKnot | not yet |  |  |
| RemoveControlPoint | not yet |  |  |
| CloseCrv | not yet |  |  |
| SetPt | not yet |  |  |
| Offset | same |  | circle (37.699) and rectangle with sharp corners (48) (Rhino-measured in rhino_measured.rs) |
| Fillet | same |  | two lines r=2: 8, 8, 3.1416 (RhinoCommon) |
| FilletCorners | same |  | rectangle r=1, 38.283 (Rhino-measured in rhino_measured.rs) |
| TweenCurves | not yet |  |  |
| Chamfer | same* |  |  |
| Ellipse | same |  |  |
| Arc | differs | #695 | three typed 3D points are flattened onto the construction plane |
| Rectangle | differs | #695 | 3Point upright is refused |
| Polygon | not yet |  |  |
| Helix | same |  | length 188.755 against 188.760 |
| Spiral | not yet |  |  |
| Points | same |  |  |
| Lines | differs | #670 | Curva pairs points, Rhino chains them |
| Slot | same |  | Curva only: Rhino 8 has no Slot command |
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
| SrfPt | same |  | 102.9399 |
| ExtrudeSrf | same |  | solid by default (Rhino's factory default) |
| DupEdge | not yet |  |  |
| DupBorder | not yet |  |  |
| ExtrudeCrvTapered | not yet |  |  |
| ExtrudeCrvAlongCrv | not yet |  |  |
| ExtrudeCrvToPoint | not yet |  |  |
| RailRevolve | same |  | 314.129 against 314.159 |
| ExtractSrf | not yet |  |  |
| DupFaceBorder | not yet |  |  |
| EdgeSrf | same |  | area 102.9399 |
| NetworkSrf | same |  | 109.8230 against 109.8216 |
| Patch | differs | #684 | 4 faces against Rhino's 1 |
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
| ExtrudeCrv | same |  | circle to a solid, volume 785.398 (Rhino-measured in rhino_measured.rs) |
| PlanarSrf | same |  |  (Rhino-measured in rhino_measured.rs) |
| Revolve | differs | #674 | result the same (area 314.159); the angle prompt shows "<{0}>" |
| Loft | same |  | area 256.3047 in both |
| Sweep1 | differs | #665 #666 | one section matches (area within 0.001); several sections, a section away from the start, and polyline rails fail |
| OffsetSrf | same |  | box offset 1: 1728 |

### Solids

| Command | State | Issue | Notes |
|---|---|---|---|
| Volume | same |  | 150 |
| Box | differs | #695 | 3Point upright is refused; other ways match |
| Sphere | same |  |  |
| Cylinder | same |  | preview now curves only, as Rhino's (#671, checked by hand in Rhino) |
| Cone | same |  | preview curves only, as Rhino's (#671) |
| TruncatedCone | same |  |  |
| Torus | same |  | preview curves only (#671) |
| Tube | same |  | area 345.5752, volume 157.0796 |
| Ellipsoid | differs | #682 | typed length matches (volume 125.6637); a point for the third axis is refused |
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
| Pipe | same |  | line, radius 1, capped (volume 31.416); Rhino's end radius step not asked (Rhino-measured in rhino_measured.rs) |
| FilletEdge | same |  | all 12 box edges r=1: volume 975.587, 26 faces; picking edges in the app works |
| ChamferEdge | differs | #683 | corners: 18 faces, 946 against 26, 945.333 |
| Shell | same |  | open top t=1: 424, 11 faces |
| BooleanUnion | same |  | volume 1500 (Rhino-measured in rhino_measured.rs) |
| BooleanDifference | same |  | volume 874.3363, area 700.531 |
| BooleanIntersection | same |  |  |

### Transforms and editing

| Command | State | Issue | Notes |
|---|---|---|---|
| Move | same |  |  (Rhino-measured in rhino_measured.rs) |
| Copy | same |  |  (Rhino-measured in rhino_measured.rs) |
| Rotate | same |  |  (Rhino-measured in rhino_measured.rs) |
| Scale | same |  |  (Rhino-measured in rhino_measured.rs) |
| Mirror | same |  | copy by default, as Rhino |
| Array | same |  | Rhino waits for an Enter to accept; Curva does not |
| ArrayPolar | same |  | Rhino waits for an Enter to accept; Curva does not |
| ArrayCrv | not yet |  |  |
| Orient | same |  |  |
| Smooth | not yet |  |  |
| History | not yet |  |  |
| Flip | not yet |  |  |
| ProjectToCPlane | not yet |  |  |
| Join | same |  |  (Rhino-measured in rhino_measured.rs) |
| Explode | same |  |  (Rhino-measured in rhino_measured.rs) |
| Split | same |  |  |
| Trim | same* |  |  |
| Orient3Pt | not yet |  |  |
| Rotate3D | same |  |  |
| Scale1D | same |  |  |
| Scale2D | same |  |  |
| ScaleNU | same |  |  |
| ArrayLinear | not yet |  |  |
| Align | not yet |  |  |
| Distribute | not yet |  |  |
| Shear | same |  |  |
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
| BoundingBox | differs | #688 | "-inf" for solids, meshes and SubDs; no box made |
| Distance | differs | #694 | value matches; Rhino also reports angles and deltas |
| Length | same |  | 31.416 |
| Area | differs | #673 | surfaces and solids give 0 |
| Angle | differs | #694 | Curva takes 3 points (corner and sides), Rhino 4 (two lines) |
| EvaluatePt | same |  |  |
| Radius | same |  | click on a circle: 5.000 |
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
| MeshBox | differs | #690 | edges welded (602 vertices against 726) |
| MeshPlane | same |  |  |
| MeshSphere | same |  |  |
| MeshCylinder | differs | #690 | caps welded (112 against 132) |
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
| SubDBox | same |  | cage 26/24 |
| SubDPlane | not yet |  |  |
| SubDSphere | differs | #689 | surface matches; extent is the cage's |
| SubDCylinder | differs | #689 | surface matches; extent is the cage's |
| SubDCone | not yet |  |  |
| SubDTorus | not yet |  |  |
| ToSubD | not yet |  |  |
| SubDDisplayToggle | not yet |  |  |
| ToNURBS | not yet |  |  |
| Crease | not yet |  |  |
| RemoveCrease | not yet |  |  |
| Fill | not yet |  |  |
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
| Text | same |  | text, then where it goes, as Rhino |
| Dim | same |  | 10.00 |
| DimAligned | same |  | 10.44 |
| DimRotated | not yet |  |  |
| DimRadius | not yet |  |  |
| DimDiameter | not yet |  |  |
| DimAngle | not yet |  |  |
| DimOrdinate | not yet |  |  |
| DimCurveLength | not yet |  |  |
| DimArea | not yet |  |  |
| DimVolume | not yet |  |  |
| DimArcLength | not yet |  |  |
| Leader | same |  |  |
| Dot | same |  |  |
| Hatch | same |  | area right; default pattern Hatch1 (Rhino's factory default is Solid) |
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
| Layout | differs | #687 | no detail and no name |
| CopyLayout | not yet |  |  |
| Layouts | not yet |  |  |
| Detail | same |  | two clicks add a Top detail (rounded scale) |
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
| Hide | same |  |  |
| Show | same |  |  |
| SelAll | not yet |  |  |
| SelNone | not yet |  |  |
| SelCrv | not yet |  |  |
| SelSrf | differs | #679 | misses a sphere |
| SelSolid | differs | #679 | takes a sphere |
| SelPt | not yet |  |  |
| SelName | not yet |  |  |
| SelLayer | not yet |  |  |
| SelDup | same |  |  |
| SelLast | same |  |  |
| SelPrev | same |  |  |
| SelFilter | not yet |  |  |
| SelFilterToggle | not yet |  |  |
| Isolate | same |  |  |
| Invert | same |  |  |
| SelLine | not yet |  |  |
| SelPolyline | not yet |  |  |
| SelCircle | not yet |  |  |
| SelArc | not yet |  |  |
| SelClosedCrv | not yet |  |  |
| SelOpenCrv | not yet |  |  |
| HideSwap | same |  |  |
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
| Lock | same |  |  |
| Unlock | same |  |  |
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
| UnweldEdge | not yet |  |  |

### Files

| Command | State | Issue | Notes |
|---|---|---|---|
| Save | not yet |  |  |
| SaveAs | not yet |  |  |
| Open | not yet |  |  |
| Import | same |  | Rhino-made .3dm (11 kinds of object incl. extrusion, trimmed solid, polycurve, mesh, SubD, text, hatch, dimension): every value matches |
| Export | same |  | .3dm with a box, sphere, circle, polyline, mesh, SubD and a dimension: every value read back in Rhino |
| ExportSelected | not yet |  |  |
| New | not yet |  |  |

