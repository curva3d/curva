//! Leaders.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A leader (Rhino's Leader): an arrowhead at its first point, a polyline through the rest, and
/// its note beside the last point, a style's text gap away. The note is text, so it takes fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leader {
    /// The arrow's tip, then each point the line passes through; the note is at the last.
    pub points: Vec<Point>,
    /// The note; a new line starts a new line of text.
    pub content: String,
    /// The plane it lies on: its X direction, which the note reads along.
    pub x: Point,
    /// The plane's Y direction.
    pub y: Point,
    /// Whether a short level line runs from the last point to the note (Rhino's Landing).
    #[serde(default)]
    pub landing: bool,
    /// The annotation style it takes its looks from, by id.
    #[serde(default)]
    pub style: u64,
}

impl Leader {
    pub(super) fn transformed(
        &self,
        point: &dyn Fn(Point) -> Point,
        direction: &dyn Fn(Point) -> Point,
    ) -> Self {
        Self {
            points: self.points.iter().map(|p| point(*p)).collect(),
            x: direction(self.x),
            y: direction(self.y),
            ..self.clone()
        }
    }

    /// Its line, arrowhead and note, with sizes from `style` scaled by `scale`; `shown` is the
    /// note with its fields worked out.
    #[must_use]
    pub fn parts(&self, style: &AnnotationStyle, scale: f64, shown: String) -> DimensionParts {
        let arrow = style.arrow_size() * scale;
        let normal = cross(self.x, self.y);
        let mut lines: Vec<(Point, Point)> = self.points.windows(2).map(|w| (w[0], w[1])).collect();
        let mut arrows = Vec::new();
        if let [tip, next, ..] = self.points[..] {
            let back = unit(std::array::from_fn(|i| next[i] - tip[i]));
            let across = unit(cross(normal, back));
            let base = plus(tip, back, arrow);
            arrows.push([
                tip,
                plus(base, across, arrow / 6.0),
                plus(base, across, -arrow / 6.0),
            ]);
        }
        let (last, before) = match self.points[..] {
            [.., before, last] => (last, before),
            [only] => (only, only),
            [] => ([0.0; 3], [0.0; 3]),
        };
        // The note reads away from the line, as Rhino's Auto alignment does.
        let leftward = dot(std::array::from_fn(|i| last[i] - before[i]), self.x) < 0.0;
        let side = if leftward { -1.0 } else { 1.0 };
        let mut end = last;
        if self.landing {
            end = plus(last, self.x, side * arrow);
            lines.push((last, end));
        }
        let gap = style.text_gap() * scale;
        let height = style.text_height * scale;
        let text = Text {
            content: shown,
            origin: plus(plus(end, self.x, side * gap), self.y, -height / 2.0),
            x: self.x,
            y: self.y,
            height,
            font: style.font.clone(),
            align: if leftward {
                TextAlign::Right
            } else {
                TextAlign::Left
            },
            style: self.style,
        };
        DimensionParts {
            lines,
            arrows,
            text,
        }
    }
}
