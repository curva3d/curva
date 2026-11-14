//! Times the questions asked of a body most often (whether it is solid, its volume, bounds,
//! mesh and edges) and the cost of copying one, as `docs/performance.md` records them.
//!
//! Run with `cargo run --release -p curva-kernel --example body_costs`.

use curva_kernel::Body;
use std::time::Instant;

/// Prints how long `work` took the first time, and on average over `rounds` more.
fn timed(label: &str, rounds: u32, mut work: impl FnMut()) {
    let start = Instant::now();
    work();
    let first = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    for _ in 0..rounds {
        work();
    }
    let each = start.elapsed().as_secs_f64() * 1000.0 / f64::from(rounds);
    println!("{label:<34} first {first:>9.3} ms, then {each:>9.3} ms");
}

fn main() {
    // A box with every edge rounded: curved faces and many edges, like real models.
    let plain = Body::cuboid([0.0; 3], [40.0, 30.0, 20.0]).expect("a box");
    let edges: Vec<usize> = (0..12).collect();
    let rounded = plain.blended(&edges, 3.0, false).expect("rounded edges");
    // Moved, so the placement has to be applied too.
    let body = rounded.translated([5.0, 5.0, 5.0]);
    println!("faces, edges, corners: {:?}", body.counts().expect("counts"));
    timed("is_solid", 50, || {
        assert!(body.is_solid());
    });
    timed("volume", 50, || {
        body.volume().expect("volume");
    });
    timed("bounds", 50, || {
        body.bounds().expect("bounds");
    });
    timed("mesh", 20, || {
        body.mesh().expect("mesh");
    });
    timed("edge_curves", 20, || {
        body.edge_curves().expect("edges");
    });
    timed("clone (as each undo step does)", 1000, || {
        std::hint::black_box(body.clone());
    });
    timed("compare with a clone", 1000, || {
        let other = body.clone();
        assert!(std::hint::black_box(&other) == &body);
    });
    timed("translate, then is_solid", 50, || {
        assert!(body.translated([1.0, 0.0, 0.0]).is_solid());
    });
}
