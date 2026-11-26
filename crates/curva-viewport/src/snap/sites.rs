//! The fixed snap places on each kind of curve, and on the pieces meshes and SubDs draw.

use std::f64::consts::TAU;

use curva_kernel::{Curve, Point};

use super::{Site, SnapKind, along, knots, length, site, sub};

/// The point half way along a run of points, by distance.
pub(super) fn half_way(points: &[Point]) -> Option<Point> {
    let total: f64 = points.windows(2).map(|p| length(sub(p[1], p[0]))).sum();
    let mut left = total / 2.0;
    for pair in points.windows(2) {
        let step = length(sub(pair[1], pair[0]));
        if step >= left && step > 0.0 {
            return Some(along(pair[0], pair[1], left / step));
        }
        left -= step;
    }
    None
}

/// The quarter points of a circle or arc turning `sweep` from `x` towards `y`, short of a full
/// turn, measured along its own axes.
pub(super) fn quarters(centre: Point, radius: f64, x: Point, y: Point, sweep: f64) -> Vec<Site> {
    (0..4)
        .map(|quarter| f64::from(quarter) * std::f64::consts::FRAC_PI_2)
        // An arc's own ends are End snaps already.
        .filter(|angle| *angle < sweep - 1e-9 && (*angle > 0.0 || sweep >= TAU - 1e-9))
        .map(|angle| {
            let (sin, cos) = angle.sin_cos();
            let at =
                std::array::from_fn(|axis| centre[axis] + radius * (cos * x[axis] + sin * y[axis]));
            site(SnapKind::Quad, at)
        })
        .collect()
}

/// The fixed snap places on a curve, given the points it is drawn through: its own kind's, then
/// its knots, so where an end and a knot are one place the end wins.
pub(crate) fn sites_of(curve: &Curve, points: &[Point]) -> Vec<Site> {
    let mut sites = kind_sites(curve, points);
    sites.extend(knots::knot_sites(curve, points));
    sites
}

/// The snap places on a piece of a mesh or SubD drawn as `line`, as Rhino 8 gives them: a
/// mesh's wires only their vertices, every point they run through; a SubD's lines their own
/// places and their two ends as vertices, where its edges meet on the surface. Anything else
/// as [`sites_of`].
pub(crate) fn piece_sites(line: crate::LineKind, curve: &Curve, points: &[Point]) -> Vec<Site> {
    match line {
        crate::LineKind::MeshWire => knots::vertex_sites(points),
        crate::LineKind::SubdEdge | crate::LineKind::SubdWire => {
            let mut sites = sites_of(curve, points);
            let ends = points.first().into_iter().chain(points.last());
            sites.extend(ends.map(|p| site(SnapKind::Vertex, *p)));
            sites
        }
        _ => sites_of(curve, points),
    }
}

/// The snap places of a curve's own kind: ends, middles, centres, quarters, points.
fn kind_sites(curve: &Curve, points: &[Point]) -> Vec<Site> {
    match curve {
        Curve::Point { at } => vec![site(SnapKind::Point, *at)],
        Curve::Line { .. } | Curve::Polyline { .. } => points
            .iter()
            .map(|p| site(SnapKind::End, *p))
            .chain(
                points
                    .windows(2)
                    .map(|p| site(SnapKind::Mid, along(p[0], p[1], 0.5))),
            )
            .collect(),
        Curve::Circle {
            centre,
            radius,
            x,
            y,
        } => std::iter::once(site(SnapKind::Center, *centre))
            .chain(quarters(*centre, *radius, *x, *y, TAU))
            .collect(),
        Curve::Arc {
            centre,
            radius,
            x,
            y,
            sweep,
        } => points
            .first()
            .into_iter()
            .chain(points.last())
            .map(|p| site(SnapKind::End, *p))
            .chain(half_way(points).map(|p| site(SnapKind::Mid, p)))
            .chain(std::iter::once(site(SnapKind::Center, *centre)))
            .chain(quarters(*centre, *radius, *x, *y, *sweep))
            .collect(),
        Curve::Nurbs { .. } => {
            let open = points.first() != points.last();
            let ends = points
                .first()
                .into_iter()
                .chain(points.last())
                .filter(|_| open)
                .map(|p| site(SnapKind::End, *p));
            ends.chain(half_way(points).map(|p| site(SnapKind::Mid, p)))
                .collect()
        }
        Curve::Compound { segments } => segments
            .iter()
            .filter_map(|part| Some(sites_of(part, &part.sample().ok()?)))
            .flatten()
            .collect(),
        Curve::Interpolated { closed, .. } => {
            let ends = points
                .first()
                .into_iter()
                .chain(points.last())
                .filter(|_| !closed)
                .map(|p| site(SnapKind::End, *p));
            ends.chain(half_way(points).map(|p| site(SnapKind::Mid, p)))
                .collect()
        }
    }
}
