//! solidcore operations Curva uses as they are: surfaces from curves (boundary, network, patch,
//! ruled), extending, untrimming and splitting surfaces, sections, intersections, projection,
//! moving faces, curve blends, extensions and curvature combs, control nets. None of these is
//! custom (spec `docs/specs/engine-gaps.md` §1): the engine does the work.

use crate::body::{Body, failed};
use crate::record_io::{from_record_curve, to_record_curve, to_record_spline};
use crate::{Curve, KernelError, Point};
use solidcore_record::brep::Curve as RecordCurve;

fn records(curves: &[Curve]) -> Result<Vec<RecordCurve>, KernelError> {
    curves.iter().map(to_record_curve).collect()
}

/// A curve in the engine's sweep form (lines, arcs, NURBS) as a Curva curve, over `interval`
/// of its own parameter.
fn from_sweep_curve(
    curve: &solidcore_ops::sweep::Curve,
    interval: [f64; 2],
) -> Result<Curve, KernelError> {
    use solidcore_ops::sweep::Curve as S;
    match curve {
        S::Line { start, end } => from_record_curve(
            &RecordCurve::Line {
                start: *start,
                end: *end,
            },
            Some(interval),
        ),
        S::Arc {
            center,
            axis,
            reference,
            radius,
            ..
        } => {
            let x = crate::unit(*reference).unwrap_or([1.0, 0.0, 0.0]);
            let y = crate::cross(crate::unit(*axis).unwrap_or([0.0, 0.0, 1.0]), x);
            from_record_curve(
                &RecordCurve::Circle {
                    frame: solidcore_record::brep::Frame {
                        origin: *center,
                        x_axis: x,
                        y_axis: y,
                    },
                    radius: *radius,
                },
                Some(interval),
            )
        }
        S::Nurbs { points, knots, .. } => from_record_curve(
            &RecordCurve::RationalSpline {
                knots: knots.clone(),
                homogeneous: points.clone(),
            },
            Some(interval),
        ),
    }
}

fn from_sweep(
    pieces: Vec<(solidcore_ops::sweep::Curve, [f64; 2])>,
) -> Result<Vec<Curve>, KernelError> {
    pieces
        .iter()
        .map(|(curve, interval)| from_sweep_curve(curve, *interval))
        .collect()
}

fn sweep_records(curves: &[Curve]) -> Result<Vec<solidcore_ops::sweep::Curve>, KernelError> {
    let mut out = Vec::new();
    for curve in curves {
        out.extend(crate::making::sweep_pieces(curve)?);
    }
    Ok(out)
}
/// How a surface or curve carries on past its edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    /// The surface or curve itself, continued.
    Natural,
    /// Tangent continuous.
    Tangent,
    /// Curvature continuous.
    Curvature,
}

/// How a blend meets a curve's end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contact {
    /// In position.
    Position,
    /// Along the tangent.
    Tangency,
    /// In curvature.
    Curvature,
}

impl Body {
    /// A surface bounded by three or four curves meeting end to end (Rhino's EdgeSrf), or more
    /// through an n-sided fill.
    pub fn from_edges(curves: &[Curve]) -> Result<Self, KernelError> {
        let made = solidcore_ops::surface::boundary(&records(curves)?)
            .map_err(|e| failed("making a surface from edges", e))?;
        Self::from_record(&made)
    }

    /// A surface through two crossing families of curves (Rhino's NetworkSrf, a Gordon
    /// surface), and how far it strays from them.
    pub fn from_network(us: &[Curve], vs: &[Curve]) -> Result<(Self, f64), KernelError> {
        let (made, off) = solidcore_ops::network::gordon(&records(us)?, &records(vs)?)
            .map_err(|e| failed("making a network surface", e))?;
        Ok((Self::from_record(&made)?, off))
    }

    /// A surface filling a closed chain of curves (Rhino's Patch), and how far it strays.
    pub fn patched(curves: &[Curve]) -> Result<(Self, f64), KernelError> {
        let (made, off) = solidcore_ops::surface::n_sided(&records(curves)?)
            .map_err(|e| failed("patching", e))?;
        Ok((Self::from_record(&made)?, off))
    }

    /// The ruled surface between two curves, straight lines joining the same fraction of each.
    pub fn ruled(a: &Curve, b: &Curve, reversed: bool) -> Result<Self, KernelError> {
        let made =
            solidcore_ops::surface::ruled(&to_record_curve(a)?, &to_record_curve(b)?, reversed)
                .map_err(|e| failed("making a ruled surface", e))?;
        Self::from_record(&made)
    }

    /// The surface extended past its edge `edge` by `distance` (Rhino's ExtendSrf).
    pub fn extended_at(&self, edge: usize, distance: f64, how: Carry) -> Result<Self, KernelError> {
        use solidcore_ops::extend::Continuation;
        let (body, order) = self.engine_with_order()?;
        let how = match how {
            Carry::Natural => Continuation::Natural,
            Carry::Tangent => Continuation::Tangent,
            Carry::Curvature => Continuation::Curvature,
        };
        let made = solidcore_ops::extend::extend_sheet(&body, &order, edge, distance, how)
            .map_err(|e| failed("extending the surface", e))?;
        Self::made(&made)
    }

    /// The faces the edge `edge` borders.
    pub fn faces_of_edge(&self, edge: usize) -> Result<Vec<usize>, KernelError> {
        let record = self.record()?;
        let edge = record
            .edges
            .get(edge)
            .ok_or_else(|| KernelError(format!("the body has no edge {edge}")))?;
        Ok(edge.uses.iter().map(|used| used.face as usize).collect())
    }

    /// The trim the edge `edge` lies on taken away (Rhino's Untrim): a hole filled, or, for an
    /// edge of a face's outer boundary, the face back to its whole surface.
    pub fn untrimmed_at(&self, edge: usize) -> Result<Self, KernelError> {
        use solidcore_ops::untrim::{Holes, untrim_face};
        let record = self.record()?;
        let used = record
            .edges
            .get(edge)
            .and_then(|e| e.uses.first().copied())
            .ok_or_else(|| KernelError(format!("the body has no edge {edge}")))?;
        let outer = used.r#loop == 0;
        let (body, order) = self.engine_with_order()?;
        let along = [edge];
        let holes = if outer {
            Holes::Along(&[])
        } else {
            Holes::Along(&along)
        };
        let made = untrim_face(&body, &order, used.face as usize, holes, outer, false)
            .map_err(|e| failed("untrimming", e))?;
        Self::made(&made)
    }
    /// The face `face` with its holes filled and, with `outer`, its outer trim dropped back to
    /// its surface's own edges (Rhino's Untrim).
    pub fn untrimmed(&self, face: usize, outer: bool) -> Result<Self, KernelError> {
        use solidcore_ops::untrim::{Holes, untrim_face};
        let (body, order) = self.engine_with_order()?;
        let made = untrim_face(&body, &order, face, Holes::All, outer, false)
            .map_err(|e| failed("untrimming", e))?;
        Self::made(&made)
    }

    /// The face `face` cut by curves lying on it (Rhino's Split with curves).
    pub fn split_by_curves(&self, face: usize, curves: &[Curve]) -> Result<Self, KernelError> {
        let (body, order) = self.engine_with_order()?;
        let (made, _) = solidcore_ops::trim::split_face_by_curves(
            &body,
            &order,
            face,
            &sweep_records(curves)?,
            crate::SEW_TOLERANCE,
        )
        .map_err(|e| failed("splitting the surface", e))?;
        Self::made(&made)
    }

    /// The faces numbered `faces` taken away.
    pub fn without_faces(&self, faces: &[usize]) -> Result<Self, KernelError> {
        let (body, order) = self.engine_with_order()?;
        let made = solidcore_ops::trim::remove_faces(&body, &order, faces)
            .map_err(|e| failed("removing faces", e))?;
        Self::made(&made)
    }

    /// Where the plane through `at` with normal `normal` cuts the body, as curves (Rhino's
    /// Section and Contour).
    pub fn section(&self, at: Point, normal: Point) -> Result<Vec<Curve>, KernelError> {
        let (body, order) = self.engine_with_order()?;
        let mut pieces = Vec::new();
        for face in 0..order.faces.len() {
            match solidcore_ops::sweep::face_section(
                &body,
                &order,
                face,
                at,
                normal,
                crate::SEW_TOLERANCE,
            ) {
                Ok(found) => pieces.extend(from_sweep(found)?),
                Err(solidcore_ops::Error::Empty) => {}
                Err(e) => return Err(failed("sectioning", e)),
            }
        }
        crate::join_all(&pieces)
    }

    /// Curves projected along `direction` onto the body's faces (Rhino's Project).
    pub fn projected(&self, curves: &[Curve], direction: Point) -> Result<Vec<Curve>, KernelError> {
        let (body, order) = self.engine_with_order()?;
        let curves = sweep_records(curves)?;
        let mut out = Vec::new();
        for face in 0..order.faces.len() {
            match solidcore_ops::sweep::project_curves(
                &body,
                &order,
                face,
                &curves,
                direction,
                crate::SEW_TOLERANCE,
            ) {
                Ok(found) => out.extend(from_sweep(found)?),
                Err(solidcore_ops::Error::Empty) => {}
                Err(e) => return Err(failed("projecting", e)),
            }
        }
        crate::join_all(&out)
    }

    /// Where two bodies meet, as curves (Rhino's Intersect): the edges imprinting one on the
    /// other adds.
    pub fn intersections(&self, other: &Body) -> Result<Vec<Curve>, KernelError> {
        let old: Vec<Curve> = (0..self.record()?.edges.len())
            .map(|edge| self.edge_curve(edge))
            .collect::<Result<_, _>>()?;
        let (imprinted, changed) =
            solidcore_ops::boolean::imprint(self.engine_body()?, other.engine_body()?)
                .map_err(|e| failed("intersecting", e))?;
        if !changed {
            return Ok(Vec::new());
        }
        let after = Self::made(&imprinted)?;
        // The new edges: those of the imprinted body not lying on one of its old edges.
        let record = after.record()?;
        let mut found = Vec::new();
        for index in 0..record.edges.len() {
            let curve = after.edge_curve(index)?;
            let mid = curve
                .sample()
                .ok()
                .and_then(|p| p.get(p.len() / 2).copied());
            let on_old = |p: Point| {
                old.iter()
                    .any(|edge| edge.closest_point(p).is_ok_and(|q| crate::gap(p, q) < 1e-6))
            };
            if let Some(mid) = mid
                && !on_old(mid)
            {
                found.push(curve);
            }
        }
        crate::join_all(&found)
    }

    /// The faces numbered `faces` moved along their normals by `distance`, the solid around
    /// them following (Rhino's MoveFace and PushPull, for faces pushed straight).
    pub fn faces_offset(&self, faces: &[usize], distance: f64) -> Result<Self, KernelError> {
        let (body, order) = self.engine_with_order()?;
        solidcore_ops::offset_faces(&body, &order, faces, distance)
            .map_err(|e| failed("moving faces", e))?;
        Self::made(&body)
    }

    /// The body under any affine map: rows are the images of x, y and z, then the move
    /// (Rhino's Shear and the like).
    pub fn transformed(&self, map: [[f64; 4]; 4]) -> Result<Self, KernelError> {
        let body = self.engine_body()?;
        body.transform(&map)
            .map_err(|e| failed("transforming", e))?;
        Self::made(&body)
    }

    /// The control net of a one-face, untrimmed surface: rows along u, columns along v.
    pub fn control_net(&self) -> Result<(usize, usize, Vec<Point>), KernelError> {
        let net = solidcore_ops::surface_cvs::control_net(&self.record()?)
            .map_err(|e| failed("reading the control net", e))?;
        Ok((net.rows, net.cols, net.points))
    }

    /// The surface with control points moved: `(row, column)` to a new place.
    pub fn with_net_moved(&self, moves: &[((usize, usize), Point)]) -> Result<Self, KernelError> {
        let made = solidcore_ops::surface_cvs::move_cvs(&self.record()?, moves)
            .map_err(|e| failed("moving control points", e))?;
        Self::from_record(&made)
    }

    /// The surface refitted with new degrees and spans each way (Rhino's RebuildUV), and how
    /// far it strays.
    pub fn rebuilt_uv(
        &self,
        degrees: [usize; 2],
        spans: [usize; 2],
    ) -> Result<(Self, f64), KernelError> {
        let (made, off) = solidcore_ops::surface_cvs::rebuild(
            &self.record()?,
            degrees[0],
            degrees[1],
            spans[0],
            spans[1],
            1e6,
        )
        .map_err(|e| failed("rebuilding the surface", e))?;
        Ok((Self::from_record(&made)?, off))
    }
}

impl Curve {
    /// The blend between an end of this curve and an end of `other` (Rhino's BlendCrv), each met
    /// as asked; `shape` is how far the blend's first inner points sit along each end's tangent,
    /// as a share of the chord.
    pub fn blended_with(
        &self,
        at_end: bool,
        contact: Contact,
        other: &Curve,
        other_at_end: bool,
        other_contact: Contact,
        shape: f64,
    ) -> Result<Curve, KernelError> {
        use solidcore_ops::curve::{Contact as C, End, blend_curve};
        let contact_of = |c: Contact| match c {
            Contact::Position => C::G0,
            Contact::Tangency => C::G1,
            Contact::Curvature => C::G2,
        };
        let end_of = |e: bool| if e { End::End } else { End::Start };
        let made = blend_curve(
            &to_record_spline(self)?,
            end_of(at_end),
            contact_of(contact),
            shape,
            &to_record_spline(other)?,
            end_of(other_at_end),
            contact_of(other_contact),
            shape,
        )
        .map_err(|e| failed("blending", e))?;
        from_record_curve(&made, None)
    }

    /// The curve under any affine map: rows are the images of x, y and z, then the move. Exact,
    /// on its NURBS control points.
    pub fn mapped_affine(&self, map: [[f64; 4]; 4]) -> Result<Curve, KernelError> {
        let apply = |p: Point| -> Point {
            std::array::from_fn(|j| {
                p[0] * map[0][j] + p[1] * map[1][j] + p[2] * map[2][j] + map[3][j]
            })
        };
        if let Curve::Point { at } = self {
            return Ok(Curve::Point { at: apply(*at) });
        }
        let mut spline = crate::Spline::of(self)?;
        for q in &mut spline.points {
            let w = q[3];
            let p = apply([q[0] / w, q[1] / w, q[2] / w]);
            *q = [p[0] * w, p[1] * w, p[2] * w, w];
        }
        spline.to_curve()
    }
    /// The curve at degree `degree` through the engine: raised exactly, or lowered when it stays
    /// within `tolerance` (then with how far it moved).
    pub fn with_degree(
        &self,
        degree: usize,
        tolerance: f64,
    ) -> Result<(Curve, Option<f64>), KernelError> {
        let (made, off) =
            solidcore_ops::curve_knots::set_degree(&to_record_spline(self)?, degree, tolerance)
                .map_err(|e| failed("changing the degree", e))?;
        Ok((from_record_curve(&made, None)?, off))
    }
    /// The curve carried on by `length` at its start or end (Rhino's Extend, by length): as its
    /// own shape (`Natural`) or straight along its end tangent (`Tangent`).
    pub fn extended_by(
        &self,
        at_start: bool,
        length: f64,
        how: Carry,
    ) -> Result<Curve, KernelError> {
        use solidcore_ops::curve::{CurveEndAt, CurveExtension, extend_curve};
        let at = if at_start {
            CurveEndAt::Start
        } else {
            CurveEndAt::End
        };
        let how = match how {
            Carry::Natural | Carry::Curvature => CurveExtension::Natural,
            Carry::Tangent => CurveExtension::Linear,
        };
        let made = extend_curve(&to_record_spline(self)?, at, length, how)
            .map_err(|e| failed("extending the curve", e))?;
        from_record_curve(&made, None)
    }

    /// The curvature comb: at `teeth` even steps, each point and its curvature vector (towards
    /// the centre of curvature, as long as the curvature).
    pub fn curvature_comb(&self, teeth: usize) -> Result<Vec<(Point, Point)>, KernelError> {
        let record = to_record_spline(self)?;
        let (a, b) = crate::Spline::of(self)?.domain();
        let interval = [a, b];
        let teeth = teeth.clamp(2, 2000);
        let comb = solidcore_ops::curve::curve_comb(&record, interval, teeth)
            .map_err(|e| failed("finding the curvature", e))?;
        Ok(comb.into_iter().map(|t| (t.at, t.curvature)).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Carry, Contact, Curve, gap};

    fn line(a: [f64; 3], b: [f64; 3]) -> Curve {
        Curve::line(a, b).unwrap()
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }

    fn square_edges(size: f64) -> Vec<Curve> {
        vec![
            line([0.0; 3], [size, 0.0, 0.0]),
            line([size, 0.0, 0.0], [size, size, 0.0]),
            line([size, size, 0.0], [0.0, size, 0.0]),
            line([0.0, size, 0.0], [0.0; 3]),
        ]
    }

    #[test]
    fn surfaces_from_edges_network_patch_and_ruled() {
        let edge = Body::from_edges(&square_edges(3.0)).unwrap();
        assert!(
            close(edge.area().unwrap(), 9.0, 1e-6),
            "{}",
            edge.area().unwrap()
        );
        let ruled = Body::ruled(
            &line([0.0; 3], [4.0, 0.0, 0.0]),
            &line([0.0, 2.0, 0.0], [4.0, 2.0, 0.0]),
            false,
        )
        .unwrap();
        assert!(close(ruled.area().unwrap(), 8.0, 1e-6));
        let (patch, _) = Body::patched(&square_edges(2.0)).unwrap();
        assert!(
            close(patch.area().unwrap(), 4.0, 1e-3),
            "{}",
            patch.area().unwrap()
        );
        let wave = |y: f64| {
            Curve::control_point(vec![[0.0, y, 0.0], [2.0, y, 1.0], [4.0, y, 0.0]], 2, false)
                .unwrap()
        };
        let straight = |x: f64| {
            let z = if (x - 2.0).abs() < 1e-9 { 0.5 } else { 0.0 };
            Curve::control_point(vec![[x, 0.0, z], [x, 1.5, z], [x, 3.0, z]], 2, false).unwrap()
        };
        let (network, off) = Body::from_network(
            &[wave(0.0), wave(3.0)],
            &[straight(0.0), straight(2.0), straight(4.0)],
        )
        .unwrap();
        assert!(off < 1e-3, "{off}");
        assert!(network.area().unwrap() > 12.0);
    }

    #[test]
    fn extend_untrim_and_split_a_surface() {
        let plane = Body::from_edges(&square_edges(4.0)).unwrap();
        let mesh = plane.mesh().unwrap();
        let longer = plane
            .extended_at(mesh.edge_ids[0], 2.0, Carry::Natural)
            .unwrap();
        assert!(
            close(longer.area().unwrap(), 24.0, 1e-6),
            "{}",
            longer.area().unwrap()
        );
        let holed = Body::planar(&[
            Curve::polyline(
                vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
                true,
            )
            .unwrap(),
            Curve::circle([2.0, 2.0, 0.0], 1.0).unwrap(),
        ])
        .unwrap();
        assert!(holed.area().unwrap() < 16.0 - 3.0);
        let filled = holed.untrimmed(0, false).unwrap();
        assert!(
            close(filled.area().unwrap(), 16.0, 1e-6),
            "{}",
            filled.area().unwrap()
        );
        let halves = plane
            .split_by_curves(0, &[line([2.0, -1.0, 0.0], [2.0, 5.0, 0.0])])
            .unwrap();
        assert_eq!(halves.face_count().unwrap(), 2);
    }

    #[test]
    fn sections_projections_and_intersections() {
        let cube = Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap();
        let cut = cube.section([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]).unwrap();
        assert_eq!(cut.len(), 1);
        assert!(
            close(cut[0].length().unwrap(), 10.0, 1e-6),
            "{}",
            cut[0].length().unwrap()
        );
        let floor = Body::from_edges(&square_edges(5.0)).unwrap();
        let shadow = floor
            .projected(&[line([1.0, 1.0, 3.0], [4.0, 1.0, 3.0])], [0.0, 0.0, -1.0])
            .unwrap();
        assert!(close(shadow[0].length().unwrap(), 3.0, 1e-6));
        let other = Body::cuboid([1.0, 1.0, 1.0], [2.0, 3.0, 4.0]).unwrap();
        let meet = cube.intersections(&other).unwrap();
        assert!(!meet.is_empty());
    }

    #[test]
    fn faces_move_and_shear_keeps_volume() {
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let top = (0..6)
            .find(|face| {
                cube.mesh()
                    .unwrap()
                    .triangle_faces
                    .iter()
                    .zip(cube.mesh().unwrap().triangles.iter())
                    .any(|(f, t)| {
                        *f == *face
                            && t.iter().all(|i| {
                                (cube.mesh().unwrap().positions[*i as usize][2] - 1.0).abs() < 1e-9
                            })
                    })
            })
            .unwrap();
        let taller = cube.faces_offset(&[top], 0.5).unwrap();
        assert!(
            close(taller.volume().unwrap(), 1.5, 1e-6),
            "{}",
            taller.volume().unwrap()
        );
        let sheared = cube
            .transformed([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.7, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
            .unwrap();
        assert!(close(sheared.volume().unwrap(), 1.0, 1e-6));
    }

    #[test]
    fn curve_blends_extensions_and_combs() {
        let a = line([0.0; 3], [5.0, 0.0, 0.0]);
        let b = line([10.0, 3.0, 0.0], [15.0, 3.0, 0.0]);
        let blend = a
            .blended_with(true, Contact::Curvature, &b, false, Contact::Curvature, 0.3)
            .unwrap();
        let points = blend.sample().unwrap();
        assert!(
            gap(points[0], [5.0, 0.0, 0.0]) < 1e-9
                && gap(*points.last().unwrap(), [10.0, 3.0, 0.0]) < 1e-9
        );
        let wave = Curve::control_point(vec![[0.0; 3], [2.0, 2.0, 0.0], [4.0, 0.0, 0.0]], 2, false)
            .unwrap();
        let longer = wave.extended_by(false, 1.0, Carry::Tangent).unwrap();
        assert!(longer.length().unwrap() > wave.length().unwrap() + 0.99);
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        for (_, k) in circle.curvature_comb(12).unwrap() {
            assert!(close(
                (k[0] * k[0] + k[1] * k[1] + k[2] * k[2]).sqrt(),
                0.2,
                1e-6
            ));
        }
    }
}
