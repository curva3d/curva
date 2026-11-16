//! Angle dimensions.

#[allow(clippy::wildcard_imports)]
use super::*;

/// An angle dimension (Rhino's DimAngle): the apex, a point along each side, and a point the arc
/// passes through, which also picks which of the angles between the sides is measured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AngleDimension {
    /// Where the sides meet.
    pub apex: Point,
    /// A point along the first side.
    pub first: Point,
    /// A point along the second side.
    pub second: Point,
    /// A point the arc passes through.
    pub arc_at: Point,
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

impl AngleDimension {
    /// Angles on the plane, in radians from its X, of a point seen from the apex.
    fn bearing(&self, p: Point) -> f64 {
        let d: Point = std::array::from_fn(|i| p[i] - self.apex[i]);
        dot(d, self.y).atan2(dot(d, self.x))
    }

    /// Where the arc starts and how far it sweeps (radians, counter-clockwise on the plane): the
    /// angle between the sides that holds the arc's point.
    fn sweep(&self) -> (f64, f64) {
        use std::f64::consts::TAU;
        let (a, b, c) = (
            self.bearing(self.first),
            self.bearing(self.second),
            self.bearing(self.arc_at),
        );
        use std::f64::consts::PI;
        let ccw = |from: f64, to: f64| (to - from).rem_euclid(TAU);
        // Of the four angles the two lines make, each at most a half turn, the one that holds
        // the arc's point, as in Rhino.
        let b = if ccw(a, b) > PI { b + PI } else { b };
        let rays = [(a, b), (b, a + PI), (a + PI, b + PI), (b + PI, a)];
        rays.iter()
            .map(|&(from, to)| (from.rem_euclid(TAU), ccw(from, to)))
            .find(|&(from, span)| ccw(from, c) <= span + 1e-12)
            .unwrap_or((a.rem_euclid(TAU), ccw(a, b)))
    }

    /// The angle measured, in degrees.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.sweep().1.to_degrees()
    }

    pub(super) fn transformed(
        &self,
        point: &dyn Fn(Point) -> Point,
        direction: &dyn Fn(Point) -> Point,
    ) -> Self {
        Self {
            apex: point(self.apex),
            first: point(self.first),
            second: point(self.second),
            arc_at: point(self.arc_at),
            x: direction(self.x),
            y: direction(self.y),
            ..self.clone()
        }
    }

    /// Its arc (as short lines), arrowheads and text, with sizes from `style` scaled by `scale`.
    #[must_use]
    pub fn parts(&self, style: &AnnotationStyle, scale: f64, shown: String) -> DimensionParts {
        let (start, span) = self.sweep();
        let r = dot(
            std::array::from_fn(|i| self.arc_at[i] - self.apex[i]),
            std::array::from_fn(|i| self.arc_at[i] - self.apex[i]),
        )
        .sqrt();
        let at = |angle: f64| {
            let (s, c) = angle.sin_cos();
            std::array::from_fn(|i| self.apex[i] + r * (self.x[i] * c + self.y[i] * s))
        };
        let steps = 32;
        let mut lines: Vec<(Point, Point)> = (0..steps)
            .map(|k| {
                #[allow(clippy::cast_precision_loss)]
                let (t0, t1) = (k as f64 / steps as f64, (k + 1) as f64 / steps as f64);
                (at(start + span * t0), at(start + span * t1))
            })
            .collect();
        // Extension lines from the sides' points out to the arc, where the arc lies beyond them.
        for p in [self.first, self.second] {
            let d: Point = std::array::from_fn(|i| p[i] - self.apex[i]);
            let reach = dot(d, d).sqrt();
            if reach < r {
                let along = unit(d);
                lines.push((
                    plus(p, along, style.extension_offset() * scale),
                    plus(self.apex, along, r + style.extension_extension() * scale),
                ));
            }
        }
        let arrow = style.arrow_size() * scale;
        let normal = cross(self.x, self.y);
        let head = |tip: Point, tangent: Point| {
            let across = unit(cross(normal, tangent));
            let base = plus(tip, tangent, arrow);
            [
                tip,
                plus(base, across, arrow / 6.0),
                plus(base, across, -arrow / 6.0),
            ]
        };
        let tangent_at = |angle: f64| {
            let (s, c) = angle.sin_cos();
            std::array::from_fn(|i| -self.x[i] * s + self.y[i] * c)
        };
        let arrows = vec![
            head(at(start), tangent_at(start)),
            head(at(start + span), tangent_at(start + span).map(|v| -v)),
        ];
        let middle = start + span / 2.0;
        let (s, c) = middle.sin_cos();
        let outward: Point = std::array::from_fn(|i| self.x[i] * c + self.y[i] * s);
        let text = Text {
            content: shown,
            origin: plus(at(middle), outward, style.text_gap() * scale),
            x: self.x,
            y: self.y,
            height: style.text_height * scale,
            font: style.font.clone(),
            align: TextAlign::Centre,
            style: self.style,
        };
        lines.retain(|(a, b)| a != b);
        DimensionParts {
            lines,
            arrows,
            text,
        }
    }
}
