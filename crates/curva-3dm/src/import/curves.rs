//! Rhino's curves as Curva's: lines, arcs and circles kept exact, splines, and parts of curves.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A file curve as a Curva curve, each kind as itself where Curva has it.
pub(super) fn curve(c: &model::Curve) -> Result<Curve, KernelError> {
    match c {
        model::Curve::Line { ends, .. } => Curve::line(ends[0], ends[1]),
        model::Curve::Polyline { points, .. } => {
            let closed =
                points.len() > 3 && curva_kernel::gap(points[0], points[points.len() - 1]) < 1e-12;
            let mut points = points.clone();
            if closed {
                points.pop();
            }
            if points.len() == 2 && !closed {
                return Curve::line(points[0], points[1]);
            }
            Curve::polyline(points, closed)
        }
        model::Curve::Arc {
            plane,
            radius,
            angles,
            ..
        } => {
            let sweep = angles[1] - angles[0];
            let dir = |a: f64| -> P3 {
                std::array::from_fn(|k| a.cos() * plane.x[k] + a.sin() * plane.y[k])
            };
            if (sweep - std::f64::consts::TAU).abs() < 1e-9 {
                Curve::circle_on(plane.origin, *radius, dir(angles[0]), dir(angles[0] + FRAC))
            } else {
                Ok(Curve::Arc {
                    centre: plane.origin,
                    radius: *radius,
                    x: dir(angles[0]),
                    y: dir(angles[0] + FRAC),
                    sweep,
                })
            }
        }
        model::Curve::Nurbs(n) => {
            let spline = spline_of(n)?;
            match as_circle(&spline) {
                Some(circle) => Ok(circle),
                None => spline.to_curve(),
            }
        }
        model::Curve::Poly { segments, .. } => {
            let parts = segments.iter().map(curve).collect::<Result<Vec<_>, _>>()?;
            if parts.len() == 1 {
                return Ok(parts
                    .into_iter()
                    .next()
                    .unwrap_or(Curve::Point { at: [0.0; 3] }));
            }
            Curve::compound(parts)
        }
    }
}

pub(super) const FRAC: f64 = std::f64::consts::FRAC_PI_2;

/// A rational quadratic spline that is exactly a circle or an arc of one, as Curva's own circle
/// or arc, as Rhino writes circles in NURBS form.
fn as_circle(s: &Spline) -> Option<Curve> {
    if s.degree != 2 || s.points.iter().all(|p| (p[3] - 1.0).abs() < 1e-12) {
        return None;
    }
    let (a, b) = s.domain();
    let at = |f: f64| s.point(a + (b - a) * f);
    let start = at(0.0);
    let end = at(1.0);
    let closed =
        curva_kernel::gap(start, end) < 1e-9 * (1.0 + start.iter().map(|v| v.abs()).sum::<f64>());
    let (p, q) = if closed {
        (at(1.0 / 3.0), at(2.0 / 3.0))
    } else {
        (at(0.5), end)
    };
    // The circle through start, p and q.
    let u = sub(p, start);
    let v = sub(q, start);
    let n = curva_kernel::cross(u, v);
    let nn: f64 = n.iter().map(|x| x * x).sum();
    if nn < 1e-24 {
        return None;
    }
    let uu: f64 = u.iter().map(|x| x * x).sum();
    let vv: f64 = v.iter().map(|x| x * x).sum();
    let t1 = curva_kernel::cross(n, u);
    let t2 = curva_kernel::cross(v, n);
    let centre: P3 = std::array::from_fn(|k| start[k] + (vv * t1[k] + uu * t2[k]) / (2.0 * nn));
    let radius = curva_kernel::gap(centre, start);
    let normal = curva_kernel::unit(n)?;
    for i in 0..=48 {
        let x = at(f64::from(i) / 48.0);
        let off: f64 = (0..3).map(|k| (x[k] - centre[k]) * normal[k]).sum();
        if (curva_kernel::gap(centre, x) - radius).abs() > 1e-9 * radius.max(1.0)
            || off.abs() > 1e-9 * radius.max(1.0)
        {
            return None;
        }
    }
    let x = curva_kernel::unit(sub(start, centre))?;
    let y = curva_kernel::cross(normal, x);
    if closed {
        return Curve::circle_on(centre, radius, x, y).ok();
    }
    let d = sub(end, centre);
    let along: f64 = (0..3).map(|k| d[k] * x[k]).sum();
    let across: f64 = (0..3).map(|k| d[k] * y[k]).sum();
    let mut sweep = across.atan2(along);
    if sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    Some(Curve::Arc {
        centre,
        radius,
        x,
        y,
        sweep,
    })
}

/// A NURBS curve as a clamped spline, keeping its parameter.
fn spline_of(n: &model::NurbsCurve) -> Result<Spline, KernelError> {
    let mut knots = Vec::with_capacity(n.knots.len() + 2);
    knots.push(*n.knots.first().unwrap_or(&0.0));
    knots.extend_from_slice(&n.knots);
    knots.push(*n.knots.last().unwrap_or(&1.0));
    Spline {
        degree: n.order - 1,
        knots,
        points: n.points.clone(),
    }
    .clamped()
}

/// The part of a file curve between parameters `a` and `b` of it, as a spline on `[a, b]`.
pub(super) fn portion(c: &model::Curve, a: f64, b: f64) -> Result<Spline, KernelError> {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    Ok(match c {
        model::Curve::Line { ends, domain } => {
            let p = |s: f64| lerp(ends[0], ends[1], fraction(*domain, s));
            Spline {
                degree: 1,
                knots: vec![a, a, b, b],
                points: vec![w(p(a)), w(p(b))],
            }
        }
        model::Curve::Polyline { points, params } => {
            let mut knots = vec![params.first().copied().unwrap_or(0.0)];
            knots.extend_from_slice(params);
            knots.push(params.last().copied().unwrap_or(1.0));
            cut(
                Spline {
                    degree: 1,
                    knots,
                    points: points.iter().map(|p| w(*p)).collect(),
                },
                a,
                b,
            )
        }
        model::Curve::Arc {
            plane,
            radius,
            angles,
            domain,
        } => {
            let angle = |s: f64| angles[0] + fraction(*domain, s) * (angles[1] - angles[0]);
            let (from, to) = (angle(a), angle(b));
            let dir = |t: f64| -> P3 {
                std::array::from_fn(|k| t.cos() * plane.x[k] + t.sin() * plane.y[k])
            };
            let arc = Curve::Arc {
                centre: plane.origin,
                radius: *radius,
                x: dir(from),
                y: dir(from + FRAC),
                sweep: (to - from).abs().max(1e-12),
            };
            Spline::of(&arc)?.reparameterized(a, b)
        }
        model::Curve::Nurbs(n) => cut(spline_of(n)?, a, b),
        model::Curve::Poly { segments, params } => {
            let mut parts = Vec::new();
            for (i, segment) in segments.iter().enumerate() {
                let (p0, p1) = (params[i], params.get(i + 1).copied().unwrap_or(params[i]));
                let (lo, hi) = (a.max(p0), b.min(p1));
                if hi - lo <= 1e-12 * (1.0 + hi.abs()) {
                    continue;
                }
                let own = domain_of(segment);
                let local = |s: f64| own[0] + fraction([p0, p1], s) * (own[1] - own[0]);
                parts.push(portion(segment, local(lo), local(hi))?.reparameterized(lo, hi));
            }
            let joined = Spline::joined(&parts)?;
            joined.reparameterized(a, b)
        }
    })
}

/// A curve's parameter interval.
pub(super) fn domain_of(c: &model::Curve) -> [f64; 2] {
    match c {
        model::Curve::Line { domain, .. } | model::Curve::Arc { domain, .. } => *domain,
        model::Curve::Polyline { params, .. } | model::Curve::Poly { params, .. } => [
            params.first().copied().unwrap_or(0.0),
            params.last().copied().unwrap_or(1.0),
        ],
        model::Curve::Nurbs(n) => {
            let d = n.order - 2;
            [
                n.knots.get(d).copied().unwrap_or(0.0),
                n.knots.get(n.points.len() - 1).copied().unwrap_or(1.0),
            ]
        }
    }
}

pub(super) fn w(p: P3) -> [f64; 4] {
    [p[0], p[1], p[2], 1.0]
}

/// The spline between `a` and `b`.
fn cut(s: Spline, a: f64, b: f64) -> Spline {
    let mut s = s;
    if let Some((_, right)) = s.split(a) {
        s = right;
    }
    if let Some((left, _)) = s.split(b) {
        s = left;
    }
    s
}
