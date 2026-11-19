//! Rhino's surfaces as Curva's: planes, revolutions and splines, and the curves on them.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A file surface as a Curva surface, untrimmed.
pub(super) fn surface_body(s: &model::Surface) -> Result<Body, KernelError> {
    match s {
        model::Surface::Revolution {
            curve: profile,
            axis,
            angles,
            ..
        } => {
            let profile = curve(profile)?;
            let direction = sub(axis[1], axis[0]);
            let profile = if angles[0].abs() > 1e-12 {
                profile.mapped_affine(turn(axis[0], direction, angles[0]))?
            } else {
                profile
            };
            Body::revolved(&[profile], axis[0], direction, angles[1] - angles[0], false)
        }
        _ => nurbs_surface(s)?.to_body(),
    }
}

/// A turn about the line through `origin` along `axis`, as a row placement.
fn turn(origin: P3, axis: P3, angle: f64) -> [[f64; 4]; 4] {
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let [x, y, z] = axis.map(|v| v / len.max(f64::MIN_POSITIVE));
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    // Column-vector rotation, then transposed into rows.
    let r = [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ];
    let mut m = [[0.0; 4]; 4];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[j][i];
        }
    }
    let moved: P3 = std::array::from_fn(|j| {
        origin[j] - (origin[0] * m[0][j] + origin[1] * m[1][j] + origin[2] * m[2][j])
    });
    m[3] = [moved[0], moved[1], moved[2], 1.0];
    m
}

/// A file surface's NURBS form, on the file's own parameters.
fn nurbs_surface(s: &model::Surface) -> Result<NurbsSurface, KernelError> {
    match s {
        model::Surface::Nurbs(n) => {
            let full = |k: &[f64]| {
                let mut out = vec![*k.first().unwrap_or(&0.0)];
                out.extend_from_slice(k);
                out.push(*k.last().unwrap_or(&1.0));
                out
            };
            clamped_surface(NurbsSurface {
                degree: [n.order[0] - 1, n.order[1] - 1],
                u_knots: full(&n.knots[0]),
                v_knots: full(&n.knots[1]),
                points: n.points.clone(),
            })
        }
        model::Surface::Plane {
            plane,
            domain,
            extents,
        } => {
            let at = |x: f64, y: f64| -> P3 {
                std::array::from_fn(|k| plane.origin[k] + x * plane.x[k] + y * plane.y[k])
            };
            let [[x0, x1], [y0, y1]] = *extents;
            let [[u0, u1], [v0, v1]] = *domain;
            NurbsSurface::new(
                [1, 1],
                vec![u0, u0, u1, u1],
                vec![v0, v0, v1, v1],
                &[vec![at(x0, y0), at(x0, y1)], vec![at(x1, y0), at(x1, y1)]],
                None,
            )
        }
        model::Surface::Sum { curves, base } => {
            let a = portion(
                &curves[0],
                domain_of(&curves[0])[0],
                domain_of(&curves[0])[1],
            )?;
            let b = portion(
                &curves[1],
                domain_of(&curves[1])[0],
                domain_of(&curves[1])[1],
            )?;
            // S(u, v) = A(u) + B(v) + base, exactly: control point (i, j) is a_i + b_j + base
            // and its weight a's weight times b's, which divides back out to the sum. The
            // splines hold their points weighted (`[x w, y w, z w, w]`).
            let plain = |p: &[f64; 4]| -> P3 { std::array::from_fn(|k| p[k] / p[3]) };
            let net: Vec<Vec<P3>> = a
                .points
                .iter()
                .map(|p| {
                    b.points
                        .iter()
                        .map(|q| {
                            let (p, q) = (plain(p), plain(q));
                            std::array::from_fn(|k| p[k] + q[k] + base[k])
                        })
                        .collect()
                })
                .collect();
            let weights: Vec<Vec<f64>> = a
                .points
                .iter()
                .map(|p| b.points.iter().map(|q| p[3] * q[3]).collect())
                .collect();
            let weighted = a
                .points
                .iter()
                .chain(&b.points)
                .any(|p| (p[3] - 1.0).abs() > 1e-12);
            NurbsSurface::new(
                [a.degree, b.degree],
                a.knots,
                b.knots,
                &net,
                weighted.then_some(weights.as_slice()),
            )
        }
        model::Surface::Revolution { .. } => Err(KernelError(
            "a surface of revolution has no NURBS form here".to_owned(),
        )),
    }
}

/// A surface with knots repeated at both ends of its domain, both ways.
fn clamped_surface(s: NurbsSurface) -> Result<NurbsSurface, KernelError> {
    // Along v: each row as a spline; along u: each column.
    let rows: Vec<Spline> = s
        .points
        .iter()
        .map(|row| {
            Spline {
                degree: s.degree[1],
                knots: s.v_knots.clone(),
                points: row.clone(),
            }
            .clamped()
        })
        .collect::<Result<_, _>>()?;
    let v_knots = rows.first().map(|r| r.knots.clone()).unwrap_or_default();
    let net: Vec<Vec<[f64; 4]>> = rows.into_iter().map(|r| r.points).collect();
    let columns: Vec<Spline> = (0..net.first().map_or(0, Vec::len))
        .map(|j| {
            Spline {
                degree: s.degree[0],
                knots: s.u_knots.clone(),
                points: net.iter().map(|row| row[j]).collect(),
            }
            .clamped()
        })
        .collect::<Result<_, _>>()?;
    let u_knots = columns.first().map(|c| c.knots.clone()).unwrap_or_default();
    let count = columns.first().map_or(0, |c| c.points.len());
    let points = (0..count)
        .map(|i| columns.iter().map(|c| c.points[i]).collect())
        .collect();
    Ok(NurbsSurface {
        degree: s.degree,
        u_knots,
        v_knots,
        points,
    })
}

pub(super) fn record_curve(s: &Spline) -> RecordCurve {
    if s.points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12) {
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
}

pub(super) fn record_curve2(s: &Spline) -> Curve2 {
    if s.points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12) {
        Curve2::RationalSpline {
            knots: s.knots.clone(),
            homogeneous: s.points.iter().map(|p| [p[0], p[1], p[3]]).collect(),
        }
    } else {
        Curve2::Spline {
            knots: s.knots.clone(),
            points: s.points.iter().map(|p| [p[0], p[1]]).collect(),
        }
    }
}

/// `x ↦ scale x + shift`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Linear {
    scale: f64,
    shift: f64,
}

impl Linear {
    /// The map taking `from` onto `to`.
    fn between(from: [f64; 2], to: [f64; 2]) -> Self {
        let span = from[1] - from[0];
        let scale = if span.abs() < f64::MIN_POSITIVE {
            1.0
        } else {
            (to[1] - to[0]) / span
        };
        Self {
            scale,
            shift: to[0] - scale * from[0],
        }
    }

    fn at(self, x: f64) -> f64 {
        self.scale * x + self.shift
    }
}

/// How a file surface's parameter along one direction becomes its record surface's.
#[derive(Clone, Debug)]
pub(super) enum Warp {
    /// Linearly, so trim curves carry over exactly.
    Linear(Linear),
    /// Through circular arcs: the file's parameter is linear in the angle, the record's runs
    /// over rational quadratic arcs, one per span of [`arc_spans`].
    Arcs {
        /// The file's parameter to the angle.
        angle: Linear,
        /// The arcs' spans of angle; each span's knots are its angles.
        spans: Vec<[f64; 2]>,
    },
}

impl Warp {
    const SAME: Self = Self::Linear(Linear {
        scale: 1.0,
        shift: 0.0,
    });

    fn at(&self, x: f64) -> f64 {
        match self {
            Self::Linear(l) => l.at(x),
            Self::Arcs { angle, spans } => {
                let theta = angle.at(x);
                let span = spans
                    .iter()
                    .find(|s| theta <= s[1].max(s[0]))
                    .or(spans.last())
                    .copied()
                    .unwrap_or([theta, theta]);
                let half = (span[1] - span[0]) / 2.0;
                if half.abs() < 1e-15 {
                    return span[0];
                }
                // A rational quadratic arc with equal end weights runs linearly in the tangent
                // of the half angle from its middle.
                let mid = span[0] + half;
                let s = 0.5 * (1.0 + ((theta - mid) / 2.0).tan() / (half / 2.0).tan());
                span[0] + s.clamp(0.0, 1.0) * (span[1] - span[0])
            }
        }
    }
}

/// Spans of at most a quarter turn covering `a` to `b`.
fn arc_spans(a: f64, b: f64) -> Vec<[f64; 2]> {
    let n = ((b - a).abs() / FRAC - 1e-9).ceil().max(1.0) as usize;
    let step = (b - a) / n as f64;
    // The last span ends at `b` itself: summed steps can stop a rounding short of it, and the
    // face's range, which ends at `b`, would then reach past the surface's, which the engine
    // refuses.
    let at = |k: usize| if k == n { b } else { a + k as f64 * step };
    (0..n).map(|k| [at(k), at(k + 1)]).collect()
}

/// The knots of arcs over `spans`, and for each control point its angle (a middle point's is
/// its span's middle) and weight.
fn arc_controls(spans: &[[f64; 2]]) -> (Vec<f64>, Vec<(f64, f64)>) {
    let mut knots = Vec::new();
    let mut controls = Vec::new();
    for (k, s) in spans.iter().enumerate() {
        if k == 0 {
            knots.extend([s[0]; 3]);
            controls.push((s[0], 1.0));
        }
        let half = (s[1] - s[0]) / 2.0;
        controls.push((s[0] + half, half.cos()));
        controls.push((s[1], 1.0));
        if k + 1 == spans.len() {
            knots.extend([s[1]; 3]);
        } else {
            knots.extend([s[1]; 2]);
        }
    }
    (knots, controls)
}

/// A point turned about the line through `origin` along unit `axis` by `angle`, pushed out by
/// `1 / weight` from the axis for an arc's middle control point.
fn turned(p: P3, origin: P3, axis: P3, angle: f64, weight: f64) -> P3 {
    let along: f64 = (0..3).map(|k| (p[k] - origin[k]) * axis[k]).sum();
    let foot: P3 = std::array::from_fn(|k| origin[k] + along * axis[k]);
    let r = sub(p, foot);
    let s = curva_kernel::cross(axis, r);
    let (sin, cos) = angle.sin_cos();
    std::array::from_fn(|k| foot[k] + (r[k] * cos + s[k] * sin) / weight)
}

/// An arc of a file as a spline whose knots are angles, spans of at most a quarter turn.
fn arc_spline(plane: &model::Plane, radius: f64, angles: [f64; 2]) -> (Spline, Vec<[f64; 2]>) {
    let spans = arc_spans(angles[0], angles[1]);
    let (knots, controls) = arc_controls(&spans);
    let points = controls
        .iter()
        .map(|(angle, weight)| {
            let (sin, cos) = angle.sin_cos();
            let p: P3 = std::array::from_fn(|k| {
                plane.origin[k] + radius * (cos * plane.x[k] + sin * plane.y[k]) / weight
            });
            [p[0] * weight, p[1] * weight, p[2] * weight, *weight]
        })
        .collect();
    (
        Spline {
            degree: 2,
            knots,
            points,
        },
        spans,
    )
}

/// A record surface, how a file surface's (u, v) become its own, and its domain.
pub(super) type Placed = (RecordSurface, [Warp; 2], [[f64; 2]; 2]);

/// A surface of revolution as a rational spline: its profile turned about the axis, the angle
/// along u, or along v when `transposed`.
fn revolution_record(
    profile: &model::Curve,
    axis: &[P3; 2],
    angles: &[f64; 2],
    domain: &[f64; 2],
    transposed: bool,
) -> Result<Placed, KernelError> {
    let (profile_spline, profile_warp) = profile_of(profile)?;
    let direction = curva_kernel::unit(sub(axis[1], axis[0]))
        .ok_or_else(|| KernelError("a revolution axis of no length".to_owned()))?;
    let spans = arc_spans(angles[0], angles[1]);
    let (angle_knots, controls) = arc_controls(&spans);
    // net[i][j]: i along the angle, j along the profile.
    let net: Vec<Vec<[f64; 4]>> = controls
        .iter()
        .map(|(angle, arc_weight)| {
            profile_spline
                .points
                .iter()
                .map(|h| {
                    let w = h[3];
                    let p = [h[0] / w, h[1] / w, h[2] / w];
                    let q = turned(p, axis[0], direction, *angle, *arc_weight);
                    let ww = w * arc_weight;
                    [q[0] * ww, q[1] * ww, q[2] * ww, ww]
                })
                .collect()
        })
        .collect();
    let angle_warp = Warp::Arcs {
        angle: Linear::between(*domain, *angles),
        spans,
    };
    let (a0, a1) = (angles[0], angles[1]);
    let (p0, p1) = profile_spline.domain();
    let (u_knots, v_knots, points, warps, record_domain) = if transposed {
        // u along the profile: the net turned over.
        let turned_net = (0..profile_spline.points.len())
            .map(|j| net.iter().map(|row| row[j]).collect())
            .collect();
        (
            profile_spline.knots.clone(),
            angle_knots,
            turned_net,
            [profile_warp, angle_warp],
            [[p0, p1], [a0, a1]],
        )
    } else {
        (
            angle_knots,
            profile_spline.knots.clone(),
            net,
            [angle_warp, profile_warp],
            [[a0, a1], [p0, p1]],
        )
    };
    Ok((
        RecordSurface::RationalSpline {
            u_knots,
            v_knots,
            homogeneous: points,
        },
        warps,
        record_domain,
    ))
}

/// A revolution's profile as a spline, arcs on angle knots so its parameter is known, with how
/// its parameter maps to the file's.
fn profile_of(profile: &model::Curve) -> Result<(Spline, Warp), KernelError> {
    Ok(match profile {
        model::Curve::Arc {
            plane,
            radius,
            angles: arc_angles,
            domain: arc_domain,
        } => {
            let (spline, spans) = arc_spline(plane, *radius, *arc_angles);
            (
                spline,
                Warp::Arcs {
                    angle: Linear::between(*arc_domain, *arc_angles),
                    spans,
                },
            )
        }
        other => {
            let d = domain_of(other);
            (portion(other, d[0], d[1])?, Warp::SAME)
        }
    })
}

/// A file surface as the engine's record surface, with how the file's (u, v) become the
/// record's, and the record's domain.
pub(super) fn record_surface(s: &model::Surface) -> Result<Placed, KernelError> {
    match s {
        model::Surface::Plane {
            plane,
            domain,
            extents,
        } => Ok((
            RecordSurface::Plane {
                origin: plane.origin,
                u_axis: plane.x,
                v_axis: plane.y,
            },
            [
                Warp::Linear(Linear::between(domain[0], extents[0])),
                Warp::Linear(Linear::between(domain[1], extents[1])),
            ],
            *extents,
        )),
        model::Surface::Revolution {
            curve: profile,
            axis,
            angles,
            domain,
            transposed,
        } => revolution_record(profile, axis, angles, domain, *transposed),
        _ => {
            let n = nurbs_surface(s)?;
            let domain = n.domain();
            Ok((n.as_record(), [Warp::SAME, Warp::SAME], domain))
        }
    }
}

/// A trim curve in the file surface's parameters taken onto the record surface's: exactly
/// where both maps are linear, as a straight line where it lies along one parameter, and
/// otherwise through closely spaced points.
pub(super) fn warped(pcurve: Spline, warps: &[Warp; 2]) -> Spline {
    if let [Warp::Linear(u), Warp::Linear(v)] = warps {
        let mut out = pcurve;
        for p in &mut out.points {
            let w = p[3];
            p[0] = u.scale * p[0] + u.shift * w;
            p[1] = v.scale * p[1] + v.shift * w;
            p[2] = 0.0;
        }
        return out;
    }
    let (a, b) = pcurve.domain();
    let count = (8 * pcurve.points.len()).clamp(16, 256);
    let samples: Vec<P3> = (0..=count)
        .map(|i| {
            let p = pcurve.point(a + (b - a) * i as f64 / count as f64);
            [warps[0].at(p[0]), warps[1].at(p[1]), 0.0]
        })
        .collect();
    let first = samples[0];
    let last = samples[samples.len() - 1];
    let along = |k: usize| {
        samples
            .iter()
            .all(|p| (p[k] - first[k]).abs() <= 1e-12 * (1.0 + first[k].abs()))
    };
    let points: Vec<[f64; 4]> = if along(0) || along(1) {
        vec![w(first), w(last)]
    } else {
        samples.iter().map(|p| w(*p)).collect()
    };
    let n = points.len();
    let mut knots = vec![a, a];
    for i in 1..n - 1 {
        knots.push(a + (b - a) * i as f64 / (n - 1) as f64);
    }
    knots.extend([b, b]);
    Spline {
        degree: 1,
        knots,
        points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quarter of a cylinder as Rhino can hold it: a weighted quarter circle summed with a
    /// straight line up. It used to be left out ("a sum surface of rational curves").
    #[test]
    fn a_sum_surface_of_a_weighted_arc_is_the_exact_cylinder() {
        let (r, h) = (5.0, 10.0);
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let arc = model::Curve::Nurbs(model::NurbsCurve {
            dimension: 3,
            order: 3,
            knots: vec![0.0, 0.0, 1.0, 1.0],
            points: vec![
                [r, 0.0, 0.0, 1.0],
                [r * w, r * w, 0.0, w],
                [0.0, r, 0.0, 1.0],
            ],
            rational: true,
        });
        let up = model::Curve::Nurbs(model::NurbsCurve {
            dimension: 3,
            order: 2,
            knots: vec![0.0, 1.0],
            points: vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, h, 1.0]],
            rational: false,
        });
        let sum = model::Surface::Sum {
            curves: [Box::new(arc), Box::new(up)],
            base: [100.0, 0.0, 0.0],
        };
        let body = surface_body(&sum).unwrap();
        let area = body.area().unwrap();
        let quarter = std::f64::consts::FRAC_PI_2 * r * h;
        assert!(
            (area - quarter).abs() < 1e-6 * quarter,
            "{area} against {quarter}"
        );
        let (low, high) = body.bounds().unwrap();
        assert!(
            (low[0] - 100.0).abs() < 1e-6 && (high[0] - 105.0).abs() < 1e-6,
            "{low:?} {high:?}"
        );
        assert!((high[2] - h).abs() < 1e-6);
    }

    /// A piece of a cylinder turned through an angle whose quarter-turn steps add up to a rounding
    /// short of its end, as in one of the owner's files. It used to be left out ("parameter interval
    /// outside the natural domain").
    #[test]
    fn a_revolution_whose_steps_fall_short_of_its_angle_comes_in() {
        let angles = [0.277_174_184_114_610_4, 3.488_076_299_119_063];
        let spans = arc_spans(angles[0], angles[1]);
        assert_eq!(spans.last().map(|s| s[1]), Some(angles[1]));
        let (r, h) = (2.0, 3.0);
        let side = model::Curve::Line {
            ends: [[r, 0.0, 0.0], [r, 0.0, h]],
            domain: [0.0, h],
        };
        let surface = model::Surface::Revolution {
            curve: Box::new(side),
            axis: [[0.0; 3], [0.0, 0.0, 1.0]],
            angles,
            domain: angles,
            transposed: false,
        };
        let area = surface_body(&surface).unwrap().area().unwrap();
        let expected = (angles[1] - angles[0]) * r * h;
        assert!(
            (area - expected).abs() < 1e-6 * expected,
            "{area} against {expected}"
        );
    }
}
