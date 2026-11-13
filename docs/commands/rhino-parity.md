# Rhino parity for NURBS modelling

The list of Rhino 8 commands for NURBS modelling (spec `docs/specs/rhino-parity.md`), by group, with
Curva's status:

- `done`: in Curva, with Rhino's steps and options (details in `rhino-reference.md`)
- `partly`: in Curva, but different from Rhino: the note says how (options still to come, or an
  approximation), with details in `rhino-reference.md`
- `missing`: not in Curva yet; the framework and the engine can do it
- `blocked`: the geometry engine (solidcore) cannot do it yet; it waits for the engine

Out of scope, as other specializations or later milestones: meshes and SubD, rendering and materials,
annotation and dimensions, layouts and printing, file formats (Milestone 2), scripting and
Grasshopper. A test (`rhino_parity_lists_only_real_commands`) checks that every command marked `done` or
`partly` exists in Curva.

Written on 12 November 2026 from the Rhino 8 command list.

## Curves: drawing

| Rhino command | Status | Note |
|---|---|---|
| Line | partly | Options to come: `Normal`, `Angled`, `Vertical`, `FourPoint`, `Bisector`, `Perpendicular`, `Tangent`, `Extension` |
| Lines | partly | Options to come: `BothSides` |
| Polyline | partly | Options to come: `Helpers`, `Length`, `Mode`, `Direction`, `ThroughPoint`, `Center`, `PersistentClose` |
| Curve | partly | Options to come: `AutoClose`, `PersistentClose`, `Sharp` |
| InterpCrv | partly | Options to come: `AutoClose`, `Degree`, `PersistentClose`, `Knots`, `StartTangent`, `EndTangent`, `Sharp` |
| InterpCrvOnSrf | done | Fitted through points of the surface along the curve |
| HandleCurve | partly | Handles are placed by clicking, not dragging; `Close` to come |
| Sketch | done |  |
| Circle | partly | Options to come: `Deformable`, `Radius`, `AroundCurve`, `FitPoints`, `Tangent`, `Vertical`, `Orientation`, `Circumference`, `Area` |
| Arc | partly | Options to come: `Center`, `Tilted`, `Length`, `Midpoint`, `Deformable`, `Direction`, `ThroughPoint`, `Tangent`, `Extension` |
| Ellipse | partly | Options to come: `Deformable`, `Vertical`, `FromFoci`, `MarkFoci`, `AroundCurve` |
| Rectangle | partly | Options to come: `EdgeMidpoint`, `Vertical`, `AroundCurve`, `Rounded` |
| Polygon | partly | Options to come: `Edge`, `Star`, `Vertical`, `AroundCurve` |
| Point | done | |
| Points | partly | Options to come: `MarkPoints` |
| Helix | partly | Options to come: `Vertical`, `AroundCurve`, `Pitch`, `ReverseTwist`, `Diameter` |
| Spiral | partly | Options to come: `Vertical`, `AroundCurve`, `Pitch`, `ReverseTwist`, `Diameter` |
| Conic | partly | Rho is typed; `Perpendicular`, `Tangent` and a point to pass through to come |
| Parabola | partly | Options to come: `Vertex`, `Marks`, `FocusMarks`, `HalfParabola` |
| Hyperbola | partly | From the centre only; `FromFocus`, `Coefficients`, `Vertices` and `Marks` to come |
| Slot | partly | Options to come: `Vertical` |
| TweenCurves | partly | Options to come: `Number`, `OutputLayer`, `MatchMethod`, `SampleNumber`, `Flip` |
| BlendCrv | partly | Joins the nearest ends; `G3`, `G4` and `Edges` to come |
| Divide | partly | Options to come: `Length`, `EqualChordLength`, `Split`, `DeleteRemainder`, `MarkEnds`, `GroupOutput` |
| Offset | partly | Options to come: `Loose`, `Corner`, `Trim`, `Tolerance`, `InCPlane`, `Cap`, `OutputLayer` |
| OffsetMultiple | partly | Options to come: `Corner`, `BothSides` |
| OffsetCrvOnSrf | partly | The offset curve is refitted through 49 points on the surface |
| Fillet | partly | Options to come: `Join`, `Trim`, `ExtendArcsBy`, `ExtendOtherCurvesBy`, `Dynamic` |
| FilletCorners | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Chamfer | partly | Options to come: `Join`, `Trim`, `ExtendArcsBy`, `ExtendOtherCurvesBy` |
| Project | partly | Along the construction plane's normal; `Loose` and `DeleteInput` to come |
| Pull | partly | The curve refitted through pulled points (48 a curve) |
| ProjectToCPlane | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Contour | partly | Options to come: `GroupOutput`, `JoinCurves` |
| Section | done |  |
| Intersect | done |  |
| DupEdge | done | |
| DupBorder | done |  |
| Silhouette | done | Seen down the construction plane's normal, from the display mesh |
| ExtractIsocurve | partly | Faces with a NURBS form only (planes and splines); `Toggle` to come |

## Curves: editing

| Rhino command | Status | Note |
|---|---|---|
| Join | partly | Options to come: `Undo` |
| Explode | done | |
| Trim | partly | Options to come: `ExtendCuttingLines`, `ApparentIntersections`, `Line` |
| Split | partly | Options to come: `Point`, `Isocurve`, `Shrink` |
| Extend | done |  |
| Rebuild | partly | Options to come: `PointCount`, `Degree`, `DeleteInput`, `CurrentLayer`, `PreserveTangents` |
| ChangeDegree | partly | Lowering the degree keeps the control points instead of fitting the old shape |
| InsertKnot | partly | Options to come: `Automatic`, `Midpoints`, `Symmetrical`, `Undo`, `Direction`, `Toggle` |
| RemoveKnot | partly | Options to come: `Direction`, `Toggle`, `Undo` |
| InsertControlPoint | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| RemoveControlPoint | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| PointsOn | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| PointsOff | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SetPt | partly | Options to come: `Copy` |
| Smooth | partly | Options to come: `SmoothX`, `SmoothY`, `SmoothZ`, `FixBoundaries`, `CoordinateSystem`, `SmoothFactor`, `Steps` |
| Fair | done |  |
| Dir | partly | Options to come: `FlipAll`, `UReverse`, `VReverse`, `SwapUV` |
| Flip | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| CloseCrv | partly | Options to come: `CloseWideGapsWithLine`, `Tolerance` |
| CurveBoolean | partly | Options to come: `AllRegions` |
| Match | partly | Matches the nearest ends; weighted curves match only approximately; `AverageCurves` and `PreserveOtherEnd` to come |
| SimplifyCrv | done |  |
| Reparameterize | partly | Lines and arcs become NURBS curves to take a range; `Automatic` to come |
| MakeUniform | done |  |
| Weight | done |  |

## Surfaces

| Rhino command | Status | Note |
|---|---|---|
| PlanarSrf | done | |
| Plane | partly | Options to come: `Deformable`, `Vertical`, `Center`, `3Point` |
| SrfPt | done | |
| EdgeSrf | done |  |
| ExtrudeCrv | partly | Options to come: `Direction`, `ToBoundary`, `SplitAtTangents`, `SetBasePoint` |
| ExtrudeCrvTapered | partly | Options to come: `Direction`, `DeleteInput`, `FlipAngle` |
| ExtrudeCrvAlongCrv | partly | Options to come: `DeleteInput`, `SubCurve` |
| ExtrudeCrvToPoint | partly | Smooth curves close to a tip a ten-thousandth of their size, not a single point; `DeleteInput` to come |
| Revolve | partly | Options to come: `AskForStartAngle`, `Deformable`, `PointCount`, `SplitAtTangents` |
| RailRevolve | partly | The surface passes through the profile at 72 angles and is skinned between them; `ScaleHeight` to come |
| Loft | partly | Options to come: `Point`, `Style`, `Closed`, `MatchStartTangent`, `MatchEndTangent`, `SplitAtTangents`, `AlignCurves`, `Rebuild`, `Refit` |
| Sweep1 | partly | Options to come: `ChainEdges`, `Point`, `FrameStyle`, `Closed`, `GlobalShapeBlending`, `UntrimmedMiters`, `RefitRail`, `AlignCrossSections` |
| Sweep2 | partly | One cross section, placed exactly at 64 stations and skinned between; `MaintainHeight` to come |
| NetworkSrf | partly | Sorts the curves into their two families itself; `EdgeMatching` and `Tolerances` to come; rational curves (arcs) are refused by the engine |
| Patch | partly | Fills a closed chain of curves; points, `UVSpans`, `Stiffness` and the other options to come |
| BlendSrf | partly | Edges that are sides of untrimmed surfaces only; `Planar Sections` to come |
| Pipe | partly | Options to come: `ChainEdges`, `Multiple`, `FitRail`, `ShapeBlending`, `WallThickness` |
| OffsetSrf | partly | Options to come: `Distance`, `Corner`, `ThroughPoint`, `FlipAll`, `Loose`, `Tolerance`, `BothSides` |
| ExtendSrf | done |  |
| MatchSrf | partly | Untrimmed edges, position and tangency; `Curvature` and `AverageSurfaces` to come |
| FilletSrf | partly | Arcs placed at 25 stations along the spine and skinned; the surfaces are not trimmed back yet |
| ChamferSrf | partly | As FilletSrf, with straight sections |
| Untrim | partly | Options to come: `KeepTrimObjects`, `All` |
| ShrinkTrimmedSrf | done |  |
| SplitSrf | done |  |
| RebuildUV | partly | Untrimmed one-face surfaces only; `DeleteInput` to come |
| UnrollSrf | partly | Ruled developable surfaces only; outlines unrolled as polylines of 96 steps |

## Solids

| Rhino command | Status | Note |
|---|---|---|
| Box | partly | Options to come: `Diagonal`, `Cube`, `3Point`, `EdgeMidpoint`, `Vertical` |
| Sphere | partly | Options to come: `3Point`, `Tangent`, `AroundCurve`, `4Point`, `FitPoints` |
| Cylinder | partly | Options to come: `DirectionConstraint`, `2Point`, `3Point`, `Tangent`, `FitPoints` |
| Cone | partly | Options to come: `DirectionConstraint`, `Solid`, `2Point`, `3Point`, `Tangent`, `FitPoints` |
| TruncatedCone | partly | Options to come: `DirectionConstraint`, `Solid`, `2Point`, `3Point`, `Tangent`, `FitPoints` |
| Torus | partly | Options to come: `Vertical`, `2Point`, `3Point`, `Tangent`, `AroundCurve`, `Diameter`, `FixInnerRadius` |
| Tube | partly | Options to come: `DirectionConstraint`, `Diameter`, `WallThickness`, `Cap` |
| Pyramid | partly | Options to come: `Star`, `Edge`, `Vertical` |
| Ellipsoid | partly | Options to come: `Corner`, `Diameter`, `FromFoci`, `AroundCurve` |
| Paraboloid | partly | Options to come: `Vertex`, `MarkFocus`, `Solid` |
| ExtrudeSrf | partly | Straight out of the construction plane; `Direction`, `Solid=No`, `ToBoundary` and `SetBasePoint` to come |
| Cap | done | Curva's own (`planar-capping`): solidcore leaves capped shells hollow |
| BooleanUnion | partly | Options to come: `MergeCoplanarFaces` |
| BooleanDifference | partly | Options to come: `DeleteCutters` |
| BooleanIntersection | done | |
| BooleanSplit | done | |
| FilletEdge | partly | Options to come: `ShowRadius`, `NextRadius`, `ChainEdges`, `FaceEdges`, `PreviousEdgeSelection`, `TrimAndJoin`, `RailType`, `AddHandle`, `RemoveHandle`, `SetAll`, `LinkHandles` |
| ChamferEdge | partly | Options to come: `ShowChamferDistance`, `NextChamferDistance`, `ChainEdges`, `FaceEdges`, `PreviousEdgeSelection`, `TrimAndJoin`, `RailType`, `AddHandle`, `RemoveHandle`, `SetAll`, `LinkHandles` |
| Shell | partly | Options to come: `Thickness`, `EditThickness` |
| ExtractSrf | done |  |
| MergeAllFaces | done |  |
| MoveFace | partly | Moves faces along their normals by a distance, not by any vector |
| PushPull | done | |
| ExtrudeFace | partly | Flat faces along their normal |
| Wirecut | partly | Cuts straight through out of the construction plane; `Direction`, `BothSides` and `DeleteInput` to come |

## Transforming

| Rhino command | Status | Note |
|---|---|---|
| Move | partly | Options to come: `Normal` |
| Copy | partly | Options to come: `FromLastPoint`, `UseLastDistance`, `UseLastDirection` |
| Rotate | partly | Options to come: `UseLastCenter` |
| Rotate3D | partly | Options to come: `SurfaceNormal`, `UseLastAxis`, `Copy` |
| Scale | partly | Options to come: `Rigid` |
| Scale1D | partly | Options to come: `Copy`, `Rigid` |
| Scale2D | partly | Options to come: `Copy`, `Rigid` |
| ScaleNU | partly | Options to come: `Rigid`, `WorldCoordinates` |
| Mirror | partly | Options to come: `3Point`, `ZAxis`, `Object` |
| Orient | partly | Options to come: `Copy`, `Scale` |
| Orient3Pt | partly | Options to come: `Copy`, `Scale` |
| Array | partly | Options to come: `Mode`, `Number`, `XNumber`, `YNumber`, `ZNumber`, `Preview`, `Spacing`, `XSpacing`, `YSpacing`, `ZSpacing` |
| ArrayPolar | partly | Options to come: `Axis`, `UseLastAxis`, `Preview`, `StepAngle`, `Rotate`, `ZOffset` |
| ArrayCrv | partly | Options to come: `Basepoint`, `Items`, `Distance`, `Orientation` |
| ArrayLinear | done | |
| Align | partly | Options to come: `ToPoint`, `ToLine` |
| Distribute | done |  |
| Shear | partly | Options to come: `Copy` |
| Twist | partly | Surfaces are refined and their control points moved (exact for flat faces, close for curved ones); `Infinite` to come |
| Bend | partly | Bends along a circular arc from the spine's start; control points moved as for Twist; `Symmetric` to come |
| Taper | partly | Sizes as a factor at the axis's end; `Flat` and `Infinite` to come |
| Flow | partly | Stretches along the target curve; `Rigid` and `Copy` to come |
| CageEdit | partly | A box cage moved by its eight corners, by clicks |

## Selecting, showing and grouping

| Rhino command | Status | Note |
|---|---|---|
| SelAll | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelNone | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Invert | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelCrv | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelLine | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelPolyline | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelCircle | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelArc | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelClosedCrv | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelOpenCrv | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelSrf | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelSolid | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelPt | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelName | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelLayer | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelDup | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelLast | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelPrev | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SelColor | partly | Picks the colour from an object (its layer's), not from a colour list |
| SelFilter | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Hide | partly | Options to come: `-Hide` |
| Show | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| HideSwap | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Isolate | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Lock | partly | Not yet on the command framework |
| Unlock | partly | Not yet on the command framework |
| Group | partly | Not yet on the command framework; groups do not nest yet |
| Ungroup | partly | Not yet on the command framework |
| Rename | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| History | partly | Options to come: `Record`, `Update`, `Lock`, `BrokenHistoryWarning` |

## Analysing and measuring

| Rhino command | Status | Note |
|---|---|---|
| Distance | partly | Options to come: `Units` |
| Length | partly | Options to come: `Units` |
| Area | partly | Options to come: `Units` |
| Volume | partly | Options to come: `Units` |
| Angle | partly | Options to come: `TwoObjects` |
| Radius | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| EvaluatePt | done | |
| What | done | |
| BoundingBox | partly | Options to come: `CoordinateSystem`, `Cumulative`, `Output` |
| AreaCentroid | partly | A solid's area centre is worked out from its mesh; closed curves to come |
| VolumeCentroid | done | |
| CurvatureGraph | done |  |
| Zebra | partly | Stripes on the whole viewport, run again to stop; stripe direction, size and colour to come |
| ShowEdges | partly | Shows naked edges or all; `NonManifoldEdges` to come |

## Construction planes and views

| Rhino command | Status | Note |
|---|---|---|
| CPlane | partly | Options to come: `All`, `Curve`, `Elevation`, `Gumball`, `Object`, `Rotate`, `Surface`, `Through`, `View`, `3Point`, `Undo`, `Redo` |
| Top | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Front | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Right | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Perspective | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| 4View | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Zoom | partly | Options to come: `Window`, `All`, `1to1`, `Dynamic`, `Factor` |
| ZoomExtents | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| ZoomSelected | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| NamedView | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| UndoView | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| RedoView | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| SetDisplayMode | partly | Some of Rhino's steps differ (see `rhino-reference.md`) |
| Plan | partly | Not yet on the command framework |
