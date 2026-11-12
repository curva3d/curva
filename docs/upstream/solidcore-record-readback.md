# Posted to solidcore: a turned box cannot be read back from its own record

Posted on 12 November 2026 as https://github.com/tangency-labs/solidcore/issues/3265, after the
owner approved it. Checked at solidcore main `2c92a4f` and at the revision Curva pins, `c440e5f`.
The report does not mention Curva. Curva works around it by keeping each body as made plus one
placement (`crates/curva-kernel/src/body.rs`).

The text as posted:

---

**Revision:** `2c92a4fceb1636c926f5120672ffb87d73c6ba11` (main, 12 November 2026). Also seen at `c440e5f`.

### What happens

A box from `make_box`, away from the origin and turned about z with `Body::transform`, gives a record (`to_record`) that `Body::from_record` refuses with `Status(1007)` (`SM_ERR_INVALID_INPUT`), with `trusted` both `true` and `false`.

It depends on the angle. For a unit box at `(1, 2, 3)`, these whole-degree angles from 1 to 89 fail: 17, 29, 37, 40, 41, 47, 53, 62, 67, 69. All others read back.

The same turns about z, for a unit box and a 2 × 1 × 1 box with a corner at the origin, read back at every whole-degree angle from 1 to 89.

### Reproduction

```rust
use solidcore_ops::{Body, make_box};

fn main() {
    let (c, s) = (37f64.to_radians().cos(), 37f64.to_radians().sin());
    let body = make_box([1.0, 2.0, 3.0], [1.0, 1.0, 1.0]).unwrap();
    body.transform(&[[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
        .unwrap();
    let (record, _) = body.to_record().unwrap();
    println!("{:?}", Body::from_record(&record, true).map(|_| ()));  // Err(Status(1007))
    println!("{:?}", Body::from_record(&record, false).map(|_| ())); // Err(Status(1007))
}
```

### Expected

A body's own record reads back after a rigid turn, as it does for the same box at the origin.
