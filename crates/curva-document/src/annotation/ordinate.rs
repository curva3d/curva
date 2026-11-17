//! Ordinate dimensions.

#[allow(clippy::wildcard_imports)]
use super::*;

/// Which coordinate an ordinate dimension gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrdinateAxis {
    /// The distance along the plane's X from the base point; its leader runs along Y.
    #[default]
    X,
    /// The distance along the plane's Y; its leader runs along X.
    Y,
}

impl OrdinateAxis {
    /// The coordinate a leader from `point` to `leader` gives, as Rhino implies it: X when the
    /// leader runs more along Y than along X (`ON_DimOrdinate::ImpliedDirection`).
    #[must_use]
    pub fn implied(point: [f64; 2], leader: [f64; 2]) -> Self {
        if (leader[0] - point[0]).abs() <= (leader[1] - point[1]).abs() {
            Self::X
        } else {
            Self::Y
        }
    }
}

/// An ordinate dimension (Rhino's DimOrdinate): a point's X or Y distance from a base point, on a
/// leader that leaves the point along the other axis, jogs at two kinks and ends at its text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrdinateDimension {
    /// The base point (Rhino's datum), from which the distance is measured.
    pub base: Point,
    /// The point measured.
    pub point: Point,
    /// Where the leader ends and the text starts.
    pub leader: Point,
    /// Which coordinate it gives.
    pub axis: OrdinateAxis,
    /// How far back from the leader's end the kinks are: the first, then the second beyond it
    /// (Rhino's kink offsets), in model units; `None` for the default, twice the text height
    /// as drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinks: Option<[f64; 2]>,
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

impl OrdinateDimension {
    /// A point's coordinates in the dimension's plane, from its base point.
    #[must_use]
    pub fn local(&self, p: Point) -> [f64; 2] {
        let d = std::array::from_fn(|i| p[i] - self.base[i]);
        [dot(d, self.x), dot(d, self.y)]
    }

    /// The distance it gives, never negative (as Rhino shows it).
    #[must_use]
    pub fn value(&self) -> f64 {
        let at = self.local(self.point);
        match self.axis {
            OrdinateAxis::X => at[0].abs(),
            OrdinateAxis::Y => at[1].abs(),
        }
    }

    /// The two kinks, in the world: the one in line with the point, then the one in line with the
    /// leader's end (`ON_DimOrdinate::Get3dPoints`), with `default` for unset kinks.
    #[must_use]
    pub fn kink_points(&self, default: f64) -> [Point; 2] {
        let kinks = self.kinks.unwrap_or([default; 2]);
        let (p, l) = (self.local(self.point), self.local(self.leader));
        let at = |u: f64, v: f64| plus(plus(self.base, self.x, u), self.y, v);
        match self.axis {
            OrdinateAxis::X => {
                let sign = if l[1] < p[1] { -1.0 } else { 1.0 };
                let (k1, k2) = (kinks[0] * sign, kinks[1] * sign);
                [at(p[0], l[1] - k1 - k2), at(l[0], l[1] - k1)]
            }
            OrdinateAxis::Y => {
                let sign = if l[0] < p[0] { -1.0 } else { 1.0 };
                let (k1, k2) = (kinks[0] * sign, kinks[1] * sign);
                [at(l[0] - k1 - k2, p[1]), at(l[0] - k1, l[1])]
            }
        }
    }

    pub(super) fn transformed(
        &self,
        point: &dyn Fn(Point) -> Point,
        direction: &dyn Fn(Point) -> Point,
    ) -> Self {
        // Kinks keep their share of the leader's length, as a resize changes it.
        let length = |a: Point, b: Point| dot(sub(b, a), sub(b, a)).sqrt();
        let before = length(self.point, self.leader);
        let after = length(point(self.point), point(self.leader));
        let stretch = if before > 1e-12 { after / before } else { 1.0 };
        Self {
            base: point(self.base),
            point: point(self.point),
            leader: point(self.leader),
            kinks: self.kinks.map(|k| k.map(|v| v * stretch)),
            x: direction(self.x),
            y: direction(self.y),
            ..self.clone()
        }
    }

    /// Its leader and text, with sizes from `style` scaled by `scale`: the leader starts the
    /// style's extension offset from the point and the text reads along its last stretch, beyond
    /// its end (`ON_DimOrdinate::GetDisplayLines` and `GetTextXform`).
    #[must_use]
    pub fn parts(&self, style: &AnnotationStyle, scale: f64, shown: String) -> DimensionParts {
        let [first, second] = self.kink_points(2.0 * style.text_height * scale);
        let gap_from_point = style.extension_offset() * scale;
        let toward = unit(sub(first, self.point));
        let start = if dot(sub(first, self.point), sub(first, self.point)).sqrt() > gap_from_point {
            plus(self.point, toward, gap_from_point)
        } else {
            self.point
        };
        let lines: Vec<(Point, Point)> = [(start, first), (first, second), (second, self.leader)]
            .into_iter()
            .filter(|(a, b)| dot(sub(*b, *a), sub(*b, *a)) > 1e-18)
            .collect();
        // The text reads along the leader's last stretch, away from the point.
        let (p, l) = (self.local(self.point), self.local(self.leader));
        let (along, forward) = match self.axis {
            OrdinateAxis::X => (self.y, l[1] >= p[1]),
            OrdinateAxis::Y => (self.x, l[0] >= p[0]),
        };
        let normal = cross(self.x, self.y);
        let across = cross(normal, along);
        let gap = style.text_gap() * scale;
        let height = style.text_height * scale;
        let text = Text {
            content: shown,
            origin: plus(
                plus(self.leader, along, if forward { gap } else { -gap }),
                across,
                -height / 2.0,
            ),
            x: along,
            y: across,
            height,
            font: style.font.clone(),
            align: if forward {
                TextAlign::Left
            } else {
                TextAlign::Right
            },
            style: self.style,
        };
        DimensionParts {
            lines,
            arrows: Vec::new(),
            text,
        }
    }
}
