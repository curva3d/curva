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
| spline-toolkit | `curva-kernel/src/spline.rs` | Match, Weight, Reparameterize, MakeUniform, SimplifyCrv, HandleCurve, Conic, Hyperbola, the arc extension | `curve_knots::insert_knot`, `remove_knot`, `set_degree` and `curve::extend_curve` cover parts on record curves; reparameterizing, uniform knots, weights, reversing and end derivatives are not there (`9342f32` adds an arc about a centre, proportional extension and proportional CV stretching, still not these) | `9342f32` |
| surface-toolkit | `curva-kernel/src/surface.rs` (`NurbsSurface`) | BlendSrf, MatchSrf, ShrinkTrimmedSrf, ExtractIsocurve, the deformations | solidcore makes sheets from surfaces only inside itself (`surface::sheet` is private); `face_surface::FaceSurface` evaluates faces, which Curva uses where it can | `9342f32` |
| isocurves | `curva-kernel/src/surface_ops.rs` (`Body::isocurve_through`) | ExtractIsocurve | `face_surface::FaceSurface` finds the point (engine); no isocurve extraction | `9342f32` |
| shrink-trimmed | `curva-kernel/src/surface_ops.rs` (`Body::shrunk`) | ShrinkTrimmedSrf | none | `9342f32` |
| surface-blending | `curva-kernel/src/surface_ops.rs` (`Body::blend_to`) | BlendSrf | `blend_surface::blend_edges` (G0 to G3, crown) since `9342f32`, but its G0 blend between straight edges is a plane whose record does not read back (status 1007), so a model holding it would not reopen | `9342f32` |
| surface-matching | `curva-kernel/src/surface_ops.rs` (`Body::matched_to`) | MatchSrf | `surface_cvs::match_edge` (G0 to G2) since `9342f32`, but it refuses rational surfaces (cylinders, revolved surfaces) and flat faces, which MatchSrf takes | `9342f32` |
| space-deformation | `curva-kernel/src/surface_ops.rs` (`Body::deformed`, `Curve::deformed`) | Twist, Bend, Taper, Flow, CageEdit | `surface_cvs::move_net` moves one untrimmed surface's points; deforming whole trimmed bodies is Curva's | `9342f32` |
| curves-on-surfaces | `curva-kernel/src/surface_ops.rs` (`Body::pulled`, `curve_on_face`, `offset_on_surface`) | Pull, InterpCrvOnSrf, OffsetCrvOnSrf | `face_surface::FaceSurface` evaluates and finds nearest points (engine); the curves are fitted by Curva; `sweep::drop_curves` (since `9342f32`) drops curves onto one face's surface with its trims ignored, so Pull across a polysurface is Curva's | `9342f32` |
| silhouettes | `curva-kernel/src/surface_ops2.rs` (`Body::silhouette`) | Silhouette | none; Curva reads its display mesh | `9342f32` |
| unrolling | `curva-kernel/src/surface_ops2.rs` (`Body::unrolled`) | UnrollSrf | none | `9342f32` |
| face-extrusion | `curva-kernel/src/surface_ops2.rs` (`Body::face_extruded`) | ExtrudeFace | `offset_faces` moves faces; extruding one face's outline and joining it is Curva's (with the engine's extrusion and boolean) | `9342f32` |
| surface-fillets | `curva-kernel/src/surface_ops2.rs` (`Body::filleted_with`) | FilletSrf, ChamferSrf | `offset_sheet` and `split_faces` (engine) find the spine; the sections and skin are Curva's; `blend_surface::fillet_sheets` (since `9342f32`) rounds only sheets that meet along an edge, not ones that cross or stand apart as FilletSrf takes them | `9342f32` |
| face-assembly | `curva-kernel/src/assemble.rs` (`Body::assembled`) | ExtrudeSrf, the gumball's extrusion, RailRevolve's seam | `stitch::stitch` sews one body; merging several bodies' records into one is Curva's | `9342f32` |
| straight-extrusion | `curva-kernel/src/assemble.rs` (`Body::extruded_straight`) | ExtrudeSrf, the gumball | `thicken` and `offset_sheet` push along the normal, not along a direction | `9342f32` |
| cone-to-point | `curva-kernel/src/making2.rs` (`Body::extruded_to_point`) | ExtrudeCrvToPoint for smooth curves | `loft_to_point` takes polygons of lines only; `surface::ruled` takes two curves | `9342f32` |
| rail-revolve | `curva-kernel/src/making2.rs` (`Body::rail_revolved`) | RailRevolve | none | `9342f32` |
| arc-extension | `curva-commands/src/commands/curves/extend.rs` (`extended`) | Extend with `Arc` | `curve::extend_curve` does natural and straight extensions only | `9342f32` |
| curve-fairing | `curva-commands/src/commands/curves/fair.rs` (`Fair`) | Fair | none | `9342f32` |
| curve-boolean-regions | `curva-commands/src/commands/curves/curve_boolean.rs` (`regions`) | CurveBoolean, Hatch with `Boundary` | `boolean::boolean2d` combines planar sheets; picking regions by clicks is Curva's | `9342f32` |
| curve-matching | `curva-commands/src/commands/curves/match_curve.rs` (`matched_end`) | Match | none | `9342f32` |
| curve-simplifying | `curva-commands/src/commands/curves/simplify_crv.rs` (`simpler`) | SimplifyCrv | none | `9342f32` |
| planar-capping | `curva-kernel/src/assemble.rs` (`Body::capped`) | Cap | `stitch` closes the shell but leaves no solid region inside it (tangency-labs/solidcore#3266); Curva fills and sews with `face-assembly` | `9342f32` |
| mesh-area-centroid | `curva-commands/src/commands/support.rs` (`area_centroid`) | AreaCentroid of solids | `mass_properties` gives a solid's volume centroid, not its area centroid | `9342f32` |
| hatch-patterns | `curva-document/src/hatch.rs` (`pattern_pieces`) | Hatch | none: `cell_grid` fills a field with closed cells, not lines; `prism::profile::Loop2::holds` tests one point. No Rust crate reads `.pat` files | `9342f32` |

## Moved to solidcore at the 0.4.2 check (`377457d`)

- BlendCrv: solidcore `curve::blend_curve` replaced Curva's quintic blend.
- Extend `Line` and `Smooth`: solidcore `curve::extend_curve` replaced Curva's.
- ChangeDegree (raising, and lowering within 0.01): solidcore `curve_knots::set_degree`.
- Taking faces out (ExtractSrf): solidcore `trim::remove_faces` replaced Curva's record subset.

## Moved to solidcore at the second 0.4.2 check (`9342f32`)

- Sweep2: solidcore `sweep::birail::sweep_two_rails` (scaled to span the rails, certified within
  0.001) replaced Curva's stations and skin. Curva only turns the rails to run from the profile.
- Looked at and kept: BlendSrf, MatchSrf, FilletSrf and Pull have solidcore counterparts since
  `9342f32` that do not yet cover what Rhino's commands take; the rows above say why.
