//! Text.

#[allow(clippy::wildcard_imports)]
use super::*;

/// How the lines of a text line up with its point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    /// Starting at the point.
    #[default]
    Left,
    /// Centred on the point.
    Centre,
    /// Ending at the point.
    Right,
}

/// A note: lines of text on a plane (spec `annotation.md` §2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Text {
    /// The words; a new line starts a new line of text.
    pub content: String,
    /// Where the first line's baseline meets the alignment.
    pub origin: Point,
    /// The direction the text reads, of length 1.
    pub x: Point,
    /// The direction up the letters, of length 1.
    pub y: Point,
    /// The height of capital letters, as drafting standards measure it.
    pub height: f64,
    /// A font installed on the computer, by name; Curva's own type when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// How the lines line up.
    #[serde(default)]
    pub align: TextAlign,
    /// The annotation style it takes its looks from, by id.
    #[serde(default)]
    pub style: u64,
}

/// The gap between lines of text, as a share of the capital height.
pub const LINE_SPACING: f64 = 1.6;

impl Text {
    /// The text moved by `by`.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        Self {
            origin: std::array::from_fn(|i| self.origin[i] + by[i]),
            ..self.clone()
        }
    }

    /// The text turned, mirrored or resized: its point moves, its directions turn and its height
    /// scales.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        let origin = change.point(self.origin);
        let direction = |d: Point| {
            let tip = change.point(std::array::from_fn(|i| self.origin[i] + d[i]));
            let v: Point = std::array::from_fn(|i| tip[i] - origin[i]);
            let length = v.iter().map(|c| c * c).sum::<f64>().sqrt().max(1e-12);
            v.map(|c| c / length)
        };
        Self {
            origin,
            x: direction(self.x),
            y: direction(self.y),
            height: self.height * change.factor(),
            ..self.clone()
        }
    }
}

impl Text {
    /// The text placed by an affine placement (rows are the images of x, y and z, then the move),
    /// as inside a block: its height follows how much its up direction is stretched.
    #[must_use]
    pub fn placed(&self, m: &[[f64; 4]; 4]) -> Self {
        let point = |p: Point| -> Point {
            std::array::from_fn(|k| p[0] * m[0][k] + p[1] * m[1][k] + p[2] * m[2][k] + m[3][k])
        };
        let direction = |d: Point| -> (Point, f64) {
            let v: Point =
                std::array::from_fn(|k| d[0] * m[0][k] + d[1] * m[1][k] + d[2] * m[2][k]);
            let length = v.iter().map(|c| c * c).sum::<f64>().sqrt().max(1e-12);
            (v.map(|c| c / length), length)
        };
        let (x, _) = direction(self.x);
        let (y, stretch) = direction(self.y);
        Self {
            origin: point(self.origin),
            x,
            y,
            height: self.height * stretch,
            ..self.clone()
        }
    }
}
