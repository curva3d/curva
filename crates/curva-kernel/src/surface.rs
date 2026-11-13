//! NURBS surfaces worked on directly, the surface sibling of `spline.rs`: evaluating, isocurves,
//! knot insertion and cutting to a box of parameters, reading a face's surface from a record and
//! making a one-face body from a surface. The engine makes sheets from surfaces only inside itself,
//! so building the record here is Curva's own.

// The algorithms keep the book's index loops, so they can be checked against it line by line.
#![allow(clippy::needless_range_loop)]

// CUSTOM KERNEL OPERATION: surface-toolkit
use crate::body::Body;
use crate::{KernelError, Point, Spline};
use solidcore_record::brep::{
    Brep as Record, Curve as RecordCurve, Curve2, Edge, EdgeUseRef, Face, Loop, OrientedEdge,
    OrientedFace, Region, Shell, Surface, Vertex,
};

/// A clamped NURBS surface with homogeneous control points, `points[i][j]` the `i`-th along u
/// and `j`-th along v.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface {
    /// Degrees along u and v.
    pub degree: [usize; 2],
    /// Knots along u.
    pub u_knots: Vec<f64>,
    /// Knots along v.
    pub v_knots: Vec<f64>,
    /// The control net, weighted: `[x w, y w, z w, w]`.
    pub points: Vec<Vec<[f64; 4]>>,
}

fn bad(why: &str) -> KernelError {
    KernelError(format!("not a usable NURBS surface: {why}"))
}

impl NurbsSurface {
    /// A surface from its degrees, knots and unweighted net (`net[i][j]`, i along u).
    pub fn new(
        degree: [usize; 2],
        u_knots: Vec<f64>,
        v_knots: Vec<f64>,
        net: &[Vec<Point>],
        weights: Option<&[Vec<f64>]>,
    ) -> Result<Self, KernelError> {
        let points = net
            .iter()
            .enumerate()
            .map(|(i, row)| {
                row.iter()
                    .enumerate()
                    .map(|(j, p)| {
                        let w = weights.map_or(1.0, |w| w[i][j]);
                        [p[0] * w, p[1] * w, p[2] * w, w]
                    })
                    .collect()
            })
            .collect();
        let surface = Self {
            degree,
            u_knots,
            v_knots,
            points,
        };
        surface.check()?;
        Ok(surface)
    }

    fn check(&self) -> Result<(), KernelError> {
        let rows = self.points.len();
        let cols = self.points.first().map_or(0, Vec::len);
        if rows <= self.degree[0] || cols <= self.degree[1] || self.degree.contains(&0) {
            return Err(bad("too few control points for its degrees"));
        }
        if self.points.iter().any(|row| row.len() != cols) {
            return Err(bad("the net is not rectangular"));
        }
        if self.u_knots.len() != rows + self.degree[0] + 1
            || self.v_knots.len() != cols + self.degree[1] + 1
        {
            return Err(bad("wrong number of knots"));
        }
        Ok(())
    }

    /// Its parameter box, `[[u0, u1], [v0, v1]]`.
    #[must_use]
    pub fn domain(&self) -> [[f64; 2]; 2] {
        let (rows, cols) = (self.points.len(), self.points[0].len());
        [
            [self.u_knots[self.degree[0]], self.u_knots[rows]],
            [self.v_knots[self.degree[1]], self.v_knots[cols]],
        ]
    }

    /// The u-direction curve through column `j` of the net.
    fn column(&self, j: usize) -> Spline {
        Spline {
            degree: self.degree[0],
            knots: self.u_knots.clone(),
            points: self.points.iter().map(|row| row[j]).collect(),
        }
    }

    /// The v-direction curve through row `i` of the net.
    fn row(&self, i: usize) -> Spline {
        Spline {
            degree: self.degree[1],
            knots: self.v_knots.clone(),
            points: self.points[i].clone(),
        }
    }

    /// The curve along v at `u` (exact).
    #[must_use]
    pub fn iso_u(&self, u: f64) -> Spline {
        let cols = self.points[0].len();
        Spline {
            degree: self.degree[1],
            knots: self.v_knots.clone(),
            points: (0..cols).map(|j| self.column(j).homogeneous(u)).collect(),
        }
    }

    /// The curve along u at `v` (exact).
    #[must_use]
    pub fn iso_v(&self, v: f64) -> Spline {
        let rows = self.points.len();
        Spline {
            degree: self.degree[0],
            knots: self.u_knots.clone(),
            points: (0..rows).map(|i| self.row(i).homogeneous(v)).collect(),
        }
    }

    /// The point at `(u, v)`.
    #[must_use]
    pub fn point(&self, u: f64, v: f64) -> Point {
        self.iso_u(u).point(v)
    }

    /// The unit normal at `(u, v)`, by small differences.
    #[must_use]
    pub fn normal(&self, u: f64, v: f64) -> Point {
        let du = self.iso_v(v).derivative(u);
        let dv = self.iso_u(u).derivative(v);
        crate::unit(crate::cross(du, dv)).unwrap_or([0.0, 0.0, 1.0])
    }

    /// The same surface with `u` (or, when `along_v`, `v`) inserted `times` more times.
    #[must_use]
    pub fn inserted(&self, along_v: bool, at: f64, times: usize) -> Self {
        let mut out = self.clone();
        if along_v {
            let rows: Vec<Spline> = (0..self.points.len())
                .map(|i| self.row(i).inserted(at, times))
                .collect();
            out.v_knots = rows[0].knots.clone();
            out.points = rows.into_iter().map(|r| r.points).collect();
        } else {
            let cols: Vec<Spline> = (0..self.points[0].len())
                .map(|j| self.column(j).inserted(at, times))
                .collect();
            out.u_knots = cols[0].knots.clone();
            let rows = cols[0].points.len();
            out.points = (0..rows)
                .map(|i| cols.iter().map(|c| c.points[i]).collect())
                .collect();
        }
        out
    }

    /// The part of the surface over `[[u0, u1], [v0, v1]]`, keeping its parameters.
    #[must_use]
    pub fn cut_to(&self, bounds: [[f64; 2]; 2]) -> Self {
        let cut = |curve: Spline, [lo, hi]: [f64; 2]| -> Spline {
            let mut c = curve;
            if let Some((_, right)) = c.split(lo) {
                c = right;
            }
            if let Some((left, _)) = c.split(hi) {
                c = left;
            }
            c
        };
        let mut s = self.clone();
        // Along u: cut every column.
        let cols: Vec<Spline> = (0..s.points[0].len())
            .map(|j| cut(s.column(j), bounds[0]))
            .collect();
        s.u_knots = cols[0].knots.clone();
        s.points = (0..cols[0].points.len())
            .map(|i| cols.iter().map(|c| c.points[i]).collect())
            .collect();
        // Along v: cut every row.
        let rows: Vec<Spline> = (0..s.points.len())
            .map(|i| cut(s.row(i), bounds[1]))
            .collect();
        s.v_knots = rows[0].knots.clone();
        s.points = rows.into_iter().map(|r| r.points).collect();
        s
    }

    /// The surface a record face lies on, in the face's own parameters, if it is (or has the
    /// NURBS form of) a spline or a plane.
    pub fn of_face(record: &Record, face: usize) -> Result<Self, KernelError> {
        let face = record.faces.get(face).ok_or_else(|| bad("no such face"))?;
        let surface = face.nurbs.as_ref().unwrap_or(&face.surface);
        let degree_of = |knots: &[f64], count: usize| knots.len().saturating_sub(count + 1);
        match surface {
            Surface::Spline {
                u_knots,
                v_knots,
                points,
            } => Self::new(
                [
                    degree_of(u_knots, points.len()),
                    degree_of(v_knots, points[0].len()),
                ],
                u_knots.clone(),
                v_knots.clone(),
                points,
                None,
            ),
            Surface::RationalSpline {
                u_knots,
                v_knots,
                homogeneous,
            } => {
                let s = Self {
                    degree: [
                        degree_of(u_knots, homogeneous.len()),
                        degree_of(v_knots, homogeneous[0].len()),
                    ],
                    u_knots: u_knots.clone(),
                    v_knots: v_knots.clone(),
                    points: homogeneous.clone(),
                };
                s.check()?;
                Ok(s)
            }
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
            } => {
                let [[u0, u1], [v0, v1]] = face.domain;
                let at = |u: f64, v: f64| -> Point {
                    std::array::from_fn(|k| origin[k] + u * u_axis[k] + v * v_axis[k])
                };
                Self::new(
                    [1, 1],
                    vec![u0, u0, u1, u1],
                    vec![v0, v0, v1, v1],
                    &[vec![at(u0, v0), at(u0, v1)], vec![at(u1, v0), at(u1, v1)]],
                    None,
                )
            }
            _ => Err(bad("this face's surface has no NURBS form in the record")),
        }
    }

    fn record_surface(&self) -> Surface {
        let rational = self
            .points
            .iter()
            .flatten()
            .any(|p| (p[3] - 1.0).abs() > 1e-12);
        if rational {
            Surface::RationalSpline {
                u_knots: self.u_knots.clone(),
                v_knots: self.v_knots.clone(),
                homogeneous: self.points.clone(),
            }
        } else {
            Surface::Spline {
                u_knots: self.u_knots.clone(),
                v_knots: self.v_knots.clone(),
                points: self
                    .points
                    .iter()
                    .map(|row| row.iter().map(|p| [p[0], p[1], p[2]]).collect())
                    .collect(),
            }
        }
    }

    /// The surface as a one-face body, untrimmed, its four sides the edges.
    pub fn to_body(&self) -> Result<Body, KernelError> {
        let [[u0, u1], [v0, v1]] = self.domain();
        let tolerance = 1e-5;
        let corner = |u: f64, v: f64| Vertex {
            point: self.point(u, v),
            tolerance,
        };
        // Vertices: (u0, v0), (u0, v1), (u1, v0), (u1, v1).
        let vertices = vec![
            corner(u0, v0),
            corner(u0, v1),
            corner(u1, v0),
            corner(u1, v1),
        ];
        let curve_of = |s: &Spline| -> RecordCurve {
            let rational = s.points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12);
            if rational {
                RecordCurve::RationalSpline {
                    knots: s.knots.clone(),
                    homogeneous: s.points.clone(),
                }
            } else {
                RecordCurve::Spline {
                    knots: s.knots.clone(),
                    points: s.points.iter().map(|p| [p[0], p[1], p[2]]).collect(),
                }
            }
        };
        // Each side: its curve (running from its first vertex to its second), its interval, its
        // two vertices, its pcurve's ends in (u, v), and whether the loop runs it forward.
        let sides = [
            (
                self.iso_u(u0),
                [v0, v1],
                [0u32, 1],
                [[u0, v0], [u0, v1]],
                false,
            ),
            (self.iso_v(v0), [u0, u1], [0, 2], [[u0, v0], [u1, v0]], true),
            (self.iso_u(u1), [v0, v1], [2, 3], [[u1, v0], [u1, v1]], true),
            (
                self.iso_v(v1),
                [u0, u1],
                [1, 3],
                [[u0, v1], [u1, v1]],
                false,
            ),
        ];
        let mut edges = Vec::new();
        let mut uses = Vec::new();
        for (position, (curve, interval, ends, uv, forward)) in sides.iter().enumerate() {
            edges.push(Edge {
                curve: curve_of(curve),
                nurbs: None,
                interval: *interval,
                vertices: *ends,
                tolerance,
                uses: vec![EdgeUseRef {
                    face: 0,
                    r#loop: 0,
                    position: position as u32,
                    upper: !forward,
                }],
            });
            uses.push(OrientedEdge {
                edge: position as u32,
                forward: *forward,
                pcurve: Curve2::Spline {
                    knots: vec![interval[0], interval[0], interval[1], interval[1]],
                    points: vec![uv[0], uv[1]],
                },
            });
        }
        let record = Record {
            version: solidcore_record::brep::VERSION,
            tolerance,
            regions: vec![Region {
                shells: vec![0],
                void: false,
            }],
            shells: vec![Shell::Faces {
                faces: vec![OrientedFace {
                    face: 0,
                    forward: true,
                }],
                wires: Vec::new(),
            }],
            faces: vec![Face {
                surface: self.record_surface(),
                nurbs: None,
                domain: [[u0, u1], [v0, v1]],
                loops: vec![Loop::Edges { edges: uses }],
            }],
            edges,
            vertices,
        };
        Body::from_record(&record)
    }
}

impl Spline {
    /// The weighted point at `u`, for building nets.
    #[must_use]
    pub fn homogeneous(&self, u: f64) -> [f64; 4] {
        let p = self.point(u);
        // The weight at u, from the weights' own curve.
        let w = Spline {
            degree: self.degree,
            knots: self.knots.clone(),
            points: self.points.iter().map(|q| [q[3], 0.0, 0.0, 1.0]).collect(),
        }
        .point(u)[0];
        [p[0] * w, p[1] * w, p[2] * w, w]
    }

    /// Curves made compatible: one degree, one parameter range `[0, 1]` and one knot vector,
    /// each the same shape as before.
    pub fn compatible(curves: &[Spline]) -> Result<Vec<Spline>, KernelError> {
        let degree = curves
            .iter()
            .map(|c| c.degree)
            .max()
            .ok_or_else(|| bad("no curves"))?;
        let mut work: Vec<Spline> = curves
            .iter()
            .map(|c| c.elevated(degree - c.degree).reparameterized(0.0, 1.0))
            .collect();
        // Every inner knot of every curve, as many times as the most any curve has it.
        let mut wanted: Vec<(f64, usize)> = Vec::new();
        for c in &work {
            let inner = &c.knots[degree + 1..c.points.len()];
            let mut seen: Vec<(f64, usize)> = Vec::new();
            for k in inner {
                match seen.iter_mut().find(|(v, _)| (*v - k).abs() < 1e-12) {
                    Some((_, n)) => *n += 1,
                    None => seen.push((*k, 1)),
                }
            }
            for (k, n) in seen {
                match wanted.iter_mut().find(|(v, _)| (*v - k).abs() < 1e-12) {
                    Some((_, m)) => *m = (*m).max(n),
                    None => wanted.push((k, n)),
                }
            }
        }
        for c in &mut work {
            for (k, n) in &wanted {
                let have = c.knots.iter().filter(|v| (**v - k).abs() < 1e-12).count();
                if have < *n {
                    *c = c.inserted(*k, n - have);
                }
            }
        }
        Ok(work)
    }
}

#[cfg(test)]
mod tests {
    use super::NurbsSurface;
    use crate::{Spline, gap};

    fn wavy() -> NurbsSurface {
        let net: Vec<Vec<[f64; 3]>> = (0..4)
            .map(|i| {
                (0..3)
                    .map(|j| {
                        [
                            f64::from(i) * 2.0,
                            f64::from(j) * 3.0,
                            if i == 1 && j == 1 { 2.0 } else { 0.0 },
                        ]
                    })
                    .collect()
            })
            .collect();
        NurbsSurface::new(
            [3, 2],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            &net,
            None,
        )
        .unwrap()
    }

    #[test]
    fn a_surface_becomes_a_body_with_its_shape() {
        let s = wavy();
        let body = s.to_body().unwrap();
        assert_eq!(body.face_count().unwrap(), 1);
        assert_eq!(body.naked_edges().unwrap().len(), 4);
        // Read back from the body's record, the surface is the same.
        let back = NurbsSurface::of_face(&body.record().unwrap(), 0).unwrap();
        for (u, v) in [(0.2, 0.3), (0.5, 0.5), (0.9, 0.1)] {
            assert!(gap(s.point(u, v), back.point(u, v)) < 1e-9);
        }
        assert!(body.area().unwrap() > 36.0);
    }

    #[test]
    fn isocurves_and_cutting_keep_the_surface() {
        let s = wavy();
        let iso = s.iso_u(0.4);
        assert!(gap(iso.point(0.7), s.point(0.4, 0.7)) < 1e-12);
        let part = s.cut_to([[0.2, 0.8], [0.1, 0.6]]);
        assert_eq!(part.domain(), [[0.2, 0.8], [0.1, 0.6]]);
        assert!(gap(part.point(0.5, 0.3), s.point(0.5, 0.3)) < 1e-9);
    }

    #[test]
    fn curves_made_compatible_share_their_knots() {
        let a = Spline::of(
            &crate::Curve::control_point(
                vec![[0.0; 3], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
                2,
                false,
            )
            .unwrap(),
        )
        .unwrap();
        let b = Spline::of(
            &crate::Curve::control_point(
                vec![
                    [0.0, 3.0, 0.0],
                    [1.0, 4.0, 0.0],
                    [2.0, 2.0, 0.0],
                    [3.0, 3.0, 0.0],
                    [4.0, 3.0, 0.0],
                ],
                3,
                false,
            )
            .unwrap(),
        )
        .unwrap();
        let both = Spline::compatible(&[a.clone(), b]).unwrap();
        assert_eq!(both[0].knots, both[1].knots);
        assert_eq!(both[0].degree, 3);
        for i in 0..=10 {
            let t = f64::from(i) / 10.0;
            assert!(gap(both[0].point(t), a.point(t)) < 1e-9);
        }
    }
}
