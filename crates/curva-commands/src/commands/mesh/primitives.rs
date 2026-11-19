//! Rhino's mesh primitives: `MeshBox`, `MeshPlane`, `MeshSphere`, `MeshCylinder`, `MeshCone` and
//! `MeshTorus` (spec `docs/specs/mesh-and-subd.md` §1). They are picked like `Box`, `Sphere`,
//! `Cylinder`, `Cone` and `Torus`, with the same prompts, and made by `curva-mesh`'s primitives
//! with Rhino's face counts. Rhino 8's defaults were measured by making each one with Enter at
//! every option (19 November 2026): 10 faces each way, solid, with triangle caps.

use crate::Message;
use crate::commands::arguments::{number, point};
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef, Value};
use curva_document::Shape;
use curva_kernel::{Curve, Point, cross, dot, sub};
use curva_mesh::{Frame, Mesh, SphereStyle};

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

const fn faces(name: &'static str) -> OptionDef {
    OptionDef::whole(name, 10, 1, MOST_FACES)
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

fn added(mesh: Mesh, done: &'static str) -> Result<Made, Message> {
    Ok(Made {
        add: vec![Shape::Mesh(mesh)],
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

/// A rectangle from two picked corners on the plane, or from its centre and a corner with
/// `Center`: its first corner and its width and depth, if it is not flat.
fn rectangle(cx: &Context<'_>) -> Option<(Point, [f64; 2])> {
    let first = point(cx, 0)?;
    let other = point(cx, 1)?;
    let corner = if cx.mode == Some(CENTER) {
        std::array::from_fn(|i| 2.0 * first[i] - other[i])
    } else {
        first
    };
    let [x, y] = cx.plane();
    let across = sub(other, corner);
    let sides = [dot(across, x), dot(across, y)];
    sides
        .iter()
        .all(|side| side.abs() >= SMALLEST)
        .then_some((corner, sides))
}

/// The two corner steps of a rectangle, offering `first` at the first and `second` at the second.
fn rectangle_steps(
    cx: &Context<'_>,
    first: &'static [&'static str],
    second: &'static [&'static str],
) -> Vec<StepDef> {
    let mut steps = vec![StepDef::point("prompt.box.first").offering(first)];
    if let Some(start) = point(cx, 0) {
        steps.push(
            StepDef::point("prompt.box.other")
                .from(start)
                .offering(second)
                .corner(),
        );
    }
    steps
}

/// `MeshBox`: a box's base as `Box` takes it, then its height (Enter: the base's width).
#[derive(Default)]
pub struct MeshBox;

impl Recipe for MeshBox {
    const NAME: &'static str = "meshbox";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::mode(CENTER),
        OptionDef::toggle(BOTH_SIDES, false),
        faces("XCount"),
        faces("YCount"),
        faces("ZCount"),
    ];

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        const COUNTS: &[&str] = &["XCount", "YCount", "ZCount"];
        let mut steps = rectangle_steps(cx, &[CENTER, "XCount", "YCount", "ZCount"], COUNTS);
        if let (Some(start), true) = (point(cx, 0), cx.values.len() >= 2) {
            let [x, y] = cx.plane();
            steps.push(
                StepDef::along("prompt.box.height", start, cross(x, y))
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
        let high = match cx.values.get(2) {
            Some(Value::Number(high)) => *high,
            // Enter uses the base's width, as in Rhino.
            Some(Value::Empty) => wide.abs(),
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
            Mesh::mesh_box(&frame_at(cx, start), [wide, deep, high], counts),
            "done.meshbox",
        )
    }
}

/// `MeshPlane`: a flat rectangle of faces from two corners, or its centre and a corner.
#[derive(Default)]
pub struct MeshPlane;

impl Recipe for MeshPlane {
    const NAME: &'static str = "meshplane";
    const OPTIONS: &'static [OptionDef] =
        &[OptionDef::mode(CENTER), faces("XCount"), faces("YCount")];

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
            Mesh::mesh_plane(&frame_at(cx, corner), size, counts),
            "done.meshplane",
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

/// `MeshSphere`: its centre, then its radius, in Rhino's UV, Quads or Triangles style.
#[derive(Default)]
pub struct MeshSphere;

impl Recipe for MeshSphere {
    const NAME: &'static str = "meshsphere";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(DIAMETER, false),
        OptionDef::name(STYLE, "UV"),
        faces(VERTICAL),
        faces(AROUND),
        // Rhino's range; its default could not be measured, so 2 (96 quads, 320 triangles).
        OptionDef::whole(SUBDIVISIONS, 2, 0, 6),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == STYLE).then(|| ["UV", "Quads", "Triangles"].map(str::to_owned).to_vec())
    }

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        circle_steps(
            cx,
            "sphere",
            &[DIAMETER, STYLE, VERTICAL, AROUND, SUBDIVISIONS],
        )
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(c), Some(r)) = (point(cx, 0), radius(cx, 1)) else {
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
            Mesh::mesh_sphere(&frame_at(cx, c), r, style),
            "done.meshsphere",
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
pub struct MeshCylinder;

impl Recipe for MeshCylinder {
    const NAME: &'static str = "meshcylinder";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(DIAMETER, false),
        OptionDef::toggle(SOLID, true),
        OptionDef::toggle(BOTH_SIDES, false),
        faces(VERTICAL),
        faces(AROUND),
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
            Mesh::mesh_cylinder(
                &frame_at(cx, start),
                r,
                high,
                (count(cx, VERTICAL), count(cx, AROUND)),
                cx.toggle(SOLID),
                quad_caps(cx),
            ),
            "done.meshcylinder",
        )
    }
}

/// `MeshCone`: the centre of its base, its radius, then the height of its point.
#[derive(Default)]
pub struct MeshCone;

impl Recipe for MeshCone {
    const NAME: &'static str = "meshcone";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(DIAMETER, false),
        OptionDef::toggle(SOLID, true),
        faces(VERTICAL),
        faces(AROUND),
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
            Mesh::mesh_cone(
                &frame_at(cx, c),
                r,
                high,
                (count(cx, VERTICAL), count(cx, AROUND)),
                cx.toggle(SOLID),
                quad_caps(cx),
            ),
            "done.meshcone",
        )
    }
}

/// `MeshTorus`: its centre, the radius to the middle of its tube, then the tube's radius.
#[derive(Default)]
pub struct MeshTorus;

impl Recipe for MeshTorus {
    const NAME: &'static str = "meshtorus";
    const OPTIONS: &'static [OptionDef] = &[faces(VERTICAL), faces(AROUND)];

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
            Mesh::mesh_torus(
                &frame_at(cx, c),
                r,
                tube,
                (count(cx, VERTICAL), count(cx, AROUND)),
            ),
            "done.meshtorus",
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
    fn a_box_has_its_size_and_is_closed_facing_out() {
        let mesh = made(&["meshbox", "0,0", "10,8", "6"]);
        assert!((mesh.volume().unwrap() - 480.0).abs() < 1e-9);
        let both = made(&["meshbox", "0,0", "10,8", "BothSides", "6"]);
        assert!((both.volume().unwrap() - 960.0).abs() < 1e-9);
        let enter = made(&["meshbox", "0,0", "10,8", ""]);
        assert!((enter.volume().unwrap() - 800.0).abs() < 1e-9);
        let backwards = made(&["meshbox", "10,8", "0,0", "-6"]);
        assert!((backwards.volume().unwrap() - 480.0).abs() < 1e-9);
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
}
