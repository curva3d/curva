//! Three-point Circle, Arc, Rectangle and Box draw in the plane of their points, whatever the
//! construction plane, with the results Rhino 8 gives (#695, measured by the testing stream).

use curva_commands::Processor;
use curva_document::Document;

fn made(steps: &[&str]) -> Document {
    let mut doc = Document::default();
    let mut processor = Processor::default();
    for step in steps {
        processor.submit(&mut doc, step);
    }
    doc
}

fn curve_length(doc: &Document) -> f64 {
    doc.state().objects[0].curve().unwrap().length().unwrap()
}

#[test]
fn upright_points_make_upright_shapes_as_in_rhino() {
    let circle = made(&["circle", "3point", "0,0,0", "10,0,0", "5,0,2"]);
    assert!((curve_length(&circle) - 45.5531).abs() < 1e-3);
    let arc = made(&["arc", "startpoint", "0,0,0", "10,0,0", "5,0,2"]);
    assert!((curve_length(&arc) - 11.0347).abs() < 1e-3);
    let rectangle = made(&["rectangle", "3point", "0,0,0", "10,0,0", "10,0,5"]);
    assert!((curve_length(&rectangle) - 30.0).abs() < 1e-6);
    let solid = made(&["box", "3point", "0,0,0", "10,0,0", "10,0,5", "2"]);
    let volume = solid.state().objects[0]
        .shape
        .body()
        .unwrap()
        .volume()
        .unwrap();
    assert!((volume - 100.0).abs() < 1e-6, "{volume}");
}
