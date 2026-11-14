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
}
