//! Cubic curves through points as Rhino 8's `InterpCrv` makes them (#669, measured): the
//! parameter runs along the summed chord lengths, a knot sits at every point, and each end leaves
//! in the direction of the parabola through its first three points (Bessel's end tangent) at a
//! speed of exactly 1 in that parameter. Two extra control points come from those ends.

use crate::{KernelError, Point, gap};

fn bad(why: &str) -> KernelError {
    KernelError(why.to_owned())
}

/// The cubic B-spline basis function `i` on `knots` at `t` (Cox–de Boor); at the last knot the
/// last function is 1.
fn basis(knots: &[f64], i: usize, degree: usize, t: f64) -> f64 {
    if degree == 0 {
        let (a, b) = (knots[i], knots[i + 1]);
        let last = knots[knots.len() - 1];
        return f64::from(u8::from(
            (a <= t && t < b) || (t >= last && b >= last && a < b),
        ));
    }
    let mut value = 0.0;
    let left = knots[i + degree] - knots[i];
    if left > 0.0 {
        value += (t - knots[i]) / left * basis(knots, i, degree - 1, t);
    }
    let right = knots[i + degree + 1] - knots[i + 1];
    if right > 0.0 {
        value += (knots[i + degree + 1] - t) / right * basis(knots, i + 1, degree - 1, t);
    }
    value
}

/// Solves `a · x = b` for each of the three coordinates, by elimination with pivoting.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<Point>) -> Result<Vec<Point>, KernelError> {
    let n = a.len();
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|x, y| a[*x][column].abs().total_cmp(&a[*y][column].abs()))
            .ok_or_else(|| bad("no points to pass through"))?;
        if a[pivot][column].abs() < 1e-300 {
            return Err(bad("the points cannot be passed through"));
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

/// Bessel's tangent at the start of `q` (parameters `t`): the derivative there of the parabola
/// through the first three points, made of length 1.
fn bessel(q: [Point; 3], t: [f64; 3]) -> Result<Point, KernelError> {
    let (h0, h1) = (t[1] - t[0], t[2] - t[1]);
    let share = h0 / (h0 + h1);
    let d: Point = std::array::from_fn(|i| {
        let first = (q[1][i] - q[0][i]) / h0;
        let second = (q[2][i] - q[1][i]) / h1;
        (1.0 + share) * first - share * second
    });
    crate::unit(d).ok_or_else(|| bad("the curve's end has no direction"))
}

/// The knots (clamped, of degree 3) and control points of Rhino's cubic curve through `q`, three
/// or more points, none repeated one after the other.
pub(crate) fn through(q: &[Point]) -> Result<(Vec<f64>, Vec<Point>), KernelError> {
    let n = q.len();
    if n < 3 {
        return Err(bad("a curve through points needs three of them"));
    }
    let mut t = vec![0.0];
    for k in 1..n {
        let chord = gap(q[k], q[k - 1]);
        if chord <= 0.0 {
            return Err(bad("two points in a row are the same"));
        }
        t.push(t[k - 1] + chord);
    }
    let (start, end) = (t[0], t[n - 1]);
    let mut knots = vec![start; 4];
    knots.extend(&t[1..n - 1]);
    knots.extend([end; 4]);
    let count = n + 2;
    let begin = bessel([q[0], q[1], q[2]], [t[0], t[1], t[2]])?;
    let finish = bessel(
        [q[n - 1], q[n - 2], q[n - 3]],
        [-t[n - 1], -t[n - 2], -t[n - 3]],
    )?;
    let finish = finish.map(|v| -v);
    let mut a: Vec<Vec<f64>> = Vec::with_capacity(count);
    let mut b: Vec<Point> = Vec::with_capacity(count);
    for (k, at) in t.iter().enumerate() {
        a.push((0..count).map(|i| basis(&knots, i, 3, *at)).collect());
        b.push(q[k]);
    }
    // A clamped cubic leaves its start at 3 (P1 − P0) / (k4 − k0); that is the unit tangent.
    let mut row = vec![0.0; count];
    row[0] = -1.0;
    row[1] = 1.0;
    a.push(row);
    b.push(begin.map(|v| v * (knots[4] - start) / 3.0));
    let mut row = vec![0.0; count];
    row[count - 2] = -1.0;
    row[count - 1] = 1.0;
    a.push(row);
    b.push(finish.map(|v| v * (end - knots[knots.len() - 5]) / 3.0));
    Ok((knots, solve(a, b)?))
}

#[cfg(test)]
mod tests {
    use crate::Curve;

    fn length(points: &[crate::Point]) -> f64 {
        let (knots, control) = super::through(points).unwrap();
        Curve::nurbs(3, knots, control, Vec::new())
            .unwrap()
            .length()
            .unwrap()
    }

    #[test]
    fn the_curve_is_rhinos_as_measured() {
        let even = [
            [0.0, 0.0, 0.0],
            [5.0, 5.0, 0.0],
            [10.0, 0.0, 0.0],
            [15.0, 5.0, 0.0],
        ];
        let uneven = [
            [0.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [10.0, 0.0, 0.0],
            [11.0, 4.0, 0.0],
        ];
        assert!((length(&even) - 22.091_093_224_525_178).abs() < 1e-6);
        assert!((length(&uneven) - 14.766_027_595_037_59).abs() < 1e-6);
        let (_, control) = super::through(&uneven).unwrap();
        let rhino = [0.623_734_835_881_703_5, 0.408_056_871_113_794_5];
        assert!((control[1][0] - rhino[0]).abs() < 1e-9 && (control[1][1] - rhino[1]).abs() < 1e-9);
    }
}
