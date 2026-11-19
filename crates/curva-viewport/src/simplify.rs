//! Fewer points for annotations' outlines and smooth curves. Curves are sampled for drawing at a fixed number of
//! points per control point, which suits a curve the size of a building but gives a letter
//! hundreds of points: a drawing of 4,893 objects from a real project became 14.9 million points,
//! 7.3 million of them its text, and its graphics card took 27 ms a frame. A letter needs only the
//! points that keep it within a small part of its own size of the sampled outline; a spline,
//! which is zoomed in on, within a ten times smaller part. Inside that drawing's blocks, 41,738
//! splines held 5 million points. Snap places are found from every sampled point first.

use curva_kernel::{Curve, Point};

/// How far, as a part of an annotation outline's own size, a point left out may lie from the
/// line kept: a letter or a mark is seen whole, so this is far below what shows.
pub(crate) const PART_OF_SIZE: f64 = 1.0 / 500.0;

/// The same for other smooth curves (splines, and curves joined from pieces), which are zoomed in
/// on: a circle would keep about one point every 4 degrees.
const FINE_PART_OF_SIZE: f64 = 1.0 / 5000.0;

/// How closely `curve`'s sampled points must be kept, as a part of its size: an annotation's
/// (`coarse`) at [`PART_OF_SIZE`], a smooth curve at [`FINE_PART_OF_SIZE`]; `None` keeps every
/// point, for lines, polylines, circles and arcs, which are sampled no finer than they need.
#[must_use]
pub(crate) fn part_of_size(curve: &Curve, coarse: bool) -> Option<f64> {
    if coarse {
        Some(PART_OF_SIZE)
    } else if matches!(
        curve,
        Curve::Nurbs { .. } | Curve::Interpolated { .. } | Curve::Compound { .. }
    ) {
        Some(FINE_PART_OF_SIZE)
    } else {
        None
    }
}

/// `points` with those left out that lie within `part` of the outline's size from the line
/// through the points kept either side (Douglas–Peucker); all of them with no `part`. The ends
/// are always kept.
#[must_use]
pub(crate) fn fewer(points: Vec<Point>, part: Option<f64>) -> Vec<Point> {
    let Some(part) = part.filter(|_| points.len() >= 3) else {
        return points;
    };
    let Some((low, high)) = crate::snap::box_around(&points) else {
        return points;
    };
    let size = (0..3)
        .map(|k| (high[k] - low[k]).powi(2))
        .sum::<f64>()
        .sqrt();
    let tolerance = size * part;
    if tolerance <= 0.0 {
        return points;
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    // Runs still to look at, as (first, last) with both ends kept.
    let mut runs = vec![(0, points.len() - 1)];
    while let Some((first, last)) = runs.pop() {
        let (mut farthest, mut at) = (0.0, 0);
        for (i, &p) in points.iter().enumerate().take(last).skip(first + 1) {
            let d = from_segment(p, points[first], points[last]);
            if d > farthest {
                (farthest, at) = (d, i);
            }
        }
        if farthest > tolerance {
            keep[at] = true;
            runs.push((first, at));
            runs.push((at, last));
        }
    }
    points
        .into_iter()
        .zip(keep)
        .filter_map(|(p, kept)| kept.then_some(p))
        .collect()
}

/// The distance from `p` to the segment from `a` to `b`.
fn from_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab: Point = std::array::from_fn(|k| b[k] - a[k]);
    let ap: Point = std::array::from_fn(|k| p[k] - a[k]);
    let length = ab.iter().map(|v| v * v).sum::<f64>();
    let t = if length > 0.0 {
        ((0..3).map(|k| ap[k] * ab[k]).sum::<f64>() / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (0..3)
        .map(|k| (ap[k] - t * ab[k]).powi(2))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_straight_run_keeps_its_ends_and_a_corner_stays() {
        let straight: Vec<Point> = (0..=100).map(|i| [f64::from(i), 0.0, 0.0]).collect();
        assert_eq!(
            fewer(straight, Some(PART_OF_SIZE)),
            vec![[0.0; 3], [100.0, 0.0, 0.0]]
        );
        let mut corner: Vec<Point> = (0..=50).map(|i| [f64::from(i), 0.0, 0.0]).collect();
        corner.extend((1..=50).map(|i| [50.0, f64::from(i), 0.0]));
        assert_eq!(
            fewer(corner, Some(PART_OF_SIZE)),
            vec![[0.0; 3], [50.0, 0.0, 0.0], [50.0, 50.0, 0.0]]
        );
    }

    /// A finely sampled circle comes out with far fewer points, all still on it, and no point of
    /// the original further from the outline kept than the tolerance.
    #[test]
    fn a_circle_keeps_its_shape_with_far_fewer_points() {
        let fine: Vec<Point> = (0..=2000)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 2000.0;
                [a.cos(), a.sin(), 0.0]
            })
            .collect();
        let kept = fewer(fine.clone(), Some(PART_OF_SIZE));
        assert!(kept.len() < 100, "{} points kept", kept.len());
        let tolerance = 8f64.sqrt() * PART_OF_SIZE;
        for p in &fine {
            let nearest = kept
                .windows(2)
                .map(|w| from_segment(*p, w[0], w[1]))
                .fold(f64::INFINITY, f64::min);
            assert!(nearest <= tolerance + 1e-12);
        }
    }
}
