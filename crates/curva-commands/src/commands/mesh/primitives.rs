//! Rhino's mesh primitives: `MeshBox`, `MeshPlane`, `MeshSphere`, `MeshCylinder`, `MeshCone` and
//! `MeshTorus` (spec `docs/specs/mesh-and-subd.md` §1). They are picked like `Box`, `Sphere`,
//! `Cylinder`, `Cone` and `Torus`, with the same prompts, and made by `curva-mesh`'s primitives
//! with Rhino's face counts. Rhino 8's defaults were measured by making each one with Enter at
//! every option (19 November 2026): 10 faces each way, solid, with triangle caps.

use crate::Message;
use crate::commands::arguments::{number, point};
use crate::commands::nurbs::solids::box_solid::{base, height_at};
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, Point, cross};
use curva_mesh::{Frame, Mesh, SmoothError, SphereStyle, SubD};

const SMALLEST: f64 = 1e-9;
const MOST_FACES: u32 = 1000;
const CENTER: &str = "Center";
const BOTH_SIDES: &str = "BothSides";
const DIAMETER: &str = "Diameter";
const SOLID: &str = "Solid";
const CAP_STYLE: &str = "CapFaceStyle";
const STYLE: &str = "Style";
const VERTICAL: &str = "VerticalFaces";
const AROUND: &str = "AroundFaces";
const SUBDIVISIONS: &str = "Subdivisions";
const TWO_POINT: &str = "2Point";

const fn faces(name: &'static str) -> OptionDef {
    OptionDef::whole(name, 10, 1, MOST_FACES)
}

/// A face count: 10 for a mesh, as Rhino 8's mesh primitives, or `subd` for a SubD, as its SubD
/// primitives (measured with Enter at every option).
const fn faces_for(name: &'static str, is_subd: bool, subd: u32) -> OptionDef {
    OptionDef::whole(name, if is_subd { subd } else { 10 }, 1, MOST_FACES)
}

fn count(cx: &Context<'_>, name: &str) -> u32 {
    cx.number(name) as u32
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

/// The plane being drawn on as a frame at `origin`, its third direction the plane's normal.
fn frame_at(cx: &Context<'_>, origin: Point) -> Frame {
    let [x, y] = cx.plane();
    Frame {
        origin,
        x,
        y,
        z: cross(x, y),
    }
}

/// A mesh, or a SubD cage when `subd`.
fn either(
    subd: bool,
    mesh: impl FnOnce() -> Mesh,
    cage: impl FnOnce() -> Result<SubD, SmoothError>,
) -> Result<Shape, Message> {
    if subd {
        cage()
            .map(Shape::SubD)
            .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))
    } else {
        Ok(Shape::Mesh(mesh()))
    }
}

fn added(shape: Result<Shape, Message>, done: &'static str) -> Result<Made, Message> {
    Ok(Made {
        add: vec![shape?],
        message: Some(Message::new(done)),
        ..Made::default()
    })
}

/// A rectangle's outline while its corners are being picked.
fn ring_guide(corner: Point, x: Point, y: Point, [wide, deep]: [f64; 2]) -> Made {
    let ring = [
        corner,
        plus(corner, x, wide),
        plus(plus(corner, x, wide), y, deep),
        plus(corner, y, deep),
    ];
    Made {
        guides: (0..4)
            .filter_map(|i| Curve::line(ring[i], ring[(i + 1) % 4]).ok())
            .collect(),
        ..Made::default()
    }
}

/// A circle's outline while its centre and radius are being picked.
fn circle_guide(cx: &Context<'_>, centre: Point, r: f64) -> Made {
    let [x, y] = cx.plane();
    Made {
        guides: Curve::circle_on(centre, r, x, y).ok().into_iter().collect(),
        ..Made::default()
    }
}

/// A rectangle as `Rectangle` and `Box` take it: two corners on the plane, its centre and a corner
/// with `Center`, or a typed length and then the width (Enter: the length). Its first corner and
/// its sides along the plane's X and Y, if it is not flat.
fn rectangle(cx: &Context<'_>) -> Option<(Point, [f64; 2])> {
    base(cx)
}

/// The corner steps of a rectangle, offering `first` at the first step and `second` after it:
/// the first corner, then the other corner or a typed length, then after a length the width, as
/// Rhino 8's `MeshPlane` and `MeshBox` ask (measured 21 November 2026).
fn rectangle_steps(
    cx: &Context<'_>,
    first: &'static [&'static str],
    second: &'static [&'static str],
) -> Vec<StepDef> {
    let mut steps = vec![StepDef::point("prompt.box.first").offering(first)];
    if let Some(start) = point(cx, 0) {
        steps.push(
            StepDef::point_or_number("prompt.box.other")
                .from(start)
                .offering(second)
                .corner(),
        );
        if matches!(cx.values.get(1), Some(Value::Number(_))) {
            let [_, y] = cx.plane();
            steps.push(
                StepDef::along("prompt.box.width", start, y)
                    .optional()
                    .offering(second),
            );
        }
    }
    steps
}

/// Where a box's height starts: the corner just clicked, where the mouse is, or the first corner
/// after a typed length and width, as `Box`. `None` until the base is complete.
fn height_from(cx: &Context<'_>) -> Option<Point> {
    match cx.values.get(1)? {
        Value::Number(_) => cx.values.get(2).and(point(cx, 0)),
        other => other.point(),
    }
}

/// `MeshBox`: a box's base as `Box` takes it, then its height (Enter: the base's side along Y).
#[derive(Default)]
pub struct Boxed<const SUBD: bool>;

/// Rhino's `MeshBox`.
pub type MeshBox = Boxed<false>;
/// Rhino's `SubDBox`: the same picking, a SubD cage with Rhino's SubD face counts.
pub type SubDBox = Boxed<true>;

impl<const SUBD: bool> Recipe for Boxed<SUBD> {
    const NAME: &'static str = if SUBD { "subdbox" } else { "meshbox" };
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(CENTER),
        OptionDef::toggle(BOTH_SIDES, false),
        faces_for("XCount", SUBD, 2),
        faces_for("YCount", SUBD, 2),
        faces_for("ZCount", SUBD, 2),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        const COUNTS: &[&str] = &["XCount", "YCount", "ZCount"];
        let mut steps = rectangle_steps(cx, &[CENTER, "XCount", "YCount", "ZCount"], COUNTS);
        if let Some(from) = height_from(cx) {
            let [x, y] = cx.plane();
            steps.push(
                StepDef::along("prompt.box.height", from, cross(x, y))
                    .optional()
                    .offering(&[BOTH_SIDES, "XCount", "YCount", "ZCount"]),
            );
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if cx.values.len() >= 2 && rectangle(cx).is_none() {
            return Err(Message::new("error.flat_rectangle"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((corner, [wide, deep])) = rectangle(cx) else {
            return Ok(Made::default());
        };
        let high = match cx.values.get(height_at(cx)) {
            Some(Value::Number(high)) => *high,
            // Enter uses the base's side along Y, as Rhino 8's `Box` and `MeshBox`.
            Some(Value::Empty) => deep.abs(),
            _ => 0.0,
        };
        let frame = frame_at(cx, corner);
        if high.abs() < SMALLEST {
            return Ok(ring_guide(corner, frame.x, frame.y, [wide, deep]));
        }
        let (start, high) = if cx.toggle(BOTH_SIDES) {
            (plus(corner, frame.z, -high), 2.0 * high)
        } else {
            (corner, high)
        };
        let counts = ["XCount", "YCount", "ZCount"].map(|name| count(cx, name));
        added(
            {
                let frame = frame_at(cx, start);
                let size = [wide, deep, high];
                either(
                    SUBD,
                    || Mesh::mesh_box(&frame, size, counts),
                    || Ok(SubD::subd_box(&frame, size, counts)),
                )
            },
            if SUBD { "done.subdbox" } else { "done.meshbox" },
        )
    }
}

/// `MeshPlane`: a flat rectangle of faces from two corners, or its centre and a corner.
#[derive(Default)]
pub struct Planed<const SUBD: bool>;

/// Rhino's `MeshPlane`.
pub type MeshPlane = Planed<false>;
/// Rhino's `SubDPlane`: the same picking, a SubD cage with Rhino's SubD face counts.
pub type SubDPlane = Planed<true>;

impl<const SUBD: bool> Recipe for Planed<SUBD> {
    const NAME: &'static str = if SUBD { "subdplane" } else { "meshplane" };
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(CENTER),
        faces_for("XCount", SUBD, 2),
        faces_for("YCount", SUBD, 2),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        rectangle_steps(cx, &[CENTER, "XCount", "YCount"], &["XCount", "YCount"])
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if cx.values.len() >= 2 && rectangle(cx).is_none() {
            return Err(Message::new("error.flat_rectangle"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((corner, size)) = rectangle(cx) else {
            return Ok(Made::default());
        };
        let counts = [count(cx, "XCount"), count(cx, "YCount")];
        added(
            {
                let frame = frame_at(cx, corner);
                either(
                    SUBD,
                    || Mesh::mesh_plane(&frame, size, counts),
                    || Ok(SubD::subd_plane(&frame, size, counts)),
                )
            },
            if SUBD {
                "done.subdplane"
            } else {
                "done.meshplane"
            },
        )
    }
}

/// A radius typed or picked, or a diameter with `Diameter` on.
fn radius(cx: &Context<'_>, index: usize) -> Option<f64> {
    let size = number(cx, index)?;
    Some(if cx.toggle(DIAMETER) {
        size / 2.0
    } else {
        size
    })
}

/// The centre and radius steps that `Sphere`, `Cylinder`, `Cone` and `Torus` begin with.
fn circle_steps(
    cx: &Context<'_>,
    shape: &'static str,
    offered: &'static [&'static str],
) -> Vec<StepDef> {
    let (centre, size) = match shape {
        "sphere" => ("prompt.sphere.centre", "prompt.sphere.radius"),
        "cylinder" => ("prompt.cylinder.centre", "prompt.cylinder.radius"),
        "cone" => ("prompt.cone.centre", "prompt.cone.radius"),
        _ => ("prompt.torus.centre", "prompt.torus.radius"),
    };
    let mut steps = vec![StepDef::point(centre).offering(offered)];
    steps.extend(point(cx, 0).map(|c| StepDef::number(size).from(c).offering(offered)));
    steps
}

/// The options offered while the ends of a diameter are picked.
const SPHERE_FACES: &[&str] = &[STYLE, VERTICAL, AROUND, SUBDIVISIONS];

/// A sphere's centre and radius: from the centre and radius, or with `2Point` from the ends of a
/// diameter, as `Sphere`.
fn sphere_size(cx: &Context<'_>) -> Option<(Point, f64)> {
    if cx.mode == Some(TWO_POINT) {
        let (a, b) = (point(cx, 0)?, point(cx, 1)?);
        let half = (0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f64>().sqrt() / 2.0;
        return Some((std::array::from_fn(|i| (a[i] + b[i]) / 2.0), half));
    }
    Some((point(cx, 0)?, radius(cx, 1)?))
}

/// `MeshSphere`: its centre, then its radius, in Rhino's UV, Quads or Triangles style.
#[derive(Default)]
pub struct Sphered<const SUBD: bool>;

/// Rhino's `MeshSphere`.
pub type MeshSphere = Sphered<false>;
/// Rhino's `SubDSphere`: the same picking, a SubD cage with Rhino's SubD face counts.
pub type SubDSphere = Sphered<true>;

impl<const SUBD: bool> Recipe for Sphered<SUBD> {
    const NAME: &'static str = if SUBD { "subdsphere" } else { "meshsphere" };
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(TWO_POINT),
        OptionDef::toggle(DIAMETER, false),
        // SubDSphere starts with Quads at 1 subdivision (24 faces), measured.
        OptionDef::name(STYLE, if SUBD { "Quads" } else { "UV" }),
        faces(VERTICAL),
        faces(AROUND),
        // Rhino's range; MeshSphere's default could not be measured, so 2 (96 quads, 320
        // triangles).
        OptionDef::whole(SUBDIVISIONS, if SUBD { 1 } else { 2 }, 0, 6),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == STYLE).then(|| ["UV", "Quads", "Triangles"].map(str::to_owned).to_vec())
    }

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        if cx.mode == Some(TWO_POINT) {
            let mut steps = vec![StepDef::point("prompt.sphere.start").offering(SPHERE_FACES)];
            steps.extend(point(cx, 0).map(|a| {
                StepDef::point("prompt.sphere.end")
                    .from(a)
                    .offering(SPHERE_FACES)
            }));
            return steps;
        }
        circle_steps(
            cx,
            "sphere",
            &[TWO_POINT, DIAMETER, STYLE, VERTICAL, AROUND, SUBDIVISIONS],
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some((c, r)) = sphere_size(cx) else {
            return Ok(Made::default());
        };
        if r <= SMALLEST {
            return Err(Message::new("error.bad_radius"));
        }
        let subdivisions = count(cx, SUBDIVISIONS);
        let style = match cx.name(STYLE).as_str() {
            "Quads" => SphereStyle::Quads { subdivisions },
            "Triangles" => SphereStyle::Triangles {
                subdivisions: subdivisions.min(5),
            },
            _ => SphereStyle::Uv {
                vertical: count(cx, VERTICAL),
                around: count(cx, AROUND),
            },
        };
        added(
            {
                let frame = frame_at(cx, c);
                either(
                    SUBD,
                    || Mesh::mesh_sphere(&frame, r, style),
                    || SubD::subd_sphere(&frame, r, style),
                )
            },
            if SUBD {
                "done.subdsphere"
            } else {
                "done.meshsphere"
            },
        )
    }
}

/// The height step of a cylinder or cone, along the plane's normal from the base's centre.
fn height_step(
    cx: &Context<'_>,
    prompt: &'static str,
    offered: &'static [&'static str],
) -> Option<StepDef> {
    let c = point(cx, 0)?;
    (cx.values.len() >= 2).then(|| {
        let [x, y] = cx.plane();
        StepDef::along(prompt, c, cross(x, y)).offering(offered)
    })
}

/// Whether the caps are quads: `CapFaceStyle` Quad, which Rhino honours for an even count around.
fn quad_caps(cx: &Context<'_>) -> bool {
    cx.name(CAP_STYLE) == "Quad" && count(cx, AROUND).is_multiple_of(2)
}

fn cap_choices(option: &str) -> Option<Vec<String>> {
    (option == CAP_STYLE).then(|| ["Tri", "Quad"].map(str::to_owned).to_vec())
}

/// `MeshCylinder`: the centre of its base, its radius, then its height.
#[derive(Default)]
pub struct Cylindered<const SUBD: bool>;

/// Rhino's `MeshCylinder`.
pub type MeshCylinder = Cylindered<false>;
/// Rhino's `SubDCylinder`: the same picking, a SubD cage with Rhino's SubD face counts.
pub type SubDCylinder = Cylindered<true>;

impl<const SUBD: bool> Recipe for Cylindered<SUBD> {
    const NAME: &'static str = if SUBD { "subdcylinder" } else { "meshcylinder" };
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(DIAMETER, false),
        OptionDef::toggle(SOLID, true),
        OptionDef::toggle(BOTH_SIDES, false),
        faces_for(VERTICAL, SUBD, 4),
        faces_for(AROUND, SUBD, 8),
        OptionDef::name(CAP_STYLE, "Tri"),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        cap_choices(option)
    }

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = circle_steps(cx, "cylinder", &[DIAMETER, VERTICAL, AROUND]);
        steps.extend(height_step(
            cx,
            "prompt.cylinder.height",
            &[SOLID, BOTH_SIDES, VERTICAL, AROUND, CAP_STYLE],
        ));
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(r)) = (point(cx, 0), radius(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(high) = number(cx, 2).filter(|h| h.abs() > SMALLEST) else {
            return Ok(circle_guide(cx, c, r));
        };
        let up = frame_at(cx, c).z;
        let (start, high) = if cx.toggle(BOTH_SIDES) {
            (plus(c, up, -high), 2.0 * high)
        } else {
            (c, high)
        };
        added(
            {
                let frame = frame_at(cx, start);
                let faces = (count(cx, VERTICAL), count(cx, AROUND));
                let (solid, quads) = (cx.toggle(SOLID), quad_caps(cx));
                either(
                    SUBD,
                    || Mesh::mesh_cylinder(&frame, r, high, faces, solid, quads),
                    || Ok(SubD::subd_cylinder(&frame, r, high, faces, solid, quads)),
                )
            },
            if SUBD {
                "done.subdcylinder"
            } else {
                "done.meshcylinder"
            },
        )
    }
}

/// `MeshCone`: the centre of its base, its radius, then the height of its point.
#[derive(Default)]
pub struct Coned<const SUBD: bool>;

/// Rhino's `MeshCone`.
pub type MeshCone = Coned<false>;
/// Rhino's `SubDCone`: the same picking, a SubD cage with Rhino's SubD face counts.
pub type SubDCone = Coned<true>;

impl<const SUBD: bool> Recipe for Coned<SUBD> {
    const NAME: &'static str = if SUBD { "subdcone" } else { "meshcone" };
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(DIAMETER, false),
        OptionDef::toggle(SOLID, true),
        faces_for(VERTICAL, SUBD, 4),
        faces_for(AROUND, SUBD, 8),
        OptionDef::name(CAP_STYLE, "Tri"),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        cap_choices(option)
    }

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = circle_steps(cx, "cone", &[DIAMETER, VERTICAL, AROUND]);
        steps.extend(height_step(
            cx,
            "prompt.cone.apex",
            &[SOLID, VERTICAL, AROUND, CAP_STYLE],
        ));
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(r)) = (point(cx, 0), radius(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(high) = number(cx, 2).filter(|h| h.abs() > SMALLEST) else {
            return Ok(circle_guide(cx, c, r));
        };
        added(
            {
                let frame = frame_at(cx, c);
                let faces = (count(cx, VERTICAL), count(cx, AROUND));
                let (solid, quads) = (cx.toggle(SOLID), quad_caps(cx));
                either(
                    SUBD,
                    || Mesh::mesh_cone(&frame, r, high, faces, solid, quads),
                    || Ok(SubD::subd_cone(&frame, r, high, faces, solid, quads)),
                )
            },
            if SUBD {
                "done.subdcone"
            } else {
                "done.meshcone"
            },
        )
    }
}

/// `MeshTorus`: its centre, the radius to the middle of its tube, then the tube's radius.
#[derive(Default)]
pub struct Torused<const SUBD: bool>;

/// Rhino's `MeshTorus`.
pub type MeshTorus = Torused<false>;
/// Rhino's `SubDTorus`: the same picking, a SubD cage with Rhino's SubD face counts.
pub type SubDTorus = Torused<true>;

impl<const SUBD: bool> Recipe for Torused<SUBD> {
    const NAME: &'static str = if SUBD { "subdtorus" } else { "meshtorus" };
    const OPTIONS: &'static [OptionDef] =
        &[faces_for(VERTICAL, SUBD, 6), faces_for(AROUND, SUBD, 10)];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let mut steps = circle_steps(cx, "torus", &[VERTICAL, AROUND]);
        if let (Some(c), Some(r)) = (point(cx, 0), number(cx, 1)) {
            let [x, _] = cx.plane();
            steps.push(
                StepDef::number("prompt.torus.second")
                    .from(plus(c, x, r))
                    .offering(&[VERTICAL, AROUND]),
            );
        }
        steps
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(r)) = (point(cx, 0), number(cx, 1)) else {
            return Ok(Made::default());
        };
        let Some(tube) = number(cx, 2) else {
            return Ok(circle_guide(cx, c, r));
        };
        if tube <= SMALLEST || tube >= r {
            return Err(Message::new("error.torus"));
        }
        added(
            {
                let frame = frame_at(cx, c);
                let faces = (count(cx, VERTICAL), count(cx, AROUND));
                either(
                    SUBD,
                    || Mesh::mesh_torus(&frame, r, tube, faces),
                    || SubD::subd_torus(&frame, r, tube, faces),
                )
            },
            if SUBD {
                "done.subdtorus"
            } else {
                "done.meshtorus"
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_mesh::Mesh;

    fn made(steps: &[&str]) -> Mesh {
        let mut doc = Document::default();
        run(&mut doc, steps);
        doc.state()
            .objects
            .last()
            .unwrap()
            .shape
            .mesh()
            .unwrap()
            .clone()
    }

    /// Faces, and of them quads: what Rhino 8 reports for its own.
    fn counts(mesh: &Mesh) -> (usize, usize) {
        let quads = mesh.faces().filter(|f| f.len() == 4).count();
        (mesh.face_count(), quads)
    }

    #[test]
    fn each_default_matches_rhinos_counts() {
        // Rhino 8, Enter at every option (19 November 2026).
        let cases: [(&[&str], _); 6] = [
            (&["meshbox", "0,0", "10,8", "6"], (600, 600)),
            (&["meshplane", "0,0", "10,8"], (100, 100)),
            (&["meshsphere", "0,0", "5"], (100, 80)),
            (&["meshcylinder", "0,0", "5", "10"], (120, 100)),
            (&["meshcone", "0,0", "5", "10"], (110, 90)),
            (&["meshtorus", "0,0", "10", "2"], (100, 100)),
        ];
        for (steps, want) in cases {
            assert_eq!(counts(&made(steps)), want, "{steps:?}");
        }
    }

    #[test]
    fn each_subd_default_matches_rhinos_cage() {
        // Rhino 8, Enter at every option (20 November 2026): cage faces and points.
        let cases: [(&[&str], _); 6] = [
            (&["subdbox", "0,0", "4,3", "2"], (24, 26)),
            (&["subdplane", "0,0", "4,3"], (4, 9)),
            (&["subdsphere", "0,0", "5"], (24, 26)),
            (&["subdcylinder", "0,0", "2", "5"], (48, 42)),
            (&["subdcone", "0,0", "2", "5"], (40, 34)),
            (&["subdtorus", "0,0", "5", "1"], (60, 60)),
        ];
        for (steps, want) in cases {
            let mut doc = Document::default();
            run(&mut doc, steps);
            let object = doc.state().objects.last().expect("made");
            let cage = object.shape.subd().expect("a SubD").cage();
            assert_eq!((cage.face_count(), cage.vertex_count()), want, "{steps:?}");
        }
    }

    #[test]
    fn a_box_has_its_size_and_is_closed_facing_out() {
        let mesh = made(&["meshbox", "0,0", "10,8", "6"]);
        assert!((mesh.volume().unwrap() - 480.0).abs() < 1e-9);
        let both = made(&["meshbox", "0,0", "10,8", "BothSides", "6"]);
        assert!((both.volume().unwrap() - 960.0).abs() < 1e-9);
        // Enter at the height uses the side along Y, as Rhino 8's `MeshBox` and `Box` (measured
        // 21 November 2026: 0,0 then 4,3 then Enter gives a height of 3).
        let enter = made(&["meshbox", "0,0", "10,8", ""]);
        assert!((enter.volume().unwrap() - 640.0).abs() < 1e-9);
        let backwards = made(&["meshbox", "10,8", "0,0", "-6"]);
        assert!((backwards.volume().unwrap() - 480.0).abs() < 1e-9);
    }

    /// Size along the plane's X, Y and up, from the mesh's points.
    fn size(mesh: &Mesh) -> [f64; 3] {
        let (mut low, mut high) = ([f64::MAX; 3], [f64::MIN; 3]);
        for p in mesh.vertices() {
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
        std::array::from_fn(|k| high[k] - low[k])
    }

    #[test]
    fn a_typed_length_then_width_as_rhino_asks() {
        // Rhino 8 (21 November 2026): MeshBox and SubDBox with 5, then 3, then Enter give 5 × 3 × 3;
        // 5 then Enter twice give 5 × 5 × 5. MeshPlane and SubDPlane: 5 then 3 give 5 × 3, and
        // 5 then Enter give 5 × 5, as Rectangle.
        let cases: [(&[&str], [f64; 3]); 6] = [
            (&["meshbox", "0,0", "5", "3", ""], [5.0, 3.0, 3.0]),
            (&["meshbox", "0,0", "5", "", ""], [5.0, 5.0, 5.0]),
            (&["meshbox", "0,0", "5", "3", "2"], [5.0, 3.0, 2.0]),
            (
                &["meshbox", "Center", "0,0", "4", "2", "1"],
                [4.0, 2.0, 1.0],
            ),
            (&["meshplane", "0,0", "5", "3"], [5.0, 3.0, 0.0]),
            (&["meshplane", "0,0", "5", ""], [5.0, 5.0, 0.0]),
        ];
        for (steps, want) in cases {
            let got = size(&made(steps));
            assert!(
                (0..3).all(|k| (got[k] - want[k]).abs() < 1e-9),
                "{steps:?}: {got:?}"
            );
        }
        let mut doc = Document::default();
        run(&mut doc, &["subdbox", "0,0", "5", "3", ""]);
        let subd = doc
            .state()
            .objects
            .last()
            .expect("made")
            .shape
            .subd()
            .cloned();
        assert!(subd.is_some(), "SubDBox takes a typed length too");
    }

    #[test]
    fn face_counts_are_options() {
        let mesh = made(&[
            "meshbox", "XCount", "2", "YCount", "3", "ZCount", "4", "0,0", "1,1", "1",
        ]);
        assert_eq!(mesh.face_count(), 2 * (2 * 3 + 3 * 4 + 2 * 4));
        let plane = made(&["meshplane", "0,0", "XCount", "4", "YCount", "5", "10,8"]);
        assert_eq!(plane.face_count(), 20);
        let torus = made(&[
            "meshtorus",
            "VerticalFaces",
            "6",
            "AroundFaces",
            "12",
            "0,0",
            "10",
            "2",
        ]);
        assert_eq!(torus.face_count(), 72);
    }

    #[test]
    fn sphere_styles() {
        let quads = made(&[
            "meshsphere",
            "Style",
            "Quads",
            "Subdivisions",
            "1",
            "0,0",
            "5",
        ]);
        assert_eq!(counts(&quads).0, 24);
        assert_eq!(counts(&quads).1, 24);
        let tris = made(&[
            "meshsphere",
            "Style",
            "Triangles",
            "Subdivisions",
            "0",
            "0,0",
            "5",
        ]);
        assert_eq!(tris.face_count(), 20);
        let diameter = made(&["meshsphere", "0,0", "Diameter", "10"]);
        let volume = diameter.volume().unwrap();
        assert!(volume < 4.0 / 3.0 * std::f64::consts::PI * 125.0 && volume > 400.0);
    }

    #[test]
    fn open_cylinders_and_quad_caps() {
        let open = made(&["meshcylinder", "0,0", "5", "Solid", "10"]);
        assert_eq!(open.face_count(), 100);
        let quads = made(&["meshcylinder", "0,0", "5", "CapFaceStyle", "Quad", "10"]);
        assert!(counts(&quads).1 > 100);
        let cone = made(&["meshcone", "0,0", "5", "Solid", "10"]);
        assert_eq!(cone.face_count(), 100);
    }

    #[test]
    fn previews_show_while_picking() {
        let mut doc = Document::default();
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "meshbox");
        processor.submit(&mut doc, "0,0");
        processor.submit(&mut doc, "10,8");
        assert!(!processor.preview(&doc, None, "6").is_empty());
        let mut doc = Document::default();
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "meshsphere");
        processor.submit(&mut doc, "0,0");
        assert!(!processor.preview(&doc, None, "5").is_empty());
    }

    #[test]
    fn a_torus_needs_a_tube_thinner_than_its_ring() {
        let mut doc = Document::default();
        run(&mut doc, &["meshtorus", "0,0", "2", "3"]);
        assert!(doc.state().objects.is_empty());
    }

    #[test]
    fn two_points_give_the_ends_of_a_diameter_as_for_sphere() {
        let mesh = made(&["meshsphere", "2Point", "0,0", "4,0"]);
        let (low, high) = mesh.bounds().unwrap();
        assert!(
            (low[0]).abs() < 1e-9 && (high[0] - 4.0).abs() < 1e-9,
            "{low:?} {high:?}"
        );
        assert!((high[2] - 2.0).abs() < 1e-9);
        assert_eq!(counts(&mesh).0, 100);
    }
}
