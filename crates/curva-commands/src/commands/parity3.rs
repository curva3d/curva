//! A third batch of Rhino's commands (spec `docs/specs/rhino-parity.md`): `Parabola`,
//! `Paraboloid`, `Align`, `ChangeDegree`, `AreaCentroid` and
//! `VolumeCentroid`.

use super::{Change, bodies_of, curves_of};
use crate::Message;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Body, Curve, Point, cross, dot, gap, sub, unit};
use std::f64::consts::TAU;

fn point(cx: &Context<'_>, index: usize) -> Option<Point> {
    cx.values.get(index).and_then(Value::point)
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

fn failed(error: &curva_kernel::KernelError) -> Message {
    Message::with("error.geometry", vec![error.to_string()])
}

/// The exact piece of the parabola with its vertex at `vertex`, opening along `axis` towards
/// its focus `focal` away, from `from` to `to` along `side`: a quadratic whose middle control
/// point is where the end tangents meet.
fn parabola_piece(
    vertex: Point,
    axis: Point,
    side: Point,
    focal: f64,
    from: f64,
    to: f64,
) -> Result<Curve, curva_kernel::KernelError> {
    let at = |s: f64, height: f64| plus(plus(vertex, side, s), axis, height);
    let points = vec![
        at(from, from * from / (4.0 * focal)),
        at((from + to) / 2.0, from * to / (4.0 * focal)),
        at(to, to * to / (4.0 * focal)),
    ];
    Curve::nurbs(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], points, Vec::new())
}

/// `Parabola`: its vertex, its focus, then an end point, which sets how far it reaches.
#[derive(Default)]
pub struct Parabola;

impl Recipe for Parabola {
    const NAME: &'static str = "parabola";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.parabola.vertex")];
        if let Some(v) = point(cx, 0) {
            steps.push(StepDef::point("prompt.parabola.focus").from(v));
            if let Some(f) = point(cx, 1) {
                steps.push(StepDef::point("prompt.parabola.end").from(f));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(v), Some(f)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(end) = point(cx, 2) else {
            return Ok(Made {
                guides: Curve::line(v, f).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let [x, y] = cx.plane();
        let axis = unit(sub(f, v)).ok_or_else(|| Message::new("error.reference_point"))?;
        let side = cross(cross(x, y), axis);
        let half = dot(sub(end, v), side).abs();
        if half <= 1e-9 {
            return Err(Message::new("error.reference_point"));
        }
        let curve =
            parabola_piece(v, axis, side, gap(v, f), -half, half).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Curve(curve)],
            message: Some(Message::new("done.parabola")),
            ..Made::default()
        })
    }
}

/// `Paraboloid`: its vertex, its focus, then how deep it goes along its axis; a solid closed by a
/// flat top.
#[derive(Default)]
pub struct Paraboloid;

impl Recipe for Paraboloid {
    const NAME: &'static str = "paraboloid";

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![StepDef::point("prompt.parabola.vertex")];
        if let Some(v) = point(cx, 0) {
            steps.push(StepDef::point("prompt.parabola.focus").from(v));
            if let Some(f) = point(cx, 1) {
                steps.push(StepDef::along("prompt.paraboloid.depth", v, sub(f, v)));
            }
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(v), Some(f)) = (point(cx, 0), point(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(depth) = cx
            .values
            .get(2)
            .and_then(Value::number)
            .filter(|d| *d > 1e-9)
        else {
            return Ok(Made {
                guides: Curve::line(v, f).ok().into_iter().collect(),
                ..Made::default()
            });
        };
        let [x, _] = cx.plane();
        let axis = unit(sub(f, v)).ok_or_else(|| Message::new("error.reference_point"))?;
        let side = unit(sub(x, plus([0.0; 3], axis, dot(x, axis))))
            .ok_or_else(|| Message::new("error.reference_point"))?;
        let focal = gap(v, f);
        let half = (4.0 * focal * depth).sqrt();
        // Half the parabola, exact, then across the top to the axis, spun a whole turn: both
        // ends on the axis, so it closes into a solid.
        let rim = plus(plus(v, side, half), axis, depth);
        let outline = Curve::compound(vec![
            parabola_piece(v, axis, side, focal, 0.0, half).map_err(|e| failed(&e))?,
            Curve::line(rim, plus(v, axis, depth)).map_err(|e| failed(&e))?,
        ])
        .map_err(|e| failed(&e))?;
        let body = Body::revolved(&[outline], v, axis, TAU, false).map_err(|e| failed(&e))?;
        Ok(Made {
            add: vec![Shape::Body(body)],
            message: Some(Message::new("done.paraboloid")),
            ..Made::default()
        })
    }
}

/// The lowest and highest corners around an object.
fn bounds_of(shape: &Shape) -> Option<(Point, Point)> {
    match shape {
        Shape::Body(body) => body.bounds().ok(),
        Shape::Curve(curve) => {
            let points = curve.sample().ok()?;
            let first = *points.first()?;
            Some(points.iter().fold((first, first), |(lo, hi), p| {
                (
                    std::array::from_fn(|i| lo[i].min(p[i])),
                    std::array::from_fn(|i| hi[i].max(p[i])),
                )
            }))
        }
    }
}

/// Which value of an extent along an axis to line up: its start, its end, or its middle.
type Extent = fn((f64, f64)) -> f64;

const ALIGNS: [&str; 6] = [
    "Bottom",
    "Top",
    "Left",
    "Right",
    "HorizontalCenter",
    "VerticalCenter",
];

/// `Align`: lines objects up on the plane being drawn on: their bottoms, tops, left or right
/// sides, or centres, on those of all of them.
#[derive(Default)]
pub struct Align;

impl Recipe for Align {
    const NAME: &'static str = "align";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::finish("Bottom"),
        OptionDef::finish("Top"),
        OptionDef::finish("Left"),
        OptionDef::finish("Right"),
        OptionDef::finish("HorizontalCenter"),
        OptionDef::finish("VerticalCenter"),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.align.objects")
                .at_least(2)
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(StepDef::point("prompt.align.how").offering(&ALIGNS));
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(how)) = (cx.values.first(), cx.mode) else {
            return Ok(Made::default());
        };
        let [x, y] = cx.plane();
        let objects: Vec<(u64, (Point, Point))> = cx
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .filter_map(|o| Some((o.id, bounds_of(&o.shape)?)))
            .collect();
        // Each object's extent along the plane's X or Y, and the target on all of them.
        let along = |axis: Point, (lo, hi): (Point, Point)| {
            let (a, b) = (dot(lo, axis), dot(hi, axis));
            (a.min(b), a.max(b))
        };
        let (axis, pick): (Point, Extent) = match how {
            "Bottom" => (y, |(lo, _)| lo),
            "Top" => (y, |(_, hi)| hi),
            "Left" => (x, |(lo, _)| lo),
            "Right" => (x, |(_, hi)| hi),
            "HorizontalCenter" => (x, |(lo, hi)| (lo + hi) / 2.0),
            _ => (y, |(lo, hi)| (lo + hi) / 2.0),
        };
        let values: Vec<f64> = objects.iter().map(|(_, b)| pick(along(axis, *b))).collect();
        let target = match how {
            "Top" | "Right" => values.iter().copied().fold(f64::MIN, f64::max),
            "Bottom" | "Left" => values.iter().copied().fold(f64::MAX, f64::min),
            _ => values.iter().sum::<f64>() / values.len().max(1) as f64,
        };
        let mut made = Made::default();
        for ((id, _), value) in objects.iter().zip(values) {
            let by = axis.map(|v| v * (target - value));
            let part = super::changed(cx, &[*id], &[Change::Move(by)], false, |_, _| None);
            made.replace.extend(part.replace);
        }
        made.message = Some(Message::with(
            "done.align",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

/// `ChangeDegree`: gives curves a new degree. Raising it keeps the shape exactly (Rhino's
/// `Deformable=No`, adding control points); lowering it, or `Deformable=Yes`, keeps the control
/// points and lets the shape change. Closed curves stay closed.
#[derive(Default)]
pub struct ChangeDegree;

impl Recipe for ChangeDegree {
    const NAME: &'static str = "changedegree";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::whole("Degree", 3, 1, 11),
        OptionDef::toggle("Deformable", false),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = vec![
            StepDef::objects("prompt.changedegree.objects")
                .curves()
                .offering(&[]),
        ];
        if !cx.values.is_empty() {
            steps.push(
                StepDef::number("prompt.changedegree.degree")
                    .remembered("Degree", 3.0)
                    .offering(&["Deformable"]),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Objects(ids)), Some(degree)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::number))
        else {
            return Ok(Made::default());
        };
        if !(1.0..=11.0).contains(&degree) || degree.fract() != 0.0 {
            return Err(Message::with(
                "error.option_whole",
                vec!["Degree".to_owned(), "1".to_owned(), "11".to_owned()],
            ));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let degree = degree as usize;
        let mut made = Made::default();
        for (id, curve) in curves_of(cx.doc, ids) {
            let spline = curva_kernel::Spline::of(&curve).map_err(|e| failed(&e))?;
            if spline.degree == degree {
                continue;
            }
            let changed = if degree > spline.degree && !cx.toggle("Deformable") {
                spline
                    .elevated(degree - spline.degree)
                    .to_curve()
                    .map_err(|e| failed(&e))?
            } else {
                let mut points = spline.control_points();
                let closed = curve.is_closed().unwrap_or(false);
                if closed
                    && points.len() > 2
                    && curva_kernel::gap(points[0], points[points.len() - 1]) < 1e-9
                {
                    points.pop();
                }
                Curve::control_point(points, degree, closed).map_err(|e| failed(&e))?
            };
            made.replace.push((id, Shape::Curve(changed)));
        }
        made.message = Some(Message::with(
            "done.changedegree",
            vec![made.replace.len().to_string()],
        ));
        Ok(made)
    }
}

/// The centre of a solid's volume, from its mesh, by summing the tetrahedra its triangles make
/// with the origin.
fn volume_centroid(body: &Body) -> Option<Point> {
    let mesh = body.mesh().ok()?;
    let (mut volume, mut moment) = (0.0, [0.0; 3]);
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.map(|i| mesh.positions[i as usize]);
        let v = dot(a, cross(b, c)) / 6.0;
        volume += v;
        for i in 0..3 {
            moment[i] += v * (a[i] + b[i] + c[i]) / 4.0;
        }
    }
    (volume.abs() > 1e-12).then(|| moment.map(|m| m / volume))
}

/// The centre of a surface's area, from its mesh.
fn area_centroid(body: &Body) -> Option<Point> {
    let mesh = body.mesh().ok()?;
    let (mut area, mut moment) = (0.0, [0.0; 3]);
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.map(|i| mesh.positions[i as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let piece = dot(n, n).sqrt() / 2.0;
        area += piece;
        for i in 0..3 {
            moment[i] += piece * (a[i] + b[i] + c[i]) / 3.0;
        }
    }
    (area > 1e-12).then(|| moment.map(|m| m / area))
}

/// `AreaCentroid` (or, with `VOLUME`, `VolumeCentroid`): places a point at the centre of each
/// surface's area, or solid's volume, and reports where.
#[derive(Default)]
pub struct Centroid<const VOLUME: bool>;

impl<const VOLUME: bool> Recipe for Centroid<VOLUME> {
    const NAME: &'static str = if VOLUME {
        "volumecentroid"
    } else {
        "areacentroid"
    };

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::objects("prompt.centroid.objects").bodies()]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Objects(ids)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        let mut said = Vec::new();
        for (_, body) in bodies_of(cx.doc, ids) {
            // Exact from the engine: a solid's volume, or a surface's area. A solid's area
            // centre comes from its mesh, which the engine does not give.
            let found = match (VOLUME, body.is_solid()) {
                (true, true) | (false, false) => body.centroid().ok().or_else(|| {
                    if VOLUME {
                        volume_centroid(&body)
                    } else {
                        area_centroid(&body)
                    }
                }),
                (true, false) => None,
                (false, true) => area_centroid(&body),
            };
            if let Some(at) = found {
                said.push(format!("{:.4}, {:.4}, {:.4}", at[0], at[1], at[2]));
                made.add.push(Shape::Curve(Curve::Point { at }));
            }
        }
        if said.is_empty() {
            return Err(Message::new("error.centroid"));
        }
        made.message = Some(Message::with("done.centroid", vec![said.join("; ")]));
        Ok(made)
    }
}

/// `AreaCentroid`.
pub type AreaCentroid = Centroid<false>;
/// `VolumeCentroid`.
pub type VolumeCentroid = Centroid<true>;

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::Document;
    use curva_kernel::{Body, Curve};

    #[test]
    fn parabola_paraboloid_and_centroids() {
        let mut doc = Document::default();
        run(&mut doc, &["parabola", "0,0", "0,1", "4,0"]);
        let points = doc.state().objects[0].curve().unwrap().sample().unwrap();
        assert!(
            points
                .iter()
                .all(|p| (p[1] - p[0] * p[0] / 4.0).abs() < 0.05),
            "{points:?}"
        );
        let mut doc = Document::default();
        run(&mut doc, &["paraboloid", "0,0,0", "0,0,1", "4"]);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        // The volume of a paraboloid is half the cylinder around it: π r² h / 2, with r² = 4fh.
        let expected = std::f64::consts::PI * 16.0 * 4.0 / 2.0;
        assert!(
            (volume - expected).abs() < 1e-4 * expected,
            "{volume} against {expected}"
        );
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([0.0; 3], [2.0, 4.0, 6.0]).unwrap());
        doc.select(&[id]);
        run(&mut doc, &["volumecentroid"]);
        let Some(Curve::Point { at }) = doc.state().objects.last().unwrap().curve() else {
            panic!("no point");
        };
        assert!(curva_kernel::gap(*at, [1.0, 2.0, 3.0]) < 1e-6, "{at:?}");
    }

    #[test]
    fn align_lines_up_bottoms() {
        let mut doc = Document::default();
        let a = doc.add(Curve::line([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]).unwrap());
        let b = doc.add(Curve::line([3.0, 5.0, 0.0], [4.0, 6.0, 0.0]).unwrap());
        doc.select(&[a, b]);
        run(&mut doc, &["align", "bottom"]);
        let lowest = doc.state().objects[1].curve().unwrap().sample().unwrap()[0][1];
        assert!(lowest.abs() < 1e-9, "{lowest}");
    }

    #[test]
    fn control_points_and_degree() {
        let mut doc = Document::default();
        let id = doc.add(
            Curve::control_point(
                vec![
                    [0.0; 3],
                    [5.0, 5.0, 0.0],
                    [10.0, 0.0, 0.0],
                    [15.0, 5.0, 0.0],
                ],
                3,
                false,
            )
            .unwrap(),
        );
        doc.select(&[id]);
        run(&mut doc, &["changedegree", "2"]);
        assert!(matches!(
            doc.state().objects[0].curve(),
            Some(Curve::Nurbs { degree: 2, .. })
        ));
    }

    #[test]
    fn raising_the_degree_keeps_the_shape_and_closed_curves_stay_closed() {
        let wavy = Curve::control_point(
            vec![
                [0.0; 3],
                [5.0, 5.0, 0.0],
                [10.0, 0.0, 0.0],
                [15.0, 5.0, 0.0],
                [20.0, 0.0, 0.0],
            ],
            3,
            false,
        )
        .unwrap();
        let mut doc = Document::default();
        let id = doc.add(wavy.clone());
        doc.select(&[id]);
        run(&mut doc, &["changedegree", "5"]);
        let raised = doc.state().objects[0].curve().unwrap().clone();
        assert!(matches!(raised, Curve::Nurbs { degree: 5, .. }));
        let (before, after) = (
            curva_kernel::Spline::of(&wavy).unwrap(),
            curva_kernel::Spline::of(&raised).unwrap(),
        );
        for i in 0..=20 {
            let u = f64::from(i) / 20.0;
            assert!(curva_kernel::gap(before.point(u), after.point(u)) < 1e-9);
        }
        // Closed curves stay closed, raised or lowered.
        for degree in ["4", "2"] {
            let mut doc = Document::default();
            let id = doc.add(Curve::circle([0.0; 3], 5.0).unwrap());
            doc.select(&[id]);
            run(&mut doc, &["changedegree", degree]);
            let changed = doc.state().objects[0].curve().unwrap();
            assert!(changed.is_closed().unwrap(), "degree {degree}");
        }
    }
}
