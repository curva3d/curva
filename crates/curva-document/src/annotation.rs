//! Annotation (spec `annotation.md`): text now, and dimensions, leaders, dots and hatches as they
//! come. An annotation is an object of its own kind, in the model or on a layout's sheet, and
//! takes its looks from a named annotation style.

use crate::{Document, Unit};
use curva_kernel::{Point, Similarity};
use serde::{Deserialize, Serialize};

/// An annotation object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Annotation {
    /// A note.
    Text(Text),
    /// A dimension (spec `annotation.md` §3).
    Dimension(Dimension),
    /// A radius or diameter dimension.
    Radial(RadialDimension),
    /// An angle dimension.
    Angle(AngleDimension),
    /// A leader: an arrow and line to a note (spec `annotation.md` §4).
    Leader(Leader),
    /// A dot: a label at a point that keeps its size on screen (spec `annotation.md` §4).
    Dot(Dot),
}

impl Annotation {
    /// The annotation moved by `by`.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        match self {
            Self::Text(t) => Self::Text(t.translated(by)),
            Self::Dimension(d) => Self::Dimension(d.translated(by)),
            Self::Radial(r) => Self::Radial(r.transformed(&|p| plus(p, by, 1.0), &|d| d)),
            Self::Angle(a) => Self::Angle(a.transformed(&|p| plus(p, by, 1.0), &|d| d)),
            Self::Leader(l) => Self::Leader(l.transformed(&|p| plus(p, by, 1.0), &|d| d)),
            Self::Dot(d) => Self::Dot(Dot {
                at: plus(d.at, by, 1.0),
                ..d.clone()
            }),
        }
    }

    /// The annotation turned, mirrored or resized.
    #[must_use]
    pub fn mapped(&self, change: &Similarity) -> Self {
        let origin = change.point([0.0; 3]);
        let direction = |d: Point| unit(std::array::from_fn(|i| change.point(d)[i] - origin[i]));
        let point = |p: Point| change.point(p);
        match self {
            Self::Text(t) => Self::Text(t.mapped(change)),
            Self::Dimension(d) => Self::Dimension(d.mapped(change)),
            Self::Radial(r) => Self::Radial(r.transformed(&point, &direction)),
            Self::Angle(a) => Self::Angle(a.transformed(&point, &direction)),
            Self::Leader(l) => Self::Leader(l.transformed(&point, &direction)),
            Self::Dot(d) => Self::Dot(Dot {
                at: point(d.at),
                ..d.clone()
            }),
        }
    }

    /// The annotation placed by an affine placement, as inside a block.
    #[must_use]
    pub fn placed(&self, m: &[[f64; 4]; 4]) -> Self {
        let point = |p: Point| -> Point {
            std::array::from_fn(|k| p[0] * m[0][k] + p[1] * m[1][k] + p[2] * m[2][k] + m[3][k])
        };
        let direction = |d: Point| -> Point {
            unit(std::array::from_fn(|k| {
                d[0] * m[0][k] + d[1] * m[1][k] + d[2] * m[2][k]
            }))
        };
        match self {
            Self::Text(t) => Self::Text(t.placed(m)),
            Self::Dimension(d) => Self::Dimension(d.placed(m)),
            Self::Radial(r) => Self::Radial(r.transformed(&point, &direction)),
            Self::Angle(a) => Self::Angle(a.transformed(&point, &direction)),
            Self::Leader(l) => Self::Leader(l.transformed(&point, &direction)),
            Self::Dot(d) => Self::Dot(Dot {
                at: point(d.at),
                ..d.clone()
            }),
        }
    }

    /// The annotation style it takes its looks from.
    #[must_use]
    pub fn style(&self) -> u64 {
        match self {
            Self::Text(t) => t.style,
            Self::Dimension(d) => d.style,
            Self::Radial(r) => r.style,
            Self::Angle(a) => a.style,
            Self::Leader(l) => l.style,
            Self::Dot(_) => 0,
        }
    }

    /// A dimension's typed text (`<>` stands for its value); `None` for a text.
    #[must_use]
    pub fn dimension_text(&self) -> Option<&Option<String>> {
        match self {
            Self::Text(_) | Self::Leader(_) | Self::Dot(_) => None,
            Self::Dimension(d) => Some(&d.text),
            Self::Radial(r) => Some(&r.text),
            Self::Angle(a) => Some(&a.text),
        }
    }

    /// The dimension with other typed text; a text is unchanged.
    #[must_use]
    pub fn with_dimension_text(&self, text: Option<String>) -> Self {
        let mut changed = self.clone();
        match &mut changed {
            Self::Text(_) | Self::Leader(_) | Self::Dot(_) => {}
            Self::Dimension(d) => d.text = text,
            Self::Radial(r) => r.text = text,
            Self::Angle(a) => a.text = text,
        }
        changed
    }
}

/// A vector of length 1 the same way, or the X axis for none.
fn unit(v: Point) -> Point {
    let length = dot(v, v).sqrt();
    if length < 1e-12 {
        [1.0, 0.0, 0.0]
    } else {
        v.map(|c| c / length)
    }
}

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

    fn transformed(
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

    fn transformed(
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

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

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
    fn transformed(
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

/// A dot (Rhino's Dot): a label at a point, drawn on the screen rather than in the model, so it
/// keeps its size and faces the viewer however the view turns, and shows in front of everything.
/// Its colour is its layer's or its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dot {
    /// The point it labels.
    pub at: Point,
    /// What it shows; a new line starts a new line.
    pub content: String,
    /// Further words it holds without showing them (Rhino's secondary text).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub secondary: String,
    /// Its letters' height on screen, in points.
    #[serde(default = "Dot::usual_height")]
    pub height: f64,
    /// A font installed on the computer, by name; Curva's own type when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

impl Dot {
    /// Rhino's usual dot height.
    #[must_use]
    pub fn usual_height() -> f64 {
        14.0
    }
}

/// What a dimension draws, where it lies.
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionParts {
    /// Extension lines and the dimension line, each from one end to the other.
    pub lines: Vec<(Point, Point)>,
    /// Arrowheads, each a filled triangle.
    pub arrows: Vec<[Point; 3]>,
    /// The value, as text centred above the dimension line.
    pub text: Text,
}

fn plus(p: Point, d: Point, by: f64) -> Point {
    std::array::from_fn(|i| p[i] + d[i] * by)
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
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

/// A named set of looks for annotation (spec `annotation.md` §1). It grows with dimensions and
/// leaders; text uses its font and height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnnotationStyle {
    /// Identifies it.
    pub id: u64,
    /// Its name.
    pub name: String,
    /// A font installed on the computer, by name; Curva's own type when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// The height of capital letters, in the unit it is drawn in: millimetres for ISO styles,
    /// inches for ANSI ones (paper sizes).
    pub text_height: f64,
    /// Whether its sizes are in inches rather than millimetres.
    pub imperial: bool,
    /// Rhino's Arrow size, when set; see [`Self::arrow_size`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_size: Option<f64>,
    /// Rhino's Extension line offset, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_offset: Option<f64>,
    /// Rhino's Extension line extension, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_extension: Option<f64>,
    /// Rhino's Text gap, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_gap: Option<f64>,
    /// Rhino's Linear resolution, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linear_resolution: Option<u8>,
}

/// The built-in styles (decision 0020, point 4): ISO text 2.5, 3.5, 5 and 7 mm (ISO 3098); ANSI
/// text 3/32", 1/8" and 3/16".
#[must_use]
pub fn standard_styles(imperial: bool) -> Vec<AnnotationStyle> {
    let make = |id: u64, name: &str, text_height: f64| AnnotationStyle {
        id,
        name: name.to_owned(),
        font: None,
        text_height,
        imperial,
        arrow_size: None,
        extension_offset: None,
        extension_extension: None,
        text_gap: None,
        linear_resolution: None,
    };
    if imperial {
        vec![
            make(0, "ANSI 1/8\"", 0.125),
            make(1, "ANSI 3/32\"", 3.0 / 32.0),
            make(2, "ANSI 3/16\"", 0.1875),
        ]
    } else {
        vec![
            make(0, "ISO 3.5", 3.5),
            make(1, "ISO 2.5", 2.5),
            make(2, "ISO 5", 5.0),
            make(3, "ISO 7", 7.0),
        ]
    }
}

impl AnnotationStyle {
    /// Its text height in a model of `unit` (on a layout, sizes are paper sizes and this is not
    /// needed).
    #[must_use]
    pub fn height_in(&self, unit: Unit) -> f64 {
        let millimetres = if self.imperial {
            self.text_height * 25.4
        } else {
            self.text_height
        };
        millimetres / unit.millimetres()
    }

    /// How many model units (or paper units on a layout of `paper`) one of the style's own size
    /// units is: its sizes are millimetres for ISO styles and inches for ANSI ones.
    #[must_use]
    pub fn scale_to(&self, unit: Unit, paper: Option<crate::layouts::PaperUnit>) -> f64 {
        let style_mm = if self.imperial { 25.4 } else { 1.0 };
        style_mm
            / paper.map_or_else(
                || unit.millimetres(),
                crate::layouts::PaperUnit::millimetres,
            )
    }

    /// Rhino's Arrow size: the arrowhead's length, the text height unless set.
    #[must_use]
    pub fn arrow_size(&self) -> f64 {
        self.arrow_size.unwrap_or(self.text_height)
    }

    /// Rhino's Extension line offset: the gap between a measured point and its extension line.
    #[must_use]
    pub fn extension_offset(&self) -> f64 {
        self.extension_offset.unwrap_or(self.text_height * 0.3)
    }

    /// Rhino's Extension line extension: how far extension lines reach past the dimension line.
    #[must_use]
    pub fn extension_extension(&self) -> f64 {
        self.extension_extension.unwrap_or(self.text_height * 0.4)
    }

    /// Rhino's Text gap: between the dimension line and its text.
    #[must_use]
    pub fn text_gap(&self) -> f64 {
        self.text_gap.unwrap_or(self.text_height * 0.3)
    }

    /// Rhino's Linear resolution: decimal places of millimetre and metre values.
    #[must_use]
    pub fn linear_resolution(&self) -> usize {
        self.linear_resolution.unwrap_or(2).into()
    }
}

impl Document {
    /// The annotation styles: the document's own, or the standard ones for its unit.
    #[must_use]
    pub fn annotation_styles(&self) -> Vec<AnnotationStyle> {
        if self.state.annotation_styles.is_empty() {
            standard_styles(matches!(self.state.unit, Unit::Feet | Unit::Inches))
        } else {
            self.state.annotation_styles.clone()
        }
    }

    /// The annotation style with this id, or the first.
    #[must_use]
    pub fn annotation_style(&self, id: u64) -> AnnotationStyle {
        let styles = self.annotation_styles();
        styles
            .iter()
            .find(|s| s.id == id)
            .or_else(|| styles.first())
            .cloned()
            .unwrap_or_else(|| standard_styles(false).remove(0))
    }

    /// Adds an annotation object; returns its id.
    pub fn add_annotation(&mut self, annotation: Annotation) -> u64 {
        self.add_shape(crate::Shape::Annotation(annotation))
    }

    /// The style new annotation takes.
    #[must_use]
    pub fn current_annotation_style(&self) -> u64 {
        let id = self.state.current_annotation_style;
        if self.annotation_styles().iter().any(|s| s.id == id) {
            id
        } else {
            self.annotation_styles().first().map_or(0, |s| s.id)
        }
    }

    /// Sets the style new annotation takes.
    pub fn set_current_annotation_style(&mut self, id: u64) {
        self.state.current_annotation_style = id;
    }

    /// The document's own styles, made from the standard ones the first time one is changed.
    fn own_styles(&mut self) -> &mut Vec<AnnotationStyle> {
        if self.state.annotation_styles.is_empty() {
            self.state.annotation_styles = self.annotation_styles();
        }
        &mut self.state.annotation_styles
    }

    /// Adds a copy of style `from`, named `name`; returns its id.
    pub fn copy_annotation_style(&mut self, from: u64, name: String) -> u64 {
        let mut style = self.annotation_style(from);
        let styles = self.own_styles();
        let id = styles.iter().map(|s| s.id).max().map_or(0, |m| m + 1);
        style.id = id;
        style.name = name;
        styles.push(style);
        id
    }

    /// Changes style `id`. Text in it keeps its size relative to the style: a note at the
    /// style's height follows a new height.
    pub fn change_annotation_style(&mut self, id: u64, change: impl FnOnce(&mut AnnotationStyle)) {
        let before = self.annotation_style(id);
        let unit = self.state.unit;
        let Some(style) = self.own_styles().iter_mut().find(|s| s.id == id) else {
            return;
        };
        change(style);
        let after = style.clone();
        let (old, new) = (before.height_in(unit), after.height_in(unit));
        if (old - new).abs() < 1e-12 && before.font == after.font {
            return;
        }
        for object in &mut self.state.objects {
            let crate::Shape::Annotation(Annotation::Text(text)) = &object.shape else {
                continue;
            };
            if text.style != id || object.space.is_some() {
                continue;
            }
            let mut text = text.clone();
            if (text.height - old).abs() < 1e-9 {
                text.height = new;
            }
            if text.font == before.font {
                text.font.clone_from(&after.font);
            }
            std::sync::Arc::make_mut(object).shape =
                crate::Shape::Annotation(Annotation::Text(text));
        }
    }

    /// Deletes style `id` if no annotation uses it and it is not the last; returns whether it
    /// was deleted.
    pub fn delete_annotation_style(&mut self, id: u64) -> bool {
        let used = self.state.objects.iter().any(
            |o| matches!(&o.shape, crate::Shape::Annotation(Annotation::Text(t)) if t.style == id),
        );
        if used || self.annotation_styles().len() <= 1 {
            return false;
        }
        self.own_styles().retain(|s| s.id != id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note() -> Text {
        Text {
            content: "Hall".to_owned(),
            origin: [1.0, 2.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            height: 3.5,
            font: None,
            align: TextAlign::Left,
            style: 0,
        }
    }

    #[test]
    fn text_moves_turns_and_scales_with_its_height() {
        let moved = note().translated([10.0, 0.0, 0.0]);
        assert_eq!(moved.origin, [11.0, 2.0, 0.0]);
        let turned = note().mapped(&Similarity::rotation(
            [0.0; 3],
            [0.0, 0.0, 1.0],
            std::f64::consts::FRAC_PI_2,
        ));
        assert!((turned.x[1] - 1.0).abs() < 1e-9);
        assert!((turned.y[0] + 1.0).abs() < 1e-9);
        let bigger = note().mapped(&Similarity::scaling([0.0; 3], 2.0));
        assert!((bigger.height - 7.0).abs() < 1e-9);
    }

    #[test]
    fn a_document_has_the_standard_styles_for_its_unit() {
        let mut doc = Document::default();
        assert_eq!(doc.annotation_styles()[0].name, "ISO 3.5");
        assert!((doc.annotation_style(0).height_in(Unit::Millimetres) - 3.5).abs() < 1e-12);
        doc.set_unit(Unit::Feet, false);
        let ansi = doc.annotation_style(0);
        assert_eq!(ansi.name, "ANSI 1/8\"");
        assert!((ansi.height_in(Unit::Feet) - 0.125 / 12.0).abs() < 1e-12);
    }

    #[test]
    fn styles_are_copied_made_current_changed_and_deleted_when_unused() {
        let mut doc = Document::default();
        let copy = doc.copy_annotation_style(0, "Titles".to_owned());
        assert_eq!(doc.annotation_style(copy).name, "Titles");
        doc.set_current_annotation_style(copy);
        assert_eq!(doc.current_annotation_style(), copy);
        let mut text = note();
        text.style = copy;
        let id = doc.add_annotation(Annotation::Text(text));
        // A note at the style's height follows the style's new height.
        doc.change_annotation_style(copy, |s| s.text_height = 7.0);
        let crate::Shape::Annotation(Annotation::Text(changed)) = &doc.state().objects[0].shape
        else {
            panic!("text");
        };
        assert!((changed.height - 7.0).abs() < 1e-12);
        assert!(!doc.delete_annotation_style(copy), "in use");
        doc.remove(&[id]);
        assert!(doc.delete_annotation_style(copy));
        assert_eq!(doc.current_annotation_style(), 0);
    }
}
