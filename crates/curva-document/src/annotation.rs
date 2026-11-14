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
