//! Reading drawings written by other programs (spec `drawing-output.md` §6).

use curva_document::Document;

/// A drawing in the test files, read into a new model.
fn open(name: &str) -> Document {
    let path = format!("{}/tests/files/{name}", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(path).unwrap();
    let cad = curva_dxf::read(&data, curva_dxf::Format::Dxf).unwrap();
    let mut doc = Document::default();
    curva_dxf::import(&cad, &mut doc);
    doc
}

#[test]
fn a_layout_rhino_wrote_comes_in_with_its_detail() {
    // Rhino 8's DXF of an A3 layout "Plan" with a 1:50 top detail from (20, 20) to (220, 170)
    // looking at (5000, 0), a rectangle on the sheet, and a 10 m line in the model.
    let doc = open("rhino8_layout.dxf");
    let layouts = doc.layouts();
    let plan = layouts
        .iter()
        .find(|l| l.name == "Plan")
        .expect("the layout");
    let detail = plan.details.first().expect("its detail");
    assert_eq!(detail.projection, "Top");
    // Its scale is left aside: Rhino writes the view height as the paper height divided by the
    // scale (3 for this 150 mm high 1:50 detail), where AutoCAD's DXF reference has the model
    // height it shows (7500), which is what Curva writes and reads.
    assert!(detail.scale.is_some());
    assert_eq!(plan.size, [420.0, 297.0], "turned by its plot rotation");
    assert!(
        (detail.target[0] - 5000.0).abs() < 1e-6,
        "{:?}",
        detail.target
    );
    let state = doc.state();
    let frame = state.objects.iter().find(|o| o.id == detail.frame).unwrap();
    let (low, high) = frame.shape.bounds().unwrap();
    assert!(
        (low[0] - 20.0).abs() < 1e-6 && (high[1] - 170.0).abs() < 1e-6,
        "{low:?} {high:?}"
    );
    assert!(
        state
            .objects
            .iter()
            .any(|o| o.space == Some(plan.id) && o.id != detail.frame),
        "the sheet's rectangle"
    );
    assert_eq!(
        state.objects.iter().filter(|o| o.space.is_none()).count(),
        1
    );
}

#[test]
fn rhinos_linetypes_come_in_on_layers_and_lines() {
    // Rhino 8's DXF, in metres: layer "Hidden lines" (Hidden) and "Axes" (Center); a line by
    // layer at y = 0, one with Border at y = 0.01, one with a custom "Wall dash" (8 on, 2 off,
    // 1 on, 2 off) at y = 30 and one with Dots at y = 40.
    use curva_document::Linetype;
    let path = format!(
        "{}/tests/files/rhino8_linetypes.dxf",
        env!("CARGO_MANIFEST_DIR")
    );
    let cad = curva_dxf::read(&std::fs::read(path).unwrap(), curva_dxf::Format::Dxf).unwrap();
    let mut doc = Document::default();
    let report = curva_dxf::import(&cad, &mut doc);
    let state = doc.state();
    let layer = |n: &str| state.layers.iter().find(|l| l.name == n).unwrap().linetype;
    assert_eq!(layer("Hidden lines"), Linetype::Hidden);
    assert_eq!(layer("Axes"), Linetype::Center);
    let at = |y: f64| {
        state
            .objects
            .iter()
            .find(|o| {
                o.curve()
                    .and_then(|c| c.sample().ok())
                    .is_some_and(|s| (s[0][1] - y).abs() < 1e-6)
            })
            .unwrap()
            .linetype
    };
    assert_eq!(at(0.0), Linetype::ByLayer);
    assert_eq!(at(0.01), Linetype::Border);
    let wall = state
        .linetype_named("Wall dash")
        .expect("the document's own");
    assert_eq!(at(30.0), wall);
    assert_eq!(state.linetype_pattern(wall), [8.0, 2.0, 1.0, 2.0]);
    assert_eq!(at(40.0), Linetype::Dotted);
    assert!(
        !report.skipped.iter().any(|(k, _)| k.contains("linetype")),
        "{:?}",
        report.skipped
    );
}

#[test]
fn ellipses_rhino_wrote_as_splines_stay_exact_ellipses() {
    // Rhino 8 writes its ellipses as rational splines: a whole one centred at (1, 2, 0) with radii
    // 6 along Y and 2, and part of one on the XZ plane with radii 4 and 1.5 from 0.7 to 2.7.
    let doc = open("rhino8_ellipses.dxf");
    let state = doc.state();
    let found: Vec<_> = state
        .objects
        .iter()
        .filter_map(|o| o.curve()?.as_ellipse())
        .collect();
    assert_eq!(found.len(), 2, "both read as ellipses");
    let length = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    let whole = found.iter().find(|e| near(length(e.major), 6.0)).unwrap();
    assert!(near(length(whole.minor), 2.0) && near(whole.sweep, std::f64::consts::TAU));
    assert!(near(whole.centre[0], 1.0) && near(whole.centre[1], 2.0));
    let part = found.iter().find(|e| near(length(e.major), 4.0)).unwrap();
    assert!(near(length(part.minor), 1.5));
    assert!(near(part.sweep, 2.0), "{part:?}");
}
