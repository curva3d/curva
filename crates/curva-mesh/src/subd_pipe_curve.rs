//! Where MultiPipe puts the corners of a curved curve. Measured in Rhino 8 (24 November 2026):
//! Rhino pipes a curved curve as the polyline through points on it, with the same rings, struts
//! and mitres as a drawn polyline, so only those points are needed. It halves the curve at the
//! middle of its parameter range, always once, and goes on halving each piece at the middle of
//! its own range while the piece's chord is at least 3 × the radius long and either at least
//! 6 × the radius long or the curve strays more than 3/16 of the radius from it. So a quarter
//! circle of radius 10 piped at radius 1 gives 8 even pieces, at radius 0.25 16, and at radius 4
//! 2; a short curve gives 2 whatever its shape. (Thresholds found to 1% of the radius: chords of
//! 2.99 and 3.01, 5.99 and 6, and strays of 0.1874 and 0.1876.)

type Point = [f64; 3];

/// How many times a piece may be halved, so that a very fine pipe on a long curve still ends.
const DEEPEST: usize = 16;

/// Points along each piece used to find how far the curve strays from the piece's chord.
const SAMPLES: usize = 48;

fn gap(a: Point, b: Point) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// How far `p` is from the line through `a` and `b`.
fn off_line(p: Point, a: Point, b: Point) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    if dd <= 0.0 {
        return gap(p, a);
    }
    let t = (v[0] * d[0] + v[1] * d[1] + v[2] * d[2]) / dd;
    gap(p, [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t])
}

/// How far the curve strays from its chord between parameters `a` and `b`.
fn stray(point_at: &impl Fn(f64) -> Point, a: f64, b: f64) -> f64 {
    let (pa, pb) = (point_at(a), point_at(b));
    #[allow(clippy::cast_precision_loss)]
    let at = |k: usize| a + (b - a) * k as f64 / SAMPLES as f64;
    let (mut best, mut k_best) = (0.0, 0);
    for k in 1..SAMPLES {
        let d = off_line(point_at(at(k)), pa, pb);
        if d > best {
            (best, k_best) = (d, k);
        }
    }
    // Close in on the farthest point between its two neighbouring samples.
    let (mut lo, mut hi) = (at(k_best.saturating_sub(1)), at((k_best + 1).min(SAMPLES)));
    for _ in 0..40 {
        let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if off_line(point_at(m1), pa, pb) < off_line(point_at(m2), pa, pb) {
            lo = m1;
        } else {
            hi = m2;
        }
    }
    off_line(point_at(0.5 * (lo + hi)), pa, pb).max(best)
}

fn halve(
    point_at: &impl Fn(f64) -> Point,
    a: f64,
    b: f64,
    radius: f64,
    depth: usize,
    out: &mut Vec<f64>,
) {
    let chord = gap(point_at(a), point_at(b));
    let split = depth < DEEPEST
        && chord >= 3.0 * radius
        && (chord >= 6.0 * radius || stray(point_at, a, b) > 0.1875 * radius);
    if split {
        let m = 0.5 * (a + b);
        halve(point_at, a, m, radius, depth + 1, out);
        halve(point_at, m, b, radius, depth + 1, out);
    } else {
        out.push(b);
    }
}

/// The parameters, from `start` to `end`, of the corners MultiPipe gives the curve `point_at`
/// when piped at `radius`, as Rhino 8 places them.
pub fn pipe_corners(
    point_at: impl Fn(f64) -> Point,
    start: f64,
    end: f64,
    radius: f64,
) -> Vec<f64> {
    let mut out = vec![start];
    let m = 0.5 * (start + end);
    halve(&point_at, start, m, radius, 1, &mut out);
    halve(&point_at, m, end, radius, 1, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::pipe_corners;
    use std::f64::consts::FRAC_PI_2;

    fn arc(radius: f64, sweep: f64) -> impl Fn(f64) -> [f64; 3] {
        move |t: f64| [radius * (t * sweep).cos(), radius * (t * sweep).sin(), 0.0]
    }

    #[test]
    fn a_quarter_circle_is_cut_evenly_finer_for_thinner_pipes_as_in_rhino() {
        // Rhino 8: 2, 4, 8 and 16 pieces at radii 4, 2, 1 and 0.25.
        for (radius, pieces) in [(4.0, 2), (2.0, 4), (1.0, 8), (0.25, 16)] {
            let corners = pipe_corners(arc(10.0, FRAC_PI_2), 0.0, 1.0, radius);
            assert_eq!(corners.len(), pieces + 1, "radius {radius}");
        }
        // A quarter of radius 100 at radius 1: 32 pieces.
        assert_eq!(pipe_corners(arc(100.0, FRAC_PI_2), 0.0, 1.0, 1.0).len(), 33);
    }

    #[test]
    fn a_short_or_straight_curve_is_halved_once_and_long_ones_more_as_in_rhino() {
        let straight = |length: f64| move |t: f64| [length * t, 0.0, 0.0];
        assert_eq!(pipe_corners(straight(2.0), 0.0, 1.0, 1.0).len(), 3);
        assert_eq!(pipe_corners(straight(11.99), 0.0, 1.0, 1.0).len(), 3);
        assert_eq!(pipe_corners(straight(12.0), 0.0, 1.0, 1.0).len(), 5);
        // A quarter of radius 1, fatter than it bends: still two pieces.
        assert_eq!(pipe_corners(arc(1.0, FRAC_PI_2), 0.0, 1.0, 1.0).len(), 3);
    }

    #[test]
    fn pieces_are_halved_only_from_three_radii_long_as_in_rhino() {
        // Half circles whose halves have chords of 2.99 and 3.01 at radius 1.
        for (chord, pieces) in [(2.99, 2), (3.01, 4)] {
            let r = chord / 2f64.sqrt();
            let corners = pipe_corners(arc(r, std::f64::consts::PI), 0.0, 1.0, 1.0);
            assert_eq!(corners.len(), pieces + 1, "chord {chord}");
        }
    }

    #[test]
    fn pieces_straying_more_than_three_sixteenths_of_the_radius_are_halved_as_in_rhino() {
        // Arcs whose halves have chords of 5 and stray 0.1874 and 0.1876 from them.
        for (stray, pieces) in [(0.1874, 2), (0.1876, 4)] {
            let quarter = (2.0 * stray / 5.0f64).atan();
            let half_sweep = 4.0 * quarter;
            let r = 5.0 / (2.0 * (half_sweep / 2.0).sin());
            let corners = pipe_corners(arc(r, 2.0 * half_sweep), 0.0, 1.0, 1.0);
            assert_eq!(corners.len(), pieces + 1, "stray {stray}");
        }
    }
}
