//! Times what one undo step costs on a large model: copying the whole state, as
//! `Document::record_change` does, saving it, and what a hundred undo steps hold, as
//! `docs/performance.md` records them.
//!
//! Run with `cargo run --release -p curva-document --example undo_costs`.

use curva_document::Document;
use curva_kernel::{Body, Curve};
use std::time::Instant;

fn main() {
    let mut doc = Document::default();
    for i in 0..5000 {
        let x = f64::from(i % 100) * 10.0;
        let y = f64::from(i / 100) * 10.0;
        doc.add(Curve::circle([x, y, 0.0], 3.0).expect("a circle"));
    }
    let solid = Body::cuboid([0.0; 3], [4.0, 4.0, 4.0])
        .and_then(|b| b.blended(&(0..12).collect::<Vec<_>>(), 0.5, false))
        .expect("a rounded box");
    for i in 0..200 {
        doc.add_body(solid.translated([f64::from(i) * 6.0, -20.0, 0.0]));
    }
    println!("objects: {}", doc.state().objects.len());
    let rounds = 50;
    let start = Instant::now();
    let mut kept = Vec::new();
    for _ in 0..rounds {
        kept.push(doc.state().clone());
    }
    let each = start.elapsed().as_secs_f64() * 1000.0 / f64::from(rounds);
    println!("copy the state (one undo step): {each:.3} ms");
    let start = Instant::now();
    for copy in &kept {
        assert!(std::hint::black_box(copy) == doc.state());
    }
    let each = start.elapsed().as_secs_f64() * 1000.0 / f64::from(rounds);
    println!("compare the state with a copy (every frame): {each:.3} ms");
    let start = Instant::now();
    let json = serde_json::to_vec(doc.state()).expect("saved");
    println!(
        "write the state as JSON: {:.1} ms, {:.1} MB",
        start.elapsed().as_secs_f64() * 1000.0,
        json.len() as f64 / 1e6
    );
    std::hint::black_box(kept);
    // A hundred edits, each moving one object, as undo steps.
    let ids: Vec<u64> = doc.state().objects.iter().map(|o| o.id).collect();
    let start = Instant::now();
    for (step, id) in ids.iter().take(100).enumerate() {
        let before = doc.state().clone();
        doc.translate(&[*id], [0.0, 0.0, 1.0 + step as f64]);
        doc.record_change(before);
    }
    let each = start.elapsed().as_secs_f64() * 1000.0 / 100.0;
    let (steps, bytes) = doc.undo_held();
    println!(
        "100 edits: {each:.3} ms each; undo holds {steps} steps, about {:.1} MB",
        bytes as f64 / 1e6
    );
}
