# Posted to solidcore: stitch closes a tube and its caps but leaves no solid region

Posted on 12 November 2026 as https://github.com/tangency-labs/solidcore/issues/3266, after the
owner approved it. Checked at solidcore main `2c92a4f` and at the revision Curva pins, `c440e5f`.
The report does not mention Curva. It blocks `Cap` (curva3d/curva#67).

The text as posted:

---

**Revision:** `2c92a4fceb1636c926f5120672ffb87d73c6ba11` (main, 12 November 2026). Also seen at `c440e5f`.

### What happens

An open square tube from `extrude` (`cap = false`) and a flat face for each open end from `planar_faces` are merged with `union_all` and sewn with `stitch`. `stitch` returns `closed = true`, and the result has the counts of a box (6 faces, 12 edges, 8 vertices). But `volume` is 0: no solid region encloses the shell. A box from `make_box` with the same corners has the same counts and volume 1.

Every face here is planar.

### Reproduction

```rust
use solidcore_ops::boolean::union_all;
use solidcore_ops::make_box;
use solidcore_ops::stitch::stitch;
use solidcore_ops::sweep::{Curve, DraftCorner, extrude, planar_faces};

fn main() {
    let square = |z: f64| -> Vec<Curve> {
        let p = [[0.0, 0.0, z], [1.0, 0.0, z], [1.0, 1.0, z], [0.0, 1.0, z]];
        (0..4).map(|i| Curve::Line { start: p[i], end: p[(i + 1) % 4] }).collect()
    };
    let tube = extrude(&square(0.0), [0.0, 0.0, 1.0], 1.0, 0.0, DraftCorner::Curvature, false).unwrap();
    let bottom = planar_faces(&square(0.0)).unwrap();
    let top = planar_faces(&square(1.0)).unwrap();
    let merged = union_all(vec![tube, bottom, top]).unwrap();
    let (sewn, closed) = stitch(merged, 1e-6).unwrap();
    println!("closed: {closed}");                     // true
    println!("counts: {:?}", sewn.counts());          // Ok([6, 12, 8])
    println!("volume: {:?}", sewn.volume(1e-6));      // Ok(0.0)
    let reference = make_box([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
    println!("{:?} {:?}", reference.counts(), reference.volume(1e-6)); // Ok([6, 12, 8]) Ok(0.9999999999999998)
}
```

### Expected

When `stitch` reports a shell closed, the space it encloses is a solid region, as for the box from `make_box`, so that open ends can be capped into a solid.

### Possibly related

#2506 (closed): a capped linear sweep that returned a closed shell with no solid region.
