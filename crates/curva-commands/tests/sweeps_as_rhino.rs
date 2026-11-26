//! Sweep1 and Sweep2 against Rhino 8's results (#665, #666, #667, measured by the testing stream).

use curva_commands::Processor;
use curva_document::Document;

fn run(doc: &mut Document, steps: &[&str]) {
    let mut processor = Processor::default();
    for step in steps {
        processor.submit(doc, step);
    }
}

fn last_body_area(doc: &Document) -> f64 {
    doc.state()
        .objects
        .iter()
        .rev()
        .find_map(|o| o.shape.body())
        .unwrap()
        .area()
        .unwrap()
}

/// Rhino's rails for Sweep2: cubics through three points each.
fn rails(doc: &mut Document) -> (u64, u64) {
    run(doc, &["interpcrv", "0,0,0", "5,-1,0", "10,0,0", ""]);
    run(doc, &["interpcrv", "0,6,0", "5,8,0", "10,6,0", ""]);
    (doc.state().objects[0].id, doc.state().objects[1].id)
}

#[test]
fn sweep2_takes_two_sections_one_at_each_end() {
    let mut doc = Document::default();
    let (a, b) = rails(&mut doc);
    run(&mut doc, &["arc", "startpoint", "0,0,0", "0,6,0", "0,3,1"]);
    run(
        &mut doc,
        &["arc", "startpoint", "10,0,0", "10,6,0", "10,3,4"],
    );
    let (s1, s2) = (doc.state().objects[2].id, doc.state().objects[3].id);
    let mut processor = Processor::default();
    processor.submit(&mut doc, "sweep2");
    for picked in [vec![a], vec![b], vec![s1, s2]] {
        doc.select(&picked);
        processor.submit(&mut doc, "");
    }
    let area = last_body_area(&doc);
    // Rhino 8: 125.6062; Curva 125.89 (0.23% over), blending the two sections along Rhino's S.
    assert!((area - 125.6062).abs() < 0.003 * 125.6062, "{area}");
}

#[test]
fn sweep2_takes_a_section_in_the_middle() {
    let mut doc = Document::default();
    let (a, b) = rails(&mut doc);
    run(
        &mut doc,
        &["arc", "startpoint", "5,-1,0", "5,8,0", "5,3.5,3"],
    );
    let section = doc.state().objects[2].id;
    let mut processor = Processor::default();
    processor.submit(&mut doc, "sweep2");
    for picked in [vec![a], vec![b], vec![section]] {
        doc.select(&picked);
        processor.submit(&mut doc, "");
    }
    let area = last_body_area(&doc);
    // Rhino 8: 104.9645.
    assert!((area - 104.9645).abs() < 0.01 * 104.9645, "{area}");
}
