//! Linear dimensions: horizontal, vertical, aligned and rotated.

#[allow(clippy::wildcard_imports)]
use super::*;

/// Which way a linear dimension measures.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DimensionKind {
    /// Along the construction plane's X (Rhino's Dim, Horizontal).
    Horizontal,
    /// Along the construction plane's Y (Rhino's Dim, Vertical).
    Vertical,
    /// Parallel to the two points (DimAligned).
    Aligned,
    /// At an angle, in radians from the construction plane's X (DimRotated).
    Rotated(f64),
}

/// A linear dimension: the distance between two points, drawn with extension lines, a dimension
/// line with arrowheads, and its value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    /// Which way it measures.
    pub kind: DimensionKind,
    /// The first point measured.
    pub first: Point,
    /// The second point measured.
    pub second: Point,
    /// A point the dimension line passes through.
    pub line: Point,
    /// The plane it lies on: its X and Y directions, of length 1.
    pub x: Point,
    /// The plane's Y direction.
    pub y: Point,
    /// The annotation style it takes its looks from, by id.
    #[serde(default)]
    pub style: u64,
    /// Typed text in place of the value; `<>` in it stands for the value, as in Rhino.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl Dimension {
    /// The direction it measures along, of length 1, on its plane.
    #[must_use]
    pub fn direction(&self) -> Point {
        let (x, y) = (self.x, self.y);
        match self.kind {
            DimensionKind::Horizontal => x,
            DimensionKind::Vertical => y,
            DimensionKind::Rotated(angle) => {
                let (s, c) = angle.sin_cos();
                std::array::from_fn(|i| x[i] * c + y[i] * s)
            }
            DimensionKind::Aligned => {
                let d: Point = std::array::from_fn(|i| self.second[i] - self.first[i]);
                // Along the plane only.
                let on: Point = std::array::from_fn(|i| x[i] * dot(d, x) + y[i] * dot(d, y));
                let length = dot(on, on).sqrt();
                if length < 1e-12 {
                    x
                } else {
                    on.map(|v| v / length)
                }
            }
        }
    }

    /// The distance it measures.
    #[must_use]
    pub fn value(&self) -> f64 {
        let d: Point = std::array::from_fn(|i| self.second[i] - self.first[i]);
        dot(d, self.direction()).abs()
    }

    /// Its lines, arrowheads and value text, with sizes from `style` scaled by `scale` (from the
    /// style's unit to where it is drawn), and `shown` as its text.
    #[must_use]
    pub fn parts(&self, style: &AnnotationStyle, scale: f64, shown: String) -> DimensionParts {
        let along = self.direction();
        // Across the dimension, on the plane, pointing to where the dimension line lies.
        let normal: Point = {
            let n = [
                self.x[1] * self.y[2] - self.x[2] * self.y[1],
                self.x[2] * self.y[0] - self.x[0] * self.y[2],
                self.x[0] * self.y[1] - self.x[1] * self.y[0],
            ];
            [
                n[1] * along[2] - n[2] * along[1],
                n[2] * along[0] - n[0] * along[2],
                n[0] * along[1] - n[1] * along[0],
            ]
        };
        let line_offset = dot(
            std::array::from_fn(|i| self.line[i] - self.first[i]),
            normal,
        );
        let side = if line_offset >= 0.0 { 1.0 } else { -1.0 };
        // Where each measured point meets the dimension line.
        let foot = |p: Point| {
            let offset = dot(std::array::from_fn(|i| self.line[i] - p[i]), normal);
            plus(p, normal, offset)
        };
        let (a, b) = (foot(self.first), foot(self.second));
        let gap = style.extension_offset() * scale;
        let beyond = style.extension_extension() * scale;
        let mut lines = Vec::new();
        for (p, f) in [(self.first, a), (self.second, b)] {
            let reach = dot(std::array::from_fn(|i| f[i] - p[i]), normal) * side;
            if reach > gap {
                lines.push((plus(p, normal, gap * side), plus(f, normal, beyond * side)));
            }
        }
        lines.push((a, b));
        // Arrowheads pointing outwards to the extension lines, from a to b.
        let arrow = style.arrow_size() * scale;
        let width = arrow / 3.0;
        let toward: Point = {
            let d: Point = std::array::from_fn(|i| b[i] - a[i]);
            let length = dot(d, d).sqrt().max(1e-12);
            d.map(|v| v / length)
        };
        let head = |tip: Point, back: Point| {
            let base = plus(tip, back, arrow);
            [
                tip,
                plus(base, normal, width / 2.0),
                plus(base, normal, -width / 2.0),
            ]
        };
        let arrows = vec![head(a, toward), head(b, toward.map(|v| -v))];
        // The value, centred on the dimension line, a text gap above it.
        let height = style.text_height * scale;
        let middle: Point = std::array::from_fn(|i| (a[i] + b[i]) / 2.0);
        // Text reads left to right and upwards on the plane.
        let (mut reads, mut up) = (toward, normal.map(|v| v * side));
        if dot(reads, self.x) < -1e-9
            || (dot(reads, self.x).abs() < 1e-9 && dot(reads, self.y) < 0.0)
        {
            reads = reads.map(|v| -v);
        }
        if dot(up, self.y) < -1e-9 || (dot(up, self.y).abs() < 1e-9 && dot(up, self.x) > 0.0) {
            up = up.map(|v| -v);
        }
        let text = Text {
            content: shown,
            origin: plus(middle, up, style.text_gap() * scale),
            x: reads,
            y: up,
            height,
            font: style.font.clone(),
            align: TextAlign::Centre,
            style: self.style,
        };
        DimensionParts {
            lines,
            arrows,
            text,
        }
    }

    /// The dimension moved by `by`.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        Self {
            first: plus(self.first, by, 1.0),
            second: plus(self.second, by, 1.0),
            line: plus(self.line, by, 1.0),
            ..self.clone()
        }
    }

    /// The dimension turned, mirrored or resized.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        let origin = change.point([0.0; 3]);
        let direction = |d: Point| {
            let tip = change.point(d);
            let v: Point = std::array::from_fn(|i| tip[i] - origin[i]);
            let length = dot(v, v).sqrt().max(1e-12);
            v.map(|c| c / length)
        };
        Self {
            first: change.point(self.first),
            second: change.point(self.second),
            line: change.point(self.line),
            x: direction(self.x),
            y: direction(self.y),
            ..self.clone()
        }
    }

    /// The dimension placed by an affine placement, as inside a block.
    #[must_use]
    pub fn placed(&self, m: &[[f64; 4]; 4]) -> Self {
        let point = |p: Point| -> Point {
            std::array::from_fn(|k| p[0] * m[0][k] + p[1] * m[1][k] + p[2] * m[2][k] + m[3][k])
        };
        let direction = |d: Point| -> Point {
            let v: Point =
                std::array::from_fn(|k| d[0] * m[0][k] + d[1] * m[1][k] + d[2] * m[2][k]);
            let length = dot(v, v).sqrt().max(1e-12);
            v.map(|c| c / length)
        };
        Self {
            first: point(self.first),
            second: point(self.second),
            line: point(self.line),
            x: direction(self.x),
            y: direction(self.y),
            ..self.clone()
        }
    }
}
