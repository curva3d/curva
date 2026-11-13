//! Curva models written as `.3dm` and read back (spec `docs/specs/3dm-files.md` §4, round trips).

use curva_document::blocks::{BlockDefinition, Instance};
use curva_document::{Layer, Linetype, Object, PrintWidth, Shape, State};
use curva_kernel::{Body, Curve};

fn object(id: u64, shape: Shape, layer: u64) -> Object {
    Object {
        id,
        shape,
        layer,
        hidden: false,
        name: String::new(),
        recipe: None,
        print_width: PrintWidth::ByLayer,
        locked: false,
        group: None,
        live: None,
        input_of: None,
        linetype: Linetype::ByLayer,
        user_text: Default::default(),
    }
}

fn layer(id: u64, name: &str, colour: [f32; 3]) -> Layer {
    Layer {
        id,
        name: name.to_owned(),
        colour,
        visible: true,
        locked: false,
        print_width: None,
        linetype: Linetype::Continuous,
    }
}

/// A model with a little of everything §2 lists.
pub fn sample() -> State {
    let mut state = State {
        layers: vec![
            layer(0, "Default", curva_document::STANDARD_COLOUR),
            layer(1, "Walls", [1.0, 0.0, 0.0]),
            layer(2, "Walls::Openings", [0.0, 0.0, 1.0]),
        ],
        next_layer: 3,
        ..State::default()
    };
    state.layers[1].locked = false;
    state.layers[2].visible = false;
    state.layers[2].print_width = Some(0.35);
    let circle = Curve::circle([0.0; 3], 2.0).unwrap();
    let arc = Curve::arc(
        [0.0, 0.0, -3.0],
        [3.0, 0.0, 0.0],
        [0.0, 0.0, 3.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    )
    .unwrap();
    let shapes = vec![
        (
            Shape::Curve(Curve::Point {
                at: [1.0, 2.0, 3.0],
            }),
            0,
        ),
        (
            Shape::Curve(Curve::line([0.0; 3], [3.0, 4.0, 0.0]).unwrap()),
            1,
        ),
        (Shape::Curve(circle.clone()), 1),
        (Shape::Curve(arc.clone()), 2),
        (
            Shape::Curve(Curve::Polyline {
                points: vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
                closed: true,
            }),
            0,
        ),
        (
            Shape::Curve(
                Curve::control_point(
                    vec![[0.0; 3], [1.0, 2.0, 0.0], [3.0, -1.0, 1.0], [5.0, 0.0, 0.0]],
                    3,
                    false,
                )
                .unwrap(),
            ),
            0,
        ),
        (
            Shape::Body(Body::cuboid([10.0, 0.0, 0.0], [1.0, 2.0, 3.0]).unwrap()),
            1,
        ),
        (
            Shape::Body(Body::extruded(&[circle], [0.0, 0.0, 1.0], 5.0, true).unwrap()),
            1,
        ),
        (
            Shape::Body(
                Body::revolved(
                    &[arc],
                    [0.0; 3],
                    [0.0, 0.0, 1.0],
                    std::f64::consts::TAU,
                    false,
                )
                .unwrap(),
            ),
            2,
        ),
    ];
    for (shape, layer) in shapes {
        let id = state.next_id;
        state.next_id += 1;
        state.objects.push(object(id, shape, layer));
    }
    state.objects[1].name = "line".to_owned();
    state.objects[1].group = Some(7);
    state.objects[2].group = Some(7);
    state.objects[2].print_width = PrintWidth::Millimetres(0.5);
    state.objects[3].hidden = true;
    state.objects[4].locked = true;
    state.objects[6]
        .user_text
        .insert("Material".to_owned(), "Oak".to_owned());

    // A block of one box, placed twice.
    state.blocks.push(BlockDefinition {
        id: 0,
        name: "Box".to_owned(),
        objects: vec![object(
            100,
            Shape::Body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap()),
            0,
        )],
        linked: None,
    });
    state.next_block = 1;
    for dx in [20.0, 25.0] {
        let id = state.next_id;
        state.next_id += 1;
        let mut placement = [[0.0; 4]; 4];
        for (i, row) in placement.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        placement[3][0] = dx;
        state.objects.push(object(
            id,
            Shape::Instance(Instance {
                definition: 0,
                placement,
            }),
            0,
        ));
    }
    state
}

fn round_trip(state: &State) -> State {
    let (model, report) = curva_3dm::export::export(state, None);
    assert!(report.skipped.is_empty(), "{}", report.summary());
    let bytes = curva_3dm::write::write(&model);
    // For opening in Rhino by hand.
    if let Ok(path) = std::env::var("CURVA_3DM_SAMPLE") {
        std::fs::write(path, &bytes).unwrap();
    }
    let back = curva_3dm::read::read(&bytes).unwrap();
    let (doc, report) = curva_3dm::import::open(&back);
    assert!(report.skipped.is_empty(), "{}", report.summary());
    doc.state().clone()
}

fn measure(shape: &Shape) -> (f64, f64) {
    match shape {
        Shape::Curve(Curve::Point { at }) => (at.iter().sum(), 0.0),
        Shape::Curve(c) => (c.length().unwrap(), 0.0),
        Shape::Body(b) => (b.area().unwrap(), b.volume().unwrap_or(0.0)),
        Shape::Instance(i) => (i.placement[3][0], i.definition as f64),
    }
}

#[test]
fn a_curva_model_comes_back_the_same() {
    let before = sample();
    let after = round_trip(&before);

    let names: Vec<&str> = after.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Default", "Walls", "Walls::Openings"]);
    for (a, b) in after.layers.iter().zip(&before.layers) {
        assert_eq!(a.visible, b.visible, "{}", a.name);
        assert_eq!(a.locked, b.locked, "{}", a.name);
        assert_eq!(a.print_width, b.print_width, "{}", a.name);
        for k in 0..3 {
            assert!((a.colour[k] - b.colour[k]).abs() < 0.01, "{}", a.name);
        }
    }

    assert_eq!(after.objects.len(), before.objects.len());
    for (a, b) in after.objects.iter().zip(&before.objects) {
        let what = format!("object {} ({:?})", b.id, std::mem::discriminant(&b.shape));
        let layer_name =
            |s: &State, id: u64| s.layers.iter().find(|l| l.id == id).unwrap().name.clone();
        assert_eq!(
            layer_name(&after, a.layer),
            layer_name(&before, b.layer),
            "{what}"
        );
        assert_eq!(a.name, b.name, "{what}");
        assert_eq!(a.hidden, b.hidden, "{what}");
        assert_eq!(a.locked, b.locked, "{what}");
        assert_eq!(a.print_width, b.print_width, "{what}");
        assert_eq!(a.user_text, b.user_text, "{what}");
        assert_eq!(a.group.is_some(), b.group.is_some(), "{what}");
        let (ma, mb) = (measure(&a.shape), measure(&b.shape));
        assert!(
            (ma.0 - mb.0).abs() < 1e-6 * mb.0.abs().max(1.0)
                && (ma.1 - mb.1).abs() < 1e-6 * mb.1.abs().max(1.0),
            "{what}: {ma:?} against {mb:?}"
        );
    }
    assert_eq!(after.objects[1].group, after.objects[2].group);
    assert_eq!(after.blocks.len(), 1);
    assert_eq!(after.blocks[0].name, "Box");
    assert_eq!(after.blocks[0].objects.len(), 1);
}

#[test]
fn exporting_the_selection_takes_only_it_and_its_blocks() {
    let state = sample();
    let last = state.objects.last().unwrap().id;
    let (model, _) = curva_3dm::export::export(&state, Some(&[1, last]));
    let top = model
        .objects
        .iter()
        .filter(|o| o.attributes.mode != 3)
        .count();
    assert_eq!(top, 2);
    assert_eq!(model.blocks.len(), 1);
}

/// Where a written curve is at the ends of `span`, for the curves edges are written as.
fn ends(c: &curva_3dm::model::Curve, span: [f64; 2]) -> [[f64; 3]; 2] {
    use curva_3dm::model::Curve as C;
    match c {
        C::Line { ends, domain } => {
            assert_eq!(*domain, span, "an edge on part of its line");
            *ends
        }
        C::Arc {
            plane,
            radius,
            angles,
            domain,
        } => {
            assert_eq!(*domain, span, "an edge on part of its arc");
            angles.map(|a| {
                std::array::from_fn(|k| {
                    plane.origin[k] + radius * (a.cos() * plane.x[k] + a.sin() * plane.y[k])
                })
            })
        }
        C::Nurbs(n) => {
            let full = [n.knots[n.order - 2], n.knots[n.points.len() - 1]];
            assert_eq!(full, span, "an edge on part of its NURBS");
            let at = |p: [f64; 4]| [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
            [at(n.points[0]), at(n.points[n.points.len() - 1])]
        }
        other => panic!("an edge written as {other:?}"),
    }
}

#[test]
fn every_edge_runs_between_its_vertices() {
    let (model, _) = curva_3dm::export::export(&sample(), None);
    let mut checked = 0;
    for o in &model.objects {
        let curva_3dm::model::Geometry::Brep(b) = &o.geometry else {
            continue;
        };
        for e in &b.edges {
            let curve = b.curves3[e.curve as usize].as_ref().unwrap();
            let [start, end] = ends(curve, e.proxy_domain);
            for (p, v) in [(start, e.vertices[0]), (end, e.vertices[1])] {
                let q = b.vertices[v as usize].point;
                let gap = (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>().sqrt();
                assert!(gap < 1e-9, "an edge {gap} from its vertex");
            }
            checked += 1;
        }
    }
    assert!(checked > 20);
}
