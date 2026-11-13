# Custom kernel operations

Geometry Curva does itself because solidcore cannot do it yet (spec `docs/specs/engine-gaps.md`,
decision 0019). Each row has a `// CUSTOM KERNEL OPERATION: <name>` mark on the code that does it; a
test (`custom_operations_match_their_list`) fails if a mark has no row or a row no mark.

**Before every release** (`docs/releasing.md`): move solidcore to its latest revision on a branch,
and for each row check whether solidcore now covers the operation fully (the operation's tests pass
on solidcore's). Where it does, switch to solidcore's, delete Curva's code and the row, and say so in
the release notes. Then update "Checked at".

| Operation | Code | Used by | solidcore today | Checked at |
|---|---|---|---|---|
| spline-toolkit | `curva-kernel/src/spline.rs` | Match, Weight, Reparameterize, MakeUniform, SimplifyCrv, HandleCurve, Conic, Hyperbola, the arc extension | `curve_knots::insert_knot`, `remove_knot`, `set_degree` and `curve::extend_curve` cover parts on record curves; reparameterizing, uniform knots, weights, reversing and end derivatives are not there | `377457d` |
| surface-toolkit | `curva-kernel/src/surface.rs` (`NurbsSurface`) | BlendSrf, MatchSrf, ShrinkTrimmedSrf, ExtractIsocurve, the deformations | solidcore makes sheets from surfaces only inside itself (`surface::sheet` is private); `face_surface::FaceSurface` evaluates faces, which Curva uses where it can | `377457d` |
| isocurves | `curva-kernel/src/surface_ops.rs` (`Body::isocurve_through`) | ExtractIsocurve | `face_surface::FaceSurface` finds the point (engine); no isocurve extraction | `377457d` |
| shrink-trimmed | `curva-kernel/src/surface_ops.rs` (`Body::shrunk`) | ShrinkTrimmedSrf | none | `377457d` |
| surface-blending | `curva-kernel/src/surface_ops.rs` (`Body::blend_to`) | BlendSrf | none (`surface::n_sided` fills, but does not blend between two edges) | `377457d` |
| surface-matching | `curva-kernel/src/surface_ops.rs` (`Body::matched_to`) | MatchSrf | none | `377457d` |
| two-rail-sweep | `curva-kernel/src/surface_ops.rs` (`Body::swept_two_rails`) | Sweep2 | `sweep::sweep_stations` skins the placed profiles (engine); placing them along two rails is Curva's | `377457d` |
| space-deformation | `curva-kernel/src/surface_ops.rs` (`Body::deformed`, `Curve::deformed`) | Twist, Bend, Taper, Flow, CageEdit | `surface_cvs::move_net` moves one untrimmed surface's points; deforming whole trimmed bodies is Curva's | `377457d` |
| curves-on-surfaces | `curva-kernel/src/surface_ops.rs` (`Body::pulled`, `curve_on_face`, `offset_on_surface`) | Pull, InterpCrvOnSrf, OffsetCrvOnSrf | `face_surface::FaceSurface` evaluates and finds nearest points (engine); the curves are fitted by Curva | `377457d` |
| face-assembly | `curva-kernel/src/assemble.rs` (`Body::assembled`) | ExtrudeSrf, the gumball's extrusion, RailRevolve's seam | `stitch::stitch` sews one body; merging several bodies' records into one is Curva's | `377457d` |
| straight-extrusion | `curva-kernel/src/assemble.rs` (`Body::extruded_straight`) | ExtrudeSrf, the gumball | `thicken` and `offset_sheet` push along the normal, not along a direction | `377457d` |
| cone-to-point | `curva-kernel/src/making2.rs` (`Body::extruded_to_point`) | ExtrudeCrvToPoint for smooth curves | `loft_to_point` takes polygons of lines only; `surface::ruled` takes two curves | `377457d` |
| rail-revolve | `curva-kernel/src/making2.rs` (`Body::rail_revolved`) | RailRevolve | none | `377457d` |
| arc-extension | `curva-commands/src/commands/parity4.rs` (`extended`) | Extend with `Arc` | `curve::extend_curve` does natural and straight extensions only | `377457d` |
| curve-fairing | `curva-commands/src/commands/parity4.rs` (`Fair`) | Fair | none | `377457d` |
| curve-boolean-regions | `curva-commands/src/commands/parity4.rs` (`CurveBoolean`) | CurveBoolean | `boolean::boolean2d` combines planar sheets; picking regions by clicks is Curva's | `377457d` |
| curve-matching | `curva-commands/src/commands/parity4.rs` (`matched_end`) | Match | none | `377457d` |
| curve-simplifying | `curva-commands/src/commands/parity4.rs` (`simpler`) | SimplifyCrv | none | `377457d` |
| planar-capping | `curva-kernel/src/assemble.rs` (`Body::capped`) | Cap | `stitch` closes the shell but leaves no solid region inside it (tangency-labs/solidcore#3266); Curva fills and sews with `face-assembly` | `377457d` |
| mesh-area-centroid | `curva-commands/src/commands/parity3.rs` (`area_centroid`) | AreaCentroid of solids | `mass_properties` gives a solid's volume centroid, not its area centroid | `377457d` |

## Moved to solidcore at the 0.4.2 check (`377457d`)

- BlendCrv: solidcore `curve::blend_curve` replaced Curva's quintic blend.
- Extend `Line` and `Smooth`: solidcore `curve::extend_curve` replaced Curva's.
- ChangeDegree (raising, and lowering within 0.01): solidcore `curve_knots::set_degree`.
- Taking faces out (ExtractSrf): solidcore `trim::remove_faces` replaced Curva's record subset.
