//! Operations on surfaces built from the surface toolkit and the engine's face evaluation:
//! isocurves, shrinking trimmed surfaces, blending and matching surfaces, two-rail sweeps,
//! deforming surfaces and curves, and curves on surfaces (pulling, interpolating, offsetting).
//! All flagged as custom (`docs/kernel/custom-operations.md`) unless the engine does the work.

// The net is built by index, column by column, as the construction reads.
#![allow(clippy::needless_range_loop)]

use crate::body::{Body, failed};
use crate::surface::NurbsSurface;
use crate::{Curve, KernelError, Point, Spline, cross, dot, gap, sub, unit};
use solidcore_ops::face_surface::FaceSurface;
use solidcore_record::brep::{Curve2, Surface};

/// Which side of its surface's parameter box an edge runs along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    U0,
    U1,
    V0,
    V1,
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

fn curve2_points(c: &Curve2) -> Vec<[f64; 2]> {
    match c {
        Curve2::Spline { points, .. } => points.clone(),
        Curve2::RationalSpline { homogeneous, .. } => homogeneous
            .iter()
            .map(|h| [h[0] / h[2], h[1] / h[2]])
            .collect(),
    }
}

/// A 2D pcurve as a spline in the plane z = 0, to evaluate.
fn curve2_spline(c: &Curve2) -> Spline {
    match c {
        Curve2::Spline { knots, points } => Spline {
            degree: knots.len() - points.len() - 1,
            knots: knots.clone(),
            points: points.iter().map(|p| [p[0], p[1], 0.0, 1.0]).collect(),
        },
        Curve2::RationalSpline { knots, homogeneous } => Spline {
            degree: knots.len() - homogeneous.len() - 1,
            knots: knots.clone(),
            points: homogeneous
                .iter()
                .map(|h| [h[0], h[1], 0.0, h[2]])
                .collect(),
        },
    }
}

impl Body {
    /// The point of face `face` nearest `p`, with its (u, v): a coarse search for a start, then
    /// the engine's exact nearest point.
    fn nearest_on_face(&self, face: usize, p: Point) -> Result<([f64; 2], Point), KernelError> {
        let record = self.record()?;
        let surface = FaceSurface::of(&record, face).map_err(|e| failed("reading a face", e))?;
        let [[u0, u1], [v0, v1]] = surface.domain();
        let mut best = ([u0, v0], f64::INFINITY);
        for i in 0..=12 {
            for j in 0..=12 {
                let uv = [
                    u0 + (u1 - u0) * f64::from(i) / 12.0,
                    v0 + (v1 - v0) * f64::from(j) / 12.0,
                ];
                if let Ok(q) = surface.point(uv)
                    && gap(q, p) < best.1
                {
                    best = (uv, gap(q, p));
                }
            }
        }
        let found = surface
            .closest_point(p, best.0)
            .map_err(|e| failed("finding the nearest point", e))?;
        Ok((found.uv, found.point))
    }

    /// The face nearest a point, with the (u, v) and the point on it.
    pub fn nearest_face(&self, p: Point) -> Result<(usize, [f64; 2], Point), KernelError> {
        let count = self.face_count()?;
        let mut best: Option<(usize, [f64; 2], Point)> = None;
        for face in 0..count {
            if let Ok((uv, q)) = self.nearest_on_face(face, p)
                && best.as_ref().is_none_or(|(_, _, b)| gap(q, p) < gap(*b, p))
            {
                best = Some((face, uv, q));
            }
        }
        best.ok_or_else(|| KernelError("no face to measure from".to_owned()))
    }

    /// The isocurve of face `face` through its point nearest `p`: along u, or along v when
    /// `along_v` (Rhino's ExtractIsocurve).
    pub fn isocurve_through(
        &self,
        face: usize,
        p: Point,
        along_v: bool,
    ) -> Result<Curve, KernelError> {
        // CUSTOM KERNEL OPERATION: isocurves
        let (uv, _) = self.nearest_on_face(face, p)?;
        let surface = NurbsSurface::of_face(&self.record()?, face)?;
        let iso = if along_v {
            surface.iso_u(uv[0])
        } else {
            surface.iso_v(uv[1])
        };
        iso.to_curve()
    }

    /// Every face's surface cut down to the box of its trims, the trims kept (Rhino's
    /// ShrinkTrimmedSrf).
    pub fn shrunk(&self) -> Result<Body, KernelError> {
        // CUSTOM KERNEL OPERATION: shrink-trimmed
        let mut record = self.record()?;
        for index in 0..record.faces.len() {
            let Ok(surface) = NurbsSurface::of_face(&record, index) else {
                continue;
            };
            let face = &record.faces[index];
            let points: Vec<[f64; 2]> = face
                .loops
                .iter()
                .flat_map(|lp| match lp {
                    solidcore_record::brep::Loop::Edges { edges } => edges
                        .iter()
                        .flat_map(|e| curve2_points(&e.pcurve))
                        .collect::<Vec<_>>(),
                    solidcore_record::brep::Loop::Vertex { .. } => Vec::new(),
                })
                .collect();
            if points.is_empty() {
                continue;
            }
            let [[du0, du1], [dv0, dv1]] = surface.domain();
            let lo = |k: usize| points.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min);
            let hi = |k: usize| {
                points
                    .iter()
                    .map(|p| p[k])
                    .fold(f64::NEG_INFINITY, f64::max)
            };
            let bounds = [
                [lo(0).max(du0), hi(0).min(du1)],
                [lo(1).max(dv0), hi(1).min(dv1)],
            ];
            if bounds[0][1] - bounds[0][0] < 1e-9 || bounds[1][1] - bounds[1][0] < 1e-9 {
                continue;
            }
            let cut = surface.cut_to(bounds);
            let rational = cut
                .points
                .iter()
                .flatten()
                .any(|p| (p[3] - 1.0).abs() > 1e-12);
            let face = &mut record.faces[index];
            face.surface = if rational {
                Surface::RationalSpline {
                    u_knots: cut.u_knots.clone(),
                    v_knots: cut.v_knots.clone(),
                    homogeneous: cut.points.clone(),
                }
            } else {
                Surface::Spline {
                    u_knots: cut.u_knots.clone(),
                    v_knots: cut.v_knots.clone(),
                    points: cut
                        .points
                        .iter()
                        .map(|r| r.iter().map(|p| [p[0], p[1], p[2]]).collect())
                        .collect(),
                }
            };
            face.nurbs = None;
            face.domain = cut.domain();
        }
        Body::from_record(&record)
    }

    /// Which side of its face's untrimmed surface the edge runs along, and that face.
    fn edge_side(&self, edge: usize) -> Result<(usize, Side, bool), KernelError> {
        let record = self.record()?;
        let e = record
            .edges
            .get(edge)
            .ok_or_else(|| KernelError(format!("the body has no edge {edge}")))?;
        let used = e
            .uses
            .first()
            .ok_or_else(|| KernelError("an edge on no face".to_owned()))?;
        let face = &record.faces[used.face as usize];
        let solidcore_record::brep::Loop::Edges { edges } = &face.loops[used.r#loop as usize]
        else {
            return Err(KernelError("an edge on a point loop".to_owned()));
        };
        let pcurve = &edges[used.position as usize].pcurve;
        let ends = curve2_points(pcurve);
        let (a, b) = (ends[0], ends[ends.len() - 1]);
        let [[u0, u1], [v0, v1]] = NurbsSurface::of_face(&record, used.face as usize)?.domain();
        let near = |x: f64, y: f64| (x - y).abs() < 1e-7 * (1.0 + y.abs());
        let side = if near(a[0], u0) && near(b[0], u0) {
            Side::U0
        } else if near(a[0], u1) && near(b[0], u1) {
            Side::U1
        } else if near(a[1], v0) && near(b[1], v0) {
            Side::V0
        } else if near(a[1], v1) && near(b[1], v1) {
            Side::V1
        } else {
            return Err(KernelError(
                "that edge is a trim, not a side of its surface; only untrimmed edges can be used"
                    .to_owned(),
            ));
        };
        Ok((used.face as usize, side, true))
    }

    /// The rows of the face's net starting at the edge and running inward: row 0 is the edge.
    fn rows_from_edge(
        &self,
        edge: usize,
        count: usize,
    ) -> Result<(NurbsSurface, Side, Vec<Spline>), KernelError> {
        let (face, side, _) = self.edge_side(edge)?;
        let mut s = NurbsSurface::of_face(&self.record()?, face)?;
        // Enough rows across the edge, by inserting knots there (the shape stays the same).
        let across_v = matches!(side, Side::V0 | Side::V1);
        loop {
            let have = if across_v {
                s.points[0].len()
            } else {
                s.points.len()
            };
            if have >= count {
                break;
            }
            let knots = if across_v { &s.v_knots } else { &s.u_knots };
            let widest = knots
                .windows(2)
                .max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
                .map_or(0.5, |w| (w[0] + w[1]) / 2.0);
            s = s.inserted(across_v, widest, 1);
        }
        let (rows, cols) = (s.points.len(), s.points[0].len());
        let row = |k: usize| -> Spline {
            match side {
                Side::U0 => Spline {
                    degree: s.degree[1],
                    knots: s.v_knots.clone(),
                    points: s.points[k].clone(),
                },
                Side::U1 => Spline {
                    degree: s.degree[1],
                    knots: s.v_knots.clone(),
                    points: s.points[rows - 1 - k].clone(),
                },
                Side::V0 => Spline {
                    degree: s.degree[0],
                    knots: s.u_knots.clone(),
                    points: s.points.iter().map(|r| r[k]).collect(),
                },
                Side::V1 => Spline {
                    degree: s.degree[0],
                    knots: s.u_knots.clone(),
                    points: s.points.iter().map(|r| r[cols - 1 - k]).collect(),
                },
            }
        };
        let most = match side {
            Side::U0 | Side::U1 => rows,
            Side::V0 | Side::V1 => cols,
        };
        Ok((s.clone(), side, (0..count.min(most)).map(row).collect()))
    }

    /// A surface joining an edge of this body to an edge of `other` (Rhino's BlendSrf),
    /// meeting each in position (0), tangency (1) or curvature (2); `bulge` scales how far the
    /// blend leaves each edge along its surface.
    pub fn blend_to(
        &self,
        edge: usize,
        other: &Body,
        other_edge: usize,
        continuity: usize,
        bulge: f64,
    ) -> Result<Body, KernelError> {
        // CUSTOM KERNEL OPERATION: surface-blending
        let need = continuity.min(2) + 1;
        let (_, _, a) = self.rows_from_edge(edge, need)?;
        let (_, _, mut b) = other.rows_from_edge(other_edge, need)?;
        if a.len() < need || b.len() < need {
            return Err(KernelError(
                "a surface has too few control points for that continuity".to_owned(),
            ));
        }
        // Run the second edge the same way as the first.
        let (a0s, a0e) = (a[0].point(a[0].domain().0), a[0].point(a[0].domain().1));
        let (b0s, b0e) = (b[0].point(b[0].domain().0), b[0].point(b[0].domain().1));
        if gap(a0s, b0e) + gap(a0e, b0s) < gap(a0s, b0s) + gap(a0e, b0e) {
            b = b.iter().map(Spline::reversed).collect();
        }
        let all: Vec<Spline> = a.iter().chain(b.iter()).cloned().collect();
        let all = Spline::compatible(&all)?;
        let (a, b) = all.split_at(need);
        let n = a[0].points.len();
        let span = (gap(a0s, b0s) + gap(a0e, b0e)) / 2.0;
        // Columns of the blend's net, from a's edge to b's.
        let degree_v = 2 * need - 1;
        let mut net = vec![vec![[0.0; 4]; degree_v + 1]; n];
        for j in 0..n {
            let place = |s: &Spline, k: usize| -> Point {
                let q = s.points[k];
                [q[0] / q[3], q[1] / q[3], q[2] / q[3]]
            };
            let pa: Vec<Point> = a.iter().map(|s| place(s, j)).collect();
            let pb: Vec<Point> = b.iter().map(|s| place(s, j)).collect();
            let mut column: Vec<Point> = vec![pa[0]];
            let mut tail: Vec<Point> = vec![pb[0]];
            if need >= 2 {
                let out_a = unit(sub(pa[0], pa[1])).unwrap_or([0.0; 3]);
                let out_b = unit(sub(pb[0], pb[1])).unwrap_or([0.0; 3]);
                let reach = bulge * span / 3.0;
                column.push(plus(pa[0], out_a, reach));
                tail.push(plus(pb[0], out_b, reach));
                if need >= 3 {
                    // Second rows matching each surface's bend across its edge.
                    let bend = |p: &[Point], out: Point| -> Point {
                        let second: Point =
                            std::array::from_fn(|k| p[0][k] - 2.0 * p[1][k] + p[2][k]);
                        let base = plus(p[0], out, 2.0 * reach);
                        let scale = (reach / gap(p[0], p[1]).max(1e-12)).powi(2);
                        plus(base, second, scale)
                    };
                    column.push(bend(&pa, out_a));
                    tail.push(bend(&pb, out_b));
                }
            }
            tail.reverse();
            column.extend(tail);
            for (k, p) in column.iter().enumerate() {
                net[j][k] = [p[0], p[1], p[2], 1.0];
            }
        }
        let mut v_knots = vec![0.0; degree_v + 1];
        v_knots.extend(std::iter::repeat_n(1.0, degree_v + 1));
        let surface = NurbsSurface {
            degree: [a[0].degree, degree_v],
            u_knots: a[0].knots.clone(),
            v_knots,
            points: net,
        };
        surface.to_body()
    }

    /// This body's surface changed along `edge` to meet `other` at `other_edge` (Rhino's
    /// MatchSrf), in position (0) or tangency (1).
    pub fn matched_to(
        &self,
        edge: usize,
        other: &Body,
        other_edge: usize,
        continuity: usize,
    ) -> Result<Body, KernelError> {
        // CUSTOM KERNEL OPERATION: surface-matching
        let (surface, side, _) = self.rows_from_edge(edge, 1)?;
        let (rows, cols) = (surface.points.len(), surface.points[0].len());
        let count = match side {
            Side::U0 | Side::U1 => rows,
            Side::V0 | Side::V1 => cols,
        };
        let (_, _, mine) = self.rows_from_edge(edge, count)?;
        let (_, _, mut theirs) = other.rows_from_edge(other_edge, 2)?;
        let (m0s, m0e) = (
            mine[0].point(mine[0].domain().0),
            mine[0].point(mine[0].domain().1),
        );
        let (t0s, t0e) = (
            theirs[0].point(theirs[0].domain().0),
            theirs[0].point(theirs[0].domain().1),
        );
        if gap(m0s, t0e) + gap(m0e, t0s) < gap(m0s, t0s) + gap(m0e, t0e) {
            theirs = theirs.iter().map(Spline::reversed).collect();
        }
        let all: Vec<Spline> = mine.iter().chain(theirs.iter()).cloned().collect();
        let all = Spline::compatible(&all)?;
        let (mine, theirs) = all.split_at(count);
        let mut rows_out: Vec<Spline> = mine.to_vec();
        rows_out[0].points.clone_from(&theirs[0].points);
        if continuity >= 1 && rows_out.len() >= 2 && theirs.len() >= 2 {
            for j in 0..rows_out[0].points.len() {
                let place = |q: [f64; 4]| [q[0] / q[3], q[1] / q[3], q[2] / q[3]];
                let edge_point = place(theirs[0].points[j]);
                let across = unit(sub(place(theirs[0].points[j]), place(theirs[1].points[j])))
                    .unwrap_or([0.0; 3]);
                let length = gap(place(mine[0].points[j]), place(mine[1].points[j]));
                // Carry on across the edge, out of theirs and into this surface.
                let p = plus(edge_point, across, length);
                rows_out[1].points[j] = [p[0], p[1], p[2], 1.0];
            }
        }
        // Back into a net, the edge on its side again.
        let along = rows_out[0].clone();
        let n = rows_out.len();
        let net: Vec<Vec<[f64; 4]>> = match side {
            Side::U0 => rows_out.iter().map(|r| r.points.clone()).collect(),
            Side::U1 => rows_out.iter().rev().map(|r| r.points.clone()).collect(),
            Side::V0 => (0..along.points.len())
                .map(|i| (0..n).map(|k| rows_out[k].points[i]).collect())
                .collect(),
            Side::V1 => (0..along.points.len())
                .map(|i| (0..n).rev().map(|k| rows_out[k].points[i]).collect())
                .collect(),
        };
        let cross_knots = match side {
            Side::U0 | Side::U1 => surface.u_knots.clone(),
            Side::V0 | Side::V1 => surface.v_knots.clone(),
        };
        let cross_degree = match side {
            Side::U0 | Side::U1 => surface.degree[0],
            Side::V0 | Side::V1 => surface.degree[1],
        };
        let out = match side {
            Side::U0 | Side::U1 => NurbsSurface {
                degree: [cross_degree, along.degree],
                u_knots: cross_knots,
                v_knots: along.knots.clone(),
                points: net,
            },
            Side::V0 | Side::V1 => NurbsSurface {
                degree: [along.degree, cross_degree],
                u_knots: along.knots.clone(),
                v_knots: cross_knots,
                points: net,
            },
        };
        out.to_body()
    }

    /// `profile` swept along two rails, its ends held on them and scaled to span them (Rhino's
    /// Sweep2), exact at `stations` places along the rails and skinned between.
    pub fn swept_two_rails(
        rail_a: &Curve,
        rail_b: &Curve,
        profile: &Curve,
        stations: usize,
    ) -> Result<Body, KernelError> {
        // CUSTOM KERNEL OPERATION: two-rail-sweep
        use solidcore_ops::sweep::{Affine, Ends, sweep_stations};
        let (a, b) = (Spline::of(rail_a)?, Spline::of(rail_b)?);
        let points = profile.sample()?;
        let (ps, pe) = (points[0], points[points.len() - 1]);
        let chord = sub(pe, ps);
        let length = dot(chord, chord).sqrt();
        let x = unit(chord).ok_or_else(|| KernelError("the profile's ends meet".to_owned()))?;
        // The profile's plane, from a point off its chord, or the first rail's direction.
        let off = points
            .iter()
            .map(|p| sub(*p, ps))
            .map(|d| sub(d, x.map(|v| v * dot(d, x))))
            .max_by(|d, e| dot(*d, *d).total_cmp(&dot(*e, *e)));
        let rail_dir = a.derivative(a.domain().0);
        let y0 = off
            .filter(|d| dot(*d, *d) > 1e-12)
            .and_then(unit)
            .or_else(|| unit(cross(rail_dir, x)))
            .ok_or_else(|| {
                KernelError("the profile and rails do not span a direction".to_owned())
            })?;
        let z0 = cross(x, y0);
        let count = stations.clamp(4, 400);
        let mut maps = Vec::with_capacity(count + 1);
        for i in 0..=count {
            let t = i as f64 / count as f64;
            let at = |s: &Spline| s.point(s.domain().0 + (s.domain().1 - s.domain().0) * t);
            let (ra, rb) = (at(&a), at(&b));
            let along = sub(rb, ra);
            let scale = dot(along, along).sqrt() / length;
            let xs = unit(along).ok_or_else(|| KernelError("the rails touch".to_owned()))?;
            // The station's frame keeps the sweep direction (the rails' mean tangent) as z.
            let tangent = {
                let ta = a.derivative(a.domain().0 + (a.domain().1 - a.domain().0) * t);
                let tb = b.derivative(b.domain().0 + (b.domain().1 - b.domain().0) * t);
                std::array::from_fn::<f64, 3, _>(|k| ta[k] + tb[k])
            };
            let zs = unit(sub(tangent, xs.map(|v| v * dot(tangent, xs)))).unwrap_or(z0);
            let zs = if dot(zs, z0) < 0.0 && i == 0 {
                zs.map(|v| -v)
            } else {
                zs
            };
            let ys = cross(zs, xs);
            // v (relative to ps) = a x + b y + c z  ->  ra + scale (a xs + b ys + c zs).
            let image = |axis: Point| -> Point {
                let (ca, cb, cc) = (dot(axis, x), dot(axis, y0), dot(axis, z0));
                std::array::from_fn(|k| scale * (ca * xs[k] + cb * ys[k] + cc * zs[k]))
            };
            let m = [
                image([1.0, 0.0, 0.0]),
                image([0.0, 1.0, 0.0]),
                image([0.0, 0.0, 1.0]),
            ];
            let moved_ps: Point =
                std::array::from_fn(|k| ps[0] * m[0][k] + ps[1] * m[1][k] + ps[2] * m[2][k]);
            maps.push(Affine {
                m,
                t: sub(ra, moved_ps),
            });
        }
        let pieces = crate::making::sweep_pieces(profile)?;
        let made = sweep_stations(&pieces, &maps, Ends::Open)
            .map_err(|e| failed("sweeping along two rails", e))?;
        Body::made(&made)
    }

    /// Every face deformed by `map`, a smooth map of space (Twist, Bend, Taper, Flow, CageEdit):
    /// each face's surface is refined and its control points moved, its trims kept in (u, v),
    /// its edges refitted to the moved surfaces.
    pub fn deformed(&self, map: &dyn Fn(Point) -> Point) -> Result<Body, KernelError> {
        // CUSTOM KERNEL OPERATION: space-deformation
        let mut record = self.record()?;
        for index in 0..record.faces.len() {
            let mut surface = NurbsSurface::of_face(&record, index)?;
            // Refine so the moved net follows the map closely: 12 spans each way at least.
            for along_v in [false, true] {
                let [[u0, u1], [v0, v1]] = surface.domain();
                let (a, b) = if along_v { (v0, v1) } else { (u0, u1) };
                for k in 1..12 {
                    let at = a + (b - a) * f64::from(k) / 12.0;
                    let knots = if along_v {
                        &surface.v_knots
                    } else {
                        &surface.u_knots
                    };
                    if !knots.iter().any(|v| (*v - at).abs() < 1e-9) {
                        surface = surface.inserted(along_v, at, 1);
                    }
                }
            }
            for row in &mut surface.points {
                for q in row.iter_mut() {
                    let w = q[3];
                    let p = map([q[0] / w, q[1] / w, q[2] / w]);
                    *q = [p[0] * w, p[1] * w, p[2] * w, w];
                }
            }
            let face = &mut record.faces[index];
            face.surface = Surface::RationalSpline {
                u_knots: surface.u_knots.clone(),
                v_knots: surface.v_knots.clone(),
                homogeneous: surface.points.clone(),
            };
            face.nurbs = None;
            face.domain = surface.domain();
        }
        // Edges refitted through points of the moved surfaces along their pcurves.
        let faces: Vec<NurbsSurface> = (0..record.faces.len())
            .map(|i| NurbsSurface::of_face(&record, i))
            .collect::<Result<_, _>>()?;
        for e in &mut record.edges {
            let Some(used) = e.uses.first().copied() else {
                continue;
            };
            let solidcore_record::brep::Loop::Edges { edges } =
                &record.faces[used.face as usize].loops[used.r#loop as usize]
            else {
                continue;
            };
            let pcurve = curve2_spline(&edges[used.position as usize].pcurve);
            let surface = &faces[used.face as usize];
            let (t0, t1) = pcurve.domain();
            let through: Vec<Point> = (0..=24)
                .map(|k| {
                    let uv = pcurve.point(t0 + (t1 - t0) * f64::from(k) / 24.0);
                    surface.point(uv[0], uv[1])
                })
                .collect();
            let closed = gap(through[0], through[24]) < 1e-9;
            let fitted = Spline::of(&Curve::interpolated(
                if closed {
                    through[..24].to_vec()
                } else {
                    through
                },
                closed,
            )?)?
            .reparameterized(t0, t1);
            e.curve = solidcore_record::brep::Curve::Spline {
                knots: fitted.knots.clone(),
                points: fitted.points.iter().map(|p| [p[0], p[1], p[2]]).collect(),
            };
            e.nurbs = None;
            e.interval = [t0, t1];
            e.tolerance = e.tolerance.max(1e-4);
        }
        for v in &mut record.vertices {
            v.point = map(v.point);
            v.tolerance = v.tolerance.max(1e-4);
        }
        record.tolerance = record.tolerance.max(1e-4);
        Body::from_record(&record)
    }

    /// A curve pulled onto the body: each of its points to the nearest point of the surfaces,
    /// fitted through (Rhino's Pull).
    pub fn pulled(&self, curve: &Curve) -> Result<Curve, KernelError> {
        // CUSTOM KERNEL OPERATION: curves-on-surfaces
        let points = curve.divided(48)?;
        let on: Vec<Point> = points
            .iter()
            .map(|p| self.nearest_face(*p).map(|(_, _, q)| q))
            .collect::<Result<_, _>>()?;
        let closed = curve.is_closed()?;
        Curve::interpolated(
            if closed {
                on[..on.len() - 1].to_vec()
            } else {
                on
            },
            closed,
        )
    }

    /// A curve through points on face `face`, interpolated in the face's (u, v) so it lies on
    /// it (Rhino's InterpCrvOnSrf).
    pub fn curve_on_face(&self, face: usize, points: &[Point]) -> Result<Curve, KernelError> {
        let uvs: Vec<Point> = points
            .iter()
            .map(|p| {
                self.nearest_on_face(face, *p)
                    .map(|(uv, _)| [uv[0], uv[1], 0.0])
            })
            .collect::<Result<_, _>>()?;
        let in_uv = Spline::of(&Curve::interpolated(uvs, false)?)?;
        let surface =
            FaceSurface::of(&self.record()?, face).map_err(|e| failed("reading a face", e))?;
        let (t0, t1) = in_uv.domain();
        let lifted: Vec<Point> = (0..=64)
            .map(|k| {
                let uv = in_uv.point(t0 + (t1 - t0) * f64::from(k) / 64.0);
                surface
                    .point([uv[0], uv[1]])
                    .map_err(|e| failed("evaluating a face", e))
            })
            .collect::<Result<_, _>>()?;
        Curve::interpolated(lifted, false)
    }

    /// A curve lying on the body moved along the surfaces by `distance`, to the side of `toward`
    /// (Rhino's OffsetCrvOnSrf).
    pub fn offset_on_surface(
        &self,
        curve: &Curve,
        distance: f64,
        toward: Point,
    ) -> Result<Curve, KernelError> {
        let spline = Spline::of(curve)?;
        let (a, b) = spline.domain();
        let mut moved = Vec::new();
        let mut side = 0.0;
        for k in 0..=48 {
            let t = a + (b - a) * f64::from(k) / 48.0;
            let p = spline.point(t);
            let (face, uv, _) = self.nearest_face(p)?;
            let surface =
                FaceSurface::of(&self.record()?, face).map_err(|e| failed("reading a face", e))?;
            let n = surface
                .normal(uv)
                .map_err(|e| failed("evaluating a face", e))?;
            let along = unit(spline.derivative(t)).unwrap_or([1.0, 0.0, 0.0]);
            let mut out = unit(cross(n, along)).unwrap_or([0.0; 3]);
            if k == 0 {
                side = dot(sub(toward, p), out).signum();
                if side == 0.0 {
                    side = 1.0;
                }
            }
            out = out.map(|v| v * side);
            let (_, _, back) = self.nearest_face(plus(p, out, distance))?;
            moved.push(back);
        }
        let closed = curve.is_closed()?;
        Curve::interpolated(
            if closed {
                moved[..moved.len() - 1].to_vec()
            } else {
                moved
            },
            closed,
        )
    }
}

impl Curve {
    /// The curve deformed by `map`: refined, then its control points moved.
    pub fn deformed(&self, map: &dyn Fn(Point) -> Point) -> Result<Curve, KernelError> {
        let mut spline = Spline::of(self)?;
        let (a, b) = spline.domain();
        for k in 1..32 {
            let at = a + (b - a) * f64::from(k) / 32.0;
            if !spline.knots.iter().any(|v| (*v - at).abs() < 1e-9) {
                spline = spline.inserted(at, 1);
            }
        }
        for q in &mut spline.points {
            let w = q[3];
            let p = map([q[0] / w, q[1] / w, q[2] / w]);
            *q = [p[0] * w, p[1] * w, p[2] * w, w];
        }
        spline.to_curve()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve, NurbsSurface, gap};

    fn sheet(size: f64, z: f64) -> Body {
        let a = Curve::line([0.0, 0.0, z], [size, 0.0, z]).unwrap();
        let b = Curve::line([0.0, size, z], [size, size, z]).unwrap();
        Body::ruled(&a, &b, false).unwrap()
    }

    #[test]
    fn isocurves_lie_on_the_surface() {
        let s = sheet(4.0, 0.0);
        let iso = s.isocurve_through(0, [1.0, 2.0, 5.0], false).unwrap();
        let points = iso.sample().unwrap();
        assert!(points.iter().all(|p| p[2].abs() < 1e-9));
        assert!((iso.length().unwrap() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn a_trimmed_square_shrinks_to_its_trim() {
        let holed = Body::planar(&[Curve::circle([2.0, 2.0, 0.0], 1.0).unwrap()]).unwrap();
        let area = holed.area().unwrap();
        let small = holed.shrunk().unwrap();
        assert!((small.area().unwrap() - area).abs() < 1e-6);
        let surface = NurbsSurface::of_face(&small.record().unwrap(), 0).unwrap();
        let [[u0, u1], [v0, v1]] = surface.domain();
        assert!(
            (u1 - u0) <= 2.0 + 1e-6 && (v1 - v0) <= 2.0 + 1e-6,
            "{:?}",
            surface.domain()
        );
    }

    #[test]
    fn blends_join_two_surfaces() {
        let low = sheet(4.0, 0.0);
        let high = sheet(4.0, 0.0)
            .transformed([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 7.0, 3.0, 1.0],
            ])
            .unwrap();
        let low_edge = (0..low.record_edge_count())
            .find(|e| {
                low.edge_curve(*e)
                    .unwrap()
                    .sample()
                    .unwrap()
                    .iter()
                    .all(|p| (p[1] - 4.0).abs() < 1e-9)
            })
            .unwrap();
        let high_edge = (0..high.record_edge_count())
            .find(|e| {
                high.edge_curve(*e)
                    .unwrap()
                    .sample()
                    .unwrap()
                    .iter()
                    .all(|p| (p[1] - 7.0).abs() < 1e-9)
            })
            .unwrap();
        for continuity in 0..=2 {
            let blend = low
                .blend_to(low_edge, &high, high_edge, continuity, 1.0)
                .unwrap();
            assert_eq!(blend.face_count().unwrap(), 1, "G{continuity}");
            let joined = Body::assembled(&[low.clone(), blend.clone(), high.clone()]).unwrap();
            assert_eq!(
                joined.naked_edges().unwrap().len(),
                4 + 2 + 2,
                "G{continuity}: the blend meets both edges"
            );
        }
    }

    #[test]
    fn match_moves_an_edge_onto_another() {
        let a = sheet(4.0, 0.0);
        let b = sheet(4.0, 0.0)
            .transformed([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 4.5, 1.0, 1.0],
            ])
            .unwrap();
        let a_edge = (0..a.record_edge_count())
            .find(|e| {
                a.edge_curve(*e)
                    .unwrap()
                    .sample()
                    .unwrap()
                    .iter()
                    .all(|p| (p[1] - 4.0).abs() < 1e-9)
            })
            .unwrap();
        let b_edge = (0..b.record_edge_count())
            .find(|e| {
                b.edge_curve(*e)
                    .unwrap()
                    .sample()
                    .unwrap()
                    .iter()
                    .all(|p| (p[1] - 4.5).abs() < 1e-9)
            })
            .unwrap();
        let matched = a.matched_to(a_edge, &b, b_edge, 1).unwrap();
        let joined = Body::assembled(&[matched, b]).unwrap();
        assert_eq!(
            joined.naked_edges().unwrap().len(),
            6,
            "matched edge sewn to the other"
        );
    }

    #[test]
    fn two_rail_sweep_spans_the_rails() {
        let rail_a = Curve::line([0.0; 3], [0.0, 10.0, 0.0]).unwrap();
        let rail_b = Curve::line([4.0, 0.0, 0.0], [8.0, 10.0, 0.0]).unwrap();
        let profile = Curve::arc(
            [2.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        )
        .unwrap();
        let body = Body::swept_two_rails(&rail_a, &rail_b, &profile, 32).unwrap();
        let mesh = body.mesh().unwrap();
        let far = mesh
            .positions
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((far - 8.0).abs() < 1e-3, "{far}");
    }

    #[test]
    fn deforming_moves_every_face_and_keeps_the_body_whole() {
        let cube = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let taller = cube.deformed(&|p| [p[0], p[1], p[2] * 2.0]).unwrap();
        assert!(taller.is_solid());
        assert!(
            (taller.volume().unwrap() - 2.0).abs() < 1e-3,
            "{}",
            taller.volume().unwrap()
        );
        let wave = Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap();
        let bent = wave
            .deformed(&|p| [p[0], (p[0] / 10.0).powi(2), p[2]])
            .unwrap();
        let end = bent.sample().unwrap().last().copied().unwrap();
        assert!(gap(end, [10.0, 1.0, 0.0]) < 1e-9);
    }

    #[test]
    fn curves_pulled_and_made_on_surfaces() {
        let s = sheet(6.0, 2.0);
        let pulled = s
            .pulled(&Curve::line([1.0, 1.0, 5.0], [5.0, 1.0, 0.0]).unwrap())
            .unwrap();
        assert!(
            pulled
                .sample()
                .unwrap()
                .iter()
                .all(|p| (p[2] - 2.0).abs() < 1e-6)
        );
        let on = s
            .curve_on_face(0, &[[1.0, 1.0, 9.0], [3.0, 4.0, 9.0], [5.0, 2.0, 9.0]])
            .unwrap();
        assert!(
            on.sample()
                .unwrap()
                .iter()
                .all(|p| (p[2] - 2.0).abs() < 1e-6)
        );
        let base = Curve::line([1.0, 1.0, 2.0], [5.0, 1.0, 2.0]).unwrap();
        let moved = s.offset_on_surface(&base, 1.0, [3.0, 5.0, 2.0]).unwrap();
        let p = moved.sample().unwrap()[0];
        assert!(
            (p[1] - 2.0).abs() < 1e-3 && (p[2] - 2.0).abs() < 1e-6,
            "{p:?}"
        );
    }
}
