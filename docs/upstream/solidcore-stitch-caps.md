# Draft issue for solidcore: capping an open tube with stitch gives a closed shell of volume 0

**Not posted yet.** Waiting for the owner's go-ahead (spec `docs/specs/m1-closing.md`, step 6).
To be posted at https://github.com/tangency-labs/solidcore/issues. Curva's own record: curva3d/curva#67.

---

**Title:** `stitch` closes a sheet and its caps but leaves the enclosed space empty (volume 0)

**Revision:** `c440e5f892452a9654aee8f3030de6095fdb1415`

**What happens**

An open square tube, from `extrude` with `cap = false`, gets a flat face for each open end from
`planar_faces`. The three are merged with `union_all` and sewn with `stitch`. `stitch` reports the
result closed (`true`), and it has the faces, edges and corners of a box (6, 12, 8). But its volume is
0: the space inside is still marked empty. The same happens with every face flat, so curved faces
are not the cause.

**Smallest case**

```rust
use solidcore_ops::boolean::union_all;
use solidcore_ops::stitch::stitch;
use solidcore_ops::sweep::{Curve, DraftCorner, extrude, planar_faces};

let square = |z: f64| -> Vec<Curve> {
    let p = [[0.0, 0.0, z], [1.0, 0.0, z], [1.0, 1.0, z], [0.0, 1.0, z]];
    (0..4).map(|i| Curve::Line { start: p[i], end: p[(i + 1) % 4] }).collect()
};
let tube = extrude(&square(0.0), [0.0, 0.0, 1.0], 1.0, 0.0, DraftCorner::Curvature, false).unwrap();
let bottom = planar_faces(&square(0.0)).unwrap();
let top = planar_faces(&square(1.0)).unwrap();
let merged = union_all(vec![tube, bottom, top]).unwrap();
let (sewn, closed) = stitch(merged, 1e-6).unwrap();
assert!(closed);                                  // true
assert_eq!(sewn.counts().unwrap(), [6, 12, 8]);   // as a box
assert!(sewn.volume(1e-6).unwrap() > 0.99);       // fails: 0.0
```

(`make_box([0.0; 3], [1.0; 3])` gives the same counts and a volume of 1.)

**Expected**

When `stitch` closes a shell, the space it encloses becomes material, so the result is a solid, as
Rhino's and SMLib's Cap produce.

**What we tried**
1. Every open loop in one `planar_faces` call: refused (`Argument`), as the two ends are in different
   planes.
2. One `planar_faces` call per loop, then `union_all` and `stitch`: closed, but the volume is 0 (the
   case above).
3. Marking the enclosed region as material in the record after sewing: writing the record is refused
   ("a face between two material regions or two empty ones").

**Guess**

`StitchFaces` sets the void flags by nesting. When the closed shell comes from a sheet plus loose
faces, nothing marks the enclosed space as material. The `close_onto_shared_edges` path does mark it
("a cap grows the solid"), but it is only taken for edges left lamina after the first pass.

**What would help**

A way to mark a closed sewn shell as solid, or a Cap operation that closes open planar loops directly,
as SMLib's own does.
