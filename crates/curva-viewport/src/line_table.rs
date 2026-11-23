//! Each kind of line's look in a view, from its style, as the graphics card reads it: the line
//! program looks a line's kind up here, so a change to the style shows at once, with nothing
//! rebuilt.

use crate::{LINE_KINDS, LineKind};
use curva_document::style::{self, FacedKind, LineWidth, ViewStyle};

/// Where hidden lines' look is kept, after the kinds of line.
pub(crate) const HIDDEN_SLOT: usize = LINE_KINDS;
/// How many looks a table holds: one for each kind of line, and one for hidden lines.
pub(crate) const LINE_SLOTS: usize = LINE_KINDS + 1;
/// How many four-number rows the graphics card reads: two for each look.
pub(crate) const LINE_ROWS: usize = 2 * LINE_SLOTS;

/// One kind of line's look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KindLook {
    /// Whether these lines are drawn.
    pub shown: bool,
    /// Their colour, or `None` for each line's own.
    pub colour: Option<[f32; 3]>,
    /// Each line's own width times this.
    pub factor: f32,
    /// A width in millimetres instead of each line's own.
    pub fixed: Option<f32>,
}

impl KindLook {
    /// As each line is given.
    pub const GIVEN: Self = Self {
        shown: true,
        colour: None,
        factor: 1.0,
        fixed: None,
    };
}

/// The look of each kind of line ([`LineKind`]), and of hidden lines, in one view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineTable(pub [KindLook; LINE_SLOTS]);

impl LineTable {
    /// Every line as given.
    pub const GIVEN: Self = Self([KindLook::GIVEN; LINE_SLOTS]);

    /// The look of lines of `kind`.
    #[must_use]
    pub fn of(&self, kind: LineKind) -> &KindLook {
        &self.0[kind as usize]
    }

    /// The look of hidden lines.
    #[must_use]
    pub fn hidden(&self) -> &KindLook {
        &self.0[HIDDEN_SLOT]
    }

    /// The table as the graphics card reads it: for each look, its colour and whether to use
    /// it, then its factor, its fixed width (below zero for none) and whether it shows.
    pub(crate) fn rows(&self) -> [[f32; 4]; LINE_ROWS] {
        let mut rows = [[0.0; 4]; LINE_ROWS];
        for (slot, look) in self.0.iter().enumerate() {
            let [r, g, b] = look.colour.unwrap_or([0.0; 3]);
            rows[2 * slot] = [r, g, b, if look.colour.is_some() { 1.0 } else { 0.0 }];
            rows[2 * slot + 1] = [
                look.factor,
                look.fixed.unwrap_or(-1.0),
                if look.shown { 1.0 } else { 0.0 },
                0.0,
            ];
        }
        rows
    }

    /// The table for `view`: each kind of line's colour, width and whether it shows.
    #[must_use]
    pub fn from_style(view: &ViewStyle) -> Self {
        let weight = view.line_rules.weight;
        let look = |shown: bool, colour: Option<[f32; 3]>, width: LineWidth| {
            let (factor, fixed) = match width {
                LineWidth::Object(factor) => (factor * weight, None),
                LineWidth::Fixed(mm) => (1.0, Some(mm * weight)),
            };
            KindLook {
                shown,
                colour,
                factor,
                fixed,
            }
        };
        let edges = &view.nurbs.edges;
        let edge_colour = colour(edges.colour);
        // A secondary line's own colour, or the edges' of its kind.
        let or_edges = |own: Option<style::LineColour>, kind: FacedKind| {
            own.map_or_else(|| colour(view.edges(kind).colour), colour)
        };
        let mut table = Self::GIVEN;
        let mut set = |kind: LineKind, value: KindLook| table.0[kind as usize] = value;
        set(
            LineKind::Curve,
            look(
                view.curves.shown,
                colour(view.curves.colour),
                view.curves.width,
            ),
        );
        set(LineKind::Edge, look(edges.shown, edge_colour, edges.width));
        // Seams show where faces do not hide the smooth surface they cut across.
        let seams = edges.shown && view.nurbs.tangent_seams && !view.solid();
        set(LineKind::Seam, look(seams, edge_colour, edges.width));
        let wires = &view.meshes.wires;
        let wire_colour = or_edges(wires.colour, FacedKind::Mesh);
        set(
            LineKind::MeshWire,
            look(wires.shown, wire_colour, LineWidth::Object(1.0)),
        );
        for (kind, line) in [
            (LineKind::SubdEdge, &view.subd.creases),
            (LineKind::SubdWire, &view.subd.smooth),
        ] {
            let line_colour = or_edges(line.colour, FacedKind::Subd);
            set(kind, look(line.shown, line_colour, line.width));
        }
        let annotation = &view.annotation;
        let annotation_colour = colour(annotation.colour);
        set(
            LineKind::Annotation,
            look(annotation.text, annotation_colour, LineWidth::Object(1.0)),
        );
        let hidden = &view.hidden_lines;
        table.0[HIDDEN_SLOT] = look(hidden.shown, Some(hidden.colour.to_f32()), hidden.width);
        table
    }
}

/// A line colour as the graphics card takes it: `None` for each line's own.
fn colour(colour: style::LineColour) -> Option<[f32; 3]> {
    match colour {
        style::LineColour::Object => None,
        style::LineColour::One(c) => Some(c.to_f32()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::style::Colour;

    #[test]
    fn shaded_darkens_edges_and_leaves_curves_their_own_colour() {
        let table = LineTable::from_style(&ViewStyle::shaded());
        assert_eq!(table.of(LineKind::Edge).colour, Some(Colour::EDGE.to_f32()));
        assert_eq!(table.of(LineKind::Curve).colour, None);
        assert_eq!(
            table.of(LineKind::MeshWire).colour,
            None,
            "wires in their own colour"
        );
        assert!(!table.of(LineKind::Seam).shown, "solid faces hide seams");
        assert_eq!(*table.of(LineKind::Given), KindLook::GIVEN);
        assert!(!table.hidden().shown);
    }

    #[test]
    fn wireframe_and_ghosted_keep_light_edges_and_seams() {
        for view in [ViewStyle::wireframe(), ViewStyle::ghosted()] {
            let table = LineTable::from_style(&view);
            assert_eq!(table.of(LineKind::Edge).colour, None, "{}", view.name);
            assert!(table.of(LineKind::Seam).shown, "{}", view.name);
        }
    }

    #[test]
    fn hidden_puts_every_model_line_in_ink() {
        let table = LineTable::from_style(&ViewStyle::hidden());
        let black = Some([0.0; 3]);
        for kind in [
            LineKind::Curve,
            LineKind::Edge,
            LineKind::MeshWire,
            LineKind::SubdEdge,
            LineKind::SubdWire,
            LineKind::Annotation,
        ] {
            assert_eq!(table.of(kind).colour, black, "{kind:?}");
        }
        assert_eq!(
            table.of(LineKind::Given).colour,
            None,
            "previews and locked keep theirs"
        );
        let technical = LineTable::from_style(&ViewStyle::technical());
        assert!(technical.hidden().shown && technical.hidden().colour == black);
    }

    #[test]
    fn widths_follow_the_object_or_are_fixed_and_scale_with_the_line_weight() {
        let mut view = ViewStyle::shaded();
        view.line_rules.weight = 2.0;
        view.curves.width = LineWidth::Fixed(0.35);
        view.nurbs.edges.width = LineWidth::Object(1.5);
        view.nurbs.edges.shown = false;
        let table = LineTable::from_style(&view);
        assert_eq!(table.of(LineKind::Curve).fixed, Some(0.7));
        let edge = table.of(LineKind::Edge);
        assert_eq!((edge.factor, edge.fixed, edge.shown), (3.0, None, false));
    }

    #[test]
    fn rows_say_each_look_in_two_rows() {
        let table = LineTable::from_style(&ViewStyle::hidden());
        let rows = table.rows();
        let edge = 2 * LineKind::Edge as usize;
        assert_eq!(rows[edge], [0.0, 0.0, 0.0, 1.0], "black, used");
        assert_eq!(rows[edge + 1], [1.0, -1.0, 1.0, 0.0], "own width, shown");
        assert_eq!(rows[0][3], 0.0, "given lines keep their colour");
    }
}
