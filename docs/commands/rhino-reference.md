# Rhino reference for Curva's commands

The reference every command is checked against (spec `docs/specs/command-framework.md` §5): for each command,
Rhino's steps in order, Rhino's options, which of them Curva has, and its status.

**How to read and keep it:**
- One row per command. Options are written in backticks, exactly as Rhino names them.
- **Status** is `done` (on the framework, with every Rhino step and option), `partly`, or `missing` (not in
  Curva yet).
- A test (`the_rhino_reference_matches_the_commands` in `crates/curva-commands/src/commands/mod.rs`) reads this
  table:
  - every Curva command must have a row
  - for every command written on the framework, each option under **In Curva** must exist, and every option it
    has must be listed there
  - a `done` row must have every option under **Rhino's options**, and only commands on the framework can be
    `done`

  Change the table and the code together.
- Rhino's behaviour is the source, never Curva's code. These rows were written on 12 November 2026 from the
  Rhino 8 help pages (`docs.mcneel.com/rhino/8/help/en-us/commands/`). Where a page could not be read, the row
  says so, and should be completed from Rhino itself.
- Options that only matter for objects Curva does not have yet (SubD, meshes, extrusion objects, blocks) are
  left out.

## Curves

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Line | Start of line; End of line (typed distance, angle, or both) | `BothSides` `Normal` `Angled` `Vertical` `FourPoint` `Bisector` `Perpendicular` `Tangent` `Extension` | `BothSides` | partly |
| Polyline | Start; next points; Enter ends | `Close` `Helpers` `Length` `Mode` `Direction` `ThroughPoint` `Center` `PersistentClose` `Undo` | `Close` `Undo` | partly |
| Curve | Start; next control points; Enter ends | `AutoClose` `Degree` `PersistentClose` `Close` `Sharp` `Undo` | `Degree` `Close` `Undo` | partly |
| InterpCrv | Start; next points; Enter ends (on Rhino's Curve page) | `AutoClose` `Degree` `PersistentClose` `Knots` `StartTangent` `EndTangent` `Close` `Sharp` `Undo` | `Close` `Undo` | partly |
| Circle | Center; radius, typed or picked | `Deformable` `2Point` `3Point` `Radius` `Diameter` `AroundCurve` `FitPoints` `Tangent` `Vertical` `Orientation` `Circumference` `Area` | `2Point` `3Point` `Diameter` | partly |
| Arc | Center; start (radius); end point or angle (`Length`). With `StartPoint`: start, end, point on arc | `Center` `Tilted` `Length` `Midpoint` `Deformable` `StartPoint` `Direction` `ThroughPoint` `Tangent` `Extension` | `StartPoint` | partly |
| Ellipse | Center; end of first axis; end of second axis | `Deformable` `Vertical` `Corner` `Diameter` `FromFoci` `MarkFoci` `AroundCurve` | `Diameter` `Corner` | partly |
| Rectangle | Two opposite corners | `3Point` `EdgeMidpoint` `Vertical` `Center` `AroundCurve` `Rounded` | `Center` `3Point` | partly |
| Point | Pick a location | none | none | done |
| Divide | Select curves; number of segments, or `Length` / `EqualChordLength` and a number | `Length` `EqualChordLength` `Split` `DeleteRemainder` `MarkEnds` `GroupOutput` | none | partly |
| CloseCrv | Select open curves; Enter | `CloseWideGapsWithLine` `Tolerance` | none | partly |
| TweenCurves | First curve; second curve; click an end to flip | `Number` `OutputLayer` `MatchMethod` `SampleNumber` `Flip` | none | partly |
| Offset | Select a curve; click the side | `Distance` `Loose` `Corner` `Trim` `ThroughPoint` `Tolerance` `BothSides` `InCPlane` `Cap` `OutputLayer` | `Distance` `ThroughPoint` `BothSides` `Cap` | partly |
| Fillet | First curve near its end; second curve near its end | `Radius` `Join` `Trim` `ExtendArcsBy` `ExtendOtherCurvesBy` `Dynamic` | `Radius` | partly |
| FilletCorners | Select polycurves; Enter; radius typed or by two points (on Rhino's Fillet page) | none | none | partly |
| Chamfer | First curve near its end; second curve near its end | `Distances` `Join` `Trim` `ExtendArcsBy` `ExtendOtherCurvesBy` | `Distances` | partly |
| Rebuild | Select curves; point count and degree (a dialog in Rhino; `-Rebuild` on the command line) | `PointCount` `Degree` `DeleteInput` `CurrentLayer` `PreserveTangents` | none | partly |

## Transforms

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Move | Select objects; point to move from (Enter: bounding box centre); point to move to (a typed distance constrains) | `Normal` `Vertical` | `Vertical` | partly |
| Copy | Select objects; point to copy from; points to copy to, once per copy; Enter ends | `Vertical` `InPlace` `FromLastPoint` `UseLastDistance` `UseLastDirection` | `Vertical` `InPlace` | partly |
| Rotate | Select objects; centre; angle typed or two reference points | `UseLastCenter` `Copy` | `Copy` | partly |
| Rotate3D | Select objects; axis start; axis end; angle typed or two reference points | `SurfaceNormal` `UseLastAxis` `Copy` | none | partly |
| Scale | Select objects; base point (Enter: bounding box centre); factor typed or two reference points | `Copy` `Rigid` | `Copy` | partly |
| Scale1D | Select objects; base point; factor and direction, or two reference points (0 flattens) | `Copy` `Rigid` | none | partly |
| Scale2D | Select objects; base point; factor or two reference points, in the construction plane | `Copy` `Rigid` | none | partly |
| ScaleNU | Select objects; origin; X, Y, Z factors in turn (Enter is 1) | `Copy` `Rigid` `WorldCoordinates` | `Copy` | partly |
| Mirror | Select objects; start of mirror plane; end of mirror plane | `3Point` `Copy` `XAxis` `YAxis` `ZAxis` `Object` | `Copy` `XAxis` `YAxis` | partly |
| Array | Select objects; numbers in X, Y, Z; unit cell corners; height or Enter for the width | `Mode` `Number` `XNumber` `YNumber` `ZNumber` `Preview` `Spacing` `XSpacing` `YSpacing` `ZSpacing` | none | partly |
| ArrayPolar | Select objects; centre; number of items; angle to fill (on Rhino's Array page) | `Axis` `UseLastAxis` `Preview` `StepAngle` `Rotate` `ZOffset` | none | partly |
| ArrayCrv | Select objects; path curve; number of items or spacing | `Basepoint` `Items` `Distance` `Orientation` | none | partly |
| Orient | Select objects; two reference points; two target points | `Copy` `Scale` | none | partly |
| Orient3Pt | Select objects; three reference points; three target points | `Copy` `Scale` | none | partly |
| SetPt | Select objects; X, Y, Z to set (a dialog in Rhino); pick a location | `Copy` (dialog: Set X, Set Y, Set Z, World or CPlane) | `SetX` `SetY` `SetZ` | partly |
| ProjectToCPlane | Select objects; keep or delete the input | none | none | partly |

## Editing

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Join | Select objects one by one; Enter | `Undo` | none | partly |
| Explode | Select objects | none | none | done |
| Split | Objects to split; cutting objects; Enter | `Point` `Isocurve` `Shrink` | none | partly |
| Trim | Cutting objects; parts to trim away (Shift extends) | `ExtendCuttingLines` `ApparentIntersections` `Line` | none | partly |
| Delete | Select objects | none | none | partly |
| InsertKnot | Select a curve or surface; pick knot locations (on Rhino's InsertControlPoint page) | `Automatic` `Midpoints` `Symmetrical` `Undo` `Direction` `Toggle` | none | partly |
| RemoveKnot | Select a curve or surface; click knots; Enter (on Rhino's InsertControlPoint page) | `Direction` `Toggle` `Undo` | none | partly |
| RemoveControlPoint | Select a curve or surface; click control points; Enter | none | none | partly |
| Smooth | Select objects or points; Enter (settings in a dialog) | `SmoothX` `SmoothY` `SmoothZ` `FixBoundaries` `CoordinateSystem` `SmoothFactor` `Steps` | none | partly |
| Flip | Select objects | none | none | partly |
| Dir | Select objects; click to flip | `Flip` `FlipAll` (`UReverse` `VReverse` `SwapUV` for surfaces) | `Flip` | partly |
| BoundingBox | Select objects | `CoordinateSystem` `Cumulative` `Output` | none | partly |
| PointsOn | Select objects | none | none | partly |
| PointsOff | none | none | none | partly |
| History | Record history for the next commands | `Record` `Update` `Lock` `BrokenHistoryWarning` | none | partly |
| Rename | Not a Rhino command (Rhino names objects in Properties) | none | none | partly |
| Undo | none | none | none | partly |
| Redo | none | none | none | partly |

## Surfaces and solids

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| ExtrudeCrv | Select curves; distance, typed or by two points | `Direction` `BothSides` `Solid` `DeleteInput` `ToBoundary` `SplitAtTangents` `SetBasePoint` | `Solid` `BothSides` `DeleteInput` | partly |
| PlanarSrf | Select planar curves in one plane | none | none | done |
| Revolve | Select curves; axis start; axis end (Enter: construction plane Z); angles | `DeleteInput` `FullCircle` `AskForStartAngle` `Deformable` `PointCount` `SplitAtTangents` | `DeleteInput` `FullCircle` | partly |
| Loft | Select curves in order; adjust seams; Enter; loft options | `Point` `Style` `Closed` `MatchStartTangent` `MatchEndTangent` `SplitAtTangents` `AlignCurves` `Rebuild` `Refit` | none | partly |
| Sweep1 | Rail; cross sections in order; sweep options | `ChainEdges` `Point` `FrameStyle` `Closed` `GlobalShapeBlending` `UntrimmedMiters` `RefitRail` `AlignCrossSections` | none | partly |
| Pipe | Select a curve; start radius; end radius; more radii; Enter | `ChainEdges` `Multiple` `Diameter` `Thick` `Cap` `FitRail` `ShapeBlending` `WallThickness` | `Cap` `Thick` `Diameter` | partly |
| OffsetSrf | Select a surface or solid; distance | `Distance` `Corner` `ThroughPoint` `FlipAll` `Solid` `Loose` `Tolerance` `BothSides` `DeleteInput` | `Solid` `DeleteInput` | partly |
| Box | First corner of base; other corner or length; height, Enter uses the width | `Diagonal` `Cube` `3Point` `EdgeMidpoint` `Vertical` `Center` `BothSides` | `Center` `Diagonal` `3Point` `Vertical` `BothSides` | partly |
| BooleanUnion | Select objects; Enter | `DeleteInput` `MergeCoplanarFaces` | `DeleteInput` | partly |
| BooleanDifference | Objects to subtract from; Enter; objects to subtract with; Enter | `DeleteInput` `DeleteCutters` | `DeleteInput` | partly |
| BooleanIntersection | First set; Enter; second set; Enter | `DeleteInput` | `DeleteInput` | done |
| FilletEdge | Select edges; edit radius handles; Enter | `ShowRadius` `NextRadius` `ChainEdges` `FaceEdges` `PreviousEdgeSelection` `TrimAndJoin` `RailType` `AddHandle` `RemoveHandle` `SetAll` `LinkHandles` | none | partly |
| ChamferEdge | Select edges; edit distance handles; Enter | `ShowChamferDistance` `NextChamferDistance` `ChainEdges` `FaceEdges` `PreviousEdgeSelection` `TrimAndJoin` `RailType` `AddHandle` `RemoveHandle` `SetAll` `LinkHandles` | none | partly |
| Shell | Select faces to remove; Enter (on Rhino's OffsetSrf page) | `Thickness` `DeleteInput` `EditThickness` | `DeleteInput` | partly |

## Selecting

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| SelAll | none | none | none | partly |
| SelNone | none | none | none | partly |
| SelCrv | none | none | none | partly |
| SelPt | none | none | none | partly |
| SelSrf | none | none | none | partly |
| SelSolid | none | none | none | partly |
| SelName | Not read: Rhino's page redirected; complete from Rhino | to confirm | none | partly |
| SelLayer | Not read: Rhino's page redirected; complete from Rhino | to confirm | none | partly |
| SelDup | Not read: Rhino's page redirected; complete from Rhino | to confirm | none | partly |
| SelLast | none | none | none | partly |
| SelPrev | none | none | none | partly |
| SelFilter | Not read: Rhino's page refused; complete from Rhino | to confirm | none | partly |
| SelFilterToggle | none | none | none | partly |
| Invert | none | none | none | partly |
| Hide | Select objects | a name for the hidden set (`-Hide`) | none | partly |
| Show | none | a name of a hidden set | none | partly |
| Isolate | Select objects to keep | none | none | partly |

## Measuring

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Distance | First point; second point | `Units` | none | partly |
| Length | Select curves or edges | `Units` | none | partly |
| Area | Select objects | `Units` | none | partly |
| Volume | Select objects | `Units` | none | partly |

## Views and files

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Top | none | none | none | partly |
| Front | none | none | none | partly |
| Right | none | none | none | partly |
| Perspective | none | none | none | partly |
| 4View | none | none | none | partly |
| UndoView | none (Home or Alt+Z in Curva) | none | none | partly |
| RedoView | none (End or Alt+Shift+Z in Curva) | none | none | partly |
| CPlane | New origin, or an option | `All` `Curve` `Elevation` `Gumball` `Object` `Rotate` `Surface` `Through` `View` `World` `3Point` `Undo` `Redo` | `World` | partly |
| Zoom | Drag a window (the default), or an option | `Window` `All` `Extents` `Selected` `1to1` `Dynamic` `Factor` `In` `Out` `Target` | `Extents` `Selected` `In` `Out` `Target` | partly |
| ZoomExtents | none | none | none | partly |
| ZoomSelected | none | none | none | partly |
| NamedView | A panel in Rhino: save, restore, delete views | `Save` `Restore` `Delete` | `Save` `Restore` `Delete` | partly |
| NamedCPlane | A panel in Rhino: save, restore, delete construction planes | `Save` `Restore` `Delete` | `Save` `Restore` `Delete` | partly |
| SetDisplayMode | Not read: Rhino's page redirected; complete from Rhino | to confirm | `Wireframe` `Shaded` `Ghosted` | partly |
| New | Choose a template | none | none | partly |
| Open | Choose a file | none | none; `.curva`, `.3dm`, and `.dxf` and `.dwg` (curves, points, text, hatches, blocks, dimensions, leaders, layers and layouts with their viewports) as a new model | partly |
| Save | File name | `Version` `SaveSmall` `GeometryOnly` `SaveTextures` `SavePlugInData` `Browse` | none | partly |
| SaveAs | Asks for a file name; saves there and keeps saving there | none | none | partly |
| Import | Choose a file (.3dm, .curva, .dxf or .dwg); its contents join the model, layers merged by name | none | none | partly |
| Export | Choose a file and its type; the model is written in that format | none | none; `.3dm`, `.dxf` and `.dwg` (curves, points, text, hatches, blocks, dimensions, leaders, layers and layouts with their details as viewports), and `.svg` for drawings: the open layout as it prints, or every layout (one file each, named after them) from the model | partly |
| ExportSelected | Select objects; choose a file; they are written as .3dm, .dxf or .dwg | none | none | partly |

## Added for Rhino parity (after 0.3.0)

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Sphere | Center; radius | `2Point` `3Point` `Tangent` `AroundCurve` `4Point` `FitPoints` `Diameter` | `2Point` `3Point` `4Point` `Diameter` | partly |
| Cylinder | Base center; radius; end of cylinder | `DirectionConstraint` `Solid` `2Point` `3Point` `Tangent` `FitPoints` `Diameter` `BothSides` | `2Point` `3Point` `Diameter` `Solid` `BothSides` | partly |
| Cone | Base center; radius; end of cone | `DirectionConstraint` `Solid` `2Point` `3Point` `Tangent` `FitPoints` `Diameter` | `2Point` `3Point` `Diameter` | partly |
| TruncatedCone | Base center; radius; end; top radius | `DirectionConstraint` `Solid` `2Point` `3Point` `Tangent` `FitPoints` | none | partly |
| Torus | Center; radius; second radius | `Vertical` `2Point` `3Point` `Tangent` `AroundCurve` `Diameter` `FixInnerRadius` | `2Point` `3Point` `Diameter` (at the first radius; Rhino also has its own at the second) | partly |
| Tube | Base center; outer radius; inner radius; end of tube | `DirectionConstraint` `Diameter` `WallThickness` `BothSides` `Cap` | `BothSides` | partly |
| Polygon | Center; corner | `NumSides` `Circumscribed` `Edge` `Star` `Vertical` `AroundCurve` | `NumSides` `Circumscribed` | partly |
| Helix | Start of axis; end of axis; radius | `Vertical` `AroundCurve` `Turns` `Pitch` `ReverseTwist` `Diameter` | `Turns` | partly |
| Spiral | Start of axis; end; first radius; second radius | `Flat` `Vertical` `AroundCurve` `Turns` `Pitch` `ReverseTwist` `Diameter` | `Turns` `Flat` | partly |
| Plane | First corner; other corner | `Deformable` `Vertical` `Center` `3Point` | none | partly |
| SrfPt | Three or four corner points | none | none | done |
| ExtrudeSrf | Select surfaces; distance | `Direction` `BothSides` `Solid` `DeleteInput` `ToBoundary` `SetBasePoint` | `BothSides` `DeleteInput` | partly |
| ArrayLinear | Select objects; number of elements; first and second reference points | none | none | done |
| Angle | Corner; point on first side; point on second side | `TwoObjects` | none | partly |
| EvaluatePt | Point to evaluate | none | none | done |
| Radius | Select a circle, arc or curve | none | none | partly |
| What | Select objects | none | none | done |
| SelLine | none | none | none | partly |
| SelPolyline | none | none | none | partly |
| SelCircle | none | none | none | partly |
| SelArc | none | none | none | partly |
| SelClosedCrv | none | none | none | partly |
| SelOpenCrv | none | none | none | partly |
| HideSwap | none | none | none | partly |
| Points | Points until Enter | `MarkPoints` | none | partly |
| Lines | Pairs of points until Enter | `BothSides` | none | partly |
| Slot | Center of first end; center of second end; width | `Vertical` | none | partly |
| Ellipsoid | Center; end of first axis; end of second; end of third | `Corner` `Diameter` `FromFoci` `AroundCurve` | none | partly |
| BooleanSplit | Objects to split; cutting objects | `DeleteInput` | `DeleteInput` | done |
| DupEdge | Select edges | none | none | done |
| Parabola | Focus or vertex; direction; end | `Vertex` `Marks` `FocusMarks` `HalfParabola` | none | partly |
| Paraboloid | Focus or vertex; direction; end | `Vertex` `MarkFocus` `Solid` | none | partly |
| Align | Select objects; alignment option; alignment point (Enter: auto align) | `Bottom` `Concentric` `HorizCenter` `Left` `Right` `Top` `VertCenter` (measured in Rhino 8) | `Bottom` `Concentric` `HorizCenter` `Left` `Right` `Top` `VertCenter` | done |
| ChangeDegree | Select curves; new degree | `Deformable` | `Degree` `Deformable` | partly |
| AreaCentroid | Select surfaces or closed curves | none | none | partly |
| VolumeCentroid | Select solids | none | none | done |

## Added in 0.4.1: the last missing commands

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| HandleCurve | Points, each with its handle; Enter | `Close` | none | partly |
| Sketch | Drag to draw | none | none | done |
| Conic | Start; end; apex; rho or a point on it | `Perpendicular` `Tangent` | none | partly |
| Hyperbola | Centre; vertex; focus; end | `FromFocus` `Coefficients` `Vertices` `Marks` | none | partly |
| BlendCrv | Two curves, near the ends to join | `Position` `Tangency` `Curvature` `G3` `G4` `Edges` | `Position` `Tangency` `Curvature` | partly |
| OffsetMultiple | Curves; distance; side | `Corner` `BothSides` | none | partly |
| DupBorder | Select surfaces | none | none | done |
| Extend | Boundaries (Enter: by length); curves | `Line` `Arc` `Smooth` | `Line` `Arc` `Smooth` `AtStart` | done |
| Fair | Curves; tolerance | none | none | done |
| CurveBoolean | Curves; clicks inside regions | `DeleteInput` `CombineRegions` `AllRegions` | `CombineRegions` `DeleteInput` | partly |
| Match | Curve to change; curve to match | `Position` `Tangency` `Curvature` `AverageCurves` `PreserveOtherEnd` | `Position` `Tangency` `Curvature` | partly |
| SimplifyCrv | Select curves | none | none | done |
| Reparameterize | Curves; start; end | `Automatic` | none | partly |
| MakeUniform | Select curves | none | none | done |
| Weight | Control points picked; weight | none | none | done |
| ExtrudeCrvTapered | Closed curves; distance | `DraftAngle` `Solid` `Sharp` `Round` `Smooth` `Direction` `DeleteInput` `FlipAngle` | `DraftAngle` `Solid` `Sharp` `Round` `Smooth` | partly |
| ExtrudeCrvAlongCrv | Curves; path | `Solid` `DeleteInput` `SubCurve` | `Solid` | partly |
| ExtrudeCrvToPoint | Curves; point | `Solid` `DeleteInput` | `Solid` | partly |
| RailRevolve | Profile; rail; axis | `ScaleHeight` | none | partly |
| Pyramid | Centre; corner; height | `NumSides` `Circumscribed` `Star` `Edge` `Vertical` | `NumSides` `Circumscribed` | partly |
| ExtractSrf | Faces | `Copy` | `Copy` | done |
| MergeAllFaces | Polysurfaces | none | none | done |
| Wirecut | Cutting curve; objects; depth | `KeepAll` `Direction` `BothSides` `DeleteInput` | `KeepAll` | partly |
| Distribute | Objects | `X` `Y` `Z` `Gap` | `X` `Y` `Z` `Gap` | done |
| SelColor | A colour | none | none | partly |
| Lock | Objects | none | none | partly |
| Unlock | none | none | none | partly |
| Group | Objects | none | none | partly |
| Ungroup | Objects | none | none | partly |
| ShowEdges | Objects | `All` `NakedEdges` `NonManifoldEdges` | `NakedEdges` | partly |
| Plan | none | none | none | partly |

## Added in 0.4.2: commands that waited on the geometry engine

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| EdgeSrf | Two to four edge curves | none | none | done |
| NetworkSrf | Curves both ways | `EdgeMatching` `Tolerances` | none | partly |
| Patch | Curves and points to fit | `PointSpacing` `UVSpans` `Stiffness` `AdjustTangency` `AutomaticTrim` | none | partly |
| ExtendSrf | Edge; distance | `Line` `Smooth` | `Smooth` `Line` | done |
| Untrim | Trim edge | `KeepTrimObjects` `All` | none | partly |
| Section | Objects; section line | none | none | done |
| Contour | Objects; base point; direction; distance | `GroupOutput` `JoinCurves` | none | partly |
| Intersect | Objects | none | none | done |
| Project | Curves; surfaces | `Loose` `DeleteInput` | none | partly |
| MoveFace | Faces; move | `BothSides` | none | partly |
| PushPull | Faces; distance | none | none | done |
| Shear | Objects; origin; reference; angle | `Copy` | none | partly |
| RebuildUV | Surfaces | `PointCountU` `PointCountV` `DegreeU` `DegreeV` `DeleteInput` | `PointCountU` `PointCountV` `DegreeU` `DegreeV` | partly |
| CurvatureGraph | Curves | `Scale` `Density` | `Scale` `Density` | done |
| Cap | Surfaces | none | none | done |
| ExtractIsocurve | Surface; point | `Direction` `Toggle` | `U` `V` `Both` | partly |
| ShrinkTrimmedSrf | Surfaces | none | none | done |
| BlendSrf | Two edges | `Position` `Tangency` `Curvature` `Bulge` `Planar Sections` | `Position` `Tangency` `Curvature` `Bulge` | partly |
| MatchSrf | Edge to change; edge to match | `Position` `Tangency` `Curvature` `AverageSurfaces` | `Position` `Tangency` | partly |
| Sweep2 | Two rails; cross sections | `MaintainHeight` `Simplify` | none | partly |
| Twist | Objects; axis; angle | `Infinite` `PreserveStructure` | none | partly |
| Bend | Objects; spine; bend point | `Symmetric` `Copy` `Straight` | none | partly |
| Taper | Objects; axis; sizes | `Flat` `Infinite` | none | partly |
| Flow | Objects; base curve; target curve | `Copy` `Rigid` `Stretch` | none | partly |
| CageEdit | Objects; cage | `BoundingBox` `Line` `Rectangle` | none | partly |
| Pull | Curves; surface | `Loose` | none | partly |
| InterpCrvOnSrf | Surface; points | none | none | done |
| OffsetCrvOnSrf | Curve; surface; distance | `ThroughPoint` | none | partly |
| Silhouette | Objects | none | none | done |
| UnrollSrf | Surfaces | `Explode` `Labels` | none | partly |
| ExtrudeFace | Face; distance | `Direction` `BothSides` | none | partly |
| FilletSrf | Two surfaces; radius | `Trim` `Extend` | none | partly |
| ChamferSrf | Two surfaces; distances | `Trim` `Extend` | none | partly |
| SplitSrf | Surface; curves | none | none | done |
| Zebra | Objects | `Stripe direction` `Stripe size` `Stripe colour` | none | partly |
| EditBoolean | A live boolean | none | none | partly |
| BreakBoolean | Live booleans | none | none | partly |
| Block | Objects; base point; name | `Description` `URL` | none | partly |
| Insert | Name or file; point; scale; rotation | `Linked` `Embedded` `Group` `Explode` | none | partly |
| BlockManager | none | none | none | partly |
| BlockEdit | An instance | none | none | partly |
| ExplodeBlock | Instances | none | none | partly |
| ReplaceBlock | Instances; name | none | none | partly |
| SelBlockInstanceNamed | Name | none | none | partly |
| Count | Objects | none | none | partly |
| ChangeLayer | Objects; layer | none | none | partly |
| MatchProperties | Objects; object to match | `MatchAll` `Linetype` `PrintWidth` `Layer` `Name` | none | partly |
| Linetype | Objects; linetype | none | none | partly |
| PrintWidth | Objects; width | none | none | partly |
| ObjectColour | Objects; a colour (red, green, blue, `#rrggbb` or `ByLayer`), as Rhino's Properties set an object's display colour | none | none | partly |
| SetUserText | Objects; key; text | none | none | partly |
| GetUserText | Object | none | none | partly |
| Purge | none | `Layers` `BlockDefinitions` `Linetypes` | none | partly |
| Audit | none | none | none | partly |
| LockSwap | none | none | none | partly |
| UnlockSelected | Locked objects | none | none | partly |
| BringToFront | Objects | none | none | partly |
| SendToBack | Objects | none | none | partly |
| BringForward | Objects | none | none | partly |
| SendBackward | Objects | none | none | partly |
| DupFaceBorder | Faces | none | none | done |
| TextObject | Text; point; height | `Curves` `Surfaces` `Solids` `Font` | none | partly |
| DraftAnalysis | Objects | `DraftAngle` `PullDirection` | none | partly |

## Milestone 3 in 0.4.3: layouts and details

Written on 14 November 2026 from the Rhino 8 help pages `commands/layout.htm`, `commands/detail.htm` and
`commands/layouts.htm` (`panels/layouts.htm` could not be read; `commands/copylayout.htm` is only a redirect,
so CopyLayout comes from the Layout page). Rhino's Layout is a dialog, not prompts; its fields are listed as
its options here.

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Layout | The New Layout dialog: name, initial detail count (1 to 4; a 4-view layout in first or third angle projection), printer, size, orientation, width and height, units (inches, millimetres, centimetres) | `Name` `InitialDetailCount` `Printer` `Size` `Portrait` `Landscape` `Width` `Height` `Units` | `Size` `Portrait` `Landscape` (asked at the prompt; renamed in the Layouts panel) | partly |
| CopyLayout | Copies the current layout to a new one (no prompts on the page) | none | none (not on the command framework yet) | partly |
| Layouts | Opens the Layouts panel: New Layout, Duplicate Layout, Delete, Move Up, Move Down (or drag), Properties, Print, Import, Search; right-click Set Current, Rename, New Detail | none | none (the panel has New, Open, Copy, Delete, Move Up/Down, drag, rename, size and orientation; Print, Import, Search and New Detail come later) | partly |
| Detail | Manages details on a layout; the prompt order is not on the page. Add draws the rectangle; Scale asks the length on the layout, then in the model | `Add` `Center` `Projection` `Enable` `FromDetail` `FirstAngle` `ThirdAngle` `Scale` `Lock` `Unlock` `EnablePage` | `Add` (two corners) `Projection` (Top, Front, Right, Perspective) `Scale` `Lock` `Unlock`; `Enable` by double-clicking | partly |

### Clipping planes

Written on 14 November 2026 from `commands/clippingplane.htm`, which also covers EnableClippingPlane and
DisableClippingPlane (`commands/enableclippingplane.htm` is only a redirect). The SectionStyle pages
(`commands/sectionstyle.htm`, `properties/section_style.htm`) refused to load (HTTP 403); their rows are to be
completed from Rhino itself; SectionStyle was checked in Rhino 8 (it has no such command).

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| ClippingPlane | Two opposite corners of a rectangle, or another way to draw it; the plane clips away what is on its back side in the viewports it is enabled in (its Views Clipped list) | `3Point` `EdgeMidpoint` `Vertical` `Center` `AroundCurve` `CustomDepth` `Depth` `Name` `Label` `SectionStyle` `FlipDirection` `ObjectsClipped` `ViewsClipped` `ResetViews` `Match` | `FlipDirection`; two corners on the construction plane; cuts the four model viewports; section fill in Properties | partly |
| EnableClippingPlane | Turns on selected clipping planes in the active viewport | none | none (in the active viewport, or the detail being worked in) | partly |
| DisableClippingPlane | Turns off selected clipping planes in the active viewport | none | none (in the active viewport, or the detail being worked in) | partly |
| SectionStyle | Not a Rhino 8 command: Rhino 8 lists none (checked in Rhino 8 on 17 November 2026) and sets section styles in Properties > Section Style, as a background fill (none, viewport or a colour), a hatch pattern with scale, rotation and colour, and the boundary. Curva's command follows its spec (`layouts.md` §3) | none | Solids or clipping planes, then Enter: sets the solids' own style or the planes'. `Fill` (Curva's standard grey, or none), `Hatch`, `Pattern` (a hatch pattern's name), `Scale`, `Rotation` (degrees), `Layers` (on the objects' layers instead), `ByLayer` (clears the objects' own style, or with `Layers` the layers'); alias SectionHatch (Serpentine's name). Not yet: fill colour other than the standard grey, hatch colour, boundary | partly |

### Text and annotation styles

Written on 14 November 2026 from `commands/text.htm` and `information/text_fields.htm`, and for annotation
styles from `documentproperties/annotation.htm`, which the owner provided (the other style pages refused to
load); every style setting is listed in `rhino-annotation-styles.md`. Rhino's Text has no command-line options
besides its scriptable `-Text`; its settings are in the text editor and the Properties panel, listed here as its
options. Rhino edits styles in Document Properties; Curva's AnnotationStyle opens them in a panel.

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Text | Type the text; specify the properties; pick the location. Double-click text to edit it | `Style` `Height` `Mask` `ModelSpaceScale` `Frame` `Font` `Bold` `Italic` `Underline` `TextAlignment` `TextField` `Stacking` `Symbols` `Match` `HorizontalToView` | `Height` `Font` `TextAlignment` (as `Alignment`, left, centre, right); words, height and alignment edited in Properties | partly |
| AnnotationStyle | Rhino's Document Properties > Annotation Styles: Current, the list, New, Match, Import, Edit, Delete (also deleting annotations in the style), Enable model space scaling, Enable layout space scaling | `Current` `New` `Match` `Import` `Edit` `Delete` `EnableModelSpaceScaling` `EnableLayoutSpaceScaling` | none (a panel: Current, New as Copy, Edit of font and height, Delete of unused styles) | partly |
| SetCurrentAnnotationStyle | Sets the current annotation style | none | none (by name) | partly |
| DupAnnotationStyle | The style to copy; the new style's name; whether to make it current | `SetCurrent` | none (styles named at the prompt; `SetCurrent` as an option, not on the command framework yet) | partly |
| ImportAnnotationStyles | Annotation styles from a `.3dm` file | none | none | missing |

### Dimensions

Written on 14 November 2026 from `commands/dimension_commands.htm` (each command's own page is a redirect to
it). DimArcLength is not on it, and Rhino 8 has no such command.
In Rhino, dimensions are attached to what they measure by History (on by default, `RecordAnnotationHistory`),
take their looks from annotation styles with per-dimension overrides, and `<>` in their text stands for the
measured value.

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Dim | Two points; a third point places the dimension line; horizontal or vertical | `AnnotationStyle` `Object` `Continue` `Undo` `Baseline` `Vertical` `Horizontal` | `Vertical` `Horizontal` (otherwise by where the line goes); the current annotation style | partly |
| DimAligned | Two points; a third point places the dimension line, parallel to the points | `AnnotationStyle` `Object` `Continue` `Baseline` `Undo` | the current annotation style | partly |
| DimRotated | The rotation angle (typed, or two points); two points; a third point places the dimension line | `AnnotationStyle` `Object` `Continue` `Baseline` | typed angle; the current annotation style | partly |
| DimAngle | An arc, or two lines (polyline segments, linear edges); the dimension location | `AnnotationStyle` `Points` | the current annotation style; lines only (no polyline segments or edges yet); the smaller of the four angles that holds the arc point | partly |
| DimRadius | A curve; the dimension location; measured as projected to the construction plane | `AnnotationStyle` `PointOnCurve` | the current annotation style; the pick sets where the arrow lands (PointOnCurve behaviour); circles and arcs only, in their own plane | partly |
| DimDiameter | A curve; the dimension location | `AnnotationStyle` | the current annotation style; circles and arcs only | partly |
| DimOrdinate | A dimension point; a leader end (X or Y implied by direction); Enter ends | `AnnotationStyle` `XDatum` `YDatum` `Basepoint` `KinkOffset` | `XDatum` `YDatum` (chosen again, back to implied) `Basepoint` (the world origin until set) `KinkOffset` (both kinks; unset is twice the text height); the current annotation style | partly |
| DimArcLength | Not a Rhino 8 command: Rhino 8 lists none (checked in Rhino 8 on 16 November 2026); its closest is DimCurveLength. Curva's arc length dimension follows AutoCAD's (spec `annotation.md` §3) | none | An arc, then where the dimension arc goes, as AutoCAD's DIMARC. `Partial`: two points on the arc, then where it goes; `Leader` (offered only for a turn over a quarter, as AutoCAD): an arrow from the dimension in to the arc, remembered between runs as Rhino's toggles are | done |
| DimCurveLength | A curve; a start point for the text or leader | `Type` | `Type` (Text or Leader; Text first); the words are the `CurveLength` field, as in Rhino; not on the command framework yet | partly |
| DimArea | A closed planar curve, surface or hatch; a start point for the text or leader | `Type` | `Type` (Text or Leader); the words are the `Area` field; closed planar curves, surfaces, solids and hatches (what a hatch fills, its holes left out); no meshes, as Curva has none; not on the command framework yet | partly |
| DimVolume | A closed solid; a start point for the text or leader | `Type` | `Type` (Text or Leader); the words are the `Volume` field; not on the command framework yet | partly |
| DimCreaseAngle | First surface; second surface; the dimension location | none | none | missing |
| DimRecenterText | Picked dimensions' text goes back to its default place | none | none | missing |
| Centermark | A cross, or cross and centre lines, at a curve's centre | `Style` | none | missing |
| SetDimensionLayer | Whether dimensions go on the current layer or a given one | `UseDimensionLayer` `DimensionLayer` `List` `Reset` | none | missing |

### Leaders and dots

Written on 14 November 2026 from `commands/leader.htm` and `commands/dot.htm`. Rhino's Leader has no command-line
options; its settings are in the Properties panel, listed here as its options.

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Leader | The arrow's point; the next points; Enter ends; the text in a dialog | `Style` `Height` `Mask` `Frame` `Font` `TextAlignment` (Left, Center, Right, Auto) `VerticalAlignment` `Bold` `Italic` `Underline` `TextField` `Arrowhead` `ArrowSize` `CurveType` (None, Polyline, Spline) `TextOrientation` (Horizontal, Aligned) `Landing` | `Landing` as a command option and in Properties; the text typed at the command line (`\n` for a new line), with fields; the current annotation style; Auto alignment; polyline only | partly |
| Dot | The text; the location. Always in front, the same size on screen, its colour from the layer or object; double-click to edit | `Font` `Height` `DisplayText` `SecondaryText` | `Height` (points on screen); text typed at the command line (`\n` for a new line); text, secondary text and height in Properties. Not yet: fonts, double-click to edit, showing in detail views | partly |
| ConvertDots | Dots to convert | `DeleteInput` `Output` (Point, Text) `TextHeight` `HorizontalAlign` `VerticalAlign` | none | missing |

### Hatches

Written on 14 November 2026 from `commands/hatch.htm` and `user_interface/hatch-fill-styles.htm`. The Hatch Patterns
page (`documentproperties/hatchpatterns.htm`) refused to load (HTTP 403), so Rhino's shipped pattern list is not
read; Curva ships Solid, Hatch1 to Hatch3, Dash, Grid, Grid60, Plus and Squares, and ANSI31 to ANSI38. Rhino sets the
pattern, scale and rotation in a dialog; Curva takes them as command options.

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Hatch | Closed planar curves, or with `Boundary=Yes` curves and clicks inside the regions they make; then the dialog: pattern, base point, rotation, scale, background, display colour, show boundary, fill style (solid; linear or radial gradients, reflected or wrapped, with repeat and colour stops) | `Boundary` `CombineRegions` `AllRegions` | `Pattern` `Scale` `Rotation` `Boundary` (regions always combined); curves inside others are holes; the pattern starts at the world origin; pattern, scale and rotation in Properties; resizing keeps the pattern's size, turning turns it. Not yet: gradients, background, base point while hatching (SetHatchBasePoint moves it after), pattern dots, boundary following its curves | partly |
| SetHatchBasePoint | Select hatches; the new base point | none | none: the patterns move to start at the point; the outlines stay (not on the command framework yet) | partly |
| HatchPatterns | The hatch pattern dialog: the document's patterns, with Import (a `.pat` file), Export, rename, delete and set current | none | none: a `.pat` file's patterns join the model's (one of the same name takes its place); sizes in inches when the file says `;%UNITS=INCH`, else millimetres. Not yet: the dialog, Export, rename, delete | partly |
| HatchBase | Changes the base point of hatches | none | none | missing |
| HatchScale | Scales hatch patterns in model viewports | `Enabled` `Disabled` `Toggle` | none | missing |

### Printing and export

Written on 15 November 2026 from `commands/print.htm`, which covers the Print dialog only (not `-Print`'s options).
Curva's Print opens the Print panel; ExportPDF prints from the command line, for scripts and the assistant
(`drawing-output.md` §1).

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Print | The Print dialog: destination (printer, size, DPI, orientation, copies), output type (vector or raster), output colour (print, display, black and white), view and scale (viewport or layouts, extents, window, several layouts, separate PDF files, scale), margins and position, linetypes and line widths, visibility, with a live preview | the dialog's settings | The Print panel: layouts ticked to print, display colours (white prints black) or black, PDF file or printer, and a preview of each page (fills shown as outlines); layouts print 1:1 on their paper as vectors: sheet objects, and detail views of the model clipped to their frames, with print widths and linetypes; text and arrowheads filled. Aliases `PrintSheets`, `Plot`. Not yet: model views, raster details, hidden lines, dots, separate files, margins | partly |
| ExportPDF | Serpentine's command: Print to a PDF | as Print | Print's choices as options: `Layouts` (current or all), `Colour`, `Output` (PDF or printer); then the file | partly |

### Make2D

Written on 15 November 2026 from `commands/make2d.htm`, which describes the dialog only (no `-Make2D` options).

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Make2D | Select objects; set the projection and options in the dialog | Projection: `View` (lower left at the world origin; register with previous), `CPlane` (on the construction plane), third angle and first angle four-view drawings. Options: object properties (from input objects or by output layers), maintain source layers, tangent edges, hidden lines, scene silhouette, clipping plane intersections, viewport rectangle, group output, register with previous; the parent layer name | `Projection` (View, CPlane, ThirdAngle or FirstAngle; the four-view drawings' perspective is Rhino's standard Perspective direction drawn flat, as Curva does not keep each viewport's camera in the document), `HiddenLines`, `TangentEdges`, `GroupOutput` (one group), `MaintainSourceLayers` (each source's layer tree under the Make2D layer it goes on, as `Make2D::Visible::Curves::Walls`); annotation lying flat to a view copied flat into its drawing on `Make2D::Visible::Annotations` (annotation standing across the view is left out); clipping planes chosen with the objects cut the drawing (bodies by the engine's split, curves where they cross), and `ClippingPlaneIntersections` (on at first) draws the cuts through the solids on `Make2D::Visible::Clipping plane intersections`; where a cut face looks towards a flat view, it is filled with the solid's section style (`SectionStyle`) as hatches under the lines: a solid hatch in the fill colour, the pattern over it (a fill partly behind other objects is drawn whole; a cut another chosen plane crosses is not filled); exact edges, curves (polylines as their segments) and silhouettes of curved faces (exact for cylinders and spheres), visible and hidden, under `Make2D` > `Visible`/`Hidden` > `Curves`, `Silhouettes` and `Tangents` (hidden ones dashed). Visibility by solidcore's B-rep rays (`make2d.md` §2). Clean output: same-visibility pieces rejoined, end-on lines, hidden lines under visible ones and doubled lines removed (a square gives four lines, a box from above its outline). Not yet: annotation standing across a view, register with previous | partly |

### Document text and notes

Written on 15 November 2026 from `commands/setdocumentusertext.htm` (which also covers GetDocumentUserText),
`commands/notes.htm` and `information/text_fields.htm`. The layout properties page (`properties/layout.htm`)
refused to load (HTTP 403); Rhino sets layout user text there, Curva with SetLayoutUserText.

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| SetDocumentUserText | The key (a leading full stop hides it from lists); the value (`""` removes the key). Read by the `DocumentText` field | none | as Rhino; Enter alone also removes the key, `\n` types a new line (not on the command framework, which has no typed-text step yet) | partly |
| GetDocumentUserText | Shows text kept with SetDocumentUserText | none | the key, or Enter for every key that is not hidden (not on the command framework yet) | partly |
| SetLayoutUserText | Curva's: Rhino sets layout user text in Layout Properties. The key; the value, on the current layout. Read by the `LayoutUserText` field | none | as SetDocumentUserText, on the open layout (not on the command framework yet) | partly |
| Notes | Opens the Notes panel, where the model's notes are typed; a Locked checkbox. Read by the `Notes` field | none | Enter opens the Notes panel, edited in place (undoable); also typed at the prompt (`\n` for a new line, `""` clears) for scripts. Not yet: Locked | partly |

### Title blocks, scale bars, sheet index and revisions

Rhino has none of these commands (`layouts.md` §4); they are Serpentine3D's, and their steps follow Serpentine's.
Rhino users build title blocks from curves and text with fields, which is what TitleBlock places.

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| TitleBlock | Not in Rhino. Serpentine: the project, the drawing title, who drew it; the block goes bottom right | none | `Standard` (ISO or ANSI, from the sheet's unit); the project, title and drawn-by, Enter keeping what the document says; places a border (ISO 5457 or a half-inch one) and an ISO 7200-style block of lines and text with fields (project, title, scale of the largest detail, date, drawn and checked by, file, sheet number of count, revision) and a revision table, grouped | partly |
| ScaleBar | Not in Rhino. Serpentine: the bar's place, for the detail there or the first one | none | `Length` `Divisions`; the detail is asked only when the sheet has more than one scaled detail; the bar follows the detail's scale, a round length when not typed, in metres for millimetre models | partly |
| SheetIndex | Not in Rhino. Serpentine: where the index goes | none | a table of every sheet's number, name and `Title` layout user text, which follows the layouts | partly |
| Revision | Not in Rhino. Serpentine: the revision mark, then the description; dated today | none | the mark (Enter takes the next: B after A, P02 after P01), what changed, who (Enter takes the `DrawnBy` document text); the row shows in the sheet's revision table and the mark in its title block | partly |

## Mesh and SubD (mesh stream)

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Mesh | Surfaces, polysurfaces, extrusions or SubDs; then the mesh options dialog (`-Mesh`: a polygon density on the command line, and detailed options). The originals stay; Rhino meshes a radius-5 sphere at its default into 2,048 polygons | Density slider; detailed: maximum angle, aspect ratio, minimum and maximum edge length, maximum distance edge to surface, minimum initial grid quads, refine, jagged seams, simple planes, pack textures, SubD level | the objects, then the density from 0 to 1 typed (Enter keeps the last); `MaxAngle` `MaxDistance` `SubDLevel`; the preview shows the mesh's edges; settings shared with STL, OBJ and PLY export; meshes of surfaces and solids follow them with History (density only). The mesh panel comes with the framework's panel step | partly |
| MeshBox | Face counts, then the base as Box takes it (other corner, or a typed length then Width, Enter: the length), then the height (Enter: the base's side along Y) | `XCount` `YCount` `ZCount` (10 each, measured); `Diagonal` `Cube` `3Point` `EdgeMidpoint` `Vertical` `Center` | `Center` `BothSides` `XCount` `YCount` `ZCount`; picked as Box is, measured 21 November 2026; Rhino's 600 quads at its defaults | partly |
| MeshPlane | Two corners or a typed length then Width (Enter: the length), as Rectangle, then the face counts | `XCount` `YCount` (10 each, measured); `3Point` `EdgeMidpoint` `Vertical` `Center` `AroundCurve` | `Center` `XCount` `YCount`; Rhino's 100 quads at its defaults | partly |
| MeshSphere | Centre, then radius; styles UV (faces up and around), Triangle or Quad (subdivisions), as Rhino 8's prompt names them | `VerticalFaces` `AroundFaces` (10 each, measured) `Subdivisions` `Diameter`; `2Point` `3Point` `Tangent` `AroundCurve` `4Point` `FitPoints` | `2Point` (the ends of a diameter, matching Rhino 8, measured) `Diameter` `Style` `VerticalFaces` `AroundFaces` `Subdivisions` (remembered per style as in Rhino: 2 at first for Quad, 1 for Triangle, measured); the face options shown follow the style, as in Rhino; Rhino's 80 quads and 20 triangles at its defaults | partly |
| MeshCylinder | Base circle, then height | `VerticalFaces` `AroundFaces` (10 each, measured) `Solid` `CapFaceStyle` (Tri) `BothSides` `DirectionConstraint`; the circle's ways | `Diameter` `Solid` `BothSides` `VerticalFaces` `AroundFaces` `CapFaceStyle`; Rhino's 120 faces at its defaults | partly |
| MeshCone | Base circle, then the point's height | `VerticalFaces` `AroundFaces` (10 each, measured) `Solid` `CapFaceStyle` `DirectionConstraint`; the circle's ways | `Diameter` `Solid` `VerticalFaces` `AroundFaces` `CapFaceStyle`; Rhino's 110 faces at its defaults | partly |
| MeshTorus | Centre and radius, then the tube's radius | `VerticalFaces` `AroundFaces` (10 each, measured) `FixInnerDimension`; the circle's ways | `VerticalFaces` `AroundFaces`; Rhino's 100 quads at its defaults | partly |
| SubDBox | As MeshBox | `XCount` `YCount` `ZCount` (2 each, measured: 24 cage faces); the box's ways of picking | `Center` `BothSides` `XCount` `YCount` `ZCount`; picked as MeshBox; the smooth surface reaches the box's sides with its corners rounded, as Rhino | partly |
| SubDPlane | As MeshPlane | `XCount` `YCount` (2 each, measured: 4 faces, border creased) | `Center` `XCount` `YCount`; picked as MeshPlane | partly |
| SubDSphere | As MeshSphere | `Style` (Quads at 1 subdivision at first, measured: 24 faces) `Subdivisions` `VerticalFaces` `AroundFaces` `Diameter`; the sphere's ways of picking | `2Point` `Diameter` `Style` `VerticalFaces` `AroundFaces` `Subdivisions`; the smooth surface passes through the sphere | partly |
| SubDCylinder | As MeshCylinder | `VerticalFaces` (4) `AroundFaces` (8) (measured: 48 faces with triangle caps) `Solid` `CapFaceStyle` `BothSides`; the circle's ways | `Diameter` `Solid` `BothSides` `VerticalFaces` `AroundFaces` `CapFaceStyle`; the side's smooth surface stands at the radius, as Rhino | partly |
| SubDCone | As MeshCone | `VerticalFaces` (4) `AroundFaces` (8) (measured: 40 faces) `Solid` `CapFaceStyle`; the circle's ways | `Diameter` `Solid` `VerticalFaces` `AroundFaces` `CapFaceStyle`; the plain cage, its smooth surface inside it, as Rhino | partly |
| SubDTorus | As MeshTorus | `VerticalFaces` (6) `AroundFaces` (10) (measured: 60 faces) `FixInnerDimension`; the circle's ways | `VerticalFaces` `AroundFaces`; the smooth surface passes through the torus | partly |
| ToSubD | Meshes, surfaces and extrusions (measured: polysurfaces are refused); Enter | `UseMesh` (ControlPoints at first, measured) `MeshCreases` (off) `MeshCorners` (off), PackFaces, DeleteInput | `UseMesh` `MeshCreases` `MeshCorners` for meshes. As measured in Rhino 8: the mesh's points are the cage, unwelded edges stay smooth, an open border is creased with no corners, and the mesh stays (a 2 × 2 × 2 box mesh gives 24 faces and 26 points). Surfaces as Rhino 8 fits them (measured): the cage is the control net of the surface rebuilt as a uniform cubic through its Greville points, borders creased, trims and holes ignored (a capped extruded circle: 8 sides, two 8-sided caps, rims creased, 50.2041 m³; a sphere: 32 quads and 16 triangles on 42 points). Curva has no extrusions, so polysurfaces are taken in their place: a prism creases its cap rims as Rhino's extrusions do (a box its top and bottom, 8 edges), other polysurfaces their sharp edges. Not yet: a box drawn on a plane other than Top creases the top and bottom as seen from Top; cones (Rhino keeps kinks at the circle's double knots) and holed extrusions (Rhino joins the inner and outer walls) differ | partly |
| SubDDisplayToggle | Nothing to pick (Tab in Rhino) | none | none; every SubD in the model switches between smooth and flat whatever is selected, with Rhino's message, as measured in Rhino 8; undoable, and with Tab when nothing is typed and no command is running | done |
| ToNURBS | SubDs, meshes or curves; Enter | `DeleteInput` (off at first, measured), PackFaces (on), Curves | `DeleteInput` `PackFaces`; SubDs, meshes and curves. Meshes become one surface per face as MeshToNURB makes them (Rhino 8: a 2 × 2 × 2 box mesh gives a solid of 24 faces); curves become NURBS curves as Rhino 8 makes them (a line degree 1 with 2 points, a circle degree 2 with 9, a line joined to a half circle one degree-2 curve of 7, measured). As measured in Rhino 8: the SubD stays and a closed one becomes a closed polysurface of the same volume (a 4 m cube cage of 2 × 2 a side: 50.3587 m³). `PackFaces` joins the patches as Rhino does (6 faces for a box, 1 for an open plane or tube, the side of a cylinder with triangle caps as 1 and each triangle as 3); a torus comes out as 2 faces where Rhino makes 1, which the kernel cannot make solid | partly |
| Crease | SubD edges; Enter | none | the picked SubD cage edges become creases, their ends darts; a picked face changes nothing; "Added creases to 1 object." (measured in Rhino 8) | done |
| RemoveCrease | SubD edges; Enter | none | the picked creases become smooth again; "Removed creases from 1 object." (measured in Rhino 8) | done |
| Fill | SubD and mesh boundary edges, then "Patch style <Automatic> ( Automatic  SingleFace  TriangleFan  Triangulated )" | the patch style | `PatchStyle` (Automatic at first, as in Rhino). SubDs: the holes the picked border edges run round, closed in that style (Automatic: four quads round a new middle point for an eight-edge hole, as measured in Rhino 8), the old border smooth. Meshes: a hole whose open edges are all picked closed the same way, as a joined but unwelded piece (an open box's top: 24 quads on 34 points, as measured; one edge picked fills nothing). The patch style is asked after the edges, as in Rhino | done |
| UnweldEdge | "Select edges ( ModifyNormals=Yes  SharpCorners=Yes )"; Enter | `ModifyNormals` (Yes) `SharpCorners` (Yes) | `ModifyNormals` (meshes, as Rhino). SubDs: the picked inner edges cut open into creased borders; a point is copied once for each piece its ring of faces is cut into, so a cut's ends stay whole (two edges in a row of a 2 × 2 × 2 box: 27 points, 4 creased border edges, as measured in Rhino 8); no message. Meshes: each point on the picked edges gets a vertex for each side (ends too: the face the edge leaves from parts off alone), and with ModifyNormals each its side's normal (one folded edge of a 2 × 2 × 2 mesh box: 26 vertices become 28, normals as Rhino 8's). SharpCorners is not here yet | partly |
| WeldEdge | SubD border edges; Enter | none | border edges at exactly the same place joined back and made smooth (the unwelded box above comes back whole, as in Rhino 8); edges with nothing at their place are left alone | done |
| Stitch | "Select first vertex or set of edges to stitch together ( EdgeLoop )", then the second set, then where they meet | `EdgeLoop`; Average (Enter), First, Second | `Location` (Average at first, as in Rhino): a point, or a run of edges, joined onto the second, point to point, the second run turned round when its ends lie nearer that way; asked after the second set, as in Rhino. EdgeLoop picking is not here yet | partly |
| Bridge | "Select first set of edges or faces to bridge ( EdgeLoop )", the second set, then "Bridge options ( Segments=1  Join=Yes  Crease=No  Straightness=1 )" | `Segments` (1) `Join` (Yes) `Crease` (No) `Straightness` (1); `EdgeLoop` | `Segments` `Join` `Crease` (Rhino's starting values, measured): two runs of open SubD border edges joined by rows of quads; two strips bridged with Enter become one SubD of 6 quads on their 12 points, the bridge smooth and the 10 outer border edges creased, as in Rhino 8; `Join` off makes the bridge a SubD of its own; the options are offered while the second set is picked. Straightness (only straight here), picking faces and EdgeLoop are not here yet | partly |
| InsertEdge | "Select edge from loop ( Type=Loop  Mode=Full )" until Enter, then "Edge positioning. Select point or give amount ( BothSides=No  OffsetMode=Absolute )" | `Type` (Loop, BothSides, Ring) `Mode` (Full, Range) `BothSides` (No) `OffsetMode` (Absolute, Proportional) | `Type` (Loop, Ring) `Mode` (Full, Range) `BothSides` `OffsetMode` (Rhino's starting values, measured): each picked edge runs on through points where four edges meet to the next point where they do not, and new edges go beside the run at the amount along each crossing edge, the faces beyond its ends taking its end points (a 4 m SubD box of 2 × 2 a side, a corner edge and 0.5: 26 faces on 29 points, as in Rhino 8). A clicked point picks the side and the distance. With Type=Ring the ring of faces across the picked edge is cut at the amount from its lower end (the measured box: 8 new points at z = 0.5, 32 quads on 34 points, as in Rhino 8), and only OffsetMode is offered, as Rhino does. With Mode=Range only the edges between the two picked ones are used (the measured box's two middle front edges: 26 faces on 29 points, as in Rhino 8). A typed amount goes on one fixed side, where Rhino follows the mouse | partly |
| MergeFaces | SubD faces; Enter | none | the picked SubD faces that touch along edges become one face round their outer border, keeping every point a face still uses; two neighbouring quads on a box side become one six-sided face (23 faces on 26 points), "Changed 1 objects.", as measured in Rhino 8 | done |
| InsertPoint | "Select mesh or SubD", then "Select first point on mesh edge" and "Select next point on mesh edge ( Undo )" until Enter | `Undo` | SubDs: each picked point goes on the cage edge nearest it, splitting the edge, and two points in a row that share a face are joined by a new edge across it; on a 4 m box of 2 × 2 a side, one point at a bottom edge's middle gives 24 faces on 27 points, and two on opposite edges of a face split it (25 faces on 29), as measured in Rhino 8. Meshes and Undo are not here yet | partly |
| OffsetSubD | SubDs, then "Select object to flip offset direction. Press Enter when done ( Distance=1  Solid=No  BothSides=No  DeleteInput=No  FlipAll )" | `Distance` (1) `Solid` (No) `BothSides` (No) `DeleteInput` (No) `FlipAll` | `Distance` `Solid` `BothSides` `DeleteInput` `FlipAll` (Rhino's starting values, measured): each cage point moves the distance along its limit normal (a 4 m box of 2 × 2 a side: cage x 199 to 205 exactly as Rhino 8's), Solid joins the offset to the SubD with side quads round its border (an open 2-face strip: 10 quads on 12 points, 12 creases), BothSides offsets both ways, SubDs picked at the second prompt go the other way | done |
| Bevel | "Select Mesh or SubD edges to bevel ( EdgeLoop )", then "Bevel positioning. Select point or give amount ( Segments=3  OffsetMode=Absolute  Straightness=0  KeepCreases=Yes )" | `Segments` (3) `OffsetMode` (Absolute) `Straightness` (0) `KeepCreases` (Yes); `EdgeLoop` | `Segments` `OffsetMode` `Straightness` (Rhino's starting values, measured): each picked SubD edge becomes a strip of quads, its ends giving way to rows of points along the side edges and a cap closing each end (the middle edge of a 4 m box's front and 0.5: 29 faces on 32 points, Rhino's rows, as in Rhino 8). Edges joined end to end bevel as one run, a row at each point and caps only at open ends (two in a row: 32 faces on 35 points; a loop round the box: 48 quads on 50 points, as in Rhino 8). Points must be inner ones where four edges meet and picked edges must not branch; KeepCreases, meshes and EdgeLoop are not here yet | partly |
| Check | Objects; reports what is wrong, or "This is a good mesh." | none | meshes reported as Rhino 8 counts (degenerate faces, very short edges at a tenth of the tolerance, non-manifold and naked edges, duplicate faces, faces pointing the wrong way, crossing faces, unused points, bad normals, pieces); other objects counted and left to the features stream | partly |
| MeshRepair | A wizard of steps over the chosen mesh | none on the command line | the main fixes in one go, each an option: `CullDegenerate` `CullDuplicates` `RemoveUnusedPoints` `Weld` `UnifyNormals` `FillHoles` `RebuildNormals` | partly |
| UnifyMeshNormals | Meshes | none | each piece one way, closed pieces out | done |
| FillMeshHoles | Meshes | none | every hole closed | done |
| CullDegenerateMeshFaces | Meshes | none | faces with no area taken out | done |
| RebuildMeshNormals | Meshes | none | normals worked out again from the faces | done |
| ReduceMesh | Meshes, then a dialog: reduce to a polygon count or by a percentage (quads count as two), or planar only; locked points. `-ReduceMesh` asks "Choose mesh reduction option ( NewFaceCount=… ReductionPercentage=0 PlanarOnly Accuracy=10 LockMeshPoints )"; PlanarOnly acts at once (measured 22 November 2026: a 10 m box of 10 × 10 faces a side, 1,200 triangles, goes to 344, joined back as 118 quads and 108 triangles on 174 points, still closed at 1,000 m³, keeping its 8 corners, 65 edge points and 101 points inside its sides). `-ReduceMesh` has `ReductionPercentage` (0 at first, measured: Enter changes nothing), `PlanarOnly` and `Accuracy` (1 fast to 10 accurate; the dialog always 10). It reduces to exactly the target in triangles, then joins pairs back into quads (measured: a 40 × 40 sphere's 3,120 triangles go to 1,560 at 50 %, 443 quads and 674 triangles) | Reduce to, Reduce by percent, Planar only, locked points, Accuracy | the meshes, then the percentage to take away (50 at first rather than Rhino's do-nothing 0; Enter keeps the last); `PolygonCount` above 0 reduces to that many triangles instead; quadric edge collapse (Garland and Heckbert) at its most accurate, borders kept, then quads joined back with QuadrangulateMesh's defaults; Rhino's counts reached exactly at 50 % and 90 %; in place. Planar only, locked points and Accuracy are not here yet | partly |
| TriangulateMesh | Meshes | none | every face of four or more corners split into triangles, quads along their shorter diagonal; in place | done |
| QuadrangulateMesh | Meshes, then a dialog | Planarity (angle), Rectangularity (diagonal ratio, 1 or more) | `Planarity` (5°) `Rectangularity` (2), Rhino 8's defaults measured: faces less than 5° apart join, quads whose longer diagonal is at most twice the shorter; unwelded diagonals are not joined; in place | done |
| ExtractMeshFaces | Mesh faces | none | the picked faces become a mesh of their own on the same layer and the rest stays, as measured in Rhino 8; a mesh left with no faces goes | done |
| MeshToNURB | A mesh | TrimTriangularFaces (on at first, measured), UseNgons | `TrimTriangularFaces` `UseNgons` (both on at first, measured); one surface per face, sewn into a polysurface (a solid when the mesh is closed); the mesh stays. As measured in Rhino 8: quads are untrimmed bilinear surfaces; triangles trimmed flat ones, or with TrimTriangularFaces off untrimmed bilinear surfaces with one side shrunk to the first corner (still a solid); a face of more than four corners one trimmed flat surface, or with UseNgons off its triangles | done |
| MeshBooleanUnion | Meshes; Enter | none | the meshes become one, replacing them; meshes that do not overlap are left alone, as measured in Rhino 8. Rhino leaves open results with faces missing where faces lie in one plane (two overlapping cubes: volume 1.333 where 1.5 is right); Curva's come out closed. Open meshes are refused | done |
| MeshBooleanDifference | Meshes to cut from; Enter; meshes to cut with; Enter | `DeleteInput` (at the second step, on at first; off keeps the cutters, measured) | `DeleteInput` (on); each mesh is cut on its own and replaced; meshes the cutters miss are left alone, as measured in Rhino 8. Rhino also cuts with an open mesh, leaving an open result; Curva refuses open meshes | done |
| MeshBooleanIntersection | First meshes; Enter; second meshes; Enter | none | each first mesh is replaced by its overlap with the second set, and the second set is deleted, as measured in Rhino 8; meshes that do not overlap are left alone. Open meshes are refused | done |
| MeshBooleanSplit | Meshes to split; Enter; meshes to split with; Enter | none | each mesh is replaced by its part outside the second set and its part inside is added; the second set stays, as measured in Rhino 8. Rhino's parts come out open where Curva's are closed. Open meshes are refused | done |
| QuadRemesh | Meshes, SubDs, surfaces or polysurfaces; Enter (a dialog in Rhino; `-QuadRemesh` takes no options and uses the dialog's choices) | TargetQuadCount (2000), TargetEdgeLength, AdaptiveSize (50), AdaptiveQuadCount (on), DetectHardEdges (on), Convert to SubD, Symmetry, guide curves | `TargetQuadCount` (2000) `TargetEdgeLength` (0 aims at the count, as in Rhino; quads come out about 1.12 times as long, as measured in Rhino 8) `DetectHardEdges` (on, a fold over 30° is kept, measured) `ConvertToSubD` (off) `Symmetry` (None, X, Y, Z and their pairs; measured in Rhino 8: its X mirrors y and its Y mirrors x, about the middle of the bounding box); the input stays and the quads are a new mesh on its layer, as measured in Rhino 8 (a 40 × 40 sphere: 1,750 quads in Rhino, 1,990 in Curva). The adaptive size and guide curves are not here yet | partly |
