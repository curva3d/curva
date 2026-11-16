//! Arc length dimensions.

#[allow(clippy::wildcard_imports)]
use super::*;

/// An arc length dimension, as AutoCAD's (`DIMARC`; Rhino 8 has none): the length along an arc,
/// drawn as a dimension arc about the same centre with arrowheads, extension lines from the
/// arc's ends, and its value under the arc symbol.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcLengthDimension {
    /// The arc's centre.
    pub centre: Point,
    /// Where the measured arc starts.
    pub start: Point,
    /// Where it ends: it runs counter-clockwise on the plane from `start` to here.
    pub end: Point,
    /// A point the dimension arc passes through.
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
    /// Whether it measures only part of the arc, as AutoCAD's `Partial`: `start` and `end` are
    /// then points chosen on the arc rather than its ends. It draws the same; files keep the flag.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
    /// Whether a leader runs from the dimension arc to the measured arc, as AutoCAD's `Leader`:
    /// straight towards the centre, at the middle of the measured part, its arrow on the arc.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leader: bool,
}

impl ArcLengthDimension {
    /// The angle on the plane, in radians from its X, of a point seen from the centre.
    fn bearing(&self, p: Point) -> f64 {
        let d: Point = std::array::from_fn(|i| p[i] - self.centre[i]);
        dot(d, self.y).atan2(dot(d, self.x))
    }

    /// Where the arc starts and how far it turns (radians, counter-clockwise on the plane).
    fn sweep(&self) -> (f64, f64) {
        use std::f64::consts::TAU;
        let from = self.bearing(self.start);
        let span = (self.bearing(self.end) - from).rem_euclid(TAU);
        (from, if span < 1e-12 { TAU } else { span })
    }

    fn radius_to(&self, p: Point) -> f64 {
        let d: Point = std::array::from_fn(|i| p[i] - self.centre[i]);
        dot(d, d).sqrt()
    }

    /// The length measured: the arc's radius times its turn.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.radius_to(self.start) * self.sweep().1
    }

    /// The measured arc's turn, in radians.
    #[must_use]
    pub fn turn(&self) -> f64 {
        self.sweep().1
    }

    /// Where its leader starts, on the dimension arc, and where it ends, on the measured arc, if
    /// it has one. Both lie on the line from the centre through the middle of the measured part,
    /// as AutoCAD draws it.
    #[must_use]
    pub fn leader_points(&self) -> Option<(Point, Point)> {
        if !self.leader {
            return None;
        }
        let (start, span) = self.sweep();
        let (s, c) = (start + span / 2.0).sin_cos();
        let way: Point = std::array::from_fn(|i| self.x[i] * c + self.y[i] * s);
        Some((
            plus(self.centre, way, self.radius_to(self.arc_at)),
            plus(self.centre, way, self.radius_to(self.start)),
        ))
    }

    pub(super) fn transformed(
        &self,
        point: &dyn Fn(Point) -> Point,
        direction: &dyn Fn(Point) -> Point,
    ) -> Self {
        Self {
            centre: point(self.centre),
            start: point(self.start),
            end: point(self.end),
            arc_at: point(self.arc_at),
            x: direction(self.x),
            y: direction(self.y),
            ..self.clone()
        }
    }

    /// Its dimension arc (as short lines), extension lines, arrowheads, arc symbol and text,
    /// with sizes from `style` scaled by `scale`. As AutoCAD draws it, the extension lines run
    /// parallel (along the arc's middle direction) for a turn under a quarter, and out from the
    /// centre otherwise.
    #[must_use]
    pub fn parts(&self, style: &AnnotationStyle, scale: f64, shown: String) -> DimensionParts {
        use std::f64::consts::FRAC_PI_2;
        let (start, span) = self.sweep();
        let r = self.radius_to(self.start);
        let reach = self.radius_to(self.arc_at);
        let along = |angle: f64| -> Point {
            let (s, c) = angle.sin_cos();
            std::array::from_fn(|i| self.x[i] * c + self.y[i] * s)
        };
        let at = |radius: f64, angle: f64| plus(self.centre, along(angle), radius);
        let middle = start + span / 2.0;
        let sign = if reach >= r { 1.0 } else { -1.0 };
        let (offset, extension) = (
            style.extension_offset() * scale,
            style.extension_extension() * scale,
        );
        // Each end's extension line, from the arc's end towards the circle the dimension arc
        // lies on, and how far along it that circle is met.
        let mut lines: Vec<(Point, Point)> = Vec::new();
        let mut ends = [start, start + span];
        for end in &mut ends {
            let foot = at(r, *end);
            let way = along(if span < FRAC_PI_2 { middle } else { *end }).map(|c| c * sign);
            let d: Point = std::array::from_fn(|i| foot[i] - self.centre[i]);
            let b = dot(d, way);
            let t = -b + sign * (b * b - (dot(d, d) - reach * reach)).max(0.0).sqrt();
            let meet = plus(foot, way, t);
            if t > offset {
                lines.push((plus(foot, way, offset), plus(meet, way, extension)));
            }
            *end = self.bearing(meet);
        }
        // The dimension arc between where the extension lines meet it.
        let from = ends[0];
        let span = (ends[1] - from).rem_euclid(std::f64::consts::TAU);
        let span = if span < 1e-12 { self.sweep().1 } else { span };
        let steps = 32;
        lines.extend((0..steps).map(|k| {
            #[allow(clippy::cast_precision_loss)]
            let (t0, t1) = (k as f64 / steps as f64, (k + 1) as f64 / steps as f64);
            (at(reach, from + span * t0), at(reach, from + span * t1))
        }));
        let start = from;
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
        let tangent_at = |angle: f64| along(angle + FRAC_PI_2);
        let mut arrows = vec![
            head(at(reach, start), tangent_at(start)),
            head(
                at(reach, start + span),
                tangent_at(start + span).map(|v| -v),
            ),
        ];
        // The leader: a line from the dimension arc in to the measured arc, its arrow on the arc,
        // pointing along the line.
        if let Some((from, to)) = self.leader_points() {
            lines.push((from, to));
            arrows.push(head(to, unit(sub(from, to))));
        }
        let height = style.text_height * scale;
        let origin = plus(at(reach, middle), along(middle), style.text_gap() * scale);
        lines.extend(self.arc_symbol(origin, height));
        let text = Text {
            content: shown,
            origin,
            x: self.x,
            y: self.y,
            height,
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

    /// The arc symbol over a value written at `origin` with letters `height` tall, as AutoCAD
    /// draws it with the symbol above: half an ellipse, as eight short lines.
    fn arc_symbol(&self, origin: Point, height: f64) -> Vec<(Point, Point)> {
        let centre = plus(origin, self.y, height * 1.1);
        let half = height * 0.6;
        #[allow(clippy::cast_precision_loss)]
        let angle = |k: usize| std::f64::consts::PI * k as f64 / 8.0;
        let point = |a: f64| -> Point {
            let (s, c) = a.sin_cos();
            std::array::from_fn(|i| centre[i] + half * c * self.x[i] + half * 0.6 * s * self.y[i])
        };
        (0..8)
            .map(|k| (point(angle(k)), point(angle(k + 1))))
            .collect()
    }
}
