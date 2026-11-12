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
| Offset | Select a curve; click the side | `Distance` `Loose` `Corner` `Trim` `ThroughPoint` `Tolerance` `BothSides` `InCPlane` `Cap` `OutputLayer` | `Distance` `ThroughPoint` `BothSides` | partly |
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
| ExtrudeCrv | Select curves; distance, typed or by two points | `Direction` `BothSides` `Solid` `DeleteInput` `ToBoundary` `SplitAtTangents` `SetBasePoint` | `Solid` `BothSides` | partly |
| PlanarSrf | Select planar curves in one plane | none | none | partly |
| Revolve | Select curves; axis start; axis end (Enter: construction plane Z); angles | `DeleteInput` `FullCircle` `AskForStartAngle` `Deformable` `PointCount` `SplitAtTangents` | none | partly |
| Loft | Select curves in order; adjust seams; Enter; loft options | `Point` `Style` `Closed` `MatchStartTangent` `MatchEndTangent` `SplitAtTangents` `AlignCurves` `Rebuild` `Refit` | none | partly |
| Sweep1 | Rail; cross sections in order; sweep options | `ChainEdges` `Point` `FrameStyle` `Closed` `GlobalShapeBlending` `UntrimmedMiters` `RefitRail` `AlignCrossSections` | none | partly |
| Pipe | Select a curve; start radius; end radius; more radii; Enter | `ChainEdges` `Multiple` `Diameter` `Thick` `Cap` `FitRail` `ShapeBlending` `WallThickness` | `Cap` `Thick` | partly |
| OffsetSrf | Select a surface or solid; distance | `Distance` `Corner` `ThroughPoint` `FlipAll` `Solid` `Loose` `Tolerance` `BothSides` `DeleteInput` | `Solid` | partly |
| Box | First corner of base; other corner or length; height, Enter uses the width | `Diagonal` `Cube` `3Point` `EdgeMidpoint` `Vertical` `Center` `BothSides` | `Center` `BothSides` | partly |
| BooleanUnion | Select objects; Enter | `DeleteInput` `MergeCoplanarFaces` | `DeleteInput` | partly |
| BooleanDifference | Objects to subtract from; Enter; objects to subtract with; Enter | `DeleteInput` `DeleteCutters` | `DeleteInput` | partly |
| BooleanIntersection | First set; Enter; second set; Enter | `DeleteInput` | `DeleteInput` | done |
| FilletEdge | Select edges; edit radius handles; Enter | `ShowRadius` `NextRadius` `ChainEdges` `FaceEdges` `PreviousEdgeSelection` `TrimAndJoin` `RailType` `AddHandle` `RemoveHandle` `SetAll` `LinkHandles` | none | partly |
| ChamferEdge | Select edges; edit distance handles; Enter | `ShowChamferDistance` `NextChamferDistance` `ChainEdges` `FaceEdges` `PreviousEdgeSelection` `TrimAndJoin` `RailType` `AddHandle` `RemoveHandle` `SetAll` `LinkHandles` | none | partly |
| Shell | Select faces to remove; Enter (on Rhino's OffsetSrf page) | `Thickness` `DeleteInput` `EditThickness` | none | partly |

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
| CPlane | New origin, or an option | `All` `Curve` `Elevation` `Gumball` `Object` `Rotate` `Surface` `Through` `View` `World` `3Point` `Undo` `Redo` | `World` | partly |
| Zoom | Drag a window (the default), or an option | `Window` `All` `Extents` `Selected` `1to1` `Dynamic` `Factor` `In` `Out` `Target` | `Extents` `Selected` `In` `Out` `Target` | partly |
| ZoomExtents | none | none | none | partly |
| ZoomSelected | none | none | none | partly |
| NamedView | A panel in Rhino: save, restore, delete views | `Save` `Restore` `Delete` | `Save` `Restore` `Delete` | partly |
| SetDisplayMode | Not read: Rhino's page redirected; complete from Rhino | to confirm | `Wireframe` `Shaded` `Ghosted` | partly |
| New | Choose a template | none | none | partly |
| Open | Choose a file | none | none | partly |
| Save | File name | `Version` `SaveSmall` `GeometryOnly` `SaveTextures` `SavePlugInData` `Browse` | none | partly |
