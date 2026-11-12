# Draft issue for solidcore: a turned box cannot be read back from its own record

**Not posted yet.** Waiting for the owner's go-ahead (spec `docs/specs/m1-closing.md`, step 6).
To be posted at https://github.com/tangency-labs/solidcore/issues.

---

**Title:** `Body::from_record` refuses (status 1007) the record of a box turned about z, at some angles

**Revision:** `c440e5f892452a9654aee8f3030de6095fdb1415`

**What happens**

A box made with `make_box` away from the origin, turned about the z axis with `Body::transform`, gives
a record (`to_record`) that `Body::from_record` then refuses with `Status(1007)`. This happens with
`trusted` true or false. It depends on the angle: for a unit box at `(1, 2, 3)`, 10 of the whole-degree
angles from 1° to 89° fail (17°, 29°, 37°, 40°, 41°, 47°, 53°, 62° and two more). The same box at the
origin reads back at every angle.

**Smallest case**

```rust
use solidcore_ops::{Body, make_box};

let (c, s) = (37f64.to_radians().cos(), 37f64.to_radians().sin());
let body = make_box([1.0, 2.0, 3.0], [1.0, 1.0, 1.0]).unwrap();
body.transform(&[[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    .unwrap();
let (record, _) = body.to_record().unwrap();
assert!(Body::from_record(&record, true).is_ok()); // fails: Err(Status(1007))
```

**Expected**

A body's own record reads back, whatever rigid motion it has been through.

**Guess**

The turn leaves vertex positions and the planes or edge lines through them a rounding error apart, and
reading the record checks them more tightly than the kernel's own tolerance.

**How Curva works around it today**

Curva keeps each body as made, plus a placement it applies when needed, so it never reads back the
record of a moved body (`crates/curva-kernel/src/body.rs`).
