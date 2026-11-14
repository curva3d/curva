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
| ExtrudeCrv | Select curves; distance, typed or by two points | `Direction` `BothSides` `Solid` `DeleteInput` `ToBoundary` `SplitAtTangents` `SetBasePoint` | `Solid` `BothSides` `DeleteInput` | partly |
| PlanarSrf | Select planar curves in one plane | none | none | done |
| Revolve | Select curves; axis start; axis end (Enter: construction plane Z); angles | `DeleteInput` `FullCircle` `AskForStartAngle` `Deformable` `PointCount` `SplitAtTangents` | `DeleteInput` `FullCircle` | partly |
| Loft | Select curves in order; adjust seams; Enter; loft options | `Point` `Style` `Closed` `MatchStartTangent` `MatchEndTangent` `SplitAtTangents` `AlignCurves` `Rebuild` `Refit` | none | partly |
| Sweep1 | Rail; cross sections in order; sweep options | `ChainEdges` `Point` `FrameStyle` `Closed` `GlobalShapeBlending` `UntrimmedMiters` `RefitRail` `AlignCrossSections` | none | partly |
| Pipe | Select a curve; start radius; end radius; more radii; Enter | `ChainEdges` `Multiple` `Diameter` `Thick` `Cap` `FitRail` `ShapeBlending` `WallThickness` | `Cap` `Thick` `Diameter` | partly |
| OffsetSrf | Select a surface or solid; distance | `Distance` `Corner` `ThroughPoint` `FlipAll` `Solid` `Loose` `Tolerance` `BothSides` `DeleteInput` | `Solid` `DeleteInput` | partly |
| Box | First corner of base; other corner or length; height, Enter uses the width | `Diagonal` `Cube` `3Point` `EdgeMidpoint` `Vertical` `Center` `BothSides` | `Center` `BothSides` | partly |
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
| Open | Choose a file | none | none | partly |
| Save | File name | `Version` `SaveSmall` `GeometryOnly` `SaveTextures` `SavePlugInData` `Browse` | none | partly |
| SaveAs | Asks for a file name; saves there and keeps saving there | none | none | partly |
| Import | Choose a file (.3dm or .curva); its contents join the model, layers merged by name | none | none | partly |
| Export | Choose a file; the model is written as .3dm | none | none | partly |
| ExportSelected | Select objects; choose a file; they are written as .3dm | none | none | partly |

## Added for Rhino parity (after 0.3.0)

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Sphere | Center; radius | `2Point` `3Point` `Tangent` `AroundCurve` `4Point` `FitPoints` `Diameter` | `2Point` `Diameter` | partly |
| Cylinder | Base center; radius; end of cylinder | `DirectionConstraint` `Solid` `2Point` `3Point` `Tangent` `FitPoints` `Diameter` `BothSides` | `Diameter` `Solid` `BothSides` | partly |
| Cone | Base center; radius; end of cone | `DirectionConstraint` `Solid` `2Point` `3Point` `Tangent` `FitPoints` `Diameter` | `Diameter` | partly |
| TruncatedCone | Base center; radius; end; top radius | `DirectionConstraint` `Solid` `2Point` `3Point` `Tangent` `FitPoints` | none | partly |
| Torus | Center; radius; second radius | `Vertical` `2Point` `3Point` `Tangent` `AroundCurve` `Diameter` `FixInnerRadius` | none | partly |
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
| Align | Select objects; how | `Bottom` `HorizontalCenter` `Left` `Right` `Top` `VerticalCenter` `ToPoint` `ToLine` | `Bottom` `Top` `Left` `Right` `HorizontalCenter` `VerticalCenter` | partly |
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
| Layouts | Opens the Layouts panel: New Layout, Duplicate Layout, Delete, Move Up, Move Down (or drag), Properties, Print, Import, Search; right-click Set Current, Rename, New Detail | none | none | missing |
| Detail | Manages details on a layout; the prompt order is not on the page. Add draws the rectangle; Scale asks the length on the layout, then in the model | `Add` `Center` `Projection` `Enable` `FromDetail` `FirstAngle` `ThirdAngle` `Scale` `Lock` `Unlock` `EnablePage` | none | missing |
