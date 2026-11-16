//! Annotation (spec `annotation.md`): text now, and dimensions, leaders, dots and hatches as they
//! come. An annotation is an object of its own kind, in the model or on a layout's sheet, and
//! takes its looks from a named annotation style.

use crate::{Document, Unit};
use curva_kernel::{Point, Similarity};
use serde::{Deserialize, Serialize};

mod angle;
mod arc_length;
mod hatch;
mod leader;
mod linear;
mod ordinate;
mod radial;
mod style;
mod text;

pub use angle::*;
pub use arc_length::*;
pub use hatch::*;
pub use leader::*;
pub use linear::*;
pub use ordinate::*;
pub use radial::*;
pub use style::*;
pub use text::*;

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
    /// An ordinate dimension: a point's X or Y from a base point.
    Ordinate(OrdinateDimension),
    /// An arc length dimension, as AutoCAD's.
    ArcLength(ArcLengthDimension),
    /// A leader: an arrow and line to a note (spec `annotation.md` §4).
    Leader(Leader),
    /// A dot: a label at a point that keeps its size on screen (spec `annotation.md` §4).
    Dot(Dot),
    /// A hatch: outlines filled with a pattern or solid (spec `annotation.md` §5).
    Hatch(Hatch),
    /// A scale bar for a detail view (spec `layouts.md` §4).
    ScaleBar(crate::sheet::ScaleBar),
    /// A table on a sheet: the sheet index or a revision table (spec `layouts.md` §4).
    Table(crate::sheet::Table),
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
            Self::Ordinate(o) => Self::Ordinate(o.transformed(&|p| plus(p, by, 1.0), &|d| d)),
            Self::ArcLength(a) => Self::ArcLength(a.transformed(&|p| plus(p, by, 1.0), &|d| d)),
            Self::Leader(l) => Self::Leader(l.transformed(&|p| plus(p, by, 1.0), &|d| d)),
            Self::Dot(d) => Self::Dot(Dot {
                at: plus(d.at, by, 1.0),
                ..d.clone()
            }),
            Self::Hatch(h) => Self::Hatch(h.with_loops(
                h.loops.iter().map(|c| c.translated(by)).collect(),
                &|p| plus(p, by, 1.0),
                &|d| d,
            )),
            Self::ScaleBar(b) => Self::ScaleBar(b.with_at(b.at.translated(by), 1.0)),
            Self::Table(t) => Self::Table(t.with_at(t.at.translated(by), 1.0)),
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
            Self::Ordinate(o) => Self::Ordinate(o.transformed(&point, &direction)),
            Self::ArcLength(a) => Self::ArcLength(a.transformed(&point, &direction)),
            Self::Leader(l) => Self::Leader(l.transformed(&point, &direction)),
            Self::Dot(d) => Self::Dot(Dot {
                at: point(d.at),
                ..d.clone()
            }),
            // As in Rhino, resizing a hatch keeps its pattern's size; turning it turns the
            // pattern with it.
            Self::Hatch(h) => Self::Hatch(h.with_loops(
                h.loops.iter().map(|c| c.mapped(change)).collect(),
                &point,
                &direction,
            )),
            Self::ScaleBar(b) => Self::ScaleBar(b.with_at(b.at.mapped(change), change.factor())),
            Self::Table(t) => Self::Table(t.with_at(t.at.mapped(change), change.factor())),
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
            Self::Ordinate(o) => Self::Ordinate(o.transformed(&point, &direction)),
            Self::ArcLength(a) => Self::ArcLength(a.transformed(&point, &direction)),
            Self::Leader(l) => Self::Leader(l.transformed(&point, &direction)),
            Self::Dot(d) => Self::Dot(Dot {
                at: point(d.at),
                ..d.clone()
            }),
            Self::Hatch(h) => Self::Hatch(
                h.with_loops(
                    h.loops
                        .iter()
                        .map(|c| c.mapped_affine(*m).unwrap_or_else(|_| c.clone()))
                        .collect(),
                    &point,
                    &direction,
                ),
            ),
            Self::ScaleBar(b) => {
                let at = b.at.placed(m);
                let stretch = at.height / b.at.height.max(1e-12);
                Self::ScaleBar(b.with_at(at, stretch))
            }
            Self::Table(t) => {
                let at = t.at.placed(m);
                let stretch = at.height / t.at.height.max(1e-12);
                Self::Table(t.with_at(at, stretch))
            }
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
            Self::Ordinate(o) => o.style,
            Self::ArcLength(a) => a.style,
            Self::Leader(l) => l.style,
            Self::ScaleBar(b) => b.at.style,
            Self::Table(t) => t.at.style,
            Self::Dot(_) | Self::Hatch(_) => 0,
        }
    }

    /// Points it at another annotation style; dots and hatches have none.
    pub fn set_style(&mut self, style: u64) {
        match self {
            Self::Text(t) => t.style = style,
            Self::Dimension(d) => d.style = style,
            Self::Radial(r) => r.style = style,
            Self::Angle(a) => a.style = style,
            Self::Ordinate(o) => o.style = style,
            Self::ArcLength(a) => a.style = style,
            Self::Leader(l) => l.style = style,
            Self::ScaleBar(b) => b.at.style = style,
            Self::Table(t) => t.at.style = style,
            Self::Dot(_) | Self::Hatch(_) => {}
        }
    }

    /// A dimension's typed text (`<>` stands for its value); `None` for a text.
    #[must_use]
    pub fn dimension_text(&self) -> Option<&Option<String>> {
        match self {
            Self::Text(_)
            | Self::Leader(_)
            | Self::Dot(_)
            | Self::Hatch(_)
            | Self::ScaleBar(_)
            | Self::Table(_) => None,
            Self::Dimension(d) => Some(&d.text),
            Self::Radial(r) => Some(&r.text),
            Self::Angle(a) => Some(&a.text),
            Self::Ordinate(o) => Some(&o.text),
            Self::ArcLength(a) => Some(&a.text),
        }
    }

    /// The dimension with other typed text; a text is unchanged.
    #[must_use]
    pub fn with_dimension_text(&self, text: Option<String>) -> Self {
        let mut changed = self.clone();
        match &mut changed {
            Self::Text(_)
            | Self::Leader(_)
            | Self::Dot(_)
            | Self::Hatch(_)
            | Self::ScaleBar(_)
            | Self::Table(_) => {}
            Self::Dimension(d) => d.text = text,
            Self::Radial(r) => r.text = text,
            Self::Angle(a) => a.text = text,
            Self::Ordinate(o) => o.text = text,
            Self::ArcLength(a) => a.text = text,
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

fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
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

    /// Adds a style as files bring them: one of the same name already held is kept and its id
    /// returned; otherwise this one, under a new id.
    pub fn add_annotation_style(&mut self, mut style: AnnotationStyle) -> u64 {
        if let Some(held) = self
            .annotation_styles()
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(style.name.trim()))
        {
            return held.id;
        }
        let styles = self.own_styles();
        let id = styles.iter().map(|s| s.id).max().map_or(0, |m| m + 1);
        style.id = id;
        styles.push(style);
        id
    }

    /// Points annotations at style `style`, leaving their geometry as it is.
    pub fn set_annotation_style_of(&mut self, ids: &[u64], style: u64) {
        for object in &mut self.state.objects {
            if !ids.contains(&object.id) {
                continue;
            }
            if let crate::Shape::Annotation(a) = &mut std::sync::Arc::make_mut(object).shape {
                a.set_style(style);
            }
        }
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
        // Text, dimensions and leaders take styles; dots and hatches have none.
        let used = self.state.objects.iter().any(|o| match &o.shape {
            crate::Shape::Annotation(Annotation::Dot(_) | Annotation::Hatch(_)) => false,
            crate::Shape::Annotation(annotation) => annotation.style() == id,
            _ => false,
        });
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

    fn ordinate(leader: Point, axis: OrdinateAxis) -> OrdinateDimension {
        OrdinateDimension {
            base: [10.0, 10.0, 0.0],
            point: [40.0, 25.0, 0.0],
            leader,
            axis,
            kinks: Some([2.0, 3.0]),
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: 0,
            text: None,
        }
    }

    #[test]
    fn an_ordinate_dimension_gives_one_coordinate_from_its_base() {
        let up = ordinate([45.0, 60.0, 0.0], OrdinateAxis::X);
        assert!((up.value() - 30.0).abs() < 1e-12);
        let left = ordinate([0.0, 20.0, 0.0], OrdinateAxis::Y);
        assert!((left.value() - 15.0).abs() < 1e-12);
        // Rhino implies X when the leader runs more along Y.
        assert_eq!(
            OrdinateAxis::implied([30.0, 15.0], [35.0, 50.0]),
            OrdinateAxis::X
        );
        assert_eq!(
            OrdinateAxis::implied([30.0, 15.0], [-10.0, 10.0]),
            OrdinateAxis::Y
        );
    }

    #[test]
    fn an_ordinate_dimensions_kinks_sit_where_rhino_puts_them() {
        // Measuring X with the leader above: the first kink is in line with the point, the
        // kink offsets below the leader's end; the second is in line with the end.
        let up = ordinate([45.0, 60.0, 0.0], OrdinateAxis::X);
        assert_eq!(up.kink_points(0.0), [[40.0, 55.0, 0.0], [45.0, 58.0, 0.0]]);
        // Measuring Y with the leader to the left: the offsets turn round.
        let left = ordinate([0.0, 20.0, 0.0], OrdinateAxis::Y);
        assert_eq!(left.kink_points(0.0), [[5.0, 25.0, 0.0], [2.0, 20.0, 0.0]]);
        // Unset kinks take the default given.
        let unset = OrdinateDimension {
            kinks: None,
            ..up.clone()
        };
        assert_eq!(
            unset.kink_points(4.0),
            [[40.0, 52.0, 0.0], [45.0, 56.0, 0.0]]
        );
    }

    #[test]
    fn an_ordinate_dimension_draws_its_leader_and_reads_along_it() {
        let style = &standard_styles(false)[0];
        let up = ordinate([45.0, 60.0, 0.0], OrdinateAxis::X);
        let parts = up.parts(style, 1.0, "30".to_owned());
        assert_eq!(parts.lines.len(), 3);
        assert!(parts.arrows.is_empty());
        // The leader starts the extension offset above the point and ends at the leader's end.
        let start = parts.lines[0].0;
        assert!((start[1] - (25.0 + style.extension_offset())).abs() < 1e-12);
        assert_eq!(parts.lines[2].1, [45.0, 60.0, 0.0]);
        // The text reads up the leader, beyond its end.
        assert_eq!(parts.text.x, [0.0, 1.0, 0.0]);
        assert!(parts.text.origin[1] > 60.0);
        assert_eq!(parts.text.align, TextAlign::Left);
        // Moved and turned, it keeps its value; resized, its kinks grow with it.
        let turned = Annotation::Ordinate(up.clone()).mapped(&Similarity::rotation(
            [0.0; 3],
            [0.0, 0.0, 1.0],
            1.0,
        ));
        let Annotation::Ordinate(turned) = turned else {
            unreachable!()
        };
        assert!((turned.value() - 30.0).abs() < 1e-9);
    }

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

    #[test]
    fn a_style_used_only_by_a_dimension_is_kept() {
        let mut doc = Document::default();
        let copy = doc.copy_annotation_style(0, "Dimensions".to_owned());
        let id = doc.add_annotation(Annotation::Dimension(Dimension {
            kind: DimensionKind::Aligned,
            first: [0.0; 3],
            second: [10.0, 0.0, 0.0],
            line: [5.0, 5.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: copy,
            text: None,
        }));
        assert!(
            !doc.delete_annotation_style(copy),
            "in use by the dimension"
        );
        doc.remove(&[id]);
        assert!(doc.delete_annotation_style(copy));
    }

    /// An arc of radius 10 about the origin from `from` to `to` degrees, its dimension arc at
    /// radius `at`, through the middle.
    fn arc_length(from: f64, to: f64, at: f64) -> ArcLengthDimension {
        let on = |r: f64, degrees: f64| {
            let (s, c) = degrees.to_radians().sin_cos();
            [r * c, r * s, 0.0]
        };
        ArcLengthDimension {
            centre: [0.0; 3],
            start: on(10.0, from),
            end: on(10.0, to),
            arc_at: on(at, (from + to) / 2.0),
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            style: 0,
            text: None,
        }
    }

    #[test]
    fn an_arc_length_dimension_measures_along_the_arc_counter_clockwise() {
        use std::f64::consts::PI;
        let half = arc_length(0.0, 180.0, 15.0);
        assert!((half.value() - 10.0 * PI).abs() < 1e-9);
        // From 300° on to 60°: a third of a turn, past the X axis.
        let third = arc_length(300.0, 420.0, 15.0);
        assert!((third.value() - 10.0 * 2.0 * PI / 3.0).abs() < 1e-9);
        // Moved and turned, it measures the same.
        let turned = Annotation::ArcLength(third.clone()).mapped(&Similarity::rotation(
            [5.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            1.0,
        ));
        let Annotation::ArcLength(turned) = turned else {
            panic!("not an arc length dimension");
        };
        assert!((turned.value() - third.value()).abs() < 1e-9);
    }

    #[test]
    fn an_arc_length_dimensions_extension_lines_are_parallel_under_a_quarter_turn() {
        let style = &standard_styles(false)[0];
        let offset = style.extension_offset();
        // A wide arc: the extension lines run out from the centre.
        let wide = arc_length(0.0, 120.0, 15.0).parts(style, 1.0, "20.94".to_owned());
        let first = wide.lines[0];
        assert!((first.0[0] - (10.0 + offset)).abs() < 1e-9 && first.0[1].abs() < 1e-9);
        assert!(first.1[1].abs() < 1e-9, "radial along X: {first:?}");
        // A narrow arc: both run along its middle direction, here straight up.
        let narrow = arc_length(60.0, 120.0, 15.0).parts(style, 1.0, "10.47".to_owned());
        for (a, b) in &narrow.lines[..2] {
            assert!((a[0] - b[0]).abs() < 1e-9, "parallel to Y: {a:?} {b:?}");
            assert!(b[1] > a[1]);
        }
        // The arrows sit on the dimension arc, the text beyond its middle under the symbol.
        for arrow in &narrow.arrows {
            assert!((arrow[0][0].hypot(arrow[0][1]) - 15.0).abs() < 1e-9);
        }
        assert!(narrow.text.origin[1] > 15.0);
        assert_eq!(narrow.text.content, "10.47");
        // A dimension arc inside the arc points its extension lines inwards.
        let inside = arc_length(0.0, 120.0, 6.0).parts(style, 1.0, String::new());
        let (a, b) = inside.lines[0];
        assert!(b[0] < a[0]);
    }
}
