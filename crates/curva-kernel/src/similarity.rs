//! Similarities: turns, mirror images and even changes of size.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A change that keeps shapes the same: a turn, a mirror image or an even change of size, about a
/// point. Circles stay circles under all of these.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Similarity {
    pub(crate) origin: Point,
    rows: [Point; 3],
    pub(crate) scale: f64,
}

impl Similarity {
    /// A turn by `angle` radians about a line through `origin` along the unit vector `axis`,
    /// by the Rodrigues rotation formula.
    #[must_use]
    pub fn rotation(origin: Point, axis: Point, angle: f64) -> Self {
        let (sin, cos) = angle.sin_cos();
        let [x, y, z] = axis;
        let rest = 1.0 - cos;
        Self {
            origin,
            rows: [
                [
                    cos + x * x * rest,
                    x * y * rest - z * sin,
                    x * z * rest + y * sin,
                ],
                [
                    y * x * rest + z * sin,
                    cos + y * y * rest,
                    y * z * rest - x * sin,
                ],
                [
                    z * x * rest - y * sin,
                    z * y * rest + x * sin,
                    cos + z * z * rest,
                ],
            ],
            scale: 1.0,
        }
    }

    /// An even change of size about `origin`.
    #[must_use]
    pub fn scaling(origin: Point, factor: f64) -> Self {
        Self {
            origin,
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            scale: factor,
        }
    }

    /// The turn about `origin` that takes the three points' frame `from` onto `to`: the first
    /// point to the second sets X, and the third sets which way Y faces. `None` when either set
    /// of points lies on one line.
    #[must_use]
    pub fn between_frames(origin: Point, from: [Point; 3], to: [Point; 3]) -> Option<Self> {
        let axes = |[a, b, c]: [Point; 3]| -> Option<[Point; 3]> {
            let x = unit(sub(b, a))?;
            let z = unit(cross(x, sub(c, a)))?;
            Some([x, cross(z, x), z])
        };
        let (from, to) = (axes(from)?, axes(to)?);
        let rows = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..3).map(|k| to[k][i] * from[k][j]).sum())
        });
        Some(Self {
            origin,
            rows,
            scale: 1.0,
        })
    }

    /// A mirror image in the plane through `origin` facing along the unit vector `normal`.
    #[must_use]
    pub fn mirror(origin: Point, normal: Point) -> Self {
        let row = |i: usize| {
            let across = |j: usize| f64::from(u8::from(i == j)) - 2.0 * normal[i] * normal[j];
            [across(0), across(1), across(2)]
        };
        Self {
            origin,
            rows: [row(0), row(1), row(2)],
            scale: 1.0,
        }
    }

    /// Where a direction goes. Its length does not change.
    pub(crate) fn direction(&self, v: Point) -> Point {
        self.rows.map(|row| dot(row, v))
    }

    /// How much it resizes: 1 for a turn or a mirror.
    #[must_use]
    pub fn factor(&self) -> f64 {
        self.scale
    }

    /// How far it turns, in radians from 0 to π, read from its rotation.
    #[must_use]
    pub fn angle(&self) -> f64 {
        let trace = self.rows[0][0] + self.rows[1][1] + self.rows[2][2];
        ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos()
    }

    /// Where a point goes.
    #[must_use]
    pub fn point(&self, p: Point) -> Point {
        let turned = self.direction(sub(p, self.origin));
        [
            self.origin[0] + turned[0] * self.scale,
            self.origin[1] + turned[1] * self.scale,
            self.origin[2] + turned[2] * self.scale,
        ]
    }
}
