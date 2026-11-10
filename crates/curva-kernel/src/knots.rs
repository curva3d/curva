//! Adding a control point to a curve without changing its shape, and evening control points out.

use crate::{Curve, KernelError, Point, gap};

fn bad(why: &str) -> KernelError {
    KernelError(why.to_owned())
}

/// Samples per knot span when looking for the parameter nearest a point.
const SAMPLES_PER_SPAN: usize = 48;
/// Halvings of the search interval once the nearest sample is found.
const REFINE_STEPS: usize = 60;

/// A control point in homogeneous form: its position times its weight, then the weight.
type Homogeneous = [f64; 4];

fn lerp4(a: Homogeneous, b: Homogeneous, t: f64) -> Homogeneous {
    [0, 1, 2, 3].map(|k| a[k] + (b[k] - a[k]) * t)
}

fn to_point(h: Homogeneous) -> Point {
    [h[0] / h[3], h[1] / h[3], h[2] / h[3]]
}

/// The knot span holding `u`: the largest `k` with `knots[k] <= u`, kept inside the curve's
/// domain. *The NURBS Book*, 2nd ed., algorithm A2.1, p. 68.
fn span(knots: &[f64], degree: usize, count: usize, u: f64) -> usize {
    if u >= knots[count] {
        return count - 1;
    }
    let mut k = degree;
    while k + 1 < count && knots[k + 1] <= u {
        k += 1;
    }
    k
}

/// The point at `u`, by de Boor's algorithm on homogeneous points.
/// *The NURBS Book*, 2nd ed., section 4.2 and algorithm A4.1, p. 124.
fn point_at(knots: &[f64], degree: usize, control: &[Homogeneous], u: f64) -> Point {
    let k = span(knots, degree, control.len(), u);
    let mut d: Vec<Homogeneous> = (0..=degree).map(|j| control[j + k - degree]).collect();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = j + k - degree;
            let width = knots[i + degree + 1 - r] - knots[i];
            let t = if width == 0.0 {
                0.0
            } else {
                (u - knots[i]) / width
            };
            d[j] = lerp4(d[j - 1], d[j], t);
        }
    }
    to_point(d[degree])
}

/// The parameter of the point on the curve nearest `at`: the nearest of many samples, then
/// a narrowing search around it.
fn nearest_parameter(knots: &[f64], degree: usize, control: &[Homogeneous], at: Point) -> f64 {
    let (start, end) = (knots[degree], knots[control.len()]);
    let reach = |u: f64| gap(point_at(knots, degree, control, u), at);
    let steps = SAMPLES_PER_SPAN * (control.len() - degree);
    let step = (end - start) / steps as f64;
    let mut best = start;
    for n in 0..=steps {
        let u = start + step * n as f64;
        if reach(u) < reach(best) {
            best = u;
        }
    }
    let (mut low, mut high) = ((best - step).max(start), (best + step).min(end));
    for _ in 0..REFINE_STEPS {
        let (a, b) = (low + (high - low) / 3.0, high - (high - low) / 3.0);
        if reach(a) < reach(b) {
            high = b;
        } else {
            low = a;
        }
    }
    (low + high) / 2.0
}

/// One knot inserted at `u`, which adds a control point and leaves the shape exactly as it
/// was. Boehm's algorithm, *The NURBS Book*, 2nd ed., section 5.2, equation 5.15, p. 142.
fn knot_inserted(
    knots: &[f64],
    degree: usize,
    control: &[Homogeneous],
    u: f64,
) -> (Vec<f64>, Vec<Homogeneous>) {
    let k = span(knots, degree, control.len(), u);
    let mut made = Vec::with_capacity(control.len() + 1);
    for i in 0..=control.len() {
        made.push(if i + degree <= k {
            control[i]
        } else if i <= k {
            let t = (u - knots[i]) / (knots[i + degree] - knots[i]);
            lerp4(control[i - 1], control[i], t)
        } else {
            control[i - 1]
        });
    }
    let mut new_knots = knots.to_vec();
    new_knots.insert(k + 1, u);
    (new_knots, made)
}

/// Where along a path of points `at` comes, as the index of the nearest one.
fn nearest_index(path: &[Point], at: Point) -> usize {
    let mut best = 0;
    for (index, p) in path.iter().enumerate() {
        if gap(*p, at) < gap(path[best], at) {
            best = index;
        }
    }
    best
}

impl Curve {
    /// The curve with one more control point, placed where the curve passes nearest `at`.
    /// Straight-sided and NURBS curves keep their shape exactly; a curve through points
    /// gains one more point to pass through, so it may shift slightly between its neighbours.
    pub fn with_point_added_at(&self, at: Point) -> Result<Self, KernelError> {
        let on = self.closest_point(at)?;
        match self.editable()? {
            Self::Line { start, end } => Self::polyline(vec![start, on, end], false),
            Self::Polyline { points, closed } => {
                let mut corners = points.clone();
                if closed {
                    corners.push(points[0]);
                }
                let mut best = (0, f64::INFINITY);
                for (index, pair) in corners.windows(2).enumerate() {
                    if let Ok(side) = Self::line(pair[0], pair[1]) {
                        let reach = gap(side.closest_point(on)?, on);
                        if reach < best.1 {
                            best = (index, reach);
                        }
                    }
                }
                if points.iter().any(|p| gap(*p, on) <= crate::JOIN_TOLERANCE) {
                    return Err(bad("there is already a control point there"));
                }
                let mut points = points;
                points.insert(best.0 + 1, on);
                Self::polyline(points, closed)
            }
            Self::Interpolated { points, closed } => {
                if points.iter().any(|p| gap(*p, on) <= crate::JOIN_TOLERANCE) {
                    return Err(bad("there is already a control point there"));
                }
                let path = self.sample()?;
                let here = nearest_index(&path, on);
                let place = points
                    .iter()
                    .position(|p| nearest_index(&path, *p) > here)
                    .unwrap_or(points.len());
                let mut points = points;
                points.insert(place, on);
                Self::interpolated(points, closed)
            }
            Self::Nurbs {
                degree,
                knots,
                points,
                weights,
            } => {
                let control: Vec<Homogeneous> = points
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let w = weights.get(i).copied().unwrap_or(1.0);
                        [p[0] * w, p[1] * w, p[2] * w, w]
                    })
                    .collect();
                let u = nearest_parameter(&knots, degree, &control, on);
                let (start, end) = (knots[degree], knots[points.len()]);
                let margin = (end - start) * 1e-6;
                if u <= start + margin || u >= end - margin {
                    return Err(bad("pick away from the end of the curve"));
                }
                let repeats = knots.iter().filter(|k| (**k - u).abs() <= margin).count();
                if repeats >= degree {
                    return Err(bad("the curve already has a kink there"));
                }
                let (knots, control) = knot_inserted(&knots, degree, &control, u);
                let points = control.iter().map(|h| to_point(*h)).collect();
                let weights = if weights.is_empty() {
                    Vec::new()
                } else {
                    control.iter().map(|h| h[3]).collect()
                };
                Self::nurbs(degree, knots, points, weights)
            }
            _ => Err(bad("this curve has no control points")),
        }
    }

    /// The curve with the control points at `picked` moved part of the way, by `factor`
    /// between 0 and 1, towards the middle of their two neighbours. The ends of an open curve
    /// stay put. This is Laplacian smoothing: G. Taubin, "A signal processing approach to fair
    /// surface design", SIGGRAPH 1995, section 3. Every point moves from where it was before,
    /// so the order of the points does not matter.
    pub fn smoothed(&self, picked: &[usize], factor: f64) -> Result<Self, KernelError> {
        let points = self.control_points()?;
        let closed = match self.editable()? {
            Self::Polyline { closed, .. } | Self::Interpolated { closed, .. } => closed,
            Self::Nurbs { points: held, .. } => held.len() > points.len(),
            _ => false,
        };
        let count = points.len();
        let mut made = points.clone();
        for &index in picked.iter().filter(|i| **i < count) {
            let (before, after) = if index == 0 || index + 1 == count {
                if !closed || count < 3 {
                    continue;
                }
                ((index + count - 1) % count, (index + 1) % count)
            } else {
                (index - 1, index + 1)
            };
            let middle = [0, 1, 2].map(|k| (points[before][k] + points[after][k]) / 2.0);
            made[index] =
                [0, 1, 2].map(|k| points[index][k] + (middle[k] - points[index][k]) * factor);
        }
        self.with_control_points(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Curve, gap};

    fn wave() -> Curve {
        Curve::nurbs(
            3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
            vec![
                [0.0; 3],
                [1.0, 2.0, 0.0],
                [2.0, -1.0, 0.0],
                [3.0, 2.0, 0.0],
                [4.0, 0.0, 0.0],
            ],
            Vec::new(),
        )
        .unwrap()
    }

    fn same_shape(a: &Curve, b: &Curve) {
        for p in a.sample().unwrap() {
            let reach = gap(b.closest_point(p).unwrap(), p);
            assert!(reach < 1e-6, "{p:?} is {reach} away");
        }
    }

    #[test]
    fn a_nurbs_curve_gains_a_point_and_keeps_its_shape() {
        let curve = wave();
        let pick = curve.sample().unwrap()[7];
        let more = curve.with_point_added_at(pick).unwrap();
        assert_eq!(more.control_points().unwrap().len(), 6);
        same_shape(&curve, &more);
        same_shape(&more, &curve);
    }

    #[test]
    fn a_circle_gains_a_point_and_stays_round() {
        let circle = Curve::circle([0.0; 3], 3.0).unwrap();
        let before = circle.control_points().unwrap().len();
        let more = circle.with_point_added_at([3.0, 1.0, 0.0]).unwrap();
        assert_eq!(more.control_points().unwrap().len(), before + 1);
        for p in more.sample().unwrap() {
            assert!((gap(p, [0.0; 3]) - 3.0).abs() < 1e-9, "{p:?}");
        }
    }

    #[test]
    fn a_polyline_gains_a_corner_on_the_side_picked() {
        let square = Curve::polyline(
            vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
            true,
        )
        .unwrap();
        let more = square.with_point_added_at([-0.1, 2.0, 0.0]).unwrap();
        assert_eq!(more.control_points().unwrap()[4], [0.0, 2.0, 0.0]);
        let line = Curve::line([0.0; 3], [4.0, 0.0, 0.0]).unwrap();
        let bent = line.with_point_added_at([1.0, 1.0, 0.0]).unwrap();
        assert_eq!(bent.control_points().unwrap()[1], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_curve_through_points_gains_one_between_the_right_neighbours() {
        let curve = Curve::interpolated(
            vec![[0.0; 3], [2.0, 2.0, 0.0], [4.0, 0.0, 0.0], [6.0, 2.0, 0.0]],
            false,
        )
        .unwrap();
        let pick = curve.closest_point([3.0, 1.0, 0.0]).unwrap();
        let more = curve.with_point_added_at(pick).unwrap();
        let points = more.control_points().unwrap();
        assert_eq!(points.len(), 5);
        assert!(gap(points[2], pick) < 1e-9);
    }

    #[test]
    fn a_pick_at_the_end_adds_nothing() {
        assert!(wave().with_point_added_at([0.0; 3]).is_err());
    }

    #[test]
    fn smoothing_keeps_the_ends_of_an_open_curve() {
        let zigzag = Curve::polyline(
            vec![[0.0; 3], [1.0, 2.0, 0.0], [2.0, 0.0, 0.0], [3.0, 2.0, 0.0]],
            false,
        )
        .unwrap();
        let smooth = zigzag.smoothed(&[0, 1, 2, 3], 0.5).unwrap();
        let points = smooth.control_points().unwrap();
        assert_eq!(points[0], [0.0; 3]);
        assert_eq!(points[3], [3.0, 2.0, 0.0]);
        assert_eq!(points[1], [1.0, 1.0, 0.0]);
        assert_eq!(points[2], [2.0, 1.0, 0.0]);
    }

    #[test]
    fn smoothing_a_closed_curve_moves_its_first_point_too() {
        let square = Curve::polyline(
            vec![[0.0; 3], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [0.0, 4.0, 0.0]],
            true,
        )
        .unwrap();
        let smooth = square.smoothed(&[0], 1.0).unwrap();
        assert_eq!(smooth.control_points().unwrap()[0], [2.0, 2.0, 0.0]);
    }
}
