use curva_kernel::{Body, Curve};
fn main() {
    let rim = Curve::circle([0.0; 3], 180.0).unwrap();
    let trunk = Body::extruded(&[rim], [0.0, 0.0, 1.0], 3000.0, true).unwrap();
    for (name, body) in [("at origin", trunk.clone()), ("moved", trunk.translated([-1800.0, -1800.0, -300.0]))] {
        let pieces = body.section([0.0, 0.0, 1200.0], [0.0, 0.0, 1.0]).unwrap();
        for piece in &pieces {
            let p = piece.sample().unwrap();
            let lo = p.iter().fold([f64::MAX; 2], |a, q| [a[0].min(q[0]), a[1].min(q[1])]);
            let hi = p.iter().fold([f64::MIN; 2], |a, q| [a[0].max(q[0]), a[1].max(q[1])]);
            println!("{name}: piece of {} points, x {:.0}..{:.0}, y {:.0}..{:.0}, z {:.0}", p.len(), lo[0], hi[0], lo[1], hi[1], p[0][2]);
        }
    }
}
