//! Radius and diameter dimensions.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A radius or diameter dimension (Rhino's DimRadius and DimDiameter): the circle's centre, the
/// point on it where the arrow lands, and where the text goes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadialDimension {
    /// Whether it gives the diameter, with a line across the circle.
    pub diameter: bool,
    /// The circle's centre.
    pub centre: Point,
    /// The point on the circle where the arrow lands.
    pub on_curve: Point,
    /// Where the text goes.
    pub text_at: Point,
    /// The plane it lies on: its X direction.
    pub x: Point,
    /// The plane's Y direction.
    pub y: Point,
    /// The annotation style it takes its looks from, by id.
    #[serde(default)]
    pub style: u64,
    /// Typed text in place of the value; `<>` stands for the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl RadialDimension {
    /// The radius, or the diameter.
    #[must_use]
    pub fn value(&self) -> f64 {
        let r = dot(
            std::array::from_fn(|i| self.on_curve[i] - self.centre[i]),
            std::array::from_fn(|i| self.on_curve[i] - self.centre[i]),
        )
        .sqrt();
        if self.diameter { 2.0 * r } else { r }
    }

    pub(super) fn transformed(
        &self,
        point: &dyn Fn(Point) -> Point,
        direction: &dyn Fn(Point) -> Point,
    ) -> Self {
        Self {
            centre: point(self.centre),
            on_curve: point(self.on_curve),
            text_at: point(self.text_at),
            x: direction(self.x),
            y: direction(self.y),
            ..self.clone()
        }
    }

    /// Its lines, arrowheads and text, with sizes from `style` scaled by `scale`.
    #[must_use]
    pub fn parts(&self, style: &AnnotationStyle, scale: f64, shown: String) -> DimensionParts {
        let outward = unit(std::array::from_fn(|i| self.on_curve[i] - self.centre[i]));
        let radius = self.value() / if self.diameter { 2.0 } else { 1.0 };
        let normal = cross(self.x, self.y);
        let arrow = style.arrow_size() * scale;
        let head = |tip: Point, back: Point| {
            let across = unit(cross(normal, back));
            let base = plus(tip, back, arrow);
            [
                tip,
                plus(base, across, arrow / 6.0),
                plus(base, across, -arrow / 6.0),
            ]
        };
        // The arrow points onto the circle from the side the text is on.
        let text_outside = dot(
            std::array::from_fn(|i| self.text_at[i] - self.centre[i]),
            outward,
        ) > radius;
        let back = if text_outside {
            outward
        } else {
            outward.map(|v| -v)
        };
        let mut lines = vec![(self.on_curve, self.text_at)];
        let mut arrows = vec![head(self.on_curve, back)];
        if self.diameter {
            let far = plus(self.centre, outward, -radius);
            lines.push((far, self.on_curve));
            arrows.push(head(far, back.map(|v| -v)));
        }
        // A short landing, then the text beside it, reading along the plane.
        let leftward = dot(
            std::array::from_fn(|i| self.text_at[i] - self.on_curve[i]),
            self.x,
        ) < 0.0;
        let landing = plus(self.text_at, self.x, if leftward { -arrow } else { arrow });
        lines.push((self.text_at, landing));
        let gap = style.text_gap() * scale;
        let text = Text {
            content: shown,
            origin: plus(
                plus(landing, self.x, if leftward { -gap } else { gap }),
                self.y,
                -style.text_height * scale / 2.0,
            ),
            x: self.x,
            y: self.y,
            height: style.text_height * scale,
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
