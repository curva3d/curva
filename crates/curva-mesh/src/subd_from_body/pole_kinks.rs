//! Rhino 8's ToSubD on a cone, measured with RhinoCommon on 26 November 2026. This covers a
//! surface with a pole at one end of a straight direction, such as a revolved line or a circle
//! extruded to a point:
//! - next to the pole Rhino keeps the extra ring a clamped cubic has, a third of the way to the
//!   next row, as at a sphere's poles (see `add_poles`). Rhino adds it even though the direction
//!   is straight;
//! - round the other way, the row is cut at each kink of the surface (an inner knot as many times
//!   over as the degree: a circle's quarters). Each piece is interpolated on its own, so the
//!   points at the kinks lie on the surface;
//! - the lines from the pole through the inner kinks are creased. The seam line is creased only
//!   when the pole is at the end of the straight direction, not at its start.
//!
//! An open cone of radius 2 and height 4 gives Rhino's 17 points, 8 triangles and 8 quads, with
//! its rim points at (2, 0) and (1.621, 1.621). A sphere's pole lies along a curved direction,
//! and a cylinder has no pole, so neither changes.

use super::{Friendly, column, greville, one_point, through};
use curva_kernel::NurbsSurface;

/// A pole at one end of a straight direction, and the kinks round the other way.
pub(super) struct PoleKinks {
    /// The direction round the pole: 0 for u, 1 for v.
    pub around: usize,
    /// The Greville indices round the pole whose lines from the pole are creased.
    pub creased: Vec<usize>,
}

/// The pole and kinks of `surface`, when it has a pole at one end of a straight direction
/// (degree 1) and is curved the other way.
pub(super) fn pole_kinks(
    surface: &NurbsSurface,
    fr: &Friendly,
    closed: [bool; 2],
    tolerance: f64,
) -> Option<PoleKinks> {
    let on = &fr.on;
    let (nu, nv) = (on.len(), on[0].len());
    let rows = |straight: usize, end: bool| -> Vec<_> {
        match (straight, end) {
            (0, false) => on[0].clone(),
            (0, true) => on[nu - 1].clone(),
            (_, false) => column(on, 0),
            _ => column(on, nv - 1),
        }
    };
    let straight = (0..2).find(|&d| {
        surface.degree[d] == 1
            && surface.degree[1 - d] >= 2
            && (one_point(&rows(d, false), tolerance) || one_point(&rows(d, true), tolerance))
    })?;
    let around = 1 - straight;
    let at_end = one_point(&rows(straight, true), tolerance);
    let knots = if around == 0 {
        &surface.u_knots
    } else {
        &surface.v_knots
    };
    let count = if around == 0 { nu } else { nv };
    let mut creased = kinks(knots, surface.degree[around], count);
    if closed[around] && at_end && !creased.is_empty() {
        creased.insert(0, 0);
    }
    Some(PoleKinks { around, creased })
}

/// The Greville indices that stand on a kink: an inner knot repeated `degree` times or more.
fn kinks(knots: &[f64], degree: usize, count: usize) -> Vec<usize> {
    let (first, last) = (knots[0], knots[knots.len() - 1]);
    let span = (last - first).abs().max(1e-300);
    let g = greville(knots, degree, count);
    let mut found = Vec::new();
    let mut k = 0;
    while k < knots.len() {
        let t = knots[k];
        let times = knots[k..]
            .iter()
            .take_while(|&&s| (s - t).abs() <= span * 1e-12)
            .count();
        let inner = (t - first).abs() > span * 1e-12 && (last - t).abs() > span * 1e-12;
        if inner
            && times >= degree
            && let Some(i) = g.iter().position(|&s| (s - t).abs() <= span * 1e-9)
        {
            found.push(i);
        }
        k += times;
    }
    found
}

/// Rhino's net round a pole: each line round the pole cut at the kinks and at its ends, and each
/// piece interpolated with its ends on the surface. Called before the pole's ring is added.
pub(super) fn cut_at_kinks(fr: &mut Friendly, pk: &PoleKinks, closed: [bool; 2]) {
    if pk.creased.is_empty() {
        return;
    }
    let on = &fr.on;
    let count = if pk.around == 0 {
        on.len()
    } else {
        on[0].len()
    };
    let mut cuts: Vec<usize> = pk.creased.clone();
    cuts.extend([0, count - 1]);
    cuts.sort_unstable();
    cuts.dedup();
    let lines = if pk.around == 0 {
        on[0].len()
    } else {
        on.len()
    };
    for line in 0..lines {
        let q: Vec<_> = if pk.around == 0 {
            column(on, line)
        } else {
            on[line].clone()
        };
        let mut net = q.clone();
        for pair in cuts.windows(2) {
            let piece = through(&q[pair[0]..=pair[1]], false);
            net[pair[0]..=pair[1]].copy_from_slice(&piece);
        }
        if closed[pk.around] {
            net[count - 1] = net[0];
        }
        for (k, p) in net.into_iter().enumerate() {
            if pk.around == 0 {
                fr.net[k][line] = p;
            } else {
                fr.net[line][k] = p;
            }
        }
    }
}

/// The creased edges from the pole along each kink's line, as pairs of cage point ids.
pub(super) fn kink_creases(grid: &[Vec<u32>], pk: &PoleKinks) -> Vec<[u32; 2]> {
    let mut edges = Vec::new();
    for &k in &pk.creased {
        let line: Vec<u32> = if pk.around == 0 {
            grid[k].clone()
        } else {
            grid.iter().map(|row| row[k]).collect()
        };
        edges.extend(
            line.windows(2)
                .filter(|w| w[0] != w[1])
                .map(|w| [w[0], w[1]]),
        );
    }
    edges
}

#[cfg(test)]
mod tests {
    use crate::SubD;
    use curva_kernel::{Body, Curve};

    fn cone(profile: Vec<[f64; 3]>) -> SubD {
        let line = Curve::polyline(profile, false).unwrap();
        let tau = std::f64::consts::TAU;
        let body = Body::revolved(&[line], [0.0; 3], [0.0, 0.0, 1.0], tau, false).unwrap();
        SubD::from_body(&body).unwrap()
    }

    fn sides(subd: &SubD) -> Vec<usize> {
        let mut n: Vec<usize> = subd.cage().faces().map(<[u32]>::len).collect();
        n.sort_unstable();
        n
    }

    #[test]
    fn an_open_cone_comes_out_as_rhinos() {
        // Rhino 8's ToSubD on a revolved line from a rim of radius 2 at height 4 to the apex:
        // 17 points, 8 triangles round the apex and 8 quads, the apex a corner. Its points:
        // the rim at (2, 0, 4) on each quarter and (1.621, 1.621, 4) between, and a ring a
        // third of the way up from the apex. 16 creases: the rim's 8 and the four quarter
        // lines (seam included, as the apex is at the profile's end).
        let subd = cone(vec![[2.0, 0.0, 4.0], [0.0, 0.0, 0.0]]);
        assert_eq!(subd.cage().vertex_count(), 17);
        assert_eq!(sides(&subd), [vec![3; 8], vec![4; 8]].concat());
        assert_eq!(subd.creases().count(), 16);
        let near = |p: [f64; 3], q: [f64; 3]| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-3);
        let has = |q: [f64; 3]| subd.cage().vertices().iter().any(|&p| near(p, q));
        let d = 1.621;
        for q in [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 4.0],
            [0.0, -2.0, 4.0],
            [d, d, 4.0],
            [-d, -d, 4.0],
            [2.0 / 3.0, 0.0, 4.0 / 3.0],
            [d / 3.0, -d / 3.0, 4.0 / 3.0],
        ] {
            assert!(has(q), "{q:?} missing from {:?}", subd.cage().vertices());
        }
    }

    #[test]
    fn the_seam_is_creased_only_with_the_apex_at_the_profiles_end() {
        // Rhino 8: the same cone revolved from the apex out gives 14 creases. The seam's line
        // is not creased, but its points stay where they were.
        let subd = cone(vec![[0.0, 0.0, 0.0], [2.0, 0.0, 4.0]]);
        assert_eq!(subd.cage().vertex_count(), 17);
        assert_eq!(subd.creases().count(), 14);
    }

    #[test]
    fn a_cylinder_and_a_truncated_cone_have_no_kink_creases() {
        // Rhino 8: no pole, so the rings stay regular (radius 2.216 for a radius of 2) and only
        // the two rims are creased (16 edges).
        for bottom in [[2.0, 0.0, 0.0], [1.0, 0.0, 0.0]] {
            let subd = cone(vec![[2.0, 0.0, 4.0], bottom]);
            assert_eq!(subd.cage().vertex_count(), 16);
            assert_eq!(subd.creases().count(), 16);
            let top = subd.cage().vertices().iter().filter(|p| p[2] > 3.9);
            for p in top {
                let r = p[0].hypot(p[1]);
                assert!((r - 2.216).abs() < 1e-3, "{p:?}");
            }
        }
    }
}
