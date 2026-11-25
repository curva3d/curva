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

## Testing stream — 25 November 2026 (after midnight)

- **More issues:** #715 (UnlockSelected unlocks without asking), #716 (Rebuild fits a different curve:
  22.007 against Rhino's 21.978; Rhino's control points posted), both features; #718 (mesh booleans
  make every face a triangle; Rhino keeps the quads away from the cut), mesh.
- **Turned from same\* into same** by running them in Rhino: ProjectToCPlane, Orient3Pt, VolumeCentroid,
  Intersect, CurveBoolean, Distribute, Cap, ExtractSrf, DupEdge, Project, TriangulateMesh, MeshToNURB,
  DimRadius, DimDiameter, SelAll, SelNone, SelCrv, SelPt, SelOpenCrv and the curve extrusions.
- **Also matching Rhino:** ToSubD, ToNURBS, MakeUniform, SetUserText and GetUserText, LockSwap,
  SelPolyline, SelLine, SelClosedCrv, DimRotated; `.curva` save and open is exact for every kind of
  object; the SubD editing commands and several mesh commands are marked same from the mesh stream's
  own Rhino measurements.
- **Note:** measuring Rebuild set Rhino's `-Rebuild` to PointCount=10, Degree=3, which this Rhino
  remembers (the owner's Rhino settings).
- **Left (114):** mostly views, printing, panels and picks of parts (edges, knots, points), which need
  the desktop; and the rechecks of #700, #705 and #706 once merged.

## Testing stream — 25 November 2026 (late night)

- **Testing no longer stops on dialogs or a missing screen** (the owner's request). With the remote
  desktop disconnected:
  - Rhino's dialogs are answered by `E:\curva-scratch\testing\rhino-dialogs.ps1` through UI Automation
    (Autosave Recovery: OK, which keeps the autosave; Update Available: No); it runs in the background
    while Rhino slots start. A stuck Rhino of the stream's own is typed into with `rhino-type.ps1`.
  - Curva is driven by a test build with egui's accessibility turned on (never committed): its controls
    are listed and pressed through UI Automation (`curva-ui.ps1`). Pointer-level checks (where a click
    lands, double-clicks, held keys) still need the desktop.
- **#701 rechecked** (features): InterpCrv, Lines, Area, Revolve's prompt, SelSrf/SelSolid and the new
  SelPolysrf/SelClosedSrf/SelOpenSrf, picks let go, Ellipsoid's third axis: all match Rhino now.
- **#698 narrowed:** New layer works when pressed through accessibility, so the panel's logic is fine and
  mouse clicks are caught by something above it; the layer rows' controls also have no accessible names.
- **New issues (features):** #707 (Parabola, Hyperbola, Paraboloid start from the focus in Rhino), #708
  (SimplifyCrv), #709 (UnrollSrf: polygon outline, caps dropped), #710 (Split takes only curves), #711
  (BooleanSplit gives one open-sided body), #712 (Silhouette is a polyline; What says little), #713
  (Taper fails on an edge axis; ratio instead of distances).
- **For the mesh stream:** Rhino's Explode splits a MeshBox into 6 sides and a MeshCylinder into 3
  (#690); Rhino's MeshCone is 112 vertices at its defaults.
- **Matching Rhino, measured in Rhino:** Spiral, Conic, CloseCrv, TweenCurves, ChangeDegree, tapered
  extrusion (5° default), Pyramid, Section, Contour, DupBorder, SplitSrf, BooleanUnion of touching
  boxes, SubDPlane, SubDCone, SubDTorus, MeshTorus, Make2D from the top, Twist, Bend.

## Testing stream — 25 November 2026 (night)

- **Blocked on the machine:** the remote desktop session disconnected around 3 pm, so there is no
  screen for computer use, and Rhino closed. A new Rhino is waiting on its "Autosave Recovery" dialog,
  which only someone at the screen can answer (Rhino's autosave folder also holds the owner's real
  project autosaves, so it is left alone). Rhino comparisons resume when the session is back.
- **Meanwhile, Curva checked against exact geometry (same\*):** tapered, to-point and along-curve
  extrusions, ExtractSrf, DupBorder, DupEdge, Pyramid, ProjectToCPlane, Orient3Pt, VolumeCentroid,
  Section, Contour, Intersect, Count, CurveBoolean, Distribute, Align, TriangulateMesh, MeshToNURB,
  Block and Insert steps, the selection filters, and radius, diameter, arc length, area and volume
  dimensions.
- **Tests pinned** (#702): Tube, Ellipsoid, Helix, SrfPt, Rotate3D, Orient and ScaleNU as Rhino 8
  measured them.
- **Timing noise confirmed:** Pyramid and Paraboloid once took 6–7 s; rerun twice, 11 ms and 4 ms. Single
  slow runs on this machine come from other sessions' builds, not Curva.
- For the features stream (#669): Rhino's InterpCrv data posted (chord-length knots, unit end
  derivatives).

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
- **#699 (display):** in a freshly started app the first solid preview froze Curva for over 5 s (Windows:
  "not responding"; keys typed meanwhile were lost), though building it takes 4 ms; probably the first
  drawing of see-through preview faces. A second box had no pause.
- **Selection in the app:** window (left to right) and crossing (right to left) match Rhino, with its
  messages; Ctrl-click removes. Shift-click acted as a plain click twice (added to #686).
- **Undo and Redo** match Rhino (a Move, and a whole Array in one step).
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
pending), **differs** (issue filed), **missing**, or **not yet** (not tested). Counts: 33 differs, 114 not yet, 155 same, 24 same*.

### Curves

| Command | State | Issue | Notes |
|---|---|---|---|
| Line | same |  | typed points, BothSides |
| Polyline | same |  | fixed in #668 (start point closes it); checked in the app |
| InterpCrv | same |  | fixed in #701: 22.091092 against 22.091093 |
| Circle | differs | #695 | 3Point with 3D points is flattened |
| Point | same |  |  |
| Divide | same |  | circle into 6 |
| Rebuild | differs | #716 | 22.007 against 21.978 |
| PointsOn | not yet |  |  |
| PointsOff | not yet |  |  |
| InsertKnot | not yet |  |  |
| RemoveKnot | not yet |  |  |
| RemoveControlPoint | not yet |  |  |
| CloseCrv | same |  | gap closed by a line, 40 |
| SetPt | not yet |  |  |
| Offset | same |  | circle (37.699) and rectangle with sharp corners (48) (Rhino-measured in rhino_measured.rs) |
| Fillet | same |  | two lines r=2: 8, 8, 3.1416 (RhinoCommon) |
| FilletCorners | same |  | rectangle r=1, 38.283 (Rhino-measured in rhino_measured.rs) |
| TweenCurves | same |  | 11.1803 |
| Chamfer | same* |  |  |
| Ellipse | same |  |  |
| Arc | differs | #695 | three typed 3D points are flattened onto the construction plane |
| Rectangle | differs | #695 | 3Point upright is refused |
| Polygon | same* |  | 6 sides circumscribed: 34.641; Rhino's Edge, Star options missing (reference row) |
| Helix | same |  | length 188.755 against 188.760 |
| Spiral | same |  | 220.170 against 220.176 |
| Points | same |  |  |
| Lines | same |  | fixed in #701: chains, as Rhino |
| Slot | same |  | Curva only: Rhino 8 has no Slot command |
| Parabola | differs | #707 | Rhino starts from the focus; Curva's default is Rhino's Vertex option |
| ChangeDegree | same |  | shape kept |
| HandleCurve | not yet |  |  |
| Sketch | not yet |  |  |
| Conic | same |  | 11.4779 in both |
| Hyperbola | differs | #707 | Rhino: centre, focus, end; Curva: centre, vertex, focus distance, end |
| BlendCrv | not yet |  |  |
| OffsetMultiple | not yet |  |  |
| Extend | not yet |  |  |
| Fair | not yet |  |  |
| CurveBoolean | same |  | lens 18.546 in both |
| SimplifyCrv | differs | #708 | collinear segments not merged |
| Reparameterize | not yet |  |  |
| MakeUniform | same |  | 22.020279 |
| Weight | not yet |  |  |
| TextObject | not yet |  |  |
| Curve | same |  | control points, degree 3 |

### Surfaces

| Command | State | Issue | Notes |
|---|---|---|---|
| Plane | same |  |  |
| SrfPt | same |  | 102.9399 |
| ExtrudeSrf | same |  | solid by default (Rhino's factory default) |
| DupEdge | same |  | 12 edges, 72 in both |
| DupBorder | same |  | 30 |
| ExtrudeCrvTapered | same |  | 5° draft by default in both (top radius 4.1251) |
| ExtrudeCrvAlongCrv | same |  | side 314.159 in both |
| ExtrudeCrvToPoint | same |  | side 175.62 in both |
| RailRevolve | same |  | 314.129 against 314.159 |
| ExtractSrf | same |  | 50 in both |
| DupFaceBorder | not yet |  |  |
| EdgeSrf | same |  | area 102.9399 |
| NetworkSrf | same |  | 109.8230 against 109.8216 |
| Patch | differs | #684 | 4 faces against Rhino's 1 |
| ExtendSrf | not yet |  |  |
| Untrim | not yet |  |  |
| Project | same |  | 12.566 in both |
| RebuildUV | not yet |  |  |
| ExtractIsocurve | not yet |  |  |
| ShrinkTrimmedSrf | not yet |  |  |
| BlendSrf | not yet |  |  |
| MatchSrf | not yet |  |  |
| Sweep2 | differs | #667 | one section at the start matches; two sections impossible, section mid-rail fails |
| Pull | same* |  |  |
| InterpCrvOnSrf | not yet |  |  |
| OffsetCrvOnSrf | not yet |  |  |
| UnrollSrf | differs | #709 | 194-sided outline; a solid's caps left out |
| FilletSrf | not yet |  |  |
| SplitSrf | same |  | 20 and 30, as Rhino's Split |
| ExtrudeCrv | same |  | circle to a solid, volume 785.398 (Rhino-measured in rhino_measured.rs) |
| PlanarSrf | same |  |  (Rhino-measured in rhino_measured.rs) |
| Revolve | same |  | fixed in #701: "Revolution angle <360>" |
| Loft | same |  | area 256.3047; picks let go afterwards (#680, fixed in #701) |
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
| Ellipsoid | same |  | fixed in #701: a point for the third axis |
| BooleanSplit | differs | #711 | one 11-face body and a loose face, not two closed solids |
| Paraboloid | differs | #707 | Rhino starts from the focus |
| Pyramid | same |  | 198.1368 in both |
| MergeAllFaces | same* |  |  |
| Wirecut | not yet |  |  |
| EditBoolean | not yet |  |  |
| BreakBoolean | not yet |  |  |
| MoveFace | same* |  | top up 2: 250 |
| PushPull | not yet |  |  |
| Cap | same |  | 785.398 in both |
| ExtrudeFace | same* |  | top 2: 250 |
| ChamferSrf | not yet |  |  |
| Pipe | same |  | line, radius 1, capped (volume 31.416); Rhino's end radius step not asked (Rhino-measured in rhino_measured.rs) |
| FilletEdge | same |  | all 12 box edges r=1: volume 975.587, 26 faces; picking edges in the app works |
| ChamferEdge | differs | #683 | corners: 18 faces, 946 against 26, 945.333 |
| Shell | same |  | open top t=1: 424, 11 faces |
| BooleanUnion | same |  | touching boxes merge to 6 faces, 2000 (Rhino-measured) |
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
| ProjectToCPlane | same |  |  |
| Join | same |  |  (Rhino-measured in rhino_measured.rs) |
| Explode | same |  |  (Rhino-measured in rhino_measured.rs) |
| Split | differs | #710 | curves only; Rhino's Split takes surfaces too |
| Trim | same* |  |  |
| Orient3Pt | same |  |  |
| Rotate3D | same |  |  |
| Scale1D | same |  |  |
| Scale2D | same |  |  |
| ScaleNU | same |  |  |
| ArrayLinear | same |  | (Rhino-measured in rhino_measured.rs) |
| Align | same* |  | Rhino's options and steps |
| Distribute | same |  | middle box to x 5 in both (Rhino: XAxis, Mode=Gap) |
| Shear | same |  |  |
| Twist | same |  | 997.15 against 999.997 |
| Bend | same |  | Rhino's steps |
| Taper | differs | #713 | fails with the axis on an edge; one ratio where Rhino asks start and end distances |
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
| Area | same |  | fixed in #701: 190, 314.159, 50 |
| Angle | differs | #694 | Curva takes 3 points (corner and sides), Rhino 4 (two lines) |
| EvaluatePt | same |  |  |
| Radius | same |  | click on a circle: 5.000 |
| What | differs | #712 | reports little beside Rhino's What |
| AreaCentroid | not yet |  |  |
| VolumeCentroid | same |  | 5, 2.5, 1.5 in both |
| ShowEdges | not yet |  |  |
| Plan | not yet |  |  |
| Zebra | not yet |  |  |
| Count | same* |  |  |
| Audit | not yet |  |  |
| DraftAnalysis | not yet |  |  |
| Section | same |  | 16 in both |
| Contour | same |  | four rectangles of 30, ends included, in both |
| Intersect | same |  | 5,5 in both |
| CurvatureGraph | not yet |  |  |
| Silhouette | differs | #712 | 64-segment polyline; Rhino's is exact |

### Mesh

| Command | State | Issue | Notes |
|---|---|---|---|
| Mesh | not yet |  |  |
| MeshBox | differs | #690 | edges welded (602 against 726); fix in #706 |
| MeshPlane | same |  |  |
| MeshSphere | same |  |  |
| MeshCylinder | differs | #690 | caps welded (112 against 132); fix in #706 |
| MeshCone | differs | #690 | 102 vertices against 112 (welding) |
| MeshTorus | same |  | 100/100/100, 690.983 |
| Check | same* |  | "good mesh" report |
| MeshRepair | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| UnifyMeshNormals | not yet |  |  |
| FillMeshHoles | not yet |  |  |
| CullDegenerateMeshFaces | not yet |  |  |
| RebuildMeshNormals | not yet |  |  |
| ReduceMesh | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| TriangulateMesh | same |  | 121/200/0 in both |
| QuadrangulateMesh | same |  | measured in Rhino 8 by the mesh stream (defaults 5° and 2) |
| ExtractMeshFaces | not yet |  |  |
| MeshToNURB | same |  | one face per mesh face, as Rhino |
| MeshBooleanUnion | differs | #718 | volume exact; all triangles (as the others) |
| MeshBooleanDifference | differs | #718 | volume exact; all triangles where Rhino keeps quads |
| MeshBooleanIntersection | differs | #718 | volume exact; all triangles |
| MeshBooleanSplit | differs | #718 | volumes exact; all triangles |
| QuadRemesh | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |

### SubD

| Command | State | Issue | Notes |
|---|---|---|---|
| SubDBox | same |  | cage 26/24 |
| SubDPlane | same |  | cage 9/4 |
| SubDSphere | differs | #689 | surface matches; extent is the cage's |
| SubDCylinder | differs | #689 | surface matches; extent is the cage's |
| SubDCone | same |  | cage 34/40; surface within 0.1 % |
| SubDTorus | same |  | cage 60/60; surface within 0.1 % |
| ToSubD | same |  | MeshBox: 602/600 in both |
| SubDDisplayToggle | not yet |  |  |
| ToNURBS | same |  | SubDBox: 6 faces, 786.865 against 786.855 |
| Crease | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| RemoveCrease | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| Fill | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| WeldEdge | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| Stitch | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| Bridge | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| InsertEdge | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| MergeFaces | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| InsertPoint | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| OffsetSubD | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| Bevel | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| SubDLoft | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| MultiPipe | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |

### Annotation

| Command | State | Issue | Notes |
|---|---|---|---|
| Text | same |  | text, then where it goes, as Rhino |
| Dim | same |  | 10.00 |
| DimAligned | same |  | 10.44 |
| DimRotated | same |  | angle first, then points: 8.660 in both |
| DimRadius | same |  | 5 in both |
| DimDiameter | same |  | 10 in both |
| DimAngle | same* |  | first line, or Points, as Rhino |
| DimOrdinate | same* |  | Rhino's steps and options |
| DimCurveLength | same* |  | 31.42 |
| DimArea | same* |  | 50 |
| DimVolume | same* |  | 150 |
| DimArcLength | same* |  | 7.854 |
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
| ExportPDF | not yet |  | needs the app (printing draws through the viewport) |
| Make2D | same |  | box from the top: 4 lines, 30 |
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
| SelAll | same |  |  |
| SelNone | same |  |  |
| SelCrv | same |  | curves only, as Rhino |
| SelSrf | same |  | fixed in #701 |
| SelSolid | same |  | fixed in #701 |
| SelPt | same |  |  |
| SelName | not yet |  |  |
| SelLayer | not yet |  |  |
| SelDup | same |  |  |
| SelLast | same |  |  |
| SelPrev | same |  |  |
| SelFilter | not yet |  |  |
| SelFilterToggle | not yet |  |  |
| Isolate | same |  |  |
| Invert | same |  |  |
| SelLine | same |  |  |
| SelPolyline | same |  | rectangle and polyline, as Rhino |
| SelCircle | same |  | Curva only: not a Rhino 8 command |
| SelArc | same |  | Curva only: not a Rhino 8 command |
| SelClosedCrv | same |  | rectangle and circle, as Rhino |
| SelOpenCrv | same |  |  |
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
| SelPolysrf | same |  | added in #701 |
| SelClosedSrf | same |  | added in #701 |
| SelOpenSrf | same |  | added in #701 |
| Rename | same* |  | names the selected objects (Rhino names objects in Properties) |
| Match | not yet |  |  |
| Lock | same |  |  |
| Unlock | same |  |  |
| Group | not yet |  |  |
| Ungroup | not yet |  |  |
| Block | same* |  | objects become an instance; base point, then name |
| Insert | same* |  | name, point, scale, rotation, as Rhino's -Insert |
| BlockManager | not yet |  |  |
| BlockEdit | not yet |  |  |
| ExplodeBlock | not yet |  |  |
| ReplaceBlock | not yet |  |  |
| ChangeLayer | same* |  | to an existing layer, typed |
| MatchProperties | not yet |  |  |
| Linetype | not yet |  |  |
| ObjectColour | not yet |  |  |
| SetUserText | same |  | key, then value |
| GetUserText | same |  | lists every key |
| SetDocumentUserText | not yet |  |  |
| GetDocumentUserText | not yet |  |  |
| Notes | not yet |  |  |
| Purge | not yet |  |  |
| LockSwap | same |  |  |
| UnlockSelected | differs | #715 | unlocks without asking which |
| BringToFront | not yet |  |  |
| SendToBack | not yet |  |  |
| BringForward | not yet |  |  |
| SendBackward | not yet |  |  |
| UnweldEdge | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |

### Files

| Command | State | Issue | Notes |
|---|---|---|---|
| Save | same* |  | round trip exact (solids, curves, mesh, SubD, dimension) |
| SaveAs | same* |  |  |
| Open | same* |  | .curva round trip exact; .3dm import exact |
| Import | same |  | Rhino-made .3dm (11 kinds of object incl. extrusion, trimmed solid, polycurve, mesh, SubD, text, hatch, dimension): every value matches |
| Export | same |  | .3dm with a box, sphere, circle, polyline, mesh, SubD and a dimension: every value read back in Rhino |
| ExportSelected | not yet |  |  |
| New | same* |  | asks to discard; Rhino asks to save first |

