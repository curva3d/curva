//! The marks the kit's controls are made of, each drawn one way everywhere: chevrons, crosses,
//! the mark of a chosen thing, glows, the fades at a strip's edges, washes behind chosen rows.

use super::{CONTROL_RADIUS, FADE, MARK, colours};
use eframe::egui;

/// A V pointing along `towards` (a unit direction on screen), centred on `centre`, as the hide
/// control, the focus tab and the view picker draw it.
pub(crate) fn chevron(
    painter: &egui::Painter,
    centre: egui::Pos2,
    towards: egui::Vec2,
    stroke: egui::Stroke,
) {
    let side = egui::vec2(-towards.y, towards.x);
    let tip = centre + towards * 2.5;
    let back = centre - towards * 2.5;
    painter.line_segment([back + side * 4.5, tip], stroke);
    painter.line_segment([tip, back - side * 4.5], stroke);
}

/// The way down, for a chevron that opens a list.
pub(crate) const DOWN: egui::Vec2 = egui::vec2(0.0, 1.0);
/// The way up.
pub(crate) const UP: egui::Vec2 = egui::vec2(0.0, -1.0);

/// An X centred on `centre`, as the close control draws it.
pub(crate) fn cross(painter: &egui::Painter, centre: egui::Pos2, stroke: egui::Stroke) {
    let arm = 3.5;
    for (a, b) in [((-arm, -arm), (arm, arm)), ((arm, -arm), (-arm, arm))] {
        painter.line_segment(
            [centre + egui::vec2(a.0, a.1), centre + egui::vec2(b.0, b.1)],
            stroke,
        );
    }
}

/// How a chosen or switched-on thing is marked everywhere, as the chosen specialization in the
/// top bar (owner, 23 November 2026): a soft wash of the accent with a thin accent edge, its words
/// in the ordinary text colour; never a solid fill or an underline.
pub(crate) fn chosen_shape(rect: egui::Rect) -> egui::Shape {
    egui::Shape::Rect(egui::epaint::RectShape::new(
        rect,
        CONTROL_RADIUS,
        colours::chosen_fill(),
        colours::chosen_edge(),
        egui::StrokeKind::Inside,
    ))
}

/// Marks `rect` as chosen; see [`chosen_shape`].
pub(crate) fn chosen_mark(painter: &egui::Painter, rect: egui::Rect) {
    painter.add(chosen_shape(rect));
}

/// The wash behind a chosen row of a list or table.
pub(crate) fn chosen_wash(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, CONTROL_RADIUS / 2, colours::chosen_fill());
}

/// A control's ground under the pointer.
pub(crate) fn hover_wash(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, CONTROL_RADIUS, colours::hover());
}

/// A glow round `rect`, in the accent at `strength` (0 to 1), as after copying the file's path.
pub(crate) fn glow_outline(painter: &egui::Painter, rect: egui::Rect, strength: f32) {
    painter.rect_filled(rect, CONTROL_RADIUS, colours::accent_at(0.22 * strength));
    painter.rect_stroke(
        rect,
        CONTROL_RADIUS,
        egui::Stroke::new(MARK, colours::accent_at(strength)),
        egui::StrokeKind::Outside,
    );
}

/// The fade over a strip's edge where more of it is scrolled out of sight: from the panel's
/// colour at `edge` to nothing [`FADE`] inwards, `inwards` saying which way that is (+1 or -1).
pub(crate) fn edge_fade(painter: &egui::Painter, strip: egui::Rect, edge: f32, inwards: f32) {
    let mut mesh = egui::Mesh::default();
    let (top, bottom) = (strip.min.y, strip.max.y);
    for (x, colour) in [
        (edge, colours::panel()),
        (edge + inwards * FADE, egui::Color32::TRANSPARENT),
    ] {
        mesh.colored_vertex(egui::pos2(x, top), colour);
        mesh.colored_vertex(egui::pos2(x, bottom), colour);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// A small filled triangle pointing along `towards`, as a tree's open and shut arrows.
pub(crate) fn arrow_head(
    painter: &egui::Painter,
    centre: egui::Pos2,
    towards: egui::Vec2,
    colour: egui::Color32,
) {
    let side = egui::vec2(-towards.y, towards.x);
    let tip = centre + towards * 3.5;
    let back = centre - towards * 2.5;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, back + side * 3.5, back - side * 3.5],
        colour,
        egui::Stroke::NONE,
    ));
}

/// The dots of a drag grip, in a column of `rows` pairs centred on `centre`.
pub(crate) fn grip_dots(painter: &egui::Painter, centre: egui::Pos2, rows: usize) {
    let colour = colours::quiet();
    for row in 0..rows {
        let y = centre.y + (row as f32 - (rows as f32 - 1.0) / 2.0) * 4.0;
        for x in [centre.x - 2.0, centre.x + 2.0] {
            painter.circle_filled(egui::pos2(x, y), 1.0, colour);
        }
    }
}
