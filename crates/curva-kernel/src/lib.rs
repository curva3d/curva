//! Curva's interface to the geometry engine. Nothing above this crate touches solidcore, so the
//! engine can change or be joined by another without the commands noticing.

use serde::{Deserialize, Serialize};
use solidcore::curve::curve::Curve as CoreCurve;
use solidcore::nurbs::b_spline_curve::BSplineCurve;
use solidcore::nurbs::crv_fit::p1::ParamMethod;
use solidcore::nurbs::crv_fit::p3::cubic_spline_interp;
use solidcore::nurbs::math::{CPoint, NOW};
use solidcore::query::api_queries::curve_closest_point;
use solidcore::{Kernel, Point3, Vec3};
use std::f64::consts::TAU;

mod vectors;
#[allow(clippy::wildcard_imports)]
pub use vectors::*;
mod engine_curves;
#[allow(clippy::wildcard_imports)]
pub(crate) use engine_curves::*;
mod join;
#[allow(clippy::wildcard_imports)]
pub use join::*;
mod similarity;
#[allow(clippy::wildcard_imports)]
pub use similarity::*;
mod curve;
mod curve_nurbs;
#[allow(clippy::wildcard_imports)]
pub use curve::*;
mod area;
mod assemble;
mod measure;
pub use assemble::SEW_TOLERANCE;
mod body;
mod boolean;
pub use boolean::Combine;
mod conic;
mod corners;
mod cut;
mod engine_ops;
mod fillet;
mod finishing;
mod grips;
mod knots;
pub use engine_ops::{Carry, Contact};
mod extrude;
pub mod hidden;
pub mod parallel;
pub mod rays;
mod split;
mod sweep;
pub use extrude::TaperCorners;
pub use extrude::nested_groups;
mod interp;
mod offset;
mod parts_moved;
mod record_io;
pub use record_io::{from_record_curve, to_record_curve, to_record_spline};
/// The engine's portable B-rep record, for reading and writing other formats.
pub use solidcore_record::brep as record;
mod spline;
mod surface;
mod surface_curves;
mod surface_edit;
mod surface_making;
pub use spline::Spline;
pub use surface::NurbsSurface;
mod text;
pub use text::{font_bytes, text_outlines};
pub mod step;
mod tween;
pub use body::{Body, BodyMesh};
pub use conic::{Chamfered, Elliptical, chamfer_lines};
pub use cut::{closest_between, crossings};
pub use fillet::fillet_curves;
pub use offset::{Filleted, fillet_lines};
use std::fmt;
pub use tween::tween_curves;

/// A position in model space, in millimetres.
pub type Point = [f64; 3];

/// Accuracy asked of curve lengths.
const LENGTH_ACCURACY: f64 = 1e-6;
/// How far plane directions may be from length 1 and a right angle.
const PLANE_TOLERANCE: f64 = 1e-9;
/// Smallest angle treated as a real turn, in radians.
const ANGLE_TOLERANCE: f64 = 1e-9;
/// Shortest length treated as a real size, in millimetres.
const SMALLEST: f64 = 1e-9;
/// Display points per input point of an interpolated curve.
const SAMPLES_PER_POINT: usize = 32;
/// Display points around a circle.
const CIRCLE_SAMPLES: usize = 128;

/// Why a curve could not be made or measured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelError(pub String);

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for KernelError {}

#[cfg(test)]
mod control_point_tests {
    use super::*;

    fn ends(curve: &Curve) -> (Point, Point) {
        let path = curve.sample().unwrap();
        (path[0], path[path.len() - 1])
    }

    /// The direction a clamped NURBS curve leaves its start in, and arrives at its end in: along
    /// its first and last legs of control points.
    fn end_directions(curve: &Curve) -> (Point, Point) {
        let path = control_points(curve);
        let n = path.len();
        let along = |a: Point, b: Point| {
            let d = gap(a, b);
            [(b[0] - a[0]) / d, (b[1] - a[1]) / d, (b[2] - a[2]) / d]
        };
        (along(path[0], path[1]), along(path[n - 2], path[n - 1]))
    }

    #[test]
    fn a_fitted_curve_passes_its_points_leaving_and_arriving_as_told() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [10.0, 5.0, 0.0],
            [20.0, 0.0, 0.0],
            [30.0, 4.0, 0.0],
        ];
        let up = [0.0, 1.0, 0.0];
        let down = [0.0, -1.0, 0.0];
        for spacing in [Spacing::Chord, Spacing::SquareRoot, Spacing::Uniform] {
            let curve =
                Curve::fitted(points.clone(), false, Some(up), Some(down), spacing).unwrap();
            assert!(matches!(curve, Curve::Nurbs { .. }), "{spacing:?}");
            for p in &points {
                let on = curve.closest_point(*p).unwrap();
                assert!(gap(on, *p) < 1e-6, "{spacing:?}: {p:?} missed by {on:?}");
            }
            let (leaving, arriving) = end_directions(&curve);
            assert!(
                gap(leaving, up) < 1e-9,
                "{spacing:?}: leaves along {leaving:?}"
            );
            assert!(
                gap(arriving, down) < 1e-9,
                "{spacing:?}: arrives along {arriving:?}"
            );
        }
        // Spacing alone changes the shape between the points.
        let plain = Curve::fitted(points.clone(), false, None, None, Spacing::Chord).unwrap();
        let even = Curve::fitted(points.clone(), false, None, None, Spacing::Uniform).unwrap();
        assert!((even.length().unwrap() - plain.length().unwrap()).abs() > 1e-6);
    }

    fn control_points(curve: &Curve) -> Vec<Point> {
        match curve {
            Curve::Nurbs { points, .. } => points.clone(),
            other => panic!("not NURBS: {other:?}"),
        }
    }

    fn same_points(a: &[Point], b: &[Point], within: f64) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(p, q)| gap(*p, *q) < within)
    }

    /// Rhino 8's control points for these AutoCAD fit-point splines (read from DXF files that
    /// held only the fit points, the tangents and the knot parameterization).
    #[test]
    fn fitted_curves_are_the_ones_rhino_reads_autocads_fit_points_as() {
        let four = vec![
            [0.0, 0.0, 0.0],
            [10.0, 5.0, 0.0],
            [20.0, 0.0, 0.0],
            [30.0, 4.0, 0.0],
        ];
        let tangents = Curve::fitted(
            four,
            false,
            Some([0.0, 1.0, 0.0]),
            Some([0.0, -1.0, 0.0]),
            Spacing::SquareRoot,
        )
        .unwrap();
        let rhino = [
            [0.0, 0.0, 0.0],
            [0.0, 1.114_567_174_960_703_3, 0.0],
            [12.030_339_317_999_369, 9.517_497_655_124_64, 0.0],
            [17.857_639_682_973_49, -4.893_636_093_012_162, 0.0],
            [30.0, 5.093_939_344_970_43, 0.0],
            [30.0, 4.0, 0.0],
        ];
        assert!(same_points(&control_points(&tangents), &rhino, 1e-9));
        // Without tangents both ends are natural (not bent).
        let five = vec![
            [0.0, 0.0, 0.0],
            [10.0, 5.0, 0.0],
            [20.0, 0.0, 0.0],
            [30.0, 4.0, 0.0],
            [36.0, 12.0, 0.0],
        ];
        let chord = Curve::fitted(five.clone(), false, None, None, Spacing::Chord).unwrap();
        let rhino = [
            [0.0, 0.0, 0.0],
            [3.365_745, 2.753_141, 0.0],
            [10.097_235, 8.259_423, 0.0],
            [19.494_761, -2.899_595, 0.0],
            [30.739_718, 3.382_552, 0.0],
            [34.290_469, 9.199_429, 0.0],
            [36.0, 12.0, 0.0],
        ];
        assert!(same_points(&control_points(&chord), &rhino, 1e-6));
        let uniform = Curve::fitted(five.clone(), false, None, None, Spacing::Uniform).unwrap();
        let rhino = [
            [0.0, 0.0, 0.0],
            [47.0 / 14.0, 2.75, 0.0],
            [141.0 / 14.0, 8.25, 0.0],
            [138.0 / 7.0, -3.0, 0.0],
            [435.0 / 14.0, 3.75, 0.0],
            [481.0 / 14.0, 9.25, 0.0],
            [36.0, 12.0, 0.0],
        ];
        assert!(same_points(&control_points(&uniform), &rhino, 1e-9));
        // One tangent: the other end natural.
        let one = Curve::fitted(five, false, Some([0.0, 1.0, 0.0]), None, Spacing::Chord).unwrap();
        let rhino = [
            [0.0, 0.0, 0.0],
            [0.0, 3.726_780, 0.0],
            [11.661_228, 7.806_993, 0.0],
            [19.093_722, -2.783_583, 0.0],
            [30.830_210, 3.356_375, 0.0],
            [34.319_878, 9.190_922, 0.0],
            [36.0, 12.0, 0.0],
        ];
        assert!(same_points(&control_points(&one), &rhino, 1e-6));
    }

    #[test]
    fn an_open_curve_starts_and_ends_on_its_end_points() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [5.0, 8.0, 0.0],
            [10.0, -3.0, 0.0],
            [15.0, 2.0, 0.0],
        ];
        let curve = Curve::control_point(points, 3, false).unwrap();
        let Curve::Nurbs {
            points: kept,
            degree,
            ..
        } = &curve
        else {
            panic!("not NURBS")
        };
        assert_eq!((kept.len(), *degree), (4, 3));
        let (first, last) = ends(&curve);
        assert!(gap(first, [0.0, 0.0, 0.0]) < 1e-6 && gap(last, [15.0, 2.0, 0.0]) < 1e-6);
    }

    #[test]
    fn closed_curves_close_for_every_degree() {
        let hexagon: Vec<Point> = (0..6)
            .map(|i| {
                let turn = f64::from(i) * std::f64::consts::TAU / 6.0;
                [10.0 * turn.cos(), 10.0 * turn.sin(), f64::from(i % 2)]
            })
            .collect();
        for degree in 1..=7 {
            let curve = Curve::control_point(hexagon.clone(), degree, true).unwrap();
            let (first, last) = ends(&curve);
            assert!(
                gap(first, last) < 1e-6,
                "degree {degree}: {first:?} {last:?}"
            );
        }
    }

    #[test]
    fn too_few_points_lower_the_degree() {
        let curve = Curve::control_point(vec![[0.0; 3], [4.0, 1.0, 0.0]], 3, false).unwrap();
        assert!(matches!(curve, Curve::Nurbs { degree: 1, .. }));
    }

    #[test]
    fn a_closed_cubic_starts_where_a_uniform_b_spline_does_and_closes() {
        let square = vec![
            [0.0, 0.0, 0.0],
            [6.0, 0.0, 0.0],
            [6.0, 6.0, 0.0],
            [0.0, 6.0, 0.0],
        ];
        let curve = Curve::control_point(square.clone(), 3, true).unwrap();
        let (first, last) = ends(&curve);
        assert!(gap(first, last) < 1e-6, "{first:?} {last:?}");
        // A uniform cubic B-spline over points a, b, c, d starts at (a + 4b + c) / 6.
        let expected: Point =
            std::array::from_fn(|i| (square[0][i] + 4.0 * square[1][i] + square[2][i]) / 6.0);
        assert!(gap(first, expected) < 1e-6, "{first:?} vs {expected:?}");
        // Symmetric points give a curve that stays inside them and keeps away from the corners.
        for point in curve.sample().unwrap() {
            assert!((0.0..=6.0).contains(&point[0]) && (0.0..=6.0).contains(&point[1]));
            assert!(gap(point, [0.0, 0.0, 0.0]) > 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLOSE: f64 = 1e-6;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < CLOSE
    }

    #[test]
    fn a_circle_lies_on_its_plane() {
        let x = [1.0, 0.0, 0.0];
        let z = [0.0, 0.0, 1.0];
        let circle = Curve::circle_on([2.0, 3.0, 4.0], 5.0, x, z).unwrap();
        for point in circle.sample().unwrap() {
            assert!(near(point[1], 3.0));
            let reach = (point[0] - 2.0).hypot(point[2] - 4.0);
            assert!(near(reach, 5.0));
        }
    }

    #[test]
    fn a_circle_on_a_plane_has_the_same_length() {
        let plane = Curve::circle_on([0.0; 3], 3.0, [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let flat = Curve::circle([0.0; 3], 3.0).unwrap();
        assert!(near(plane.length().unwrap(), flat.length().unwrap()));
    }

    #[test]
    fn a_circle_needs_directions_at_a_right_angle() {
        let tilted = [1.0, 1.0, 0.0];
        assert!(Curve::circle_on([0.0; 3], 3.0, [1.0, 0.0, 0.0], tilted).is_err());
        assert!(Curve::circle_on([0.0; 3], 3.0, [2.0, 0.0, 0.0], [0.0, 1.0, 0.0]).is_err());
    }

    #[test]
    fn moving_a_circle_keeps_its_plane() {
        let circle = Curve::circle_on([0.0; 3], 3.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        assert_eq!(
            circle.translated([1.0, 2.0, 3.0]),
            Curve::circle_on([1.0, 2.0, 3.0], 3.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
        );
    }

    fn points_near(a: Point, b: Point) -> bool {
        gap(a, b) < CLOSE
    }

    const X: Point = [1.0, 0.0, 0.0];
    const Y: Point = [0.0, 1.0, 0.0];
    const Z: Point = [0.0, 0.0, 1.0];

    #[test]
    fn a_rotation_turns_counterclockwise() {
        let turn = Similarity::rotation([0.0; 3], Z, std::f64::consts::FRAC_PI_2);
        assert!(points_near(turn.point([1.0, 0.0, 5.0]), [0.0, 1.0, 5.0]));
    }

    #[test]
    fn a_rotation_is_about_its_origin() {
        let turn = Similarity::rotation([2.0, 0.0, 0.0], Z, std::f64::consts::PI);
        assert!(points_near(turn.point([3.0, 0.0, 0.0]), [1.0, 0.0, 0.0]));
    }

    #[test]
    fn scaling_is_about_its_origin() {
        let bigger = Similarity::scaling([1.0, 1.0, 1.0], 3.0);
        assert!(points_near(bigger.point([2.0, 1.0, 1.0]), [4.0, 1.0, 1.0]));
    }

    #[test]
    fn a_mirror_reflects_across_its_plane() {
        let mirror = Similarity::mirror([5.0, 0.0, 0.0], X);
        assert!(points_near(mirror.point([3.0, 7.0, 1.0]), [7.0, 7.0, 1.0]));
    }

    #[test]
    fn scaling_a_circle_scales_its_radius_only() {
        let circle = Curve::circle([2.0, 0.0, 0.0], 3.0).unwrap();
        let bigger = circle.mapped(&Similarity::scaling([0.0; 3], 2.0));
        assert_eq!(bigger, Curve::circle([4.0, 0.0, 0.0], 6.0).unwrap());
    }

    #[test]
    fn turning_a_flat_circle_on_edge_keeps_it_a_circle() {
        let circle = Curve::circle([0.0; 3], 3.0).unwrap();
        let turned = circle.mapped(&Similarity::rotation(
            [0.0; 3],
            X,
            std::f64::consts::FRAC_PI_2,
        ));
        for point in turned.sample().unwrap() {
            assert!(point[1].abs() < CLOSE);
            assert!(near(point[0].hypot(point[2]), 3.0));
        }
    }

    #[test]
    fn an_arc_goes_counterclockwise_from_start_to_end() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 9.0, 0.0], X, Y).unwrap();
        assert!(near(
            arc.length().unwrap(),
            5.0 * std::f64::consts::FRAC_PI_2
        ));
        let points = arc.sample().unwrap();
        assert!(points_near(points[0], [5.0, 0.0, 0.0]));
        assert!(points_near(*points.last().unwrap(), [0.0, 5.0, 0.0]));
    }

    #[test]
    fn an_arc_can_turn_past_half_way() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, -1.0, 0.0], X, Y).unwrap();
        assert!(near(
            arc.length().unwrap(),
            5.0 * 3.0 * std::f64::consts::FRAC_PI_2
        ));
    }

    #[test]
    fn an_arc_lies_on_its_plane() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 0.0, 5.0], X, Z).unwrap();
        assert!(arc.sample().unwrap().iter().all(|p| p[1].abs() < CLOSE));
    }

    #[test]
    fn an_arc_needs_a_real_radius_and_turn() {
        assert!(Curve::arc([0.0; 3], [0.0; 3], [1.0, 0.0, 0.0], X, Y).is_err());
        assert!(Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [9.0, 0.0, 0.0], X, Y).is_err());
    }

    #[test]
    fn mirroring_an_arc_keeps_its_ends_mirrored() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        let mirrored = arc.mapped(&Similarity::mirror([0.0; 3], X));
        let points = mirrored.sample().unwrap();
        assert!(points_near(points[0], [-5.0, 0.0, 0.0]));
        assert!(points_near(*points.last().unwrap(), [0.0, 5.0, 0.0]));
    }

    #[test]
    fn moving_an_arc_keeps_its_shape() {
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        let moved = arc.translated([1.0, 0.0, 0.0]);
        assert!(points_near(moved.sample().unwrap()[0], [6.0, 0.0, 0.0]));
    }

    fn quarter_circle() -> Curve {
        let corner_weight = std::f64::consts::FRAC_1_SQRT_2;
        Curve::nurbs(
            2,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            vec![1.0, corner_weight, 1.0],
        )
        .unwrap()
    }

    fn smooth_curve() -> Curve {
        Curve::interpolated(
            vec![
                [0.0; 3],
                [10.0, 5.0, 0.0],
                [20.0, -3.0, 4.0],
                [30.0, 0.0, 0.0],
            ],
            false,
        )
        .unwrap()
    }

    #[test]
    fn a_rational_nurbs_curve_can_be_an_exact_quarter_circle() {
        let curve = quarter_circle();
        for point in curve.sample().unwrap() {
            assert!(near(point[0].hypot(point[1]), 1.0), "{point:?}");
        }
        assert!(near(curve.length().unwrap(), std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn a_nurbs_curve_starts_and_ends_on_its_end_control_points() {
        let points = quarter_circle().sample().unwrap();
        assert!(points_near(points[0], X));
        assert!(points_near(*points.last().unwrap(), Y));
    }

    #[test]
    fn turning_a_smooth_curve_into_nurbs_keeps_its_shape() {
        let original = smooth_curve();
        let exact = original.to_nurbs().unwrap();
        assert!(matches!(exact, Curve::Nurbs { .. }));
        assert!(near(exact.length().unwrap(), original.length().unwrap()));
        let points = exact.sample().unwrap();
        assert!(points_near(points[0], [0.0; 3]));
        assert!(points_near(*points.last().unwrap(), [30.0, 0.0, 0.0]));
        let through = [10.0, 5.0, 0.0];
        let closest = points;
        assert!(closest.iter().any(|p| gap(*p, through) < 0.5));
    }

    #[test]
    fn a_closed_smooth_curve_turns_into_nurbs_too() {
        let points = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let original = Curve::interpolated(points, true).unwrap();
        let exact = original.to_nurbs().unwrap();
        assert!(near(exact.length().unwrap(), original.length().unwrap()));
    }

    #[test]
    fn lines_turn_into_nurbs_of_degree_one() {
        // As Rhino 8's ToNURBS makes them (#550); they were refused before.
        let exact = Curve::line([0.0; 3], X).unwrap().to_nurbs().unwrap();
        assert!(matches!(exact, Curve::Nurbs { degree: 1, .. }));
    }

    #[test]
    fn moving_a_nurbs_curve_moves_it_exactly() {
        let moved = quarter_circle().translated([5.0, 0.0, 0.0]);
        assert!(points_near(moved.sample().unwrap()[0], [6.0, 0.0, 0.0]));
        assert!(near(moved.length().unwrap(), std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn resizing_and_turning_a_nurbs_curve_keeps_its_weights() {
        let bigger = quarter_circle().mapped(&Similarity::scaling([0.0; 3], 3.0));
        assert!(near(
            bigger.length().unwrap(),
            3.0 * std::f64::consts::FRAC_PI_2
        ));
        let turned = quarter_circle().mapped(&Similarity::rotation(
            [0.0; 3],
            Z,
            std::f64::consts::FRAC_PI_2,
        ));
        assert!(points_near(turned.sample().unwrap()[0], Y));
        for point in turned.sample().unwrap() {
            assert!(near(point[0].hypot(point[1]), 1.0));
        }
    }

    #[test]
    fn bad_nurbs_data_is_refused() {
        let points = vec![[0.0; 3], X, Y];
        let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        assert!(Curve::nurbs(0, knots.clone(), points.clone(), vec![]).is_err());
        assert!(Curve::nurbs(2, vec![0.0, 1.0], points.clone(), vec![]).is_err());
        let falling = vec![0.0, 0.0, 1.0, 0.5, 1.0, 1.0];
        assert!(Curve::nurbs(2, falling, points.clone(), vec![]).is_err());
        assert!(Curve::nurbs(2, knots.clone(), points.clone(), vec![1.0]).is_err());
        assert!(Curve::nurbs(2, knots, points, vec![1.0, -1.0, 1.0]).is_err());
    }

    #[test]
    fn a_nurbs_curve_survives_being_saved_as_text() {
        let curve = quarter_circle();
        let text = serde_json::to_string(&curve).unwrap();
        let back: Curve = serde_json::from_str(&text).unwrap();
        assert_eq!(back, curve);
    }

    #[test]
    fn lines_meeting_end_to_end_join_into_one_polyline() {
        let a = Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        let b = Curve::line([1.0, 0.0, 0.0], [1.0, 1.0, 0.0]).unwrap();
        let made = join(&[a, b]).unwrap();
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].0, [0, 1]);
        assert_eq!(
            made[0].1,
            Curve::polyline(vec![[0.0; 3], X, [1.0, 1.0, 0.0]], false).unwrap()
        );
    }

    #[test]
    fn join_flips_pieces_and_finds_them_in_any_order() {
        let a = Curve::line([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap();
        let b = Curve::line([3.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap();
        let c = Curve::line([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]).unwrap();
        let made = join(&[a, b, c]).unwrap();
        assert_eq!(made.len(), 1);
        assert!(near(made[0].1.length().unwrap(), 3.0));
    }

    #[test]
    fn lines_forming_a_loop_join_closed() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::line(X, Y).unwrap();
        let c = Curve::line(Y, [0.0; 3]).unwrap();
        let made = join(&[a, b, c]).unwrap();
        assert!(matches!(made[0].1, Curve::Polyline { closed: true, .. }));
        assert!(near(made[0].1.length().unwrap(), 2.0 + 2.0_f64.sqrt()));
    }

    #[test]
    fn curves_that_do_not_meet_are_left_alone() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap();
        assert!(join(&[a, b]).unwrap().is_empty());
    }

    #[test]
    fn smooth_curves_join_with_their_lengths_adding_up() {
        let a =
            Curve::interpolated(vec![[0.0; 3], [5.0, 3.0, 0.0], [10.0, 0.0, 0.0]], false).unwrap();
        let b = Curve::interpolated(
            vec![[10.0, 0.0, 0.0], [15.0, -3.0, 0.0], [20.0, 0.0, 0.0]],
            false,
        )
        .unwrap();
        let total = a.length().unwrap() + b.length().unwrap();
        let made = join(&[a, b]).unwrap();
        assert!(matches!(made[0].1, Curve::Nurbs { .. }));
        assert!((made[0].1.length().unwrap() - total).abs() < 1e-4);
        let points = made[0].1.sample().unwrap();
        assert!(points_near(points[0], [0.0; 3]));
        assert!(points_near(*points.last().unwrap(), [20.0, 0.0, 0.0]));
    }

    #[test]
    fn a_reversed_smooth_curve_joins_the_right_way_round() {
        let a =
            Curve::interpolated(vec![[0.0; 3], [5.0, 3.0, 0.0], [10.0, 0.0, 0.0]], false).unwrap();
        let b = Curve::interpolated(
            vec![[20.0, 0.0, 0.0], [15.0, -3.0, 0.0], [10.0, 0.0, 0.0]],
            false,
        )
        .unwrap();
        let total = a.length().unwrap() + b.length().unwrap();
        let made = join(&[a, b]).unwrap();
        assert!((made[0].1.length().unwrap() - total).abs() < 1e-4);
    }

    fn line_then_arc() -> (Curve, Curve) {
        let line = Curve::line([-5.0, 0.0, 0.0], [5.0, 0.0, 0.0]).unwrap();
        let arc = Curve::arc([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], X, Y).unwrap();
        (line, arc)
    }

    #[test]
    fn a_line_and_an_arc_join_into_a_compound_curve_that_keeps_them() {
        let (line, arc) = line_then_arc();
        let total = line.length().unwrap() + arc.length().unwrap();
        let made = join(&[line.clone(), arc.clone()]).unwrap();
        assert_eq!(made.len(), 1);
        assert!(near(made[0].1.length().unwrap(), total));
        assert_eq!(made[0].1.exploded().unwrap(), [line, arc]);
    }

    #[test]
    fn a_line_and_a_smooth_curve_join_into_a_compound_curve() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::interpolated(vec![X, [5.0, 3.0, 0.0], [10.0, 0.0, 0.0]], false).unwrap();
        let total = a.length().unwrap() + b.length().unwrap();
        let made = join(&[a, b]).unwrap();
        assert!(matches!(made[0].1, Curve::Compound { .. }));
        assert!(near(made[0].1.length().unwrap(), total));
    }

    #[test]
    fn a_compound_curve_runs_continuously() {
        let (line, arc) = line_then_arc();
        let curve = Curve::compound(vec![line, arc]).unwrap();
        let points = curve.sample().unwrap();
        assert!(points_near(points[0], [-5.0, 0.0, 0.0]));
        assert!(points_near(*points.last().unwrap(), [0.0, 5.0, 0.0]));
        assert!(points.windows(2).all(|pair| gap(pair[0], pair[1]) > 0.0));
    }

    #[test]
    fn parts_that_do_not_meet_cannot_make_a_compound_curve() {
        let a = Curve::line([0.0; 3], X).unwrap();
        let b = Curve::line([5.0, 0.0, 0.0], [6.0, 0.0, 0.0]).unwrap();
        assert!(Curve::compound(vec![a.clone(), b]).is_err());
        assert!(Curve::compound(vec![a]).is_err());
    }

    #[test]
    fn a_compound_curve_moves_turns_and_resizes_as_a_whole() {
        let (line, arc) = line_then_arc();
        let curve = Curve::compound(vec![line, arc]).unwrap();
        let moved = curve.translated([0.0, 0.0, 7.0]);
        assert!(moved.sample().unwrap().iter().all(|p| near(p[2], 7.0)));
        let bigger = curve.mapped(&Similarity::scaling([0.0; 3], 2.0));
        assert!(near(
            bigger.length().unwrap(),
            2.0 * curve.length().unwrap()
        ));
    }

    #[test]
    fn a_reversed_curve_keeps_its_shape_and_swaps_its_ends() {
        let (line, arc) = line_then_arc();
        let smooth = smooth_curve();
        let exact = quarter_circle();
        let joined = Curve::compound(vec![line.clone(), arc.clone()]).unwrap();
        for curve in [line, arc, smooth, exact, joined] {
            let forward = curve.sample().unwrap();
            let back = curve.reversed().sample().unwrap();
            assert!(near(
                curve.reversed().length().unwrap(),
                curve.length().unwrap()
            ));
            assert!(points_near(back[0], *forward.last().unwrap()), "{curve:?}");
            assert!(points_near(*back.last().unwrap(), forward[0]), "{curve:?}");
        }
    }

    #[test]
    fn an_arc_joins_when_it_has_to_be_flipped() {
        let (line, arc) = line_then_arc();
        let made = join(&[line.reversed(), arc.reversed()]).unwrap();
        assert_eq!(made.len(), 1);
        assert!(near(
            made[0].1.length().unwrap(),
            line.length().unwrap() + arc.length().unwrap()
        ));
    }

    #[test]
    fn a_compound_curve_joins_on_as_one_flat_list() {
        let (line, arc) = line_then_arc();
        let first = Curve::compound(vec![line, arc]).unwrap();
        let next = Curve::line([0.0, 5.0, 0.0], [0.0, 9.0, 0.0]).unwrap();
        let made = join(&[first, next]).unwrap();
        assert_eq!(made[0].1.exploded().unwrap().len(), 3);
    }

    #[test]
    fn a_polyline_explodes_into_lines() {
        let polyline = Curve::polyline(vec![[0.0; 3], X, [1.0, 1.0, 0.0]], false).unwrap();
        assert_eq!(polyline.exploded().unwrap().len(), 2);
    }

    #[test]
    fn a_closed_polyline_explodes_into_every_side() {
        let polyline = Curve::polyline(vec![[0.0; 3], X, [1.0, 1.0, 0.0]], true).unwrap();
        assert_eq!(polyline.exploded().unwrap().len(), 3);
    }

    #[test]
    fn a_single_curve_does_not_explode() {
        assert!(Curve::line([0.0; 3], X).unwrap().exploded().is_none());
        assert!(smooth_curve().exploded().is_none());
    }

    #[test]
    fn a_point_moves_turns_and_has_no_length() {
        let point = Curve::Point { at: X };
        assert_eq!(point.length().unwrap(), 0.0);
        assert_eq!(point.sample().unwrap(), [X]);
        assert_eq!(
            point.translated(Y),
            Curve::Point {
                at: [1.0, 1.0, 0.0]
            }
        );
        let bigger = point.mapped(&Similarity::scaling([0.0; 3], 3.0));
        assert_eq!(
            bigger,
            Curve::Point {
                at: [3.0, 0.0, 0.0]
            }
        );
    }

    #[test]
    fn a_circle_divides_exactly_on_itself() {
        let circle = Curve::circle([0.0; 3], 5.0).unwrap();
        let found = circle.divided(4).unwrap();
        assert_eq!(found.len(), 4);
        assert!(points_near(found[1], [0.0, 5.0, 0.0]), "{:?}", found[1]);
        assert!(found.iter().all(|p| near(p[0].hypot(p[1]), 5.0)));
    }

    #[test]
    fn a_polyline_divides_by_distance_along_it() {
        let path = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        let found = Curve::polyline(path, false).unwrap().divided(2).unwrap();
        assert!(points_near(found[1], [10.0, 0.0, 0.0]));
        assert!(points_near(found[2], [10.0, 10.0, 0.0]));
    }

    #[test]
    fn a_point_cannot_be_divided() {
        assert!(Curve::Point { at: X }.divided(2).is_err());
    }
    #[test]
    fn line_length_is_exact() {
        let line = Curve::line([0.0; 3], [3.0, 4.0, 0.0]).unwrap();
        assert!(near(line.length().unwrap(), 5.0));
    }

    #[test]
    fn zero_length_line_is_refused() {
        assert!(Curve::line([1.0; 3], [1.0; 3]).is_err());
    }

    #[test]
    fn closed_polyline_counts_the_last_segment() {
        let square = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let open = Curve::polyline(square.clone(), false).unwrap();
        let closed = Curve::polyline(square, true).unwrap();
        assert!(near(open.length().unwrap(), 30.0));
        assert!(near(closed.length().unwrap(), 40.0));
    }

    #[test]
    fn circle_length_and_points_are_exact() {
        let circle = Curve::circle([1.0, 2.0, 3.0], 5.0).unwrap();
        assert!(near(circle.length().unwrap(), TAU * 5.0));
        for point in circle.sample().unwrap() {
            assert!(near(gap(point, [1.0, 2.0, 3.0]), 5.0));
        }
    }

    #[test]
    fn non_positive_radius_is_refused() {
        assert!(Curve::circle([0.0; 3], 0.0).is_err());
        assert!(Curve::circle([0.0; 3], -1.0).is_err());
    }

    #[test]
    fn interpolated_curve_passes_through_every_point() {
        let points = vec![
            [0.0, 0.0, 0.0],
            [10.0, 5.0, 2.0],
            [20.0, -3.0, 6.0],
            [30.0, 4.0, 1.0],
        ];
        let curve = Curve::interpolated(points.clone(), false).unwrap();
        let drawn = curve.sample().unwrap();
        for point in points {
            let distance = curve.distance_to(point).unwrap().unwrap();
            assert!(distance < CLOSE, "{point:?} is {distance} away");
        }
        assert!(near(gap(drawn[0], [0.0; 3]), 0.0));
        assert!(near(gap(*drawn.last().unwrap(), [30.0, 4.0, 1.0]), 0.0));
    }

    #[test]
    fn closed_interpolated_curve_returns_to_its_start() {
        let points = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let curve = Curve::interpolated(points.clone(), true).unwrap();
        let drawn = curve.sample().unwrap();
        assert!(gap(drawn[0], *drawn.last().unwrap()) < CLOSE);
        for point in points {
            let distance = curve.distance_to(point).unwrap().unwrap();
            assert!(distance < CLOSE, "{point:?} is {distance} away");
        }
    }

    #[test]
    fn closed_interpolated_curve_is_smooth_across_its_join() {
        let points = vec![
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let drawn = Curve::interpolated(points, true).unwrap().sample().unwrap();
        let count = drawn.len();
        let direction = |a: Point, b: Point| {
            let length = gap(a, b);
            [
                (b[0] - a[0]) / length,
                (b[1] - a[1]) / length,
                (b[2] - a[2]) / length,
            ]
        };
        let leaving = direction(drawn[0], drawn[1]);
        let arriving = direction(drawn[count - 2], drawn[count - 1]);
        let turn = (0..3).map(|i| leaving[i] * arriving[i]).sum::<f64>();
        // Neighbouring sample segments of a smooth curve point almost the same way.
        assert!(turn > 0.99, "the join turns sharply: cosine {turn}");
    }

    #[test]
    fn closed_curve_through_three_points_closes_on_them() {
        let points = vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
        let curve = Curve::interpolated(points.clone(), true).unwrap();
        let drawn = curve.sample().unwrap();
        let last = *drawn.last().unwrap();
        assert!(
            gap(drawn[0], last) < CLOSE,
            "open by {}: starts {:?}, ends {last:?}",
            gap(drawn[0], last),
            drawn[0]
        );
        for point in points {
            let distance = curve.distance_to(point).unwrap().unwrap();
            assert!(distance < CLOSE, "{point:?} is {distance} away");
        }
    }

    #[test]
    fn translation_keeps_shape_and_moves_position() {
        let by = [1.0, 2.0, 3.0];
        let curve =
            Curve::interpolated(vec![[0.0; 3], [5.0, 5.0, 0.0], [9.0, 0.0, 0.0]], false).unwrap();
        let moved = curve.translated(by);
        assert!((moved.length().unwrap() - curve.length().unwrap()).abs() < 1e-3);
        let (a, b) = (curve.sample().unwrap(), moved.sample().unwrap());
        assert!(near(gap(shifted(a[0], by), b[0]), 0.0));
    }
}
