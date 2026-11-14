//! Times what the app does each frame and after a change on a large model: telling whether the
//! model changed, and drawing it again, as `docs/performance.md` records them.
//!
//! Run with `cargo run --release -p curva-viewport --example redraw_costs`.

use curva_document::Document;
use curva_kernel::{Body, Curve};
use curva_viewport::{MeshCache, Outlines};
use std::time::Instant;

/// Average milliseconds `work` takes over `rounds` calls.
fn timed(rounds: u32, mut work: impl FnMut()) -> f64 {
    let start = Instant::now();
    for _ in 0..rounds {
        work();
    }
    start.elapsed().as_secs_f64() * 1000.0 / f64::from(rounds)
}

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
    let drawn = doc.state().clone();
    println!("objects: {}", drawn.objects.len());
    let equal = timed(50, || assert!(std::hint::black_box(&drawn) == doc.state()));
    let same = timed(50, || assert!(std::hint::black_box(&drawn).same_as(doc.state())));
    println!("tell nothing changed (every frame): compare {equal:.3} ms, same_as {same:.3} ms");
    let mut cache = MeshCache::default();
    let _ = Outlines::of_cached(doc.state(), &mut cache);
    let all = timed(5, || {
        std::hint::black_box(Outlines::of(doc.state()));
    });
    let first = doc.state().objects[0].id;
    let mut step = 0.0;
    let one = timed(20, || {
        step += 1.0;
        doc.translate(&[first], [0.0, 0.0, step]);
        std::hint::black_box(Outlines::of_cached(doc.state(), &mut cache));
    });
    println!("draw again after moving one object: everything {all:.1} ms, reusing {one:.1} ms");
}
