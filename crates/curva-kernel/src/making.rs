//! Making surfaces and solids from curves: extruding, filling flat outlines, and the rest of the
//! engine's sweeps.

use crate::{Body, Curve, KernelError, Point, cross};
use solidcore_ops::sweep::{Curve as SweepCurve, DraftCorner, extrude, planar_faces};
use std::f64::consts::TAU;

fn failed(what: &str, error: impl std::fmt::Debug) -> KernelError {
    KernelError(format!("{what} failed: {error:?}"))
}

/// A curve in the engine's terms: lines and arcs stay exact, smooth curves become NURBS.
/// A polyline or joined curve becomes several pieces.
pub(crate) fn sweep_pieces(curve: &Curve) -> Result<Vec<SweepCurve>, KernelError> {
    Ok(match curve {
        Curve::Point { .. } => return Err(KernelError("a point cannot be swept".to_owned())),
        Curve::Line { start, end } => vec![SweepCurve::Line {
            start: *start,
            end: *end,
        }],
        Curve::Polyline { points, closed } => {
            let mut corners = points.clone();
            if *closed {
                corners.push(points[0]);
            }
            corners
                .windows(2)
                .map(|pair| SweepCurve::Line {
                    start: pair[0],
                    end: pair[1],
                })
                .collect()
        }
        Curve::Circle {
            centre,
            radius,
            x,
            y,
        } => vec![arc(*centre, *radius, *x, *y, TAU)],
        Curve::Arc {
            centre,
            radius,
            x,
            y,
            sweep,
        } => vec![arc(*centre, *radius, *x, *y, *sweep)],
        Curve::Compound { segments } => {
            let mut pieces = Vec::new();
            for segment in segments {
                pieces.extend(sweep_pieces(segment)?);
            }
            pieces
        }
        Curve::Interpolated { .. } | Curve::Nurbs { .. } => {
            let Curve::Nurbs {
                degree,
                knots,
                points,
                weights,
            } = curve.to_nurbs()?
            else {
                return Err(KernelError(
                    "a smooth curve did not become NURBS".to_owned(),
                ));
            };
            if !weights.is_empty() {
                // Edited circles keep weights; how the engine reads them is not yet checked,
                // so they are refused rather than swept wrongly.
                return Err(KernelError(
                    "weighted curves cannot be swept yet; rebuild the curve first".to_owned(),
                ));
            }
            vec![SweepCurve::Nurbs {
                degree,
                points: points.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect(),
                knots,
            }]
        }
    })
}

fn arc(centre: Point, radius: f64, x: Point, y: Point, sweep: f64) -> SweepCurve {
    SweepCurve::Arc {
        center: centre,
        axis: cross(x, y),
        reference: x,
        radius,
        start: 0.0,
        end: sweep,
    }
}

fn pieces_of(curves: &[Curve]) -> Result<Vec<SweepCurve>, KernelError> {
    let mut pieces = Vec::new();
    for curve in curves {
        pieces.extend(sweep_pieces(curve)?);
    }
    Ok(pieces)
}

/// Whether `p` is inside the flat outline `ring`, both given in the plane's coordinates, by
/// counting crossings of a ray (the even-odd rule; Haines, "Point in Polygon Strategies",
/// Graphics Gems IV, 1994).
fn inside(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut crossed = false;
    for (index, a) in ring.iter().enumerate() {
        let b = ring[(index + 1) % ring.len()];
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                crossed = !crossed;
            }
        }
    }
    crossed
}

/// Closed flat curves sorted into outlines with the holes directly inside them, as Rhino
/// groups them for PlanarSrf and solid extrusions. A curve inside a hole starts a new outline.
/// Each group is the curves' places in `curves`, outline first. `normal` is the direction
/// straight out of the curves' plane.
#[must_use]
pub fn nested_groups(curves: &[Curve], normal: Point) -> Vec<Vec<usize>> {
    let x = crate::unit(cross(normal, [0.0, 0.0, 1.0]))
        .or_else(|| crate::unit(cross(normal, [1.0, 0.0, 0.0])))
        .unwrap_or([1.0, 0.0, 0.0]);
    let y = cross(normal, x);
    let flat = |p: &Point| [crate::dot(*p, x), crate::dot(*p, y)];
    let rings: Vec<Vec<[f64; 2]>> = curves
        .iter()
        .map(|curve| {
            curve
                .sample()
                .unwrap_or_default()
                .iter()
                .map(flat)
                .collect()
        })
        .collect();
    let contains = |outer: usize, inner: usize| {
        outer != inner
            && rings[outer].len() > 2
            && rings[inner]
                .first()
                .is_some_and(|p| inside(*p, &rings[outer]))
    };
    let depth: Vec<usize> = (0..curves.len())
        .map(|i| (0..curves.len()).filter(|&j| contains(j, i)).count())
        .collect();
    let mut groups: Vec<Vec<usize>> = (0..curves.len())
        .filter(|&i| depth[i].is_multiple_of(2))
        .map(|i| vec![i])
        .collect();
    for hole in (0..curves.len()).filter(|&i| !depth[i].is_multiple_of(2)) {
        if let Some(group) = groups
            .iter_mut()
            .find(|group| depth[group[0]] + 1 == depth[hole] && contains(group[0], hole))
        {
            group.push(hole);
        }
    }
    groups
}

impl Body {
    /// The curves pulled straight along `direction` (of length 1) by `distance`. With `cap`,
    /// closed flat curves are closed at both ends into a solid, and curves inside others make
    /// holes through it.
    pub fn extruded(
        curves: &[Curve],
        direction: Point,
        distance: f64,
        cap: bool,
    ) -> Result<Self, KernelError> {
        let (direction, distance) = if distance < 0.0 {
            (direction.map(|v| -v), -distance)
        } else {
            (direction, distance)
        };
        let made = extrude(
            &pieces_of(curves)?,
            direction,
            distance,
            0.0,
            DraftCorner::default(),
            cap,
        )
        .map_err(|e| failed("extruding", e))?;
        Self::made(&made)
    }

    /// Flat surfaces filling closed flat curves; a curve inside another makes a hole.
    pub fn planar(curves: &[Curve]) -> Result<Self, KernelError> {
        let made = planar_faces(&pieces_of(curves)?).map_err(|e| failed("filling", e))?;
        Self::made(&made)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn square(side: f64) -> Curve {
        Curve::polyline(
            vec![
                [0.0; 3],
                [side, 0.0, 0.0],
                [side, side, 0.0],
                [0.0, side, 0.0],
            ],
            true,
        )
        .unwrap()
    }

    #[test]
    fn a_closed_curve_extruded_with_caps_is_a_solid() {
        let body = Body::extruded(&[square(10.0)], [0.0, 0.0, 1.0], 5.0, true).unwrap();
        assert!(close(body.volume().unwrap(), 500.0));
        let circle = Curve::circle([0.0; 3], 2.0).unwrap();
        let tube = Body::extruded(&[circle], [0.0, 0.0, 1.0], 3.0, true).unwrap();
        assert!(close(
            tube.volume().unwrap(),
            std::f64::consts::PI * 4.0 * 3.0
        ));
    }

    #[test]
    fn without_caps_it_is_an_open_surface() {
        let body = Body::extruded(&[square(10.0)], [0.0, 0.0, 1.0], 5.0, false).unwrap();
        assert!(close(body.area().unwrap(), 4.0 * 50.0));
        assert!(!body.is_solid());
    }

    #[test]
    fn an_open_curve_becomes_a_surface() {
        let line = Curve::line([0.0; 3], [4.0, 0.0, 0.0]).unwrap();
        let body = Body::extruded(&[line], [0.0, 0.0, 1.0], 3.0, false).unwrap();
        assert!(close(body.area().unwrap(), 12.0));
    }

    #[test]
    fn a_negative_distance_goes_the_other_way() {
        let body = Body::extruded(&[square(1.0)], [0.0, 0.0, 1.0], -2.0, true).unwrap();
        let (low, high) = body.bounds().unwrap();
        assert!(close(low[2], -2.0) && close(high[2], 0.0));
    }

    #[test]
    fn a_hole_goes_through_the_solid() {
        let hole = Curve::circle([5.0, 5.0, 0.0], 2.0).unwrap();
        let body = Body::extruded(&[square(10.0), hole], [0.0, 0.0, 1.0], 1.0, true).unwrap();
        assert!(close(
            body.volume().unwrap(),
            100.0 - std::f64::consts::PI * 4.0
        ));
    }

    #[test]
    fn nested_curves_group_into_outlines_with_their_holes() {
        let hole_a = Curve::circle([3.0, 5.0, 0.0], 1.0).unwrap();
        let hole_b = Curve::circle([7.0, 5.0, 0.0], 1.0).unwrap();
        let island = Curve::circle([7.0, 5.0, 0.0], 0.5).unwrap();
        let apart = Curve::circle([30.0, 0.0, 0.0], 1.0).unwrap();
        let curves = [hole_a, square(10.0), island, apart, hole_b];
        let groups = super::nested_groups(&curves, [0.0, 0.0, 1.0]);
        assert_eq!(groups, [vec![1, 0, 4], vec![2], vec![3]]);
    }

    #[test]
    fn flat_curves_fill_with_holes() {
        let hole = Curve::circle([5.0, 5.0, 0.0], 2.0).unwrap();
        let body = Body::planar(&[square(10.0), hole]).unwrap();
        assert!(close(
            body.area().unwrap(),
            100.0 - std::f64::consts::PI * 4.0
        ));
    }

    #[test]
    fn a_smooth_curve_can_be_extruded() {
        let wave = Curve::interpolated(
            vec![[0.0; 3], [2.0, 2.0, 0.0], [4.0, 0.0, 0.0], [6.0, 2.0, 0.0]],
            false,
        )
        .unwrap();
        let length = wave.length().unwrap();
        let body = Body::extruded(&[wave], [0.0, 0.0, 1.0], 2.0, false).unwrap();
        assert!((body.area().unwrap() - 2.0 * length).abs() < 1e-3 * length);
    }
}
