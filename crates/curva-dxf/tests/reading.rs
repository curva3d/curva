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
