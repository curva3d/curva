//! Curva models written as `.3dm` and read back (spec `docs/specs/3dm-files.md` §4, round trips).

use curva_document::blocks::{BlockDefinition, Instance};
use curva_document::{Layer, Linetype, Object, PrintWidth, Shape, State};
use curva_kernel::{Body, Curve};

fn object(id: u64, shape: Shape, layer: u64) -> Object {
    Object {
        space: None,
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
        colour: None,
        linetype: Linetype::ByLayer,
        user_text: Default::default(),
        section: None,
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
        parent: None,
        collapsed: false,
        section: None,
    }
}

/// A model with a little of everything §2 lists.
pub fn sample() -> State {
    let mut state = State {
        layers: vec![
            layer(0, "Default", curva_document::STANDARD_COLOUR),
            layer(1, "Walls", [1.0, 0.0, 0.0]),
            layer(2, "Openings", [0.0, 0.0, 1.0]),
        ],
        next_layer: 3,
        ..State::default()
    };
    state.layers[1].locked = false;
    state.layers[2].visible = false;
    state.layers[2].print_width = Some(0.35);
    // A sublayer (spec `sublayers.md`): Rhino's nested layer, both ways.
    state.layers[2].parent = Some(1);
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
        state.objects.push(object(id, shape, layer).into());
    }
    std::sync::Arc::make_mut(&mut state.objects[1]).name = "line".to_owned();
    std::sync::Arc::make_mut(&mut state.objects[1]).group = Some(7);
    std::sync::Arc::make_mut(&mut state.objects[2]).group = Some(7);
    std::sync::Arc::make_mut(&mut state.objects[2]).print_width = PrintWidth::Millimetres(0.5);
    std::sync::Arc::make_mut(&mut state.objects[3]).hidden = true;
    std::sync::Arc::make_mut(&mut state.objects[4]).locked = true;
    std::sync::Arc::make_mut(&mut state.objects[6])
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
        state.objects.push(
            object(
                id,
                Shape::Instance(Instance {
                    definition: 0,
                    placement,
                }),
                0,
            )
            .into(),
        );
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
        Shape::Annotation(_) => unreachable!("the sample has no annotation"),
    }
}

#[test]
fn a_curva_model_comes_back_the_same() {
    let before = sample();
    let after = round_trip(&before);

    let names: Vec<String> = after
        .layers
        .iter()
        .map(|l| after.layer_path(l.id))
        .collect();
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
        let layer_name = |s: &State, id: u64| s.layer_path(id);
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

#[test]
fn named_views_and_construction_planes_come_back() {
    use curva_document::{Document, SavedCPlane, SavedView};
    let mut doc = Document::default();
    doc.restore(sample());
    let views = [
        SavedView {
            name: "Corner".to_owned(),
            kind: "Perspective".to_owned(),
            target: [1.0, 2.0, 3.0],
            yaw: -1.0,
            pitch: 0.6,
            distance: 150.0,
            axes: None,
            parallel: false,
        },
        SavedView {
            name: "Plan".to_owned(),
            kind: "Top".to_owned(),
            target: [10.0, 5.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            distance: 330.0,
            axes: Some([[0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            parallel: true,
        },
        SavedView {
            name: "Elevation".to_owned(),
            kind: "Front".to_owned(),
            target: [0.0, 0.0, 4.0],
            yaw: 0.0,
            pitch: 0.0,
            distance: 80.0,
            axes: Some([[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]),
            parallel: true,
        },
    ];
    for v in &views {
        doc.save_view(v.clone());
    }
    let planes = [
        SavedCPlane {
            name: "Level 2".to_owned(),
            origin: [0.0, 0.0, 3.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
        },
        SavedCPlane {
            name: "Slope".to_owned(),
            origin: [5.0, 0.0, 0.0],
            x: [0.0, 1.0, 0.0],
            y: [-0.6, 0.0, 0.8],
        },
    ];
    for p in &planes {
        doc.save_cplane(p.clone());
    }

    let (model, _) = curva_3dm::export::export_document(&doc, None);
    let bytes = curva_3dm::write::write(&model);
    if let Ok(path) = std::env::var("CURVA_3DM_VIEWS") {
        std::fs::write(path, &bytes).unwrap();
    }
    let (back, report) = curva_3dm::import::open(&curva_3dm::read::read(&bytes).unwrap());
    assert!(report.skipped.is_empty(), "{}", report.summary());

    let near = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert_eq!(back.views().len(), views.len());
    for (a, b) in back.views().iter().zip(&views) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.kind, b.kind, "{}", b.name);
        assert_eq!(a.parallel, b.parallel, "{}", b.name);
        assert!(near(&a.target, &b.target), "{}: {:?}", b.name, a.target);
        assert!(
            (a.distance - b.distance).abs() < 1e-9,
            "{}: {}",
            b.name,
            a.distance
        );
        match (a.axes, b.axes) {
            (None, None) => {
                assert!((a.yaw - b.yaw).abs() < 1e-9 && (a.pitch - b.pitch).abs() < 1e-9);
            }
            (Some(x), Some(y)) => {
                for k in 0..3 {
                    assert!(near(&x[k], &y[k]), "{}: {x:?}", b.name);
                }
            }
            other => panic!("{}: {other:?}", b.name),
        }
    }
    assert_eq!(back.cplanes().len(), planes.len());
    for (a, b) in back.cplanes().iter().zip(&planes) {
        assert_eq!(a.name, b.name);
        assert!(
            near(&a.origin, &b.origin) && near(&a.x, &b.x) && near(&a.y, &b.y),
            "{a:?}"
        );
    }
}

#[test]
fn rhinos_own_named_view_comes_in() {
    let path = format!("{}/tests/files/v5_teacup.3dm", env!("CARGO_MANIFEST_DIR"));
    let model = curva_3dm::read::read(&std::fs::read(path).unwrap()).unwrap();
    let (doc, _) = curva_3dm::import::open(&model);
    let view = doc
        .saved_view("Perspective1")
        .expect("the teacup's named view");
    assert!(!view.parallel && view.distance > 0.0);
}

#[test]
fn named_views_and_planes_saved_by_rhino_come_in() {
    let path = format!(
        "{}/tests/files/v7_named_views.3dm",
        env!("CARGO_MANIFEST_DIR")
    );
    let model = curva_3dm::read::read(&std::fs::read(path).unwrap()).unwrap();
    let (doc, report) = curva_3dm::import::open(&model);
    assert!(report.skipped.is_empty(), "{}", report.summary());
    let near = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);

    // A flat view looking down: Curva's Top, its zoom from Rhino's frustum height.
    let plan = doc.saved_view("Plan from above").unwrap();
    assert_eq!(plan.kind, "Top");
    assert!(plan.parallel);
    assert!(near(&plan.target, &[50.0, 20.0, 0.0]));
    let axes = plan.axes.unwrap();
    assert!(near(&axes[0], &[0.0, 0.0, -1.0]) && near(&axes[2], &[0.0, 1.0, 0.0]));
    let half = 128.124_939_024_375_66;
    assert!((plan.distance - half / 0.35f64.tan()).abs() < 1e-6);

    // An upright perspective view: yaw and pitch, from the same eye.
    let bird = doc.saved_view("Bird's eye").unwrap();
    assert_eq!(bird.kind, "Perspective");
    assert!(bird.axes.is_none() && !bird.parallel);
    let eye: Vec<f64> = (0..3)
        .map(|k| {
            let (cp, sp) = (bird.pitch.cos(), bird.pitch.sin());
            bird.target[k] + bird.distance * [cp * bird.yaw.cos(), cp * bird.yaw.sin(), sp][k]
        })
        .collect();
    assert!(near(&eye, &[300.0, -400.0, 250.0]), "{eye:?}");

    let level = doc.saved_cplane("Level 1").unwrap();
    assert!(near(&level.origin, &[0.0, 0.0, 3000.0]));
    let wall = doc.saved_cplane("Wall A").unwrap();
    assert!(near(&wall.y, &[0.0, 0.0, 1.0]));
}

#[test]
fn dots_come_back_with_their_words_height_and_font() {
    use curva_document::annotation::{Annotation, Dot};
    let mut state = State::default();
    let dot = Dot {
        at: [1.0, 2.0, 3.0],
        content: "Room 101".to_owned(),
        secondary: "Storage".to_owned(),
        height: 14.0,
        font: Some("Arial".to_owned()),
    };
    state
        .objects
        .push(object(0, Shape::Annotation(Annotation::Dot(dot.clone())), 0).into());
    state.next_id = 1;
    let back = round_trip(&state);
    assert_eq!(back.objects.len(), 1);
    assert_eq!(
        back.objects[0].shape,
        Shape::Annotation(Annotation::Dot(dot))
    );
}

#[test]
fn hatches_come_back_with_their_pattern_scale_and_holes() {
    use curva_document::annotation::{Annotation, Hatch};
    let mut state = State::default();
    let outline = Curve::polyline(
        vec![
            [0.0, 0.0, 5.0],
            [100.0, 0.0, 5.0],
            [100.0, 50.0, 5.0],
            [0.0, 50.0, 5.0],
        ],
        true,
    )
    .unwrap();
    let hole = Curve::circle([50.0, 25.0, 5.0], 10.0).unwrap();
    let hatch = |pattern: &str| Hatch {
        loops: vec![outline.clone(), hole.clone()],
        pattern: pattern.to_owned(),
        scale: 2.5,
        rotation: 0.4,
        base: [0.0, 0.0, 5.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
    };
    state
        .objects
        .push(object(0, Shape::Annotation(Annotation::Hatch(hatch("ANSI31"))), 0).into());
    state
        .objects
        .push(object(1, Shape::Annotation(Annotation::Hatch(hatch("Solid"))), 0).into());
    state.next_id = 2;
    let back = round_trip(&state);
    let hatches: Vec<Hatch> = back
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Hatch(h)) => Some(h.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(hatches.len(), 2);
    assert_eq!(hatches[0].pattern, "ANSI31");
    assert!(
        (hatches[0].scale - 2.5).abs() < 1e-9,
        "{}",
        hatches[0].scale
    );
    assert!((hatches[0].rotation - 0.4).abs() < 1e-12);
    assert_eq!(hatches[0].loops.len(), 2, "the hole stays");
    // The hole comes back as the same circle (as an exact NURBS circle).
    for p in hatches[0].loops[1].sample().unwrap() {
        let r = ((p[0] - 50.0).powi(2) + (p[1] - 25.0).powi(2)).sqrt();
        assert!(
            (r - 10.0).abs() < 1e-9 && (p[2] - 5.0).abs() < 1e-9,
            "{p:?}"
        );
    }
    assert!(
        hatches[0].loops[0]
            .sample()
            .unwrap()
            .iter()
            .all(|p| (p[2] - 5.0).abs() < 1e-9),
        "at its height"
    );
    assert_eq!(hatches[1].pattern, "Solid");
}

#[test]
fn notes_come_back_with_their_words_height_alignment_and_place() {
    use curva_document::annotation::{Annotation, Text, TextAlign};
    let mut state = State::default();
    let (s, c) = 0.5_f64.sin_cos();
    let note = |words: &str, height: f64, align: TextAlign, origin: [f64; 3]| Text {
        content: words.to_owned(),
        origin,
        x: [c, s, 0.0],
        y: [-s, c, 0.0],
        height,
        font: None,
        align,
        style: 0,
    };
    let notes = [
        note("Hall", 3.5, TextAlign::Left, [10.0, 20.0, 0.0]),
        note(
            "Ground floor\nPlan",
            5.0,
            TextAlign::Centre,
            [50.0, 0.0, 2.0],
        ),
        note("%<PageName()>%", 3.5, TextAlign::Right, [0.0, -30.0, 0.0]),
    ];
    for (i, n) in notes.iter().enumerate() {
        state
            .objects
            .push(object(i as u64, Shape::Annotation(Annotation::Text(n.clone())), 0).into());
    }
    state.next_id = 3;
    let back = round_trip(&state);
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
    for (o, wanted) in back.objects.iter().zip(&notes) {
        let Shape::Annotation(Annotation::Text(t)) = &o.shape else {
            panic!("not a note");
        };
        assert_eq!(t.content, wanted.content);
        assert!((t.height - wanted.height).abs() < 1e-12);
        assert_eq!(t.align, wanted.align);
        assert!(near(t.origin, wanted.origin), "{:?}", t.origin);
        assert!(near(t.x, wanted.x) && near(t.y, wanted.y));
    }
}

#[test]
fn linear_dimensions_come_back_measuring_the_same() {
    use curva_document::annotation::{Annotation, Dimension, DimensionKind};
    let mut state = State::default();
    let dim = |kind, second: [f64; 3], text: Option<&str>| Dimension {
        kind,
        first: [10.0, 10.0, 0.0],
        second,
        line: [20.0, 40.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        style: 0,
        text: text.map(str::to_owned),
    };
    let dims = [
        dim(DimensionKind::Horizontal, [60.0, 30.0, 0.0], None),
        dim(DimensionKind::Vertical, [60.0, 30.0, 0.0], None),
        dim(DimensionKind::Aligned, [60.0, 30.0, 0.0], Some("<> clear")),
        dim(DimensionKind::Rotated(0.3), [60.0, 30.0, 0.0], None),
    ];
    for (i, d) in dims.iter().enumerate() {
        state.objects.push(
            object(
                i as u64,
                Shape::Annotation(Annotation::Dimension(d.clone())),
                0,
            )
            .into(),
        );
    }
    state.next_id = 4;
    let back = round_trip(&state);
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
    for (o, wanted) in back.objects.iter().zip(&dims) {
        let Shape::Annotation(Annotation::Dimension(d)) = &o.shape else {
            panic!("not a dimension");
        };
        assert!(
            (d.value() - wanted.value()).abs() < 1e-9,
            "{:?}",
            wanted.kind
        );
        assert!(near(d.first, wanted.first) && near(d.second, wanted.second));
        assert_eq!(d.text, wanted.text);
        // The dimension line passes the same distance from the measured points.
        let side = |x: &Dimension| {
            let dir = x.direction();
            let v: Vec<f64> = (0..3).map(|k| x.line[k] - x.first[k]).collect();
            let across = [-dir[1], dir[0], 0.0];
            (0..3).map(|k| v[k] * across[k]).sum::<f64>()
        };
        assert!((side(d) - side(wanted)).abs() < 1e-9, "{:?}", wanted.kind);
    }
}

#[test]
fn radial_and_angle_dimensions_and_leaders_come_back() {
    use curva_document::annotation::{AngleDimension, Annotation, Leader, RadialDimension};
    let mut state = State::default();
    let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let radial = |diameter| RadialDimension {
        diameter,
        centre: [10.0, 10.0, 0.0],
        on_curve: [13.0, 14.0, 0.0],
        text_at: [20.0, 25.0, 0.0],
        x,
        y,
        style: 0,
        text: None,
    };
    let angle = AngleDimension {
        apex: [0.0, 0.0, 0.0],
        first: [10.0, 0.0, 0.0],
        second: [0.0, 5.0, 0.0],
        arc_at: [3.0, 3.0, 0.0],
        x,
        y,
        style: 0,
        text: Some("<> min".to_owned()),
    };
    let leader = Leader {
        points: vec![[0.0, 0.0, 0.0], [10.0, 10.0, 0.0], [20.0, 10.0, 0.0]],
        content: "Oak floor\nOiled".to_owned(),
        x,
        y,
        landing: true,
        style: 0,
    };
    let shapes = [
        Annotation::Radial(radial(false)),
        Annotation::Radial(radial(true)),
        Annotation::Angle(angle.clone()),
        Annotation::Leader(leader.clone()),
    ];
    for (i, a) in shapes.iter().enumerate() {
        state
            .objects
            .push(object(i as u64, Shape::Annotation(a.clone()), 0).into());
    }
    state.next_id = 4;
    let back = round_trip(&state);
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
    let got: Vec<&Annotation> = back
        .objects
        .iter()
        .map(|o| match &o.shape {
            Shape::Annotation(a) => a,
            _ => panic!("not annotation"),
        })
        .collect();
    for (a, wanted) in got.iter().zip(&shapes) {
        match (a, wanted) {
            (Annotation::Radial(a), Annotation::Radial(w)) => {
                assert_eq!(a.diameter, w.diameter);
                assert!((a.value() - w.value()).abs() < 1e-9);
                assert!(near(a.on_curve, w.on_curve) && near(a.text_at, w.text_at));
            }
            (Annotation::Angle(a), Annotation::Angle(w)) => {
                assert!(
                    (a.value() - w.value()).abs() < 1e-9,
                    "{} {}",
                    a.value(),
                    w.value()
                );
                assert!(near(a.first, w.first) && near(a.second, w.second));
                assert!(near(a.arc_at, w.arc_at));
                assert_eq!(a.text, w.text);
            }
            (Annotation::Leader(a), Annotation::Leader(w)) => {
                assert_eq!(a.content, w.content);
                assert_eq!(a.points.len(), w.points.len());
                for (p, q) in a.points.iter().zip(&w.points) {
                    assert!(near(*p, *q));
                }
            }
            _ => panic!("came back as another kind"),
        }
    }
}

#[test]
fn layouts_come_back_with_their_details_and_sheet_drawing() {
    use curva_document::layouts::{Detail, Layout, PaperUnit};
    let mut state = State::default();
    let on_sheet = |id: u64, curve: Curve, space: Option<u64>| {
        let mut o = object(id, Shape::Curve(curve), 0);
        o.space = space;
        o
    };
    let frame = Curve::polyline(
        vec![
            [20.0, 20.0, 0.0],
            [220.0, 20.0, 0.0],
            [220.0, 170.0, 0.0],
            [20.0, 170.0, 0.0],
        ],
        true,
    )
    .unwrap();
    let note = Curve::line([250.0, 20.0, 0.0], [400.0, 20.0, 0.0]).unwrap();
    // An imperial sheet too: its drawing goes out in millimetres and comes back in them.
    let small = Curve::line([1.0, 1.0, 0.0], [2.0, 1.0, 0.0]).unwrap();
    state.objects.push(
        on_sheet(
            0,
            Curve::line([0.0; 3], [10_000.0, 0.0, 0.0]).unwrap(),
            None,
        )
        .into(),
    );
    state.objects.push(on_sheet(1, frame, Some(0)).into());
    state.objects.push(on_sheet(2, note, Some(0)).into());
    state.objects.push(on_sheet(3, small, Some(1)).into());
    state.next_id = 4;
    let detail = Detail {
        frame: 1,
        projection: "Top".to_owned(),
        target: [5000.0, 0.0, 0.0],
        distance: 100.0,
        scale: Some(0.02),
        display: "Wireframe".to_owned(),
        locked: false,
        hidden_layers: Vec::new(),
    };
    state.layouts = vec![
        Layout {
            id: 0,
            name: "Plan".to_owned(),
            size: [420.0, 297.0],
            unit: PaperUnit::Millimetres,
            details: vec![detail],
            user_text: Default::default(),
            revisions: Vec::new(),
        },
        Layout {
            id: 1,
            name: "Notes".to_owned(),
            size: [17.0, 11.0],
            unit: PaperUnit::Inches,
            details: Vec::new(),
            user_text: Default::default(),
            revisions: Vec::new(),
        },
    ];
    state.next_layout = 2;
    let back = round_trip(&state);
    assert_eq!(back.layouts.len(), 2);
    let plan = &back.layouts[0];
    assert_eq!((plan.name.as_str(), plan.size), ("Plan", [420.0, 297.0]));
    let d = &plan.details[0];
    assert_eq!(d.projection, "Top");
    assert!((d.scale.unwrap() - 0.02).abs() < 1e-12);
    assert!((d.target[0] - 5000.0).abs() < 1e-9);
    let frame = back.objects.iter().find(|o| o.id == d.frame).unwrap();
    assert_eq!(frame.space, Some(plan.id));
    let (low, high) = frame.shape.bounds().unwrap();
    assert!((low[0] - 20.0).abs() < 1e-9 && (high[0] - 220.0).abs() < 1e-9);
    let notes = &back.layouts[1];
    assert_eq!(notes.size, [17.0 * 25.4, 11.0 * 25.4], "in millimetres");
    let small = back
        .objects
        .iter()
        .find(|o| o.space == Some(notes.id))
        .unwrap();
    assert!((small.curve().unwrap().length().unwrap() - 25.4).abs() < 1e-9);
    assert_eq!(
        back.objects.iter().filter(|o| o.space.is_none()).count(),
        1,
        "the model's line stays in the model"
    );
}

#[test]
fn arc_length_dimensions_go_out_as_their_plain_drawing() {
    use curva_document::annotation::{Annotation, ArcLengthDimension};
    let mut state = State::default();
    let dimension = ArcLengthDimension {
        centre: [0.0; 3],
        start: [10.0, 0.0, 0.0],
        end: [0.0, 10.0, 0.0],
        arc_at: [10.0, 10.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        style: 0,
        text: None,
        partial: false,
        leader: false,
    };
    state
        .objects
        .push(object(0, Shape::Annotation(Annotation::ArcLength(dimension)), 0).into());
    state.next_id = 1;
    let back = round_trip(&state);
    // Rhino has no arc length dimension: its lines, two arrowheads and its value, grouped.
    let lines = back
        .objects
        .iter()
        .filter(|o| matches!(o.shape, Shape::Curve(_)))
        .count();
    let heads = back
        .objects
        .iter()
        .filter(|o| matches!(o.shape, Shape::Annotation(Annotation::Hatch(_))))
        .count();
    let words: Vec<String> = back
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Text(t)) => Some(t.content.clone()),
            _ => None,
        })
        .collect();
    assert!(lines > 32, "{lines}");
    assert_eq!(heads, 2);
    assert_eq!(words, vec!["15.71".to_owned()]);
    let group = back.objects[0].group;
    assert!(group.is_some() && back.objects.iter().all(|o| o.group == group));
}

#[test]
fn scale_bars_and_tables_go_out_as_their_plain_drawing() {
    use curva_document::annotation::{Annotation, Text, TextAlign};
    use curva_document::layouts::{Detail, Layout, PaperUnit};
    use curva_document::sheet::{ScaleBar, Table, TableKind};
    let mut state = State::default();
    let place = Text {
        content: String::new(),
        origin: [250.0, 100.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        height: 2.5,
        font: None,
        align: TextAlign::Left,
        style: 0,
    };
    let frame = Curve::polyline(
        vec![
            [20.0, 20.0, 0.0],
            [220.0, 20.0, 0.0],
            [220.0, 170.0, 0.0],
            [20.0, 170.0, 0.0],
        ],
        true,
    )
    .unwrap();
    let mut on = |id: u64, shape: Shape| {
        let mut o = object(id, shape, 0);
        o.space = Some(0);
        state.objects.push(o.into());
    };
    on(0, Shape::Curve(frame));
    on(
        1,
        Shape::Annotation(Annotation::ScaleBar(ScaleBar {
            at: place.clone(),
            detail: 0,
            length: None,
            divisions: 0,
        })),
    );
    on(
        2,
        Shape::Annotation(Annotation::Table(Table {
            at: Text {
                origin: [250.0, 280.0, 0.0],
                ..place
            },
            kind: TableKind::SheetIndex,
            headings: vec!["No.".into(), "Sheet".into()],
            title: None,
            width: None,
            unnamed: None,
        })),
    );
    state.next_id = 3;
    state.layouts = vec![Layout {
        id: 0,
        name: "Plan".to_owned(),
        size: [420.0, 297.0],
        unit: PaperUnit::Millimetres,
        details: vec![Detail {
            frame: 0,
            projection: "Top".to_owned(),
            target: [0.0; 3],
            distance: 100.0,
            scale: Some(0.02),
            display: "Wireframe".to_owned(),
            locked: false,
            hidden_layers: Vec::new(),
        }],
        user_text: Default::default(),
        revisions: Vec::new(),
    }];
    state.next_layout = 1;
    let back = round_trip(&state);
    let on_sheet: Vec<_> = back.objects.iter().filter(|o| o.space == Some(0)).collect();
    let texts: Vec<String> = on_sheet
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Text(t)) => Some(t.content.clone()),
            _ => None,
        })
        .collect();
    // The bar's numbers and scale, the index's headings and its one sheet.
    for wanted in ["0", "2 m", "1:50", "No.", "Sheet", "1", "Plan"] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted} in {texts:?}");
    }
    let solid = on_sheet
        .iter()
        .filter(
            |o| matches!(&o.shape, Shape::Annotation(Annotation::Hatch(h)) if h.pattern == "Solid"),
        )
        .count();
    assert!(solid >= 2, "the bar's filled blocks");
    assert!(
        on_sheet
            .iter()
            .any(|o| matches!(o.shape, Shape::Curve(_)) && o.group.is_some())
    );
}

#[test]
fn fields_rhino_works_out_stay_fields_and_their_text_comes_along() {
    use curva_document::annotation::{Annotation, Text, TextAlign};
    let mut state = State::default();
    state
        .document_text
        .insert("Project".to_owned(), "Harbour Library".to_owned());
    state
        .document_text
        .insert("Client".to_owned(), "City of Porto".to_owned());
    state.notes = "Issued for planning\nSecond line".to_owned();
    let note = Text {
        content: "%<DocumentText(\"Project\")>% / %<PageName()>% / %<Notes()>%".to_owned(),
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        height: 2.5,
        font: None,
        align: TextAlign::Left,
        style: 0,
    };
    state
        .objects
        .push(object(0, Shape::Annotation(Annotation::Text(note)), 0).into());
    state.next_id = 1;
    let back = round_trip(&state);
    let Shape::Annotation(Annotation::Text(t)) = &back.objects[0].shape else {
        panic!("not a note");
    };
    // Rhino works these out itself from the file's document text and notes.
    assert_eq!(
        t.content,
        "%<DocumentText(\"Project\")>% / %<PageName()>% / %<Notes()>%"
    );
    assert_eq!(back.document_text, state.document_text);
    assert_eq!(back.notes, state.notes);
}

#[test]
fn layout_user_text_and_its_fields_come_back() {
    use curva_document::annotation::{Annotation, Text, TextAlign};
    use curva_document::layouts::{Layout, PaperUnit};
    let mut state = State::default();
    for id in [0, 1] {
        state.layouts.push(Layout {
            id,
            name: format!("Sheet {id}"),
            size: [420.0, 297.0],
            unit: PaperUnit::Millimetres,
            details: Vec::new(),
            user_text: [("Revision".to_owned(), format!("R{id}"))].into(),
            revisions: Vec::new(),
        });
    }
    state.next_layout = 2;
    let note = |content: &str| Text {
        content: content.to_owned(),
        origin: [10.0, 10.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        height: 2.5,
        font: None,
        align: TextAlign::Left,
        style: 0,
    };
    let mut on_first = object(
        0,
        Shape::Annotation(Annotation::Text(note(
            "%<LayoutUserText(\"Revision\")>% of %<LayoutUserText(\"1\",\"Revision\")>%",
        ))),
        0,
    );
    on_first.space = Some(0);
    state.objects.push(on_first.into());
    state.next_id = 1;
    let back = round_trip(&state);
    assert_eq!(back.layouts.len(), 2);
    for (sheet, was) in back.layouts.iter().zip(&state.layouts) {
        assert_eq!(sheet.user_text, was.user_text);
    }
    let o = &back.objects[0];
    let Shape::Annotation(Annotation::Text(t)) = &o.shape else {
        panic!("not a note");
    };
    let second = back.layouts[1].id;
    assert_eq!(
        t.content,
        format!(
            "%<LayoutUserText(\"Revision\")>% of %<LayoutUserText(\"{second}\",\"Revision\")>%"
        )
    );
    let shown = curva_document::fields::resolve(
        &t.content,
        &back,
        o.space,
        &curva_document::fields::FieldContext::default(),
    );
    assert_eq!(shown, "R0 of R1");
}

#[test]
fn fields_measuring_an_object_go_out_naming_it_as_rhino_does_and_come_back() {
    use curva_document::annotation::{Annotation, Leader, Text, TextAlign};
    let mut state = State::default();
    let mut square = object(
        3,
        Shape::Curve(
            Curve::polyline(
                vec![
                    [0.0; 3],
                    [20.0, 0.0, 0.0],
                    [20.0, 20.0, 0.0],
                    [0.0, 20.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        ),
        0,
    );
    square.name = "Room".to_owned();
    let note = Text {
        content: "Edge %<CurveLength(\"3\")>% m, twice %<CurveLength(\"3\")*2>%".to_owned(),
        origin: [30.0, 0.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        height: 2.5,
        font: None,
        align: TextAlign::Left,
        style: 0,
    };
    let leader = Leader {
        points: vec![[10.0, 10.0, 0.0], [30.0, 30.0, 0.0]],
        content: "%<Area(\"3\")>%".to_owned(),
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        landing: false,
        style: 0,
    };
    // The notes come before the curve they measure, as a file may hold them.
    state
        .objects
        .push(object(1, Shape::Annotation(Annotation::Text(note)), 0).into());
    state
        .objects
        .push(object(2, Shape::Annotation(Annotation::Leader(leader)), 0).into());
    state.objects.push(square.into());
    state.next_id = 4;

    // Going out, the fields name the curve by its Rhino id.
    let (model, _) = curva_3dm::export::export(&state, None);
    let room = model
        .objects
        .iter()
        .find(|o| o.attributes.name == "Room")
        .unwrap()
        .attributes
        .id
        .text();
    let words: Vec<String> = model
        .objects
        .iter()
        .filter_map(|o| match &o.geometry {
            curva_3dm::model::Geometry::Text(t) => Some(t.rich.clone()),
            curva_3dm::model::Geometry::Leader(l) => Some(l.rich.clone()),
            _ => None,
        })
        .collect();
    let edge = format!("%<CurveLength(\"{room}\")>% m, twice %<CurveLength(\"{room}\")*2>%");
    assert!(words.iter().any(|w| w.contains(&edge)), "{words:?}");
    let area = format!("%<Area(\"{room}\")>%");
    assert!(words.iter().any(|w| w.contains(&area)), "{words:?}");

    // Coming back, they name it by Curva's number again, whatever it is now.
    let back = round_trip(&state);
    let room = back.objects.iter().find(|o| o.name == "Room").unwrap().id;
    let contents: Vec<String> = back
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Text(t)) => Some(t.content.clone()),
            Shape::Annotation(Annotation::Leader(l)) => Some(l.content.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        contents,
        vec![
            format!("Edge %<CurveLength(\"{room}\")>% m, twice %<CurveLength(\"{room}\")*2>%"),
            format!("%<Area(\"{room}\")>%"),
        ]
    );
}

#[test]
fn fields_only_curva_can_work_out_go_out_worked_out() {
    use curva_document::annotation::{Annotation, Text, TextAlign};
    let mut state = State::default();
    let mut wall = object(
        0,
        Shape::Curve(Curve::line([0.0; 3], [5000.0, 0.0, 0.0]).unwrap()),
        0,
    );
    wall.name = "North wall".to_owned();
    state.objects.push(wall.into());
    let note = Text {
        // An object named by Curva's number, which Rhino cannot follow, beside Rhino's own field.
        content: "%<ObjectName(\"0\")>% / %<PageName()>%".to_owned(),
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        height: 2.5,
        font: None,
        align: TextAlign::Left,
        style: 0,
    };
    state
        .objects
        .push(object(1, Shape::Annotation(Annotation::Text(note)), 0).into());
    state.next_id = 2;
    let back = round_trip(&state);
    let Shape::Annotation(Annotation::Text(t)) = &back.objects[1].shape else {
        panic!("not a note");
    };
    assert_eq!(t.content, "North wall / %<PageName()>%");
}

#[test]
fn linetypes_come_back_on_layers_and_objects() {
    let mut state = State {
        layers: vec![
            Layer {
                linetype: Linetype::Hidden,
                ..layer(0, "Hidden lines", [0.5, 0.5, 0.5])
            },
            Layer {
                linetype: Linetype::Center,
                ..layer(1, "Axes", [0.0, 0.0, 1.0])
            },
            layer(2, "Plain", [0.0, 0.0, 0.0]),
        ],
        ..State::default()
    };
    let line = |y: f64| Shape::Curve(Curve::line([0.0, y, 0.0], [100.0, y, 0.0]).unwrap());
    state.objects.push(object(0, line(0.0), 0).into());
    state.objects.push(
        Object {
            linetype: Linetype::Border,
            ..object(1, line(10.0), 1)
        }
        .into(),
    );
    state.objects.push(
        Object {
            linetype: Linetype::Continuous,
            ..object(2, line(20.0), 0)
        }
        .into(),
    );
    state.next_id = 3;
    state.next_layer = 3;
    let (model, _) = curva_3dm::export::export(&state, None);
    let names: Vec<&str> = model.linetypes.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Hidden", "Center", "Border"], "only the ones used");
    assert_eq!(
        model.linetypes[0].segments,
        [(1.5, true), (1.0, false)],
        "Curva's pattern"
    );
    let back = round_trip(&state);
    let by_name = |n: &str| back.layers.iter().find(|l| l.name == n).unwrap().linetype;
    assert_eq!(by_name("Hidden lines"), Linetype::Hidden);
    assert_eq!(by_name("Axes"), Linetype::Center);
    assert_eq!(by_name("Plain"), Linetype::Continuous);
    let kinds: Vec<Linetype> = back.objects.iter().map(|o| o.linetype).collect();
    assert_eq!(
        kinds,
        [Linetype::ByLayer, Linetype::Border, Linetype::Continuous]
    );
}

#[test]
fn object_colours_and_the_documents_own_linetypes_come_back() {
    let mut state = State {
        layers: vec![layer(0, "Walls", [0.5, 0.5, 0.5])],
        ..State::default()
    };
    let wall = state.add_linetype("Wall dash", &[8.0, 2.0, 1.0, 2.0]);
    state.layers[0].linetype = wall;
    let line = |y: f64| Shape::Curve(Curve::line([0.0, y, 0.0], [100.0, y, 0.0]).unwrap());
    state.objects.push(
        Object {
            colour: Some([1.0, 0.0, 0.0]),
            ..object(0, line(0.0), 0)
        }
        .into(),
    );
    state.objects.push(
        Object {
            linetype: wall,
            ..object(1, line(10.0), 0)
        }
        .into(),
    );
    state.next_id = 2;
    state.next_layer = 1;
    let back = round_trip(&state);
    assert_eq!(back.objects[0].colour, Some([1.0, 0.0, 0.0]));
    assert_eq!(back.objects[1].colour, None, "by layer");
    let wall_back = back.linetype_named("Wall dash").expect("the linetype");
    assert_eq!(back.linetype_pattern(wall_back), [8.0, 2.0, 1.0, 2.0]);
    assert_eq!(back.layers[0].linetype, wall_back);
    assert_eq!(back.objects[1].linetype, wall_back);
}

#[test]
fn annotation_styles_go_out_and_come_back_with_their_looks() {
    use curva_document::annotation::{
        Annotation, AnnotationStyle, Dimension, DimensionKind, Text, TextAlign, standard_styles,
    };
    let mut state = State::default();
    let mut styles = standard_styles(false);
    styles.push(AnnotationStyle {
        id: 40,
        name: "Plans".to_owned(),
        font: Some("Arial".to_owned()),
        text_height: 3.5,
        imperial: false,
        arrow_size: Some(2.0),
        extension_offset: Some(1.0),
        extension_extension: Some(1.5),
        text_gap: Some(0.8),
        linear_resolution: Some(1),
    });
    state.annotation_styles = styles;
    let dimension = Dimension {
        kind: DimensionKind::Horizontal,
        first: [0.0; 3],
        second: [5000.0, 0.0, 0.0],
        line: [0.0, 800.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        style: 40,
        text: None,
    };
    let note = |height: f64, align| Text {
        content: "Kitchen".to_owned(),
        origin: [0.0, 2000.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        height,
        font: None,
        align,
        style: 40,
    };
    let shapes = [
        Shape::Annotation(Annotation::Dimension(dimension)),
        Shape::Annotation(Annotation::Text(note(3.5, TextAlign::Left))),
        Shape::Annotation(Annotation::Text(note(7.0, TextAlign::Centre))),
    ];
    for (i, shape) in shapes.into_iter().enumerate() {
        state.objects.push(object(i as u64, shape, 0).into());
    }
    state.next_id = 3;
    let (model, _) = curva_3dm::export::export(&state, None);
    let plans = model
        .dim_styles
        .iter()
        .find(|s| s.name == "Plans")
        .expect("the style under its own name");
    assert_eq!(plans.font.as_deref(), Some("Arial"));
    assert_eq!(
        (
            plans.arrow_size,
            plans.extension_offset,
            plans.extension_extension
        ),
        (2.0, 1.0, 1.5)
    );
    assert_eq!((plans.text_gap, plans.length_resolution), (0.8, 1));
    // The note of another height and alignment overrides them in the style, as Rhino's notes do,
    // rather than getting a style of its own.
    assert_eq!(
        model
            .dim_styles
            .iter()
            .filter(|s| s.name.starts_with("Plans"))
            .count(),
        1
    );
    let owns: Vec<Option<(f64, u32)>> = model
        .objects
        .iter()
        .filter_map(|o| match &o.geometry {
            curva_3dm::model::Geometry::Text(t) => {
                assert_eq!(t.style, plans.id);
                Some(t.own.as_ref().map(|s| (s.text_height, s.h_align)))
            }
            _ => None,
        })
        .collect();
    assert_eq!(owns, [None, Some((7.0, 1))]);
    let back = round_trip(&state);
    let held: Vec<&AnnotationStyle> = back
        .annotation_styles
        .iter()
        .filter(|s| s.name == "Plans")
        .collect();
    assert_eq!(
        held.len(),
        1,
        "one style, the note's own folded back into it"
    );
    let style = held[0];
    assert_eq!(style.font.as_deref(), Some("Arial"));
    assert_eq!(style.text_height, 3.5);
    assert_eq!(style.arrow_size, Some(2.0));
    assert_eq!(style.extension_offset, Some(1.0));
    assert_eq!(style.extension_extension, Some(1.5));
    assert_eq!(style.text_gap, Some(0.8));
    assert_eq!(style.linear_resolution, Some(1));
    for o in &back.objects {
        let Shape::Annotation(a) = &o.shape else {
            panic!("annotation");
        };
        assert_eq!(a.style(), style.id, "everything names the style");
    }
}

#[test]
fn details_keep_their_display_mode_and_lock() {
    use curva_document::layouts::{Detail, Layout, PaperUnit};
    let mut state = State::default();
    let frame = |x: f64| {
        Curve::polyline(
            vec![
                [x, 20.0, 0.0],
                [x + 150.0, 20.0, 0.0],
                [x + 150.0, 170.0, 0.0],
                [x, 170.0, 0.0],
            ],
            true,
        )
        .unwrap()
    };
    for (id, x) in [(0, 20.0), (1, 200.0)] {
        let mut o = object(id, Shape::Curve(frame(x)), 0);
        o.space = Some(0);
        state.objects.push(o.into());
    }
    state.next_id = 2;
    let detail = |frame: u64, display: &str, locked: bool| Detail {
        frame,
        projection: "Top".to_owned(),
        target: [0.0; 3],
        distance: 100.0,
        scale: Some(0.01),
        display: display.to_owned(),
        locked,
        hidden_layers: Vec::new(),
    };
    state.layouts = vec![Layout {
        id: 0,
        name: "Plan".to_owned(),
        size: [420.0, 297.0],
        unit: PaperUnit::Millimetres,
        details: vec![detail(0, "Ghosted", true), detail(1, "Technical", false)],
        user_text: Default::default(),
        revisions: Vec::new(),
    }];
    state.next_layout = 1;
    let back = round_trip(&state);
    let mut got: Vec<(String, bool)> = back.layouts[0]
        .details
        .iter()
        .map(|d| (d.display.clone(), d.locked))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("Ghosted".to_owned(), true),
            ("Technical".to_owned(), false)
        ]
    );
}

#[test]
fn layers_turned_off_in_a_detail_stay_off_in_it() {
    use curva_document::layouts::{Detail, Layout, PaperUnit};
    let mut state = State {
        layers: vec![
            layer(0, "Default", curva_document::STANDARD_COLOUR),
            layer(1, "Furniture", [0.0, 0.5, 0.0]),
        ],
        next_layer: 2,
        ..State::default()
    };
    let frame = |x: f64| {
        Curve::polyline(
            vec![
                [x, 20.0, 0.0],
                [x + 150.0, 20.0, 0.0],
                [x + 150.0, 170.0, 0.0],
                [x, 170.0, 0.0],
            ],
            true,
        )
        .unwrap()
    };
    for (id, x) in [(0, 20.0), (1, 200.0)] {
        let mut o = object(id, Shape::Curve(frame(x)), 0);
        o.space = Some(0);
        state.objects.push(o.into());
    }
    state.objects.push(
        object(
            2,
            Shape::Curve(Curve::line([0.0; 3], [1000.0, 0.0, 0.0]).unwrap()),
            1,
        )
        .into(),
    );
    state.next_id = 3;
    let detail = |frame: u64, hidden: Vec<u64>| Detail {
        frame,
        projection: "Top".to_owned(),
        target: [0.0; 3],
        distance: 100.0,
        scale: Some(0.01),
        display: "Wireframe".to_owned(),
        locked: false,
        hidden_layers: hidden,
    };
    state.layouts = vec![Layout {
        id: 0,
        name: "Plan".to_owned(),
        size: [420.0, 297.0],
        unit: PaperUnit::Millimetres,
        details: vec![detail(0, vec![1]), detail(1, Vec::new())],
        user_text: Default::default(),
        revisions: Vec::new(),
    }];
    state.next_layout = 1;
    let back = round_trip(&state);
    let furniture = back
        .layers
        .iter()
        .find(|l| l.name == "Furniture")
        .unwrap()
        .id;
    let frame_x = |d: &Detail| {
        back.objects
            .iter()
            .find(|o| o.id == d.frame)
            .and_then(|o| o.shape.bounds())
            .map(|(low, _)| low[0])
            .unwrap()
    };
    let details = &back.layouts[0].details;
    let left = details.iter().find(|d| frame_x(d) < 100.0).unwrap();
    let right = details.iter().find(|d| frame_x(d) > 100.0).unwrap();
    assert_eq!(left.hidden_layers, [furniture], "off in the left detail");
    assert!(right.hidden_layers.is_empty(), "on in the right one");
}

#[test]
fn clipping_planes_come_back_cutting_the_same_side_in_the_same_views() {
    use curva_document::clipping::{ClipView, ClippingPlane, SectionFill};
    use curva_document::layouts::{Detail, Layout, PaperUnit};
    let mut state = State::default();
    // A rectangle at z = 1000 running counter-clockwise seen from above, flipped: it keeps
    // what is below it.
    let rectangle = Curve::polyline(
        vec![
            [0.0, 0.0, 1000.0],
            [4000.0, 0.0, 1000.0],
            [4000.0, 3000.0, 1000.0],
            [0.0, 3000.0, 1000.0],
        ],
        true,
    )
    .unwrap();
    state
        .objects
        .push(object(0, Shape::Curve(rectangle), 0).into());
    let mut detail_frame = object(
        1,
        Shape::Curve(
            Curve::polyline(
                vec![
                    [20.0, 20.0, 0.0],
                    [170.0, 20.0, 0.0],
                    [170.0, 170.0, 0.0],
                    [20.0, 170.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        ),
        0,
    );
    detail_frame.space = Some(0);
    state.objects.push(detail_frame.into());
    state.next_id = 2;
    state.layouts = vec![Layout {
        id: 0,
        name: "Plan".to_owned(),
        size: [420.0, 297.0],
        unit: PaperUnit::Millimetres,
        details: vec![Detail {
            frame: 1,
            projection: "Top".to_owned(),
            target: [0.0; 3],
            distance: 100.0,
            scale: Some(0.01),
            display: "Wireframe".to_owned(),
            locked: false,
            hidden_layers: Vec::new(),
        }],
        user_text: Default::default(),
        revisions: Vec::new(),
    }];
    state.next_layout = 1;
    state.clipping_planes = vec![ClippingPlane {
        frame: 0,
        flipped: true,
        views: vec![
            ClipView::Viewport(0),
            ClipView::Viewport(1),
            ClipView::Detail(1),
        ],
        section: SectionFill::default(),
        hatch: None,
    }];
    let back = round_trip(&state);
    assert_eq!(back.clipping_planes.len(), 1);
    let clip = &back.clipping_planes[0];
    let frame = back.objects.iter().find(|o| o.id == clip.frame).unwrap();
    let mut points = frame.curve().unwrap().sample().unwrap();
    points.pop();
    let mut normal_z = 0.0;
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        normal_z += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let keeps_below = (normal_z < 0.0) != clip.flipped;
    assert!(keeps_below, "still keeps what is below it");
    assert!(points.iter().all(|p| (p[2] - 1000.0).abs() < 1e-9));
    let detail = back.layouts[0].details[0].frame;
    let mut views = clip.views.clone();
    views.sort_by_key(|v| format!("{v:?}"));
    let mut wanted = vec![
        ClipView::Viewport(0),
        ClipView::Viewport(1),
        ClipView::Detail(detail),
    ];
    wanted.sort_by_key(|v| format!("{v:?}"));
    assert_eq!(views, wanted);
}

#[test]
fn ordinate_dimensions_come_back_measuring_the_same() {
    use curva_document::annotation::{Annotation, OrdinateAxis, OrdinateDimension};
    let mut state = State::default();
    let ordinate = |leader, axis, kinks| OrdinateDimension {
        base: [10.0, 10.0, 0.0],
        point: [40.0, 25.0, 0.0],
        leader,
        axis,
        kinks,
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        style: 0,
        text: None,
    };
    let shapes = [
        ordinate([45.0, 60.0, 0.0], OrdinateAxis::X, Some([2.0, 3.0])),
        ordinate([0.0, 20.0, 0.0], OrdinateAxis::Y, None),
    ];
    for (i, d) in shapes.iter().enumerate() {
        state.objects.push(
            object(
                i as u64,
                Shape::Annotation(Annotation::Ordinate(d.clone())),
                0,
            )
            .into(),
        );
    }
    state.next_id = 2;
    let back = round_trip(&state);
    let got: Vec<OrdinateDimension> = back
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Ordinate(d)) => Some(d.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(got.len(), 2);
    for (a, w) in got.iter().zip(&shapes) {
        assert_eq!(a.axis, w.axis);
        assert_eq!(a.kinks, w.kinks, "unset kinks stay unset");
        assert!((a.value() - w.value()).abs() < 1e-9);
        assert_eq!(a.kink_points(1.0), w.kink_points(1.0));
    }
}

#[test]
fn section_styles_come_back_on_objects_layers_and_clipping_planes() {
    use curva_document::clipping::{ClipView, ClippingPlane, SectionFill};
    use curva_document::section::{SectionHatch, SectionStyle};
    let mut state = State {
        layers: vec![layer(0, "Walls", [0.5, 0.5, 0.5])],
        ..State::default()
    };
    state.layers[0].section = Some(SectionStyle {
        fill: SectionFill::Solid([0.0, 0.0, 1.0]),
        hatch: Some(SectionHatch::new("Hatch1")),
    });
    let own = SectionStyle {
        fill: SectionFill::Solid([1.0, 0.0, 0.0]),
        hatch: Some(SectionHatch {
            pattern: "Grid".to_owned(),
            scale: 2.0,
            rotation: 0.5,
        }),
    };
    let mut wall = object(
        0,
        Shape::Body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap()),
        0,
    );
    wall.section = Some(own.clone());
    state.objects.push(wall.into());
    // A second solid with no style of its own: it takes its layer's.
    state.objects.push(
        object(
            1,
            Shape::Body(Body::cuboid([20.0, 0.0, 0.0], [30.0, 10.0, 10.0]).unwrap()),
            0,
        )
        .into(),
    );
    let rectangle = Curve::polyline(
        vec![
            [-5.0, -5.0, 5.0],
            [35.0, -5.0, 5.0],
            [35.0, 15.0, 5.0],
            [-5.0, 15.0, 5.0],
        ],
        true,
    )
    .unwrap();
    state
        .objects
        .push(object(2, Shape::Curve(rectangle), 0).into());
    state.next_id = 3;
    state.clipping_planes = vec![ClippingPlane {
        frame: 2,
        flipped: false,
        views: vec![ClipView::Viewport(0)],
        section: SectionFill::None,
        hatch: Some(SectionHatch::new("Plus")),
    }];
    let back = round_trip(&state);
    let body = |x: f64| {
        back.objects
            .iter()
            .find(|o| {
                o.shape.body().is_some()
                    && o.shape
                        .bounds()
                        .is_some_and(|(lo, _)| (lo[0] - x).abs() < 1e-6)
            })
            .unwrap()
    };
    assert_eq!(body(0.0).section, Some(own));
    assert_eq!(body(20.0).section, None, "its layer's, not its own");
    let walls = back.layers.iter().find(|l| l.name == "Walls").unwrap();
    assert_eq!(
        walls.section,
        Some(SectionStyle {
            fill: SectionFill::Solid([0.0, 0.0, 1.0]),
            hatch: Some(SectionHatch::new("Hatch1")),
        })
    );
    let clip = &back.clipping_planes[0];
    assert_eq!(clip.section, SectionFill::None);
    assert_eq!(clip.hatch, Some(SectionHatch::new("Plus")));
    let frame = back.objects.iter().find(|o| o.id == clip.frame).unwrap();
    assert_eq!(frame.section, None, "the plane's style is the plane's");
}
