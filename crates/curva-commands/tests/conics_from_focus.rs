//! Parabola and Hyperbola from the focus, as Rhino 8 makes them (#707, measured by the testing
//! stream).

use curva_commands::Processor;
use curva_document::Document;

fn curve_of(steps: &[&str]) -> curva_kernel::Curve {
    let mut doc = Document::default();
    let mut processor = Processor::default();
    for step in steps {
        processor.submit(&mut doc, step);
    }
    doc.state().objects[0].curve().unwrap().clone()
}

fn lowest_and_highest(curve: &curva_kernel::Curve) -> (f64, f64) {
    let points = curve.sample().unwrap();
    let ys = points.iter().map(|p| p[1]);
    (
        ys.clone().fold(f64::MAX, f64::min),
        ys.fold(f64::MIN, f64::max),
    )
}

#[test]
fn a_parabola_from_its_focus_opens_as_rhinos() {
    let parabola = curve_of(&["parabola", "0,0,0", "0,5,0", "10,0,0"]);
    assert!((parabola.length().unwrap() - 22.9559).abs() < 1e-3);
    let (low, high) = lowest_and_highest(&parabola);
    assert!(
        (low + 5.0).abs() < 1e-6 && high.abs() < 1e-6,
        "{low} {high}"
    );
}

#[test]
fn a_hyperbola_from_centre_focus_and_end_is_rhinos() {
    let hyperbola = curve_of(&["hyperbola", "0,0,0", "5,0,0", "10,3,0"]);
    assert!((hyperbola.length().unwrap() - 12.4422).abs() < 1e-3);
    let nearest = hyperbola
        .sample()
        .unwrap()
        .iter()
        .map(|p| p[0])
        .fold(f64::MAX, f64::min);
    assert!((nearest - 4.733).abs() < 1e-3, "{nearest}");
}
