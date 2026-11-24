//! Rhino's `MultiPipe`. Measured in Rhino 8 (23 and 24 November 2026, curves picked by id): it
//! asks "Select curves to pipe. Press Enter when done", then "Radius <1> ( Cap=No Struts=1 )".
//! Curves meeting end to end join into one pipe, and where three or more meet (or one ends partway
//! along another) a block joins them; each connected group gives one SubD. One line of length 10
//! gives 12 points and 8 faces, a square corner 20, a T 29, and the curves stay. A curve whose
//! parts are all straight pipes through its corners; any other is piped through points on it, each
//! part on its own, placed as `curva_mesh::pipe_corners` says (a quarter circle of radius 10 at
//! radius 1: 8 pieces, 68 points). See `curva_mesh::SubD::pipes`.

use crate::Message;
use crate::commands::arguments::{number, objects};
use crate::commands::curves_of;
use crate::recipe::{Context, Derived, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;
use curva_kernel::{Curve, Point, Spline};
use curva_mesh::{PipeOptions, SubD, pipe_corners};

const CAP: &str = "Cap";
const STRUTS: &str = "Struts";

/// The recipe.
#[derive(Default)]
pub struct MultiPipe;

/// Whether every part of the curve is straight.
fn straight(curve: &Curve) -> bool {
    match curve {
        Curve::Line { .. } | Curve::Polyline { .. } | Curve::Nurbs { degree: 1, .. } => true,
        Curve::Compound { segments } => segments.iter().all(straight),
        _ => false,
    }
}

/// The corners of a straight curve, as segments.
fn straight_segments(curve: &Curve) -> Vec<[Point; 2]> {
    let joined = |points: &[Point], closed: bool| {
        let mut out: Vec<[Point; 2]> = points.windows(2).map(|w| [w[0], w[1]]).collect();
        if closed && points.len() > 2 {
            out.push([points[points.len() - 1], points[0]]);
        }
        out
    };
    match curve {
        Curve::Line { start, end } => vec![[*start, *end]],
        Curve::Polyline { points, closed } => joined(points, *closed),
        Curve::Nurbs { points, .. } => joined(points, false),
        Curve::Compound { segments } => segments.iter().flat_map(straight_segments).collect(),
        _ => Vec::new(),
    }
}

/// The segments MultiPipe pipes through for one curve part, at `radius`.
fn piece_segments(curve: &Curve, radius: f64) -> Vec<[Point; 2]> {
    let through = |point_at: &dyn Fn(f64) -> Point, a: f64, b: f64| -> Vec<Point> {
        pipe_corners(point_at, a, b, radius)
            .into_iter()
            .map(point_at)
            .collect()
    };
    let points = match curve {
        Curve::Point { .. } => return Vec::new(),
        Curve::Line { start, end } => {
            let (a, b) = (*start, *end);
            through(
                &|t| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t),
                0.0,
                1.0,
            )
        }
        Curve::Polyline { .. } => {
            return straight_segments(curve)
                .iter()
                .flat_map(|[a, b]| piece_segments(&Curve::Line { start: *a, end: *b }, radius))
                .collect();
        }
        Curve::Compound { segments } => {
            return segments
                .iter()
                .flat_map(|s| piece_segments(s, radius))
                .collect();
        }
        Curve::Arc {
            centre,
            radius: r,
            x,
            y,
            sweep,
        } => through(&|t| on_circle(*centre, *r, *x, *y, t), 0.0, *sweep),
        Curve::Circle {
            centre,
            radius: r,
            x,
            y,
        } => through(
            &|t| on_circle(*centre, *r, *x, *y, t),
            0.0,
            std::f64::consts::TAU,
        ),
        _ => {
            let Ok(spline) = Spline::of(curve) else {
                return Vec::new();
            };
            let (a, b) = spline.domain();
            through(&|u| spline.point(u), a, b)
        }
    };
    points.windows(2).map(|w| [w[0], w[1]]).collect()
}

fn on_circle(centre: Point, r: f64, x: Point, y: Point, t: f64) -> Point {
    let (s, c) = t.sin_cos();
    std::array::from_fn(|i| centre[i] + r * (c * x[i] + s * y[i]))
}

/// The segments MultiPipe pipes through for a curve, at `radius`.
fn segments_of(curve: &Curve, radius: f64) -> Vec<[Point; 2]> {
    if straight(curve) {
        straight_segments(curve)
    } else {
        piece_segments(curve, radius)
    }
}

/// The segments with ends closer than a billionth of their size made the same point, so curves
/// meeting at ends worked out in different ways still join.
fn snapped(segments: Vec<[Point; 2]>) -> Vec<[Point; 2]> {
    let mut seen: Vec<Point> = Vec::new();
    let mut snap = |p: Point| {
        let size = 1.0 + p.iter().map(|c| c.abs()).fold(0.0, f64::max);
        let near = |q: &&Point| (0..3).all(|i| (p[i] - q[i]).abs() < 1e-9 * size);
        let found = seen.iter().find(near).copied();
        found.unwrap_or_else(|| {
            seen.push(p);
            p
        })
    };
    segments
        .into_iter()
        .map(|[a, b]| [snap(a), snap(b)])
        .collect()
}

impl Recipe for MultiPipe {
    const NAME: &'static str = "multipipe";
    // Rhino 8's options and their starting values, measured.
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(CAP, false),
        OptionDef::whole(STRUTS, 1, 1, 100),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::objects("prompt.multipipe.curves").curves()];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::number("prompt.multipipe.radius")
                    .remembered("Radius", 1.0)
                    .offering(&[CAP, STRUTS]),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(radius) = number(cx, 1) else {
            return Ok(Made::default());
        };
        if radius <= 0.0 {
            return Err(Message::new("error.bad_distance"));
        }
        let curves = curves_of(cx.doc, objects(cx, 0));
        let Some(first) = curves.first().map(|(id, _)| *id) else {
            return Err(Message::new("error.multipipe_none"));
        };
        let mut segments = Vec::new();
        for (_, curve) in &curves {
            segments.extend(segments_of(curve, radius));
        }
        if segments.is_empty() {
            return Err(Message::new("error.multipipe_none"));
        }
        let segments = snapped(segments);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let options = PipeOptions {
            radius,
            cap: cx.toggle(CAP),
            struts: cx.number(STRUTS) as usize,
        };
        let mut made = Made::default();
        let pipes = SubD::pipes(&segments, options)
            .map_err(|e| Message::with("error.geometry", vec![e.0]))?;
        for subd in pipes {
            made.derived.push(Derived {
                shape: Shape::SubD(subd),
                from: first,
                history: None,
            });
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::{Document, Shape};
    use curva_kernel::Curve;

    fn line(doc: &mut Document, a: [f64; 3], b: [f64; 3]) -> u64 {
        doc.add_shape(Shape::Curve(Curve::Line { start: a, end: b }))
    }

    #[test]
    fn two_lines_meeting_pipe_as_one_corner_as_in_rhino() {
        let mut doc = Document::default();
        let a = line(&mut doc, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
        let b = line(&mut doc, [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]);
        doc.select(&[a, b]);
        run(&mut doc, &["multipipe", ""]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 3, "the curves stay");
        let cage = objects[2].shape.subd().unwrap().cage();
        assert_eq!((cage.vertex_count(), cage.face_count()), (20, 16));
    }

    #[test]
    fn a_junction_of_three_joins_as_in_rhino() {
        let mut doc = Document::default();
        let ids = [
            line(&mut doc, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]),
            line(&mut doc, [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]),
            line(&mut doc, [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]),
        ];
        doc.select(&ids);
        run(&mut doc, &["multipipe", ""]);
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 4, "one SubD, and the curves stay");
        let cage = objects[3].shape.subd().unwrap().cage();
        assert_eq!((cage.vertex_count(), cage.face_count()), (29, 24));
    }

    #[test]
    fn the_pipe_shows_while_the_radius_is_asked() {
        let mut doc = Document::default();
        let a = line(&mut doc, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
        doc.select(&[a]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "multipipe");
        assert!(!processor.preview(&doc, None, "2").is_empty());
    }
}

#[cfg(test)]
#[path = "multipipe_data.rs"]
mod rhino;

#[cfg(test)]
mod curved {
    use super::{rhino, segments_of, snapped};
    use curva_kernel::{Curve, Spline};
    use curva_mesh::{PipeOptions, SubD};
    use std::f64::consts::FRAC_PI_2;

    const X: [f64; 3] = [1.0, 0.0, 0.0];
    const Y: [f64; 3] = [0.0, 1.0, 0.0];
    const O: [f64; 3] = [0.0; 3];

    fn arc(centre: [f64; 3], radius: f64, x: [f64; 3], y: [f64; 3], sweep: f64) -> Curve {
        Curve::Arc {
            centre,
            radius,
            x,
            y,
            sweep,
        }
    }

    fn cubic(knots: &[f64], points: &[[f64; 3]]) -> Curve {
        Curve::Nurbs {
            degree: 3,
            knots: knots.to_vec(),
            points: points.to_vec(),
            weights: Vec::new(),
        }
    }

    fn space_curve() -> Curve {
        let points = [
            [0.0, 0.0, 0.0],
            [5.0, 0.0, 3.0],
            [5.0, 5.0, 6.0],
            [0.0, 5.0, 9.0],
            [0.0, 0.0, 12.0],
        ];
        cubic(&[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0], &points)
    }

    fn line(start: [f64; 3], end: [f64; 3]) -> Curve {
        Curve::Line { start, end }
    }

    /// A line across the start of a run, turned 30° off the run's first bend.
    fn tilted_bar() -> Curve {
        let (s, c) = 30f64.to_radians().sin_cos();
        line([0.0, -5.0 * c, -5.0 * s], [0.0, 5.0 * c, 5.0 * s])
    }

    fn bent_run() -> Curve {
        let points = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 20.0, 0.0]];
        Curve::Polyline {
            points,
            closed: false,
        }
    }

    /// Pipes the curves at radius 1 and checks the one SubD's points against Rhino's (sorted, to
    /// 4 places), each matched to the nearest unused one.
    fn pipes_as_in_rhino(curves: &[Curve], struts: usize, rhino: &[f64]) {
        let segments: Vec<_> = curves.iter().flat_map(|c| segments_of(c, 1.0)).collect();
        let options = PipeOptions {
            radius: 1.0,
            cap: false,
            struts,
        };
        let made = SubD::pipes(&snapped(segments), options).unwrap();
        assert_eq!(made.len(), 1);
        let mut ours: Vec<[f64; 3]> = made[0].cage().vertices().to_vec();
        assert_eq!(ours.len() * 3, rhino.len(), "point count");
        for p in rhino.chunks(3) {
            let gap = |q: &[f64; 3]| (0..3).map(|i| (q[i] - p[i]).abs()).fold(0.0, f64::max);
            let nearest = (0..ours.len())
                .min_by(|&a, &b| gap(&ours[a]).total_cmp(&gap(&ours[b])))
                .unwrap();
            assert!(gap(&ours[nearest]) < 2e-4, "Rhino's {p:?} not matched");
            ours.swap_remove(nearest);
        }
    }

    #[test]
    fn arcs_and_circles_pipe_as_in_rhino() {
        pipes_as_in_rhino(&[arc(O, 10.0, X, Y, FRAC_PI_2)], 1, &rhino::QUARTER_ARC);
        pipes_as_in_rhino(&[arc(O, 1.0, X, Y, FRAC_PI_2)], 1, &rhino::SHORT_ARC);
        let circle = Curve::Circle {
            centre: O,
            radius: 5.0,
            x: X,
            y: Y,
        };
        pipes_as_in_rhino(&[circle], 1, &rhino::CIRCLE);
    }

    #[test]
    fn smooth_curves_and_joined_curves_pipe_as_in_rhino() {
        let points = [
            [0.0, 0.0, 0.0],
            [2.635_231_383, 2.635_231_383, 0.0],
            [10.418_861_17, 8.418_861_17, 0.0],
            [19.581_138_83, -3.418_861_17, 0.0],
            [27.364_768_617, 2.364_768_617, 0.0],
            [30.0, 5.0, 0.0],
        ];
        let knots = [0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0];
        pipes_as_in_rhino(&[cubic(&knots, &points)], 2, &rhino::CUBIC_STRUTS_2);
        let joined = Curve::Compound {
            segments: vec![
                line([-10.0, 0.0, 0.0], O),
                arc([0.0, 5.0, 0.0], 5.0, [0.0, -1.0, 0.0], X, FRAC_PI_2),
            ],
        };
        pipes_as_in_rhino(&[joined], 1, &rhino::LINE_AND_ARC);
    }

    #[test]
    fn curves_in_space_turn_their_rings_as_in_rhino() {
        pipes_as_in_rhino(&[space_curve()], 2, &rhino::SPACE_CURVE_STRUTS_2);
        // The same points as a polyline give the same pipe.
        let spline = Spline::of(&space_curve()).unwrap();
        let points = (0..9).map(|k| spline.point(f64::from(k) * 0.25)).collect();
        let polyline = Curve::Polyline {
            points,
            closed: false,
        };
        let curve = SubD::pipes(&segments_of(&space_curve(), 1.0), PipeOptions::default()).unwrap();
        let flat: Vec<f64> = curve[0].cage().vertices().concat();
        pipes_as_in_rhino(&[polyline], 1, &flat);
        // A closed polyline out of one plane goes all round.
        let closed = Curve::Polyline {
            points: vec![O, [10.0, 0.0, 0.0], [10.0, 10.0, 5.0], [0.0, 10.0, 0.0]],
            closed: true,
        };
        pipes_as_in_rhino(&[closed], 1, &rhino::CLOSED_IN_SPACE);
    }

    #[test]
    fn curves_and_bends_meet_junctions_as_in_rhino() {
        let over = arc(
            [0.0, -7.5, 0.0],
            12.5,
            [-0.8, 0.6, 0.0],
            [0.6, 0.8, 0.0],
            2.0 * 0.8f64.atan2(0.6),
        );
        pipes_as_in_rhino(
            &[over, line([10.0, 0.0, 0.0], [10.0, -10.0, 0.0])],
            1,
            &rhino::ARC_T,
        );
        pipes_as_in_rhino(&[tilted_bar(), bent_run()], 2, &rhino::BEND_OFF_JUNCTION);
        let t = [
            tilted_bar(),
            bent_run(),
            line([10.0, 20.0, 0.0], [20.0, 20.0, 0.0]),
            line([10.0, 20.0, 0.0], [10.0, 30.0, 0.0]),
        ];
        pipes_as_in_rhino(&t, 2, &rhino::BEND_BETWEEN_JUNCTIONS);
    }
}
