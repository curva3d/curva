# Rhino parity for NURBS modelling

The list of Rhino 8 commands for NURBS modelling (spec `docs/specs/rhino-parity.md`), by group, with
Curva's status:

- `done`: in Curva, with Rhino's steps and options (details in `rhino-reference.md`)
- `partly`: in Curva, some options still to come (details in `rhino-reference.md`)
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
| Line | partly | |
| Lines | partly | |
| Polyline | partly | |
| Curve | partly | |
| InterpCrv | partly | |
| InterpCrvOnSrf | blocked | needs curves on surfaces |
| HandleCurve | missing | |
| Sketch | missing | |
| Circle | partly | |
| Arc | partly | |
| Ellipse | partly | |
| Rectangle | partly | |
| Polygon | partly | |
| Point | done | |
| Points | partly | |
| Helix | partly | |
| Spiral | partly | |
| Conic | missing | |
| Parabola | partly | |
| Hyperbola | missing | |
| Slot | partly | |
| TweenCurves | partly | |
| BlendCrv | missing | |
| Divide | partly | |
| Offset | partly | |
| OffsetMultiple | missing | |
| OffsetCrvOnSrf | blocked | needs curves on surfaces |
| Fillet | partly | |
| FilletCorners | partly | |
| Chamfer | partly | |
| Project | blocked | needs projection onto surfaces |
| Pull | blocked | needs projection onto surfaces |
| ProjectToCPlane | partly | |
| Contour | blocked | needs plane–surface intersections |
| Section | blocked | needs plane–surface intersections |
| Intersect | blocked | needs surface intersections |
| DupEdge | done | |
| DupBorder | missing | |
| Silhouette | blocked | needs silhouettes from the engine |
| ExtractIsocurve | blocked | needs surface isocurves |

## Curves: editing

| Rhino command | Status | Note |
|---|---|---|
| Join | partly | |
| Explode | done | |
| Trim | partly | |
| Split | partly | |
| Extend | missing | |
| Rebuild | partly | |
| ChangeDegree | partly | |
| InsertKnot | partly | |
| RemoveKnot | partly | |
| InsertControlPoint | partly | |
| RemoveControlPoint | partly | |
| PointsOn | partly | |
| PointsOff | partly | |
| SetPt | partly | |
| Smooth | partly | |
| Fair | missing | |
| Dir | partly | |
| Flip | partly | |
| CloseCrv | partly | |
| CurveBoolean | missing | |
| Match | missing | |
| SimplifyCrv | missing | |
| Reparameterize | missing | |
| MakeUniform | missing | |
| Weight | missing | |

## Surfaces

| Rhino command | Status | Note |
|---|---|---|
| PlanarSrf | done | |
| Plane | partly | |
| SrfPt | done | |
| EdgeSrf | blocked | needs Coons patches |
| ExtrudeCrv | partly | |
| ExtrudeCrvTapered | missing | |
| ExtrudeCrvAlongCrv | missing | |
| ExtrudeCrvToPoint | missing | |
| Revolve | partly | |
| RailRevolve | missing | |
| Loft | partly | |
| Sweep1 | partly | |
| Sweep2 | blocked | needs two-rail sweeps |
| NetworkSrf | blocked | needs network surfaces |
| Patch | blocked | needs patch fitting |
| BlendSrf | blocked | needs surface blending |
| Pipe | partly | |
| OffsetSrf | partly | |
| ExtendSrf | blocked | needs surface extension |
| MatchSrf | blocked | needs surface matching |
| FilletSrf | blocked | needs surface fillets |
| ChamferSrf | blocked | needs surface chamfers |
| Untrim | blocked | needs trim removal |
| ShrinkTrimmedSrf | blocked | |
| SplitSrf | blocked | needs surface splitting |
| RebuildUV | blocked | |
| UnrollSrf | blocked | needs developing surfaces |

## Solids

| Rhino command | Status | Note |
|---|---|---|
| Box | partly | |
| Sphere | partly | |
| Cylinder | partly | |
| Cone | partly | |
| TruncatedCone | partly | |
| Torus | partly | |
| Tube | partly | |
| Pyramid | missing | |
| Ellipsoid | partly | |
| Paraboloid | partly | |
| ExtrudeSrf | partly | |
| Cap | blocked | the engine leaves capped solids hollow (tangency-labs/solidcore#3266) |
| BooleanUnion | partly | |
| BooleanDifference | partly | |
| BooleanIntersection | done | |
| BooleanSplit | done | |
| FilletEdge | partly | |
| ChamferEdge | partly | |
| Shell | partly | |
| ExtractSrf | missing | |
| MergeAllFaces | missing | |
| MoveFace | blocked | needs face editing |
| ExtrudeFace | blocked | needs face editing |
| Wirecut | missing | |

## Transforming

| Rhino command | Status | Note |
|---|---|---|
| Move | partly | |
| Copy | partly | |
| Rotate | partly | |
| Rotate3D | partly | |
| Scale | partly | |
| Scale1D | partly | |
| Scale2D | partly | |
| ScaleNU | partly | |
| Mirror | partly | |
| Orient | partly | |
| Orient3Pt | partly | |
| Array | partly | |
| ArrayPolar | partly | |
| ArrayCrv | partly | |
| ArrayLinear | done | |
| Align | partly | |
| Distribute | missing | |
| Shear | blocked | needs shear transforms in the engine |
| Twist | blocked | needs deformations |
| Bend | blocked | needs deformations |
| Taper | blocked | needs deformations |
| Flow | blocked | needs deformations |
| CageEdit | blocked | needs deformations |

## Selecting, showing and grouping

| Rhino command | Status | Note |
|---|---|---|
| SelAll | partly | |
| SelNone | partly | |
| Invert | partly | |
| SelCrv | partly | |
| SelLine | partly | |
| SelPolyline | partly | |
| SelCircle | partly | |
| SelArc | partly | |
| SelClosedCrv | partly | |
| SelOpenCrv | partly | |
| SelSrf | partly | |
| SelSolid | partly | |
| SelPt | partly | |
| SelName | partly | |
| SelLayer | partly | |
| SelDup | partly | |
| SelLast | partly | |
| SelPrev | partly | |
| SelColor | missing | |
| SelFilter | partly | |
| Hide | partly | |
| Show | partly | |
| HideSwap | partly | |
| Isolate | partly | |
| Lock | missing | |
| Unlock | missing | |
| Group | missing | |
| Ungroup | missing | |
| Rename | partly | |
| History | partly | |

## Analysing and measuring

| Rhino command | Status | Note |
|---|---|---|
| Distance | partly | |
| Length | partly | |
| Area | partly | |
| Volume | partly | |
| Angle | partly | |
| Radius | partly | |
| EvaluatePt | done | |
| What | done | |
| BoundingBox | partly | |
| AreaCentroid | partly | |
| VolumeCentroid | done | |
| CurvatureGraph | blocked | needs curvature from the engine |
| Zebra | blocked | needs analysis display |
| ShowEdges | missing | |

## Construction planes and views

| Rhino command | Status | Note |
|---|---|---|
| CPlane | partly | |
| Top | partly | |
| Front | partly | |
| Right | partly | |
| Perspective | partly | |
| 4View | partly | |
| Zoom | partly | |
| ZoomExtents | partly | |
| ZoomSelected | partly | |
| NamedView | partly | |
| UndoView | partly | |
| RedoView | partly | |
| SetDisplayMode | partly | |
| Plan | missing | |
