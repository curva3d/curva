//! Rebuilding a surface one way, as Rhino's RebuildUV does (#740, measured in Rhino 8): the
//! chosen direction refitted as a cubic on uniform knots with a given number of control points,
//! the other direction kept as it was, weights and all. A direction that closes on itself, as a
//! sphere's way round does, is refitted periodic: the number counts the distinct points, and the
//! three that wrap round come on top (a sphere rebuilt to 10 points has 13 along u).

use crate::body::Body;
use crate::rebuild::solve;
use crate::surface::NurbsSurface;
use crate::{KernelError, Spline, gap};

fn bad(why: &str) -> KernelError {
    KernelError(format!("rebuilding the surface: {why}"))
}

/// How near a column's ends must be for the direction to count as closed.
const CLOSED: f64 = 1e-7;

impl NurbsSurface {
    /// The surface rebuilt along u (along v when `along_v`) with `count` control points.
    pub fn rebuilt_one_way(&self, along_v: bool, count: usize) -> Result<Self, KernelError> {
        if along_v {
            Ok(self.swapped().rebuilt_along_u(count)?.swapped())
        } else {
            self.rebuilt_along_u(count)
        }
    }

    /// The same surface with u and v swapped.
    fn swapped(&self) -> Self {
        let cols = self.points[0].len();
        Self {
            degree: [self.degree[1], self.degree[0]],
            u_knots: self.v_knots.clone(),
            v_knots: self.u_knots.clone(),
            points: (0..cols)
                .map(|j| self.points.iter().map(|row| row[j]).collect())
                .collect(),
        }
    }

    fn rebuilt_along_u(&self, count: usize) -> Result<Self, KernelError> {
        let cols = self.points[0].len();
        // Each column refitted as a curve of its own. A closed way is refitted unweighted and each
        // column's largest weight put back on all its new points, as Rhino keeps a sphere's
        // ring at the equator unweighted; an open way is refitted weighted.
        let columns: Vec<Spline> = (0..cols)
            .map(|j| Spline {
                degree: self.degree[0],
                knots: self.u_knots.clone(),
                points: self.points.iter().map(|row| row[j]).collect(),
            })
            .collect();
        let weights: Vec<f64> = (0..cols)
            .map(|j| self.points.iter().map(|row| row[j][3]).fold(0.0, f64::max))
            .collect();
        let closed = columns.iter().all(|c| {
            let (a, b) = c.domain();
            gap(c.point(a), c.point(b)) < CLOSED
        });
        let fit = if closed {
            Fit::periodic(count)?
        } else {
            Fit::open(count)?
        };
        // Every column is sampled at the same old parameters, spaced evenly along the longest
        // column's length, as Rhino spaces a sphere's new points evenly round it.
        let along = columns
            .iter()
            .map(length_table)
            .max_by(|a, b| a[a.len() - 1].1.total_cmp(&b[b.len() - 1].1))
            .ok_or_else(|| bad("the surface has no control points"))?;
        let refitted = columns
            .iter()
            .map(|c| fit.fitted(c, &along))
            .collect::<Result<Vec<_>, _>>()?;
        let rows = refitted[0].points.len();
        Ok(Self {
            degree: [refitted[0].degree, self.degree[1]],
            u_knots: refitted[0].knots.clone(),
            v_knots: self.v_knots.clone(),
            points: (0..rows)
                .map(|i| {
                    (0..cols)
                        .map(|j| {
                            let p = refitted[j].points[i];
                            if !closed {
                                return p;
                            }
                            let w = weights[j];
                            [p[0] / p[3] * w, p[1] / p[3] * w, p[2] / p[3] * w, w]
                        })
                        .collect()
                })
                .collect(),
        })
    }
}

/// A uniform cubic put through curves at set places along them, the same knots for every curve.
struct Fit {
    degree: usize,
    knots: Vec<f64>,
    /// How many distinct control points it solves for.
    count: usize,
    /// Whether it wraps round, its first `degree` points repeated at the end.
    periodic: bool,
    /// Where it meets the old curve: the new parameter there, as a share of the way along, the
    /// same share of the old curve's length.
    samples: Vec<f64>,
}

impl Fit {
    fn open(count: usize) -> Result<Self, KernelError> {
        if count < 2 {
            return Err(bad("it needs at least two points"));
        }
        let degree = 3.min(count - 1);
        let spans = count - degree;
        let mut knots = vec![0.0; degree + 1];
        knots.extend((1..spans).map(|k| k as f64));
        knots.extend(vec![spans as f64; degree + 1]);
        // Through the old curve at each inner point's own place (its Greville abscissa), the ends
        // kept. Rhino's RebuildUV lands near this but not on it: its open fit is still unknown.
        let samples = (1..count - 1)
            .map(|i| knots[i + 1..=i + degree].iter().sum::<f64>() / (degree * spans) as f64)
            .collect();
        Ok(Self::with(degree, knots, count, false, samples))
    }

    fn periodic(count: usize) -> Result<Self, KernelError> {
        if count < 4 {
            return Err(bad("a closed direction needs at least four points"));
        }
        let degree = 3;
        // Uniform knots running `degree` spans past each end, so the curve wraps smoothly.
        let knots = (0..=count + 2 * degree)
            .map(|k| k as f64 - degree as f64)
            .collect();
        // Through the old curve at each knot, evenly along it, as Rhino's sphere rebuilt to 10
        // points shows (its ring of points at 5.3399, a circle of 5 met at every knot).
        let samples = (0..count).map(|k| k as f64 / count as f64).collect();
        Ok(Self::with(degree, knots, count, true, samples))
    }

    fn with(
        degree: usize,
        knots: Vec<f64>,
        count: usize,
        periodic: bool,
        samples: Vec<f64>,
    ) -> Self {
        Self {
            degree,
            knots,
            count,
            periodic,
            samples,
        }
    }

    /// The new domain, `(start, end)`.
    fn domain(&self) -> (f64, f64) {
        let n = self.knots.len() - self.degree - 1;
        (self.knots[self.degree], self.knots[n])
    }

    /// How much each distinct control point counts at new parameter `t`.
    fn row(&self, t: f64) -> Vec<f64> {
        let all = self.knots.len() - self.degree - 1;
        let mut row = vec![0.0; self.count];
        for i in 0..all {
            row[i % self.count] += crate::interp::basis(&self.knots, i, self.degree, t);
        }
        row
    }

    /// `old` refitted through its points at the fit's places along its length (the longest
    /// column's, `along`, so every column is met at the same old parameters). An open curve is
    /// fitted weighted, weights and all, as Rhino keeps a sphere's way pole to pole rational; a
    /// closed one unweighted, its points coming back with weight 1.
    fn fitted(&self, old: &Spline, along: &[(f64, f64)]) -> Result<Spline, KernelError> {
        let (s0, s1) = old.domain();
        let (t0, t1) = self.domain();
        let total = along[along.len() - 1].1;
        let at = |u: f64| -> [f64; 4] {
            if self.periodic {
                let p = old.point(u);
                [p[0], p[1], p[2], 1.0]
            } else {
                old.homogeneous(u)
            }
        };
        let ends = (at(s0), at(s1));
        // Open: the first and last points are the old ends, so only the inner ones are solved.
        let free: Vec<usize> = if self.periodic {
            (0..self.count).collect()
        } else {
            (1..self.count - 1).collect()
        };
        let m = free.len();
        let mut normal = vec![vec![0.0; m]; m];
        let mut right = vec![[0.0; 4]; m];
        for share in &self.samples {
            let n = self.row(t0 + (t1 - t0) * share);
            let u = if total > 1e-12 {
                at_length(along, share * total)
            } else {
                s0 + (s1 - s0) * share
            };
            let mut want = at(u);
            if !self.periodic {
                for (c, w) in want.iter_mut().enumerate() {
                    *w -= n[0] * ends.0[c] + n[self.count - 1] * ends.1[c];
                }
            }
            for (a, &i) in free.iter().enumerate() {
                for c in 0..4 {
                    right[a][c] += n[i] * want[c];
                }
                for (b, &k) in free.iter().enumerate() {
                    normal[a][b] += n[i] * n[k];
                }
            }
        }
        let solved: Vec<Vec<f64>> = (0..4)
            .map(|c| solve(normal.clone(), right.iter().map(|r| r[c]).collect()))
            .collect::<Result<_, _>>()?;
        let mut points: Vec<[f64; 4]> = (0..m)
            .map(|a| std::array::from_fn(|c| solved[c][a]))
            .collect();
        if self.periodic {
            points.extend_from_within(..self.degree);
        } else {
            points.insert(0, ends.0);
            points.push(ends.1);
        }
        let spline = Spline {
            degree: self.degree,
            knots: self.knots.clone(),
            points,
        };
        spline.clamped()
    }
}

/// How many pieces a column is cut into to measure its length.
const LENGTH_STEPS: usize = 2000;

/// The old parameter and the length so far, at evenly spaced parameters along `c`.
fn length_table(c: &Spline) -> Vec<(f64, f64)> {
    let (s0, s1) = c.domain();
    let mut table = vec![(s0, 0.0)];
    let mut last = c.point(s0);
    for k in 1..=LENGTH_STEPS {
        let u = s0 + (s1 - s0) * k as f64 / LENGTH_STEPS as f64;
        let p = c.point(u);
        let so_far = table[k - 1].1 + gap(last, p);
        table.push((u, so_far));
        last = p;
    }
    table
}

/// The parameter at length `want` along a [`length_table`].
fn at_length(table: &[(f64, f64)], want: f64) -> f64 {
    let k = table
        .partition_point(|l| l.1 < want)
        .clamp(1, table.len() - 1);
    let ((u0, a), (u1, b)) = (table[k - 1], table[k]);
    if b > a {
        u0 + (u1 - u0) * (want - a) / (b - a)
    } else {
        u0
    }
}

impl Body {
    /// A one-face surface rebuilt one way as Rhino's RebuildUV does (see
    /// [`NurbsSurface::rebuilt_one_way`]). A surface that closes on itself is sewn shut again
    /// when sewing gives a sound solid; a sphere, its poles closing to points, stays a surface
    /// with its seam open.
    pub fn rebuilt_one_way(&self, along_v: bool, count: usize) -> Result<Body, KernelError> {
        let record = self.record()?;
        if record.faces.len() != 1 {
            return Err(bad("only a single surface can be rebuilt"));
        }
        let made = NurbsSurface::of_face(&record, 0)?
            .rebuilt_one_way(along_v, count)?
            .to_body()?;
        let area = made.area()?;
        let sewn = Body::assembled(std::slice::from_ref(&made))
            .ok()
            .filter(|s| {
                s.is_solid()
                    && s.area()
                        .is_ok_and(|a| (a - area).abs() <= 1e-6 * area.max(1.0))
            });
        Ok(sewn.unwrap_or(made))
    }
}

#[cfg(test)]
mod tests {
    use crate::surface::NurbsSurface;
    use crate::{Body, Point};

    #[test]
    fn a_sphere_rebuilt_round_is_periodic_and_keeps_its_way_pole_to_pole_as_in_rhino() {
        let sphere = Body::sphere([0.0; 3], 5.0).unwrap();
        let old = NurbsSurface::of_face(&sphere.record().unwrap(), 0).unwrap();
        let new = old.rebuilt_one_way(false, 10).unwrap();
        // Rhino 8: degree 3 round with 13 points (10 and the three that wrap), the way pole to
        // pole untouched (degree 2, 5 points, weights 1, 0.7071, 1, 0.7071, 1), the ring at the
        // equator 5.3399 out, area 314.06.
        assert_eq!(
            (new.degree, new.points.len(), new.points[0].len()),
            ([3, 2], 13, 5)
        );
        assert_eq!(new.v_knots, old.v_knots);
        let weights: Vec<f64> = new.points[4].iter().map(|p| p[3]).collect();
        assert!((weights[1] - 0.5f64.sqrt()).abs() < 1e-9, "{weights:?}");
        let ring = new.points[4][2];
        let out = (ring[0] * ring[0] + ring[1] * ring[1]).sqrt() / ring[3];
        assert!((out - 5.3399).abs() < 1e-4, "{out}");
        let area = sphere.rebuilt_one_way(false, 10).unwrap().area().unwrap();
        assert!((area - 314.06).abs() < 0.01, "{area}");
    }

    #[test]
    fn an_open_way_keeps_its_ends_and_the_other_way() {
        let net: Vec<Vec<Point>> = (0..12)
            .map(|i| {
                let x = f64::from(i);
                vec![
                    [x, 0.0, (x * 0.6).sin() * 2.0],
                    [x, 3.0, (x * 0.6).sin() * 2.0],
                ]
            })
            .collect();
        let mut knots = vec![0.0; 4];
        knots.extend((1..9).map(f64::from));
        knots.extend([9.0; 4]);
        let old = NurbsSurface::new([3, 1], knots, vec![0.0, 0.0, 1.0, 1.0], &net, None).unwrap();
        let new = old.rebuilt_one_way(false, 8).unwrap();
        assert_eq!((new.degree, new.points.len()), ([3, 1], 8));
        assert_eq!(new.points[0][0], old.points[0][0]);
        assert_eq!(new.points[7][1], old.points[11][1]);
        // Rhino 8's RebuildUV here: the second point at (0.6411, 1.09), the fourth at (4.7795,
        // 0.9646). Curva meets the old curve at each point's own place instead, as Rhino's
        // Rebuild does: near but not on RebuildUV's.
        let fourth = new.points[3][0];
        assert!((fourth[0] - 4.7795).abs() < 0.05 && (fourth[2] - 0.9646).abs() < 0.1);
        // The same rebuilt along v instead keeps u as it was.
        let across = old.rebuilt_one_way(true, 4).unwrap();
        assert_eq!((across.points.len(), across.points[0].len()), (12, 4));
        assert_eq!(across.u_knots, old.u_knots);
    }
}
