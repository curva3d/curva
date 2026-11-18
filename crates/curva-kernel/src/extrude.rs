//! Extruding curves into surfaces and solids: straight, drafted, to a point, and filling flat
//! outlines, with curves inside others making holes.

use crate::body::failed;
use crate::{Body, Curve, KernelError, Point, cross};
use solidcore_ops::sweep::{
    Affine, Curve as SweepCurve, DraftCorner, Ends, extrude, loft_to_point, planar_faces,
    sweep_stations,
};
use std::f64::consts::TAU;

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
            // The engine takes each control point as it is, with its weight beside it
            // (`[x, y, z, w]`), as it gives them back (`engine_ops::from_sweep_curve`).
            vec![SweepCurve::Nurbs {
                degree,
                points: points
                    .iter()
                    .enumerate()
                    .map(|(i, p)| [p[0], p[1], p[2], weights.get(i).copied().unwrap_or(1.0)])
                    .collect(),
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

pub(crate) fn pieces_of(curves: &[Curve]) -> Result<Vec<SweepCurve>, KernelError> {
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

/// How the corners of a drafted extrusion close, as Rhino's ExtrudeCrvTapered `Corners`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaperCorners {
    /// Each wall continues until it meets the next (Rhino's `Sharp`).
    Sharp,
    /// An arc about each corner (Rhino's `Round`).
    Round,
    /// Each wall continues straight along its end (Rhino's `Smooth`).
    Smooth,
}

const SAME: Affine = Affine {
    m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    t: [0.0; 3],
};

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

    /// A closed flat curve pulled along `direction` by `distance`, its walls leaning in by
    /// `draft` radians (out when negative), closed at both ends when `cap`.
    pub fn extruded_tapered(
        curve: &Curve,
        direction: Point,
        distance: f64,
        draft: f64,
        corners: TaperCorners,
        cap: bool,
    ) -> Result<Self, KernelError> {
        let corner = match corners {
            TaperCorners::Sharp => DraftCorner::Curvature,
            TaperCorners::Round => DraftCorner::Round,
            TaperCorners::Smooth => DraftCorner::Tangent,
        };
        let (direction, distance) = if distance < 0.0 {
            (direction.map(|v| -v), -distance)
        } else {
            (direction, distance)
        };
        let made = extrude(
            &pieces_of(std::slice::from_ref(curve))?,
            direction,
            distance,
            draft,
            corner,
            cap,
        )
        .map_err(|e| failed("extruding with a draft", e))?;
        Self::made(&made)
    }

    /// A pyramid on a closed polygon of lines, to `apex`.
    pub fn pyramid(base: &Curve, apex: Point) -> Result<Self, KernelError> {
        let made =
            loft_to_point(&sweep_pieces(base)?, apex).map_err(|e| failed("making a pyramid", e))?;
        Self::made(&made)
    }

    /// Lines from every point of `curve` to `apex`, as Rhino's ExtrudeCrvToPoint: a pyramid for
    /// a closed polygon when `cap`; otherwise a ruled surface whose tip closes to a ten-thousandth
    /// of the curve's size, as the engine's sections cannot shrink to nothing.
    pub fn extruded_to_point(curve: &Curve, apex: Point, cap: bool) -> Result<Self, KernelError> {
        if cap && matches!(curve, Curve::Polyline { closed: true, .. }) {
            return Self::pyramid(curve, apex);
        }
        // CUSTOM KERNEL OPERATION: cone-to-point
        let ends = if cap { Ends::Capped } else { Ends::Open };
        let profile = pieces_of(std::slice::from_ref(curve))?;
        // The smallest tip the engine closes reliably, then a larger one if it will not.
        let mut last = None;
        for tiny in [1e-4, 1e-3] {
            let toward = Affine {
                m: [[tiny, 0.0, 0.0], [0.0, tiny, 0.0], [0.0, 0.0, tiny]],
                t: apex.map(|v| v * (1.0 - tiny)),
            };
            match sweep_stations(&profile, &[SAME, toward], ends) {
                Ok(made) => return Self::made(&made),
                Err(e) => last = Some(e),
            }
        }
        Err(failed("extruding to a point", last))
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve, TaperCorners};

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

    #[test]
    fn a_square_tapered_in_makes_a_frustum() {
        // 10 × 10, 2 high, walls leaning in 45°: the top is 6 × 6; volume h/3 (A + B + √AB).
        let solid = Body::extruded_tapered(
            &square(10.0),
            [0.0, 0.0, 1.0],
            2.0,
            std::f64::consts::FRAC_PI_4,
            TaperCorners::Sharp,
            true,
        )
        .unwrap();
        let expected = 2.0 / 3.0 * (100.0 + 36.0 + 60.0);
        assert!(
            (solid.volume().unwrap() - expected).abs() < 1e-3,
            "{}",
            solid.volume().unwrap()
        );
    }

    #[test]
    fn pyramids_and_cones_to_a_point() {
        let pyramid = Body::extruded_to_point(&square(3.0), [1.5, 1.5, 4.0], true).unwrap();
        assert!((pyramid.volume().unwrap() - 12.0).abs() < 1e-6);
        let circle = Curve::circle([0.0; 3], 2.0).unwrap();
        let cone = Body::extruded_to_point(&circle, [0.0, 0.0, 6.0], true).unwrap();
        let expected = std::f64::consts::PI * 4.0 * 6.0 / 3.0;
        assert!((cone.volume().unwrap() - expected).abs() < 1e-3 * expected);
    }

    #[test]
    fn a_weighted_curve_extruded_is_the_shape_it_draws() {
        // A circle as a weighted NURBS curve (as Rhino files and edited circles hold them),
        // far from the origin: its solid has the true cylinder's volume.
        let (cx, cy, r) = (3000.0, -2000.0, 5.0);
        let corner = std::f64::consts::FRAC_1_SQRT_2;
        let around = [
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
            (1.0, 0.0),
        ];
        let weighted = Curve::nurbs(
            2,
            vec![
                0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
            ],
            around
                .iter()
                .map(|(x, y)| [cx + r * x, cy + r * y, 0.0])
                .collect(),
            (0..9)
                .map(|k| if k % 2 == 1 { corner } else { 1.0 })
                .collect(),
        )
        .unwrap();
        let solid = Body::extruded(&[weighted], [0.0, 0.0, 1.0], 12.0, true).unwrap();
        let volume = solid.volume().unwrap();
        assert!(
            close(volume, std::f64::consts::PI * 25.0 * 12.0),
            "{volume}"
        );
        // Its top edge lies on the circle.
        let (low, high) = solid.bounds().unwrap();
        assert!(close(high[0] - low[0], 10.0) && close(high[1] - low[1], 10.0));
    }
}
