//! Sweeping along one rail as Rhino's Sweep1 does (#665, #666, measured by the testing stream):
//! one surface along the whole rail through every cross section, blending from one to the next,
//! and a rail with corners swept piece by piece, the pieces meeting in mitres.

use crate::{Body, Curve, KernelError, Point, Similarity, Spline, cross, dot, gap, sub, unit};

fn bad(why: &str) -> KernelError {
    KernelError(why.to_owned())
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

/// How many places along the rail a blended sweep is skinned through, besides the sections.
const STATIONS: usize = 24;

/// How many places along the rail its turning frames are worked out at.
const FRAMES: usize = 256;

/// A place along the rail and the frame there: the tangent and two directions across it.
#[derive(Clone, Copy)]
struct Frame {
    at: Point,
    axes: [Point; 3],
}

/// Frames along `rail` that turn as little as they can (rotation-minimising, by double
/// reflection), at `FRAMES + 1` even steps of its parameter, with those parameters.
fn frames(rail: &Spline) -> Result<Vec<(f64, Frame)>, KernelError> {
    let (start, end) = rail.domain();
    let tangent = |u: f64| unit(rail.derivative(u)).ok_or_else(|| bad("the rail stops turning"));
    let t0 = tangent(start)?;
    let pick = if t0[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let n0 = unit(cross(t0, pick)).ok_or_else(|| bad("the rail has no direction"))?;
    let mut out = vec![(
        start,
        Frame {
            at: rail.point(start),
            axes: [t0, n0, cross(t0, n0)],
        },
    )];
    for k in 1..=FRAMES {
        let u = start + (end - start) * k as f64 / FRAMES as f64;
        let (_, previous) = out[k - 1];
        let (x0, [t0, n0, _]) = (previous.at, previous.axes);
        let (x1, t1) = (rail.point(u), tangent(u)?);
        // Reflect across the plane halfway between the two places, then across the plane
        // halfway between the reflected tangent and the new one.
        let v1 = sub(x1, x0);
        let c1 = dot(v1, v1);
        let (n_l, t_l) = if c1 > 1e-24 {
            (
                plus(n0, v1, -2.0 * dot(v1, n0) / c1),
                plus(t0, v1, -2.0 * dot(v1, t0) / c1),
            )
        } else {
            (n0, t0)
        };
        let v2 = sub(t1, t_l);
        let c2 = dot(v2, v2);
        let n1 = if c2 > 1e-24 {
            plus(n_l, v2, -2.0 * dot(v2, n_l) / c2)
        } else {
            n_l
        };
        let n1 = unit(sub(n1, t1.map(|v| v * dot(n1, t1)))).unwrap_or(n_l);
        out.push((
            u,
            Frame {
                at: x1,
                axes: [t1, n1, cross(t1, n1)],
            },
        ));
    }
    Ok(out)
}

/// The frame at parameter `u`, from the nearest worked out.
fn frame_at(frames: &[(f64, Frame)], u: f64) -> Frame {
    frames
        .iter()
        .min_by(|a, b| (a.0 - u).abs().total_cmp(&(b.0 - u).abs()))
        .map_or(frames[0].1, |(_, f)| *f)
}

/// `p` in frame `f`'s own coordinates, and back.
fn local(f: &Frame, p: Point) -> Point {
    let d = sub(p, f.at);
    f.axes.map(|axis| dot(d, axis))
}

fn world(f: &Frame, l: Point) -> Point {
    let mut p = f.at;
    for (axis, amount) in f.axes.iter().zip(l) {
        p = plus(p, *axis, amount);
    }
    p
}

/// Where along the rail a section sits: the rail's point nearest the middle of its points.
fn station(rail: &Spline, section: &Curve) -> Result<f64, KernelError> {
    let points = section.divided(32)?;
    let count = points.len() as f64;
    let middle: Point = std::array::from_fn(|i| points.iter().map(|p| p[i]).sum::<f64>() / count);
    Ok(rail.closest(middle))
}

/// Whether the rail is made of straight pieces with corners between them.
fn corners(rail: &Curve) -> Option<Vec<Point>> {
    match rail {
        Curve::Polyline {
            points,
            closed: false,
        } if points.len() >= 3 => Some(points.clone()),
        Curve::Polyline { .. } => None,
        // A degree-1 curve is a polyline through its control points, as Rhino treats it (an
        // imported L, or one rebuilt at degree 1, #666).
        _ => {
            let spline = Spline::of(rail).ok()?;
            let points = spline.control_points();
            let (a, b) = (points.first()?, points.last()?);
            (spline.degree == 1 && points.len() >= 3 && crate::gap(*a, *b) > 1e-9).then_some(points)
        }
    }
}

impl Body {
    /// The sections swept along `rail` as Rhino's Sweep1 makes it: one surface along the whole
    /// rail through every section, each held where it lies and the shape blending from one to
    /// the next; a rail of straight pieces swept piece by piece, meeting in mitres.
    pub fn swept_along(sections: &[Curve], rail: &Curve) -> Result<Self, KernelError> {
        if sections.is_empty() {
            return Err(bad("there is no cross section to sweep"));
        }
        if let (Some(points), [section]) = (corners(rail), sections) {
            return mitred(section, &points);
        }
        let spline = Spline::of(rail)?.clamped()?;
        let frames = frames(&spline)?;
        let (start, _) = spline.domain();
        let mut placed: Vec<(f64, &Curve)> = sections
            .iter()
            .map(|s| Ok((station(&spline, s)?, s)))
            .collect::<Result<_, KernelError>>()?;
        placed.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let [(at, section)] = placed[..] {
            let (_, end) = spline.domain();
            // One section part way along: Rhino sweeps it only from the rail's start to where it
            // lies (#665), so the rail is cut there and swept from the section back.
            if at - start > 1e-6 * (end - start) && end - at > 1e-6 * (end - start) {
                if let Some((before, _)) = spline.split(at) {
                    return Self::swept(&[section.clone()], &before.reversed().to_curve()?, false);
                }
            }
            // At either end: carried to the rail's start along the turning frames, then swept
            // exactly along the whole rail.
            let (from, to) = (frame_at(&frames, at), frame_at(&frames, start));
            let moved = moved_between(section, &from, &to)?;
            return Self::swept(&[moved], rail, false);
        }
        blended(&placed, &spline, &frames)
    }
}

impl Body {
    /// The sections swept along two rails as Rhino's Sweep2 makes it (#667): any number of
    /// sections, anywhere along the rails, each held where it lies and scaled to the gap between
    /// the rails, the shape blending from one to the next.
    pub fn swept_along_rails(
        a: &Curve,
        b: &Curve,
        sections: &[Curve],
    ) -> Result<Self, KernelError> {
        let first = sections
            .first()
            .ok_or_else(|| bad("there is no cross section to sweep"))?;
        let rails = Rails {
            a: Spline::of(a)?.clamped()?,
            b: Spline::of(b)?.clamped()?,
        };
        // A single section at the rails' start is swept by the engine, exactly.
        let (start, end) = ends(first)?;
        if sections.len() == 1 && rails.along(start).min(rails.along(end)) < 1e-6 {
            return Self::swept_two_rails(a, b, first);
        }
        let mut placed = sections
            .iter()
            .map(|s| rails.placed(s))
            .collect::<Result<Vec<_>, _>>()?;
        placed.sort_by(|x, y| x.0.total_cmp(&y.0));
        let splines =
            Spline::compatible(&placed.iter().map(|(_, s)| s.clone()).collect::<Vec<_>>())?;
        // Each section's control points in the rails' frame where it lies, as shares of the gap.
        let shapes = placed
            .iter()
            .zip(&splines)
            .map(|((t, _), s)| {
                let (o, axes, size) = rails.frame(*t)?;
                Ok(s.points
                    .iter()
                    .map(|h| {
                        let d = sub([h[0] / h[3], h[1] / h[3], h[2] / h[3]], o);
                        (axes.map(|axis| dot(d, axis) / size), h[3])
                    })
                    .collect::<Vec<_>>())
            })
            .collect::<Result<Vec<_>, KernelError>>()?;
        let params: Vec<f64> = placed.iter().map(|(t, _)| *t).collect();
        let at = stations((0.0, 1.0), &params);
        let mut rows = Vec::with_capacity(at.len());
        for &t in &at {
            let (before, next, w) = between(&params, t);
            let (o, axes, size) = rails.frame(t)?;
            let row = shapes[before]
                .iter()
                .zip(&shapes[next])
                .map(|((pa, wa), (pb, wb))| {
                    let mut p = o;
                    for (axis, k) in axes.iter().zip(0..3) {
                        p = plus(p, *axis, size * (pa[k] * (1.0 - w) + pb[k] * w));
                    }
                    let weight = wa * (1.0 - w) + wb * w;
                    [p[0] * weight, p[1] * weight, p[2] * weight, weight]
                });
            rows.push(row.collect());
        }
        skinned(&splines[0], &at, &rows)
    }
}

/// A curve's two ends.
fn ends(curve: &Curve) -> Result<(Point, Point), KernelError> {
    let points = curve.divided(1)?;
    Ok((points[0], points[points.len() - 1]))
}

/// The places along the rail a blended sweep is skinned through: even steps over `domain` and
/// every section's own place, `params`.
fn stations(domain: (f64, f64), params: &[f64]) -> Vec<f64> {
    let (start, end) = domain;
    let mut at: Vec<f64> = (0..=STATIONS)
        .map(|k| start + (end - start) * k as f64 / STATIONS as f64)
        .chain(params.iter().copied())
        .collect();
    at.sort_by(f64::total_cmp);
    at.dedup_by(|a, b| (*a - *b).abs() < 1e-9 * (end - start).max(1.0));
    at
}

/// For a place `t` along the rail, the sections either side (by their sorted places `params`)
/// and how far the shape has gone from one to the other; before the first and after the last,
/// that section alone. The shape changes along an S, slow by each section and quickest halfway
/// (3s² − 2s³), as Rhino's Sweep2 does (measured, #667).
fn between(params: &[f64], t: f64) -> (usize, usize, f64) {
    let next = params
        .iter()
        .position(|s| *s >= t)
        .unwrap_or(params.len() - 1);
    let before = next.saturating_sub(1);
    let (a, b) = (params[before], params[next]);
    let s = if b - a > 1e-12 {
        ((t - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (before, next, s * s * (3.0 - 2.0 * s))
}

/// Two rails, taken at the same share of their parameter.
struct Rails {
    a: Spline,
    b: Spline,
}

impl Rails {
    /// How far along the first rail its point nearest `p` is, from 0 to 1.
    fn along(&self, p: Point) -> f64 {
        let (s, e) = self.a.domain();
        (self.a.closest(p) - s) / (e - s).max(1e-12)
    }

    /// A section as a spline running from the first rail to the second, and its place.
    fn placed(&self, section: &Curve) -> Result<(f64, Spline), KernelError> {
        let (start, end) = ends(section)?;
        let near_a = |p: Point| gap(p, self.a.point(self.a.closest(p)));
        let mut spline = Spline::of(section)?.clamped()?;
        let begin = if near_a(end) < near_a(start) {
            spline = spline.reversed();
            end
        } else {
            start
        };
        Ok((self.along(begin), spline))
    }

    /// At share `t` along the rails: the first rail's point, the frame (across to the second
    /// rail, up, along) and the gap between the rails.
    fn frame(&self, t: f64) -> Result<(Point, [Point; 3], f64), KernelError> {
        let u = |r: &Spline| {
            let (s, e) = r.domain();
            s + (e - s) * t
        };
        let (pa, pb) = (self.a.point(u(&self.a)), self.b.point(u(&self.b)));
        let across = sub(pb, pa);
        let size = dot(across, across).sqrt();
        let ex = unit(across).ok_or_else(|| bad("the rails meet"))?;
        let tangent = plus(
            self.a.derivative(u(&self.a)),
            self.b.derivative(u(&self.b)),
            1.0,
        );
        let ez = unit(plus(tangent, ex, -dot(tangent, ex))).ok_or_else(|| bad("the rails meet"))?;
        Ok((pa, [ex, cross(ez, ex), ez], size))
    }
}

/// `section` carried rigidly from frame `from` to frame `to`.
fn moved_between(section: &Curve, from: &Frame, to: &Frame) -> Result<Curve, KernelError> {
    let points = |f: &Frame| [f.at, plus(f.at, f.axes[0], 1.0), plus(f.at, f.axes[1], 1.0)];
    let turn = Similarity::between_frames(from.at, points(from), points(to))
        .ok_or_else(|| bad("the rail's frames are not usable"))?;
    Ok(section.mapped(&turn).translated(sub(to.at, from.at)))
}

/// Several sections, each held where it lies: the shape between two is their blend, carried by
/// the rail's frames, skinned through stations along the whole rail.
fn blended(
    placed: &[(f64, &Curve)],
    rail: &Spline,
    frames: &[(f64, Frame)],
) -> Result<Body, KernelError> {
    let splines = placed
        .iter()
        .map(|(_, s)| Spline::of(s).and_then(Spline::clamped))
        .collect::<Result<Vec<_>, _>>()?;
    let splines = Spline::compatible(&splines)?;
    // Each section's control points in the frame where it lies, with their weights.
    let shapes: Vec<Vec<(Point, f64)>> = placed
        .iter()
        .zip(&splines)
        .map(|((at, _), s)| {
            let f = frame_at(frames, *at);
            s.points
                .iter()
                .map(|h| (local(&f, [h[0] / h[3], h[1] / h[3], h[2] / h[3]]), h[3]))
                .collect()
        })
        .collect();
    let params: Vec<f64> = placed.iter().map(|(u, _)| *u).collect();
    let at = stations(rail.domain(), &params);
    let mut rows: Vec<Vec<[f64; 4]>> = Vec::with_capacity(at.len());
    for &u in &at {
        let (before, next, w) = between(&params, u);
        let f = frame_at(frames, u);
        let points = shapes[before]
            .iter()
            .zip(&shapes[next])
            .map(|((pa, wa), (pb, wb))| {
                let l: Point = std::array::from_fn(|i| pa[i] * (1.0 - w) + pb[i] * w);
                let weight = wa * (1.0 - w) + wb * w;
                let p = world(&f, l);
                [p[0] * weight, p[1] * weight, p[2] * weight, weight]
            })
            .collect();
        rows.push(points);
    }
    skinned(&splines[0], &at, &rows)
}

/// The surface through the stations' control points `rows` (one row per station, at rail parameters
/// `at`): along each section as `like` (degree and knots), and across them a cubic through every
/// station, so it passes exactly through each.
fn skinned(like: &Spline, at: &[f64], rows: &[Vec<[f64; 4]>]) -> Result<Body, KernelError> {
    let n = rows.len();
    let degree = 3.min(n - 1);
    let (first, last) = (at[0], at[n - 1]);
    let t: Vec<f64> = at
        .iter()
        .map(|u| (u - first) / (last - first).max(1e-12))
        .collect();
    // Knots by averaging the stations' parameters, so the system is well placed.
    let mut knots = vec![0.0; degree + 1];
    for j in 1..n - degree {
        knots.push(t[j..j + degree].iter().sum::<f64>() / degree as f64);
    }
    knots.extend(vec![1.0; degree + 1]);
    let basis: Vec<Vec<f64>> = t
        .iter()
        .map(|tk| {
            (0..n)
                .map(|i| crate::interp::basis(&knots, i, degree, *tk))
                .collect()
        })
        .collect();
    let columns = rows[0].len();
    let mut net = vec![vec![[0.0; 3]; n]; columns];
    let mut weights = vec![vec![1.0; n]; columns];
    for i in 0..columns {
        let solved = solve4(basis.clone(), rows.iter().map(|row| row[i]).collect())?;
        for (k, h) in solved.into_iter().enumerate() {
            net[i][k] = [h[0] / h[3], h[1] / h[3], h[2] / h[3]];
            weights[i][k] = h[3];
        }
    }
    crate::NurbsSurface::new(
        [like.degree, degree],
        like.knots.clone(),
        knots,
        &net,
        Some(&weights),
    )?
    .to_body()
}

/// Solves `a · x = b` for four columns at once, by elimination with pivoting.
fn solve4(mut a: Vec<Vec<f64>>, mut b: Vec<[f64; 4]>) -> Result<Vec<[f64; 4]>, KernelError> {
    let n = a.len();
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|x, y| a[*x][column].abs().total_cmp(&a[*y][column].abs()))
            .ok_or_else(|| bad("no stations"))?;
        if a[pivot][column].abs() < 1e-300 {
            return Err(bad("the stations cannot be passed through"));
        }
        a.swap(column, pivot);
        b.swap(column, pivot);
        let (lead, lead_b) = (a[column].clone(), b[column]);
        for row in (0..n).filter(|row| *row != column) {
            let factor = a[row][column] / lead[column];
            if factor == 0.0 {
                continue;
            }
            for (value, from) in a[row].iter_mut().zip(&lead).skip(column) {
                *value -= factor * from;
            }
            for (value, from) in b[row].iter_mut().zip(lead_b) {
                *value -= factor * from;
            }
        }
    }
    Ok((0..n).map(|row| b[row].map(|v| v / a[row][row])).collect())
}

/// `section` swept along the straight pieces between `points`, each piece the ruled surface
/// between the section where the mitres at its two ends cut it, the section turning at each
/// corner as the rail does.
fn mitred(section: &Curve, points: &[Point]) -> Result<Body, KernelError> {
    let directions = points
        .windows(2)
        .map(|w| unit(sub(w[1], w[0])).ok_or_else(|| bad("two corners of the rail are the same")))
        .collect::<Result<Vec<_>, _>>()?;
    let n = directions.len();
    // The plane at each corner: square to the rail at its ends, halfway at a corner.
    let mitre = |k: usize| -> Point {
        if k == 0 {
            directions[0]
        } else if k == n {
            directions[n - 1]
        } else {
            unit(plus(directions[k - 1], directions[k], 1.0)).unwrap_or(directions[k])
        }
    };
    // Where the section's points land in the plane through `origin` facing `normal`, slid along
    // `along`: an affine map, exact on the curve.
    let onto = |curve: &Curve, origin: Point, normal: Point, along: Point| {
        let dn = dot(along, normal);
        let row = |j: usize| -> [f64; 4] {
            let e: Point = std::array::from_fn(|i| f64::from(u8::from(i == j)));
            let p = plus(e, along, -normal[j] / dn);
            [p[0], p[1], p[2], 0.0]
        };
        let shift = along.map(|v| v * dot(origin, normal) / dn);
        curve.mapped_affine([row(0), row(1), row(2), [shift[0], shift[1], shift[2], 1.0]])
    };
    let mut carried = section.clone();
    let mut pieces = Vec::with_capacity(n);
    for k in 0..n {
        if k > 0 {
            // Turn with the rail at the corner, then slide to it.
            let (a, b) = (directions[k - 1], directions[k]);
            let axis = cross(a, b);
            let angle = dot(axis, axis).sqrt().atan2(dot(a, b));
            if let Some(axis) = unit(axis) {
                carried = carried.mapped(&Similarity::rotation(points[k], axis, angle));
            }
        }
        let near = onto(&carried, points[k], mitre(k), directions[k])?;
        let far = onto(&carried, points[k + 1], mitre(k + 1), directions[k])?;
        pieces.push(Body::ruled(&near, &far, false)?);
        if gap(points[k], points[k + 1]) <= 0.0 {
            return Err(bad("two corners of the rail are the same"));
        }
    }
    Body::assembled(&pieces)
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    #[test]
    fn a_section_away_from_the_rails_start_stays_on_the_rail() {
        let rail = Curve::line([0.0; 3], [0.0, 0.0, 10.0]).unwrap();
        let circle = Curve::circle([0.0, 0.0, 5.0], 1.0).unwrap();
        let swept = Body::swept_along(&[circle], &rail).unwrap();
        // Rhino 8 (#665, measured by the testing stream): a section part way along sweeps only
        // from the rail's start to it, here the first half.
        let (low, high) = swept.bounds().unwrap();
        assert!(
            low[2].abs() < 1e-6 && (high[2] - 5.0).abs() < 1e-6,
            "{low:?} {high:?}"
        );
        let area = swept.area().unwrap();
        assert!((area - std::f64::consts::TAU * 5.0).abs() < 1e-3, "{area}");
    }

    #[test]
    fn two_sections_make_one_surface_from_one_to_the_other() {
        let rail = Curve::line([0.0; 3], [0.0, 0.0, 10.0]).unwrap();
        let circle = Curve::circle([0.0; 3], 1.0).unwrap();
        let square = Curve::polyline(
            vec![
                [-1.0, -1.0, 10.0],
                [1.0, -1.0, 10.0],
                [1.0, 1.0, 10.0],
                [-1.0, 1.0, 10.0],
            ],
            true,
        )
        .unwrap();
        let swept = Body::swept_along(&[circle, square], &rail).unwrap();
        let (low, high) = swept.bounds().unwrap();
        assert!(
            low[2].abs() < 1e-6 && (high[2] - 10.0).abs() < 1e-6,
            "{low:?} {high:?}"
        );
    }

    #[test]
    fn a_rail_with_a_corner_is_swept_in_mitred_pieces_as_in_rhino() {
        let corners = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 8.0, 0.0]];
        // The same L as a polyline and as a degree-1 NURBS curve, which Rhino sweeps alike (#666).
        let polyline = Curve::polyline(corners.clone(), false).unwrap();
        let nurbs = Curve::nurbs(1, vec![0.0, 0.0, 1.0, 2.0, 2.0], corners, Vec::new()).unwrap();
        for rail in [polyline, nurbs] {
            swept_round_the_corner(&rail);
        }
    }

    fn swept_round_the_corner(rail: &Curve) {
        let section = Curve::polyline(
            vec![
                [0.0, -0.5, 0.0],
                [0.0, 0.5, 0.0],
                [0.0, 0.5, 2.0],
                [0.0, -0.5, 2.0],
            ],
            true,
        )
        .unwrap();
        let swept = Body::swept_along(&[section], rail).unwrap();
        // Rhino: area 108 (perimeter 6 along 18), box (0, -0.5, 0) to (10.5, 8, 2).
        assert!(
            (swept.area().unwrap() - 108.0).abs() < 1e-4,
            "{}",
            swept.area().unwrap()
        );
        let (low, high) = swept.bounds().unwrap();
        assert!(
            (high[0] - 10.5).abs() < 1e-6 && (low[1] + 0.5).abs() < 1e-6,
            "{low:?} {high:?}"
        );
    }
}
