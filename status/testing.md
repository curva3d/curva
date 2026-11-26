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

## Testing stream — 26 November 2026 (afternoon)

- **Mesh fixes rechecked; they match Rhino 8 exactly.** FillMeshHoles gives 161/138/100 (#747, #759). The mesh boolean difference, intersection, union and split give 759/660/540, 216/204/96, 1302/1116/984 and 875 + 125 (#755, #760).
- **First tests on screen** (remote desktop):
  - Top, Front, Right, Perspective, 4View, ZoomExtents, UndoView and RedoView behave as in Rhino.
  - Layouts creates an A3 sheet.
  - Zebra and DraftAnalysis are set aside until the display moves to Bevy.
  - Screenshots are taken with a PowerShell `CopyFromScreen` helper, because the screen-capture tool masks the whole screen behind NVIDIA's overlay.
- **New issues:**
  - #768: Flip, Dir and PointsOn take curves only.
  - #770: Match's Curvature mode, its default, and the picked end. Rhino's rule is in the issue: P2 = P1 + d·N.
  - #771: Purge.
  - #772: SelBlockInstanceNamed wildcards.
  - #773: ClippingPlane options. Rhino's Vertical and 3Point were measured for the features stream.
  - #776: CPlane, Zoom and named-view options.
- **Rhino startup:** a new Rhino that times out at startup is usually waiting on the "Autosave Recovery" dialog. The owner's rule is to press Cancel, and the testing stream's watcher script now does that.

## Testing stream — 26 November 2026 (morning)

Fixes rechecked against Rhino 8 on `main` at fba9278 (headless processor; Rhino measured in a spawned slot).

- **Now as Rhino:** Group on one object (#752), SelName wildcards (#743), RebuildUV (#740), ArrayCrv turning (#738), Smooth 0.2 (#737), RemoveKnot (#736), FilletSrf and ChamferSrf trimming (#732), ExtractIsocurve U only (#730), Wirecut keeping one piece (#729), Rebuild (#716, 21.97809 against 21.97804), SubDCylinder extent (#689). Within 0.1%: Flow (#739) and Sweep2 (#667). Taper about an edge axis works with start and end distances (#713).
- **MatchSrf (#750):** the Rhino figures first filed came from RhinoCommon's `CreateFromMatch`, which pairs the edges crosswise. They were re-measured with the real `-MatchSrf` (Position 158.1139, Tangency 159.2704, Curvature 160.5650), the issue was corrected, and Curva now matches all three. New **#766**: Curva's default mode is Tangency, and Rhino's is Curvature.
- **Sweep1 reopened (#665, #666).** On a line rail, circle to square gives 52.632 against Rhino's 67.903 (4 faces). On the S rail, it gives 103.80 against 138.22. A single section mid-rail sweeps the whole rail, where Rhino stops at the section (63.97). The polyline rail matches (108), but the same L as a degree-1 NURBS curve fails.
- **Fair (#731)** stays open: the default is now 1.0, but the shape differs.
- **BlendCrv (#728, #748):** six cases measured for the features stream with the real `-BlendCrv`. Rhino carries the previous handle across a continuity change, so each case was run twice.
- **Picks without a screen:** edges that a command won't take pre-selected (MatchSrf) can be picked by posting mouse clicks to the Perspective view at `WorldToClient` of the edge.

## Testing stream — 26 November 2026 (early morning)

- **#706 rechecked on main against Rhino 8:** SubDSphere's extent is ±5 (#689). MeshBox has 726 points,
  MeshCylinder 132 and MeshCone 112 / 110 / 90 quads (#690). Mesh Explode gives 6 pieces of 121 / 100
  for the box and 11/10, 11/10, 110/100 for the cylinder. The boolean faces match (#718): difference
  660 / 540, intersection 204 / 96, union 1116 / 984.
- **New issues, mesh:** #755 (the booleans still weld their points: difference 602 against Rhino's 759,
  intersection 152 against 216, union 1052 against 1302); fixes for it and #747 are up (#760, #759).
  On #733: Rhino meshes a polysurface box at the default density to 48 points / 36 triangles; the
  24 / 8 measured earlier was its extrusion box.
- **New issue, features:** #758 SelColor (the object picked at its prompt ends up unselected).
- **Also matching Rhino:** RemoveControlPoint (13.3866), Set/GetDocumentUserText; SelLayer and
  ObjectColour match the exact result. HandleCurve is now pinned in `rhino_measured.rs` (#757).

## Testing stream — 26 November 2026 (night)

- **New issues, features:** #748 BlendSrf (the curvature blend gives 88.4969 against 100.0503 from
  Rhino's real command; position blends match), #750 MatchSrf (moves the whole surface; Rhino moves
  only the matched edge), #751 ExtendSrf (smooth extension 103.5398 against Rhino's 99.1362),
  #752 Group (refuses a single object).
- **New issue, mesh:** #747 FillMeshHoles (the same 138 faces, but 121 vertices against Rhino's 161,
  because Rhino keeps the fill unwelded).
- **Also behaving as expected:** UnifyMeshNormals, RebuildMeshNormals, CullDegenerateMeshFaces,
  Delete / Undo / Redo, Ungroup, Audit.
- **How:** Rhino uses an edge pre-selected with `SelectSubObject`, so BlendSrf and ExtendSrf were
  measured with Rhino's real commands without a screen. RhinoCommon's `CreateBlendSurface` turned
  out to use shorter handles than the BlendSrf command, so commands are the reference.
- **Owner's Rhino:** it was force-closed with the owner's OK after it hung. Its autosave was
  recovered and saved, untouched, in the testing scratch folder.
- **Left (73):** views, layouts and printing, annotation styles, blocks and display analysis, which need
  the desktop (still disconnected); and the recheck of #706 once it merges.

## Testing stream — 25 November 2026 (midday)

- **New issues, features:** #736 RemoveKnot (deletes the nearest control point; Rhino removes the knot
  and refits: 13.3866 against 12.4849), #737 Smooth (factor 0.5 against Rhino's 0.2), #738 ArrayCrv
  (copies only moved; Rhino turns them along the curve), #739 Flow (stretches and replaces; Rhino's
  Stretch=No, Copy=Yes: 14.8797 against 9.4999), #740 RebuildUV (fails on a sphere; acts as Rebuild),
  #741 Reparameterize (no Automatic, curves only), #743 SelName (wildcards select nothing).
- **Also matching Rhino:** Weight (13.496487), HandleCurve (10.8485, same control points), InsertKnot,
  SetPt, ShowEdges.
- **Defaults** (Smooth 0.2, Fair 1.0, Mesh density 50) were confirmed in a freshly started Rhino, so
  they are Rhino's own, not settings left over in the owner's Rhino.
- **Correction to the morning note:** reading Rhino's prompt from a Python thread is not safe. It hung
  Rhino for good four times, including the owner's own Rhino, which is still not responding. It is
  not needed either: when the watchdog cancels a waiting command, the prompt already appears in the
  script's output.

## Testing stream — 25 November 2026 (morning, second pass)

- **Fixed and rechecked against Rhino:** #683 ChamferEdge (26 faces, 945.3333, area 526.6927, as Rhino),
  #709 UnrollSrf (3 pieces, rectangle 314.159 and two discs 78.54), and before that #717, #721 and #722.
- **New issues, features:** #728 BlendCrv (5.3765 against Rhino's 5.1084, and the picked end is
  ignored), #729 Wirecut (KeepAll defaults to Yes; Rhino keeps one piece), #730 ExtractIsocurve
  (copies U and V; Rhino U only), #731 Fair (default tolerance 0.01 changes nothing; Rhino uses 1.0),
  #732 FilletSrf / ChamferSrf (inputs not trimmed; Rhino's Trim=Yes), #734 Split shrinks its pieces'
  surfaces (Untrim then gives 36 where Rhino gives 100).
- **New issue, mesh:** #733 Mesh from NURBS makes only triangles (sphere 2019 / 3968 against Rhino's
  2143 / 2048, mostly quads).
- **Also matching Rhino:** InterpCrvOnSrf (15.8639), AreaCentroid; OffsetCrvOnSrf matches the exact
  result.
- **How:** (see the midday correction) the prompts were read while a scripted command waited, so
  defaults (Fair's `Tolerance <1.00000>`, Mesh's `PolygonDensity=50`) are read without changing any
  setting.
- **Waiting:** #705 (first-preview freeze; needs the desktop) and #706 (SubD extent, unwelded mesh
  primitives; rechecked headlessly as soon as it lands).

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
pending), **differs** (issue filed), **missing**, or **not yet** (not tested). Counts: 27 differs, 2 missing, 19 not yet, 217 same, 61 same*.

### Curves

| Command | State | Issue | Notes |
|---|---|---|---|
| Line | same |  | typed points, BothSides |
| Polyline | same |  | fixed in #668 (start point closes it); checked in the app |
| InterpCrv | same |  | fixed in #701: 22.091092 against 22.091093 |
| Circle | same |  | fixed in #717 |
| Point | same |  |  |
| Divide | same |  | circle into 6 |
| Rebuild | same | #716 | 21.97809 against Rhino 21.97804 |
| PointsOn | differs | #768 | no surface control points (Rhino shows the plane's 4) |
| PointsOff | same |  | turns control points off with no prompt; Rhino's PointsOff likewise |
| InsertKnot | same |  | click at 5,2: shape kept (12.188403), 1 point added, as Rhino's 5 control points |
| RemoveKnot | same | #736 | 12.4849 as Rhino |
| RemoveControlPoint | same |  | point (4,0) removed: 13.3866, as Rhino |
| CloseCrv | same |  | gap closed by a line, 40 |
| SetPt | same |  | all of X, Y, Z set by default: polyline to 0,5,0 as Rhino (no Alignment/Copy options) |
| Offset | same |  | circle (37.699) and rectangle with sharp corners (48) (Rhino-measured in rhino_measured.rs) |
| Fillet | same |  | two lines r=2: 8, 8, 3.1416 (RhinoCommon) |
| FilletCorners | same |  | rectangle r=1, 38.283 (Rhino-measured in rhino_measured.rs) |
| TweenCurves | same |  | 11.1803 |
| Chamfer | same* |  |  |
| Ellipse | same |  |  |
| Arc | same |  | fixed in #717: three 3D points in their plane |
| Rectangle | same |  | fixed in #717 |
| Polygon | same* |  | 6 sides circumscribed: 34.641; Rhino's Edge, Star options missing (reference row) |
| Helix | same |  | length 188.755 against 188.760 |
| Spiral | same |  | 220.170 against 220.176 |
| Points | same |  |  |
| Lines | same |  | fixed in #701: chains, as Rhino |
| Slot | same |  | Curva only: Rhino 8 has no Slot command |
| Parabola | same |  | fixed in #721: from the focus |
| ChangeDegree | same |  | shape kept |
| HandleCurve | same |  | two points with handles: 10.8485, control points 0,0 2,3 8,-3 10,0 as Rhino |
| Sketch | same* |  | click or drag to sketch; Rhino adds Closed/OnSurface/OnMesh options; needs a screen to compare shapes |
| Conic | same |  | 11.4779 in both |
| Hyperbola | same |  | fixed in #721: centre, focus, end, 12.4422 |
| BlendCrv | differs | #728 | 5.3765 against 5.1084; picked end ignored |
| OffsetMultiple | same |  | 37.699 in both |
| Extend | same* |  | reaches the boundary (10); result in 2 pieces, as RhinoCommon with Line style (Rhino command default Natural not checked) |
| Fair | differs | #731 | default tol 0.01 changes nothing; Rhino 1.0 → 12.8702 |
| CurveBoolean | same |  | lens 18.546 in both |
| SimplifyCrv | same |  | fixed in #717 |
| Reparameterize | differs | #741 | no Automatic, no surfaces; end defaults to 1 |
| MakeUniform | same |  | 22.020279 |
| Weight | same |  | point 1 to weight 5: 13.496487, top 4.6947, as Rhino |
| TextObject | same* |  | Curves/Surfaces/Solids/Font like Rhino's CreateGeometry; Rhino run of the output pending |
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
| DupFaceBorder | same* |  | top face: 30 |
| EdgeSrf | same |  | area 102.9399 |
| NetworkSrf | same |  | 109.8230 against 109.8216 |
| Patch | same |  | fixed in #717 for four sides: 102.9399 against 102.9385 |
| ExtendSrf | differs | #751 | smooth by 5: 103.5398 against Rhino 99.1362 (exact circle vs NURBS continuation) |
| Untrim | differs | #734 | after Split gives 36 (Rhino 100: Split keeps the full surface) |
| Project | same |  | 12.566 in both |
| RebuildUV | same | #740 | sphere rebuilt in U, area 314.06 as Rhino |
| ExtractIsocurve | same | #730 | default copies U only (31.4159) as Rhino |
| ShrinkTrimmedSrf | differs | #734 | Rhino leaves a 1% margin (37.4544 on untrim); Curva exact 36 |
| BlendSrf | differs | #748 | default G2: 88.4969 against Rhino command 100.0503; position 70.7107 same |
| MatchSrf | differs | #766 | all three modes match real -MatchSrf (158.1139/159.2704/160.5650); default Tangency where Rhino's is Curvature |
| Sweep2 | same | #667 | sections anywhere match (91.0738, 104.9644); two end arcs 125.7622 against Rhino 125.6062 (0.1%) |
| Pull | same* |  |  |
| InterpCrvOnSrf | same |  | 15.8639 through 3 points on a plane |
| OffsetCrvOnSrf | same* |  | line on plane offset 2: y=7, length 8 |
| UnrollSrf | same |  | fixed in #722: 3 pieces, rectangle 314.159 |
| FilletSrf | same | #732 | fillet 15.708, inputs trimmed to 90/90 as Rhino |
| SplitSrf | same |  | 20 and 30, as Rhino's Split |
| ExtrudeCrv | same |  | circle to a solid, volume 785.398 (Rhino-measured in rhino_measured.rs) |
| PlanarSrf | same |  |  (Rhino-measured in rhino_measured.rs) |
| Revolve | same |  | fixed in #701: "Revolution angle <360>" |
| Loft | same |  | area 256.3047; picks let go afterwards (#680, fixed in #701) |
| Sweep1 | differs | #665 #666 | polyline rail 108 matches; circle to square 52.632 against Rhino 67.903 (4 faces); mid section sweeps the whole rail; degree-1 NURBS rail fails |
| OffsetSrf | same |  | box offset 1: 1728 |

### Solids

| Command | State | Issue | Notes |
|---|---|---|---|
| Volume | same |  | 150 |
| Box | same |  | fixed in #717; all ways match |
| Sphere | same |  |  |
| Cylinder | same |  | preview now curves only, as Rhino's (#671, checked by hand in Rhino) |
| Cone | same |  | preview curves only, as Rhino's (#671) |
| TruncatedCone | same |  |  |
| Torus | same |  | preview curves only (#671) |
| Tube | same |  | area 345.5752, volume 157.0796 |
| Ellipsoid | same |  | fixed in #701: a point for the third axis |
| BooleanSplit | same |  | fixed in #721: two closed 500 solids |
| Paraboloid | same |  | fixed in #721: from the focus |
| Pyramid | same |  | 198.1368 in both |
| MergeAllFaces | same* |  |  |
| Wirecut | same | #729 | keeps one piece (500) as Rhino |
| EditBoolean | same* |  | Curva-only (live booleans); no Rhino 8 command to compare |
| BreakBoolean | same* |  | Curva-only (live booleans); no Rhino 8 command to compare |
| MoveFace | same* |  | top up 2: 250 |
| PushPull | same* |  | alias of MoveFace; Rhino 8 PushPull asks for a surface |
| Cap | same |  | 785.398 in both |
| ExtrudeFace | same* |  | top 2: 250 |
| ChamferSrf | same | #732 | chamfer 14.1421, inputs trimmed to 90/90 as Rhino |
| Pipe | same |  | line, radius 1, capped (volume 31.416); Rhino's end radius step not asked (Rhino-measured in rhino_measured.rs) |
| FilletEdge | same |  | all 12 box edges r=1: volume 975.587, 26 faces; picking edges in the app works |
| ChamferEdge | same | #683 | fixed in #724: 26 faces, 945.3333, area 526.6927 |
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
| ArrayCrv | same | #738 | copies turn with the curve; last copy (-1,0,0)-(0,1,1) as Rhino |
| Orient | same |  |  |
| Smooth | same | #737 | default 0.2: 12.0950 as Rhino |
| History | same* |  | toggles history for mirrors and offsets; Rhino's History is a settings prompt (Record=No Update=Yes Lock=No) |
| Flip | differs | #768 | curves only; on a plane it says Flipped 1 curves and changes nothing (Rhino reverses the face) |
| ProjectToCPlane | same |  |  |
| Join | same |  |  (Rhino-measured in rhino_measured.rs) |
| Explode | same |  |  (Rhino-measured in rhino_measured.rs) |
| Split | same |  | fixed in #717: surfaces too |
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
| Taper | same | #713 | edge axis, start/end distances; 583.33 exact frustum against Rhino's space morph 592.86 |
| Flow | same | #739 | Copy=Yes Stretch=No; bounds match; volume 9.489 against Rhino 9.4999 (0.1%) |
| CageEdit | same* |  | prompt for captive objects as Rhino; cage steps need a screen |
| Delete | same* |  | deleted object comes back with Undo |
| Undo | same* |  | undoes delete, then the sphere; Redo redoes |
| Redo | same* |  | redoes the delete |

### Analysis

| Command | State | Issue | Notes |
|---|---|---|---|
| Dir | differs | #768 | curves only; Rhino shows surface normals with UReverse VReverse SwapUV Flip |
| BoundingBox | differs | #688 | "-inf" for solids, meshes and SubDs; no box made |
| Distance | same |  | fixed in #717: angles and deltas |
| Length | same |  | 31.416 |
| Area | same |  | fixed in #701: 190, 314.159, 50 |
| Angle | same |  | fixed in #717: four points, 45° |
| EvaluatePt | same |  |  |
| Radius | same |  | click on a circle: 5.000 |
| What | differs | #712 | reports little beside Rhino's What |
| AreaCentroid | same |  | point at 5,2,0 for a 10x4 plane |
| VolumeCentroid | same |  | 5, 2.5, 1.5 in both |
| ShowEdges | same |  | box 12 edges 0 naked; holed plane 5 edges 5 naked, as Rhino |
| Plan | same* |  | no prompt, as Rhino |
| Zebra | missing |  | set aside until the display moves to Bevy (Curva's message) |
| Count | same* |  |  |
| Audit | same* |  | "No problems found" on a box |
| DraftAnalysis | missing |  | set aside until the display moves to Bevy (Curva's message) |
| Section | same |  | 16 in both |
| Contour | same |  | four rectangles of 30, ends included, in both |
| Intersect | same |  | 5,5 in both |
| CurvatureGraph | same* |  | Scale 10 / Density 60 options; Rhino shows its graph panel; display needs a screen |
| Silhouette | same |  | fixed in #721: exact circle 31.4159 |

### Mesh

| Command | State | Issue | Notes |
|---|---|---|---|
| Mesh | differs | #733 | all triangles: sphere 2019/3968 against Rhino 2143/2048 (1920 quads) |
| MeshBox | same | #690 | fixed in #706: 726 points, 600 faces |
| MeshPlane | same |  |  |
| MeshSphere | same |  |  |
| MeshCylinder | same | #690 | fixed in #706: 132 points, 120 faces |
| MeshCone | same | #690 | fixed in #706: 112 points, 110 faces, 90 quads |
| MeshTorus | same |  | 100/100/100, 690.983 |
| Check | same* |  | "good mesh" report |
| MeshRepair | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| UnifyMeshNormals | same* |  | sphere mesh: 0 faces turned |
| FillMeshHoles | same | #747 | 161/138/100 as Rhino |
| CullDegenerateMeshFaces | same* |  | sphere mesh: 0 faces taken out |
| RebuildMeshNormals | same* |  | sphere mesh: normals rebuilt, no change |
| ReduceMesh | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |
| TriangulateMesh | same |  | 121/200/0 in both |
| QuadrangulateMesh | same |  | measured in Rhino 8 by the mesh stream (defaults 5° and 2) |
| ExtractMeshFaces | not yet |  |  |
| MeshToNURB | same |  | one face per mesh face, as Rhino |
| MeshBooleanUnion | same | #755 | 1302/1116/984, volume 1875, as Rhino |
| MeshBooleanDifference | same | #755 | 759/660/540, volume 875, as Rhino |
| MeshBooleanIntersection | same | #755 | 216/204/96, volume 125, as Rhino |
| MeshBooleanSplit | same | #755 | 875 + 125 pieces with Rhino's points |
| QuadRemesh | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |

### SubD

| Command | State | Issue | Notes |
|---|---|---|---|
| SubDBox | same |  | cage 26/24 |
| SubDPlane | same |  | cage 9/4 |
| SubDSphere | same | #689 | fixed in #706: extent ±5 |
| SubDCylinder | same | #689 | extent from the smooth surface (-3,-3,0)-(3,3,10) |
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
| SetCurrentAnnotationStyle | same |  | asks the style name, as Rhino's -SetCurrentAnnotationStyle |
| DupAnnotationStyle | same* |  | asks the style to copy, as Rhino (Rhino offers the current style as default) |
| Styles | not yet |  |  |

### Layouts

| Command | State | Issue | Notes |
|---|---|---|---|
| PrintWidth | not yet |  |  |
| SetLayoutUserText | not yet |  |  |
| Layout | differs | #687 | no detail and no name |
| CopyLayout | same* |  | asks for an open layout first; copying on screen pending |
| Layouts | same |  | panel lists layouts; New layout asks paper size (Enter = A3) and opens the sheet (on screen) |
| Detail | same |  | two clicks add a Top detail (rounded scale) |
| Print | not yet |  |  |
| ExportPDF | not yet |  | needs the app (printing draws through the viewport) |
| Make2D | same |  | box from the top: 4 lines, 30 |
| TitleBlock | same* |  | asks for an open layout first; placing on screen pending |
| ScaleBar | not yet |  |  |
| SheetIndex | not yet |  |  |
| Revision | not yet |  |  |
| ClippingPlane | differs | #773 | only FlipDirection; Rhino has 3Point Vertical Center AroundCurve CustomDepth |
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
| SelName | same | #743 | exact name and al* wildcard both select as Rhino |
| SelLayer | same* |  | Default selects both objects on it |
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
| SelColor | differs | #758 | object picked at the prompt ends up unselected |
| SelBlockInstanceNamed | differs | #772 | exact name selects all 3 instances; wildcard T* selects nothing (Rhino matches) |

### Views

| Command | State | Issue | Notes |
|---|---|---|---|
| ZoomSelected | not yet |  |  |
| EnableClippingPlane | not yet |  |  |
| DisableClippingPlane | not yet |  |  |
| ZoomExtents | same |  | zooms the active viewport, as Rhino (on screen) |
| 4View | same |  | four viewports Top/Perspective/Front/Right (on screen) |
| UndoView | same |  | restores the previous view (on screen) |
| RedoView | same |  | redoes the view change (on screen) |
| Top | same |  | sets the active viewport's view, as Rhino (checked on screen) |
| Front | same |  | sets the active viewport's view, as Rhino (checked on screen) |
| Right | same |  | sets the active viewport's view, as Rhino (checked on screen) |
| Perspective | same |  | sets the active viewport's view, as Rhino (checked on screen) |
| CPlane | differs | #776 | works; fewer options than Rhino 8 |
| Zoom | differs | #776 | works; fewer options than Rhino 8 |
| NamedView | differs | #776 | works; fewer options than Rhino 8 |
| NamedCPlane | differs | #776 | works; fewer options than Rhino 8 |
| SetDisplayMode | differs | #776 | works; fewer options than Rhino 8 |

### Organising

| Command | State | Issue | Notes |
|---|---|---|---|
| SelPolysrf | same |  | added in #701 |
| SelClosedSrf | same |  | added in #701 |
| SelOpenSrf | same |  | added in #701 |
| Rename | same* |  | names the selected objects (Rhino names objects in Properties) |
| Match | differs | #770 | Position 6.5410 and Tangency 6.7321 match; Curvature 7.3323 against 6.8420; default Tangency (Rhino Curvature); picked end ignored |
| Lock | same |  |  |
| Unlock | same |  |  |
| Group | same | #752 | groups a single object, as Rhino (fba9278) |
| Ungroup | same* |  | breaks up the group |
| Block | same* |  | objects become an instance; base point, then name |
| Insert | same* |  | name, point, scale, rotation, as Rhino's -Insert |
| BlockManager | same* |  | opens the manager panel; Rhino's -BlockManager options list (Properties Export Delete Update ...) |
| BlockEdit | same* |  | picks an instance; Rhino's -BlockEdit offers Open/SaveAndClose/DiscardAndCancel |
| ExplodeBlock | same* |  | instance at 10,0 explodes to box 10..11 and circle at 13,0 |
| ReplaceBlock | same* |  | selects instances then the new block, as Rhino |
| ChangeLayer | same* |  | to an existing layer, typed |
| MatchProperties | same* |  | objects to change, then the source, as Rhino's MatchProperties (Select objects to modify) |
| Linetype | same* |  | Curva prompt picks objects; Rhino 8 has no -Linetype command (Linetypes panel / properties) |
| ObjectColour | same* |  | 255,0,0 sets the colour; SelColor finds it |
| SetUserText | same |  | key, then value |
| GetUserText | same |  | lists every key |
| SetDocumentUserText | same |  | Client = ACME stored, as Rhino |
| GetDocumentUserText | same |  | reads ACME back, as Rhino |
| Notes | same* |  | sets and shows document notes; prompt differs from Rhino's -Notes (Clear Edit Import List Visible) |
| Purge | differs | #771 | layers and blocks only, no options; Rhino purges 10 kinds of unused data |
| LockSwap | same |  |  |
| UnlockSelected | differs | #715 | unlocks without asking which |
| BringToFront | same* |  | takes objects as Rhino; draw order only visible on screen |
| SendToBack | same* |  | takes objects as Rhino; draw order only visible on screen |
| BringForward | same* |  | takes objects as Rhino; draw order only visible on screen |
| SendBackward | same* |  | takes objects as Rhino; draw order only visible on screen |
| UnweldEdge | same |  | measured in Rhino 8 by the mesh stream (status/mesh.md) |

### Files

| Command | State | Issue | Notes |
|---|---|---|---|
| Save | same* |  | round trip exact (solids, curves, mesh, SubD, dimension) |
| SaveAs | same* |  |  |
| Open | same* |  | .curva round trip exact; .3dm import exact |
| Import | same |  | Rhino-made .3dm (11 kinds of object incl. extrusion, trimmed solid, polycurve, mesh, SubD, text, hatch, dimension): every value matches |
| Export | same |  | .3dm with a box, sphere, circle, polyline, mesh, SubD and a dimension: every value read back in Rhino |
| ExportSelected | same* |  | no Rhino 8 command of this name (Rhino's Export exports the selection) |
| New | same* |  | asks to discard; Rhino asks to save first |

