//! Rebuilding a curve as Rhino's Rebuild does (#716, measured): a new curve of a chosen degree and
//! number of control points, on uniform knots, fitted to the old one by least squares, its ends
//! where the old one's are and leaving along the old one's end tangents.

use crate::{Curve, KernelError, Point, Spline, unit};

fn bad(why: &str) -> KernelError {
    KernelError(why.to_owned())
}

/// How many places along the old curve the new one is fitted to, for each new control point.
const SAMPLES_PER_POINT: usize = 400;

impl Curve {
    /// The curve with the inner knot `u` taken out, as Rhino's RemoveKnot does it (#736): one control
    /// point fewer, the ends kept, the others fitted to the old curve by least squares over the
    /// same parameters.
    pub fn without_knot(&self, u: f64) -> Result<Curve, KernelError> {
        let old = Spline::of(self)?.clamped()?;
        let degree = old.degree;
        let mut knots = old.knots.clone();
        let at = knots
            .iter()
            .position(|k| (*k - u).abs() < 1e-12)
            .ok_or_else(|| bad("the curve has no such knot"))?;
        knots.remove(at);
        let count = old.points.len() - 1;
        if count < degree + 1 {
            return Err(bad("the curve has too few control points to lose a knot"));
        }
        let (s0, s1) = old.domain();
        let (start, end) = (old.point(s0), old.point(s1));
        let inner = count - 2;
        let mut normal = vec![vec![0.0; inner]; inner];
        let mut right = vec![[0.0; 3]; inner];
        // The old curve hit at each free control point's own place (its Greville abscissa).
        for i in 1..count - 1 {
            let t = knots[i + 1..=i + degree].iter().sum::<f64>() / degree as f64;
            let target = old.point(t);
            let n: Vec<f64> = (0..count)
                .map(|i| crate::interp::basis(&knots, i, degree, t))
                .collect();
            let want: Point =
                std::array::from_fn(|c| target[c] - n[0] * start[c] - n[count - 1] * end[c]);
            for a in 0..inner {
                if n[a + 1] == 0.0 {
                    continue;
                }
                for c in 0..3 {
                    right[a][c] += n[a + 1] * want[c];
                }
                for b in 0..inner {
                    normal[a][b] += n[a + 1] * n[b + 1];
                }
            }
        }
        let mut points = vec![start];
        let columns: Vec<Vec<f64>> = (0..3)
            .map(|c| solve(normal.clone(), right.iter().map(|r| r[c]).collect()))
            .collect::<Result<_, _>>()?;
        points.extend((0..inner).map(|i| [columns[0][i], columns[1][i], columns[2][i]]));
        points.push(end);
        Curve::nurbs(degree, knots, points, Vec::new())
    }

    /// The open curve rebuilt with `count` control points of `degree`, as Rhino rebuilds it:
    /// uniform knots, the same ends, the same end tangents, and in between as near the old curve
    /// as least squares makes it.
    pub fn rebuilt(&self, count: usize, degree: usize) -> Result<Curve, KernelError> {
        let degree = degree.clamp(1, count.saturating_sub(1).max(1));
        if count < degree + 1 || count < 4 {
            return Err(bad("a rebuilt curve needs at least four control points"));
        }
        let old = Spline::of(self)?.clamped()?;
        let (s0, s1) = old.domain();
        let spans = count - degree;
        // Uniform knots over 0..spans, clamped at both ends, as Rhino lists them.
        let mut knots = vec![0.0; degree + 1];
        knots.extend((1..spans).map(|k| k as f64));
        knots.extend(vec![spans as f64; degree + 1]);
        let (start, end) = (old.point(s0), old.point(s1));
        let t0 = unit(old.derivative(s0))
            .ok_or_else(|| bad("the curve has no direction at its start"))?;
        let t1 =
            unit(old.derivative(s1)).ok_or_else(|| bad("the curve has no direction at its end"))?;
        // Unknowns: how far the second and second-last points lie along the end tangents, then
        // every inner point's three coordinates.
        let inner = count - 4;
        let unknowns = 2 + 3 * inner;
        let mut normal = vec![vec![0.0; unknowns]; unknowns];
        let mut right = vec![0.0; unknowns];
        for (u, target) in &points_to_fit(&old, &knots, count, degree) {
            let (u, target) = (*u, target);
            let n: Vec<f64> = (0..count)
                .map(|i| crate::interp::basis(&knots, i, degree, u))
                .collect();
            for c in 0..3 {
                // The row for coordinate c, and what is left once the fixed ends are taken away.
                let mut row = vec![0.0; unknowns];
                row[0] = n[1] * t0[c];
                row[1] = -n[count - 2] * t1[c];
                for i in 0..inner {
                    row[2 + 3 * i + c] = n[2 + i];
                }
                let fixed = (n[0] + n[1]) * start[c] + (n[count - 2] + n[count - 1]) * end[c];
                let want = target[c] - fixed;
                for a in 0..unknowns {
                    if row[a] == 0.0 {
                        continue;
                    }
                    right[a] += row[a] * want;
                    for b in 0..unknowns {
                        normal[a][b] += row[a] * row[b];
                    }
                }
            }
        }
        let x = solve(normal, right)?;
        let mut points: Vec<Point> = vec![start, plus(start, t0, x[0])];
        points.extend((0..inner).map(|i| [x[2 + 3 * i], x[3 + 3 * i], x[4 + 3 * i]]));
        points.push(plus(end, t1, -x[1]));
        points.push(end);
        Curve::nurbs(degree, knots, points, Vec::new())
    }
}

/// The places a rebuilt curve passes through, with its parameters there: at each inner control
/// point's own place (its Greville abscissa), found along the old curve's length, as Rhino's
/// Rebuild runs the new curve's parameter along the old one's length.
fn points_to_fit(old: &Spline, knots: &[f64], count: usize, degree: usize) -> Vec<(f64, Point)> {
    let (s0, s1) = old.domain();
    let spans = (count - degree) as f64;
    let samples = SAMPLES_PER_POINT * count;
    let dense: Vec<Point> = (0..=samples)
        .map(|k| old.point(s0 + (s1 - s0) * k as f64 / samples as f64))
        .collect();
    let mut along = vec![0.0];
    for pair in dense.windows(2) {
        along.push(along[along.len() - 1] + crate::gap(pair[0], pair[1]));
    }
    let length = along[samples].max(1e-12);
    let at_length = |share: f64| -> Point {
        let want = share * length;
        let k = along.partition_point(|s| *s < want).clamp(1, samples);
        let (a, b) = (along[k - 1], along[k]);
        let w = if b > a { (want - a) / (b - a) } else { 0.0 };
        std::array::from_fn(|c| dense[k - 1][c] * (1.0 - w) + dense[k][c] * w)
    };
    (1..count - 1)
        .map(|i| {
            let u = knots[i + 1..=i + degree].iter().sum::<f64>() / degree as f64;
            (u, at_length(u / spans))
        })
        .collect()
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

/// Solves `a · x = b` by elimination with pivoting.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Result<Vec<f64>, KernelError> {
    let n = a.len();
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|x, y| a[*x][column].abs().total_cmp(&a[*y][column].abs()))
            .ok_or_else(|| bad("nothing to fit"))?;
        if a[pivot][column].abs() < 1e-300 {
            return Err(bad("the curve cannot be fitted"));
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
            b[row] -= factor * lead_b;
        }
    }
    Ok((0..n).map(|row| b[row] / a[row][row]).collect())
}

#[cfg(test)]
mod tests {
    use crate::Curve;

    #[test]
    fn rebuilding_gives_rhinos_curve() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [5.0, 5.0, 0.0],
            [10.0, 0.0, 0.0],
            [15.0, 5.0, 0.0],
            [20.0, 0.0, 0.0],
        ];
        let curve = Curve::control_point(points, 3, false).unwrap();
        let rebuilt = curve.rebuilt(10, 3).unwrap();
        let control = rebuilt.control_points().unwrap();
        // Rhino 8's Rebuild(10, 3, preserve tangents).
        let rhino = [
            [0.74374, 0.74374],
            [2.312562, 2.116629],
            [5.318602, 3.466423],
        ];
        for (got, want) in control[1..4].iter().zip(rhino) {
            assert!(
                (got[0] - want[0]).abs() < 2e-3 && (got[1] - want[1]).abs() < 2e-3,
                "{control:?}"
            );
        }
        assert!((rebuilt.length().unwrap() - 21.97868).abs() < 1e-3);
    }
}
