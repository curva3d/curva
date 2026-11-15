//! Painting dots (Rhino's Dot, spec `annotation.md` §4): labels drawn on the screen at their
//! points, the same size however the view zooms or turns, in front of everything.

use crate::*;

/// Space between a dot's words and its edge, in points.
const DOT_PADDING: f32 = 4.0;

/// Black or white, whichever reads better on `colour`.
fn ink_on(colour: egui::Color32) -> egui::Color32 {
    let [r, g, b, _] = colour.to_array().map(f32::from);
    if 0.299 * r + 0.587 * g + 0.114 * b > 150.0 {
        egui::Color32::BLACK
    } else {
        egui::Color32::WHITE
    }
}

impl Curva {
    /// The dots on show in a pane, each a filled box in its colour with its words on it.
    pub(crate) fn paint_dots(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let selected = self.doc.selection();
        for object in &self.doc.state().objects {
            let Shape::Annotation(curva_document::annotation::Annotation::Dot(dot)) = &object.shape
            else {
                continue;
            };
            let Some(colour) = self.outlines.colour_of(object.id, &selected) else {
                continue;
            };
            let Some(p) = camera.project(dot.at, size) else {
                continue;
            };
            let [r, g, b] = colour.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
            let fill = egui::Color32::from_rgb(r, g, b);
            let centre = rect.min + egui::vec2(p[0] as f32, p[1] as f32);
            let galley = painter.layout(
                dot.content.clone(),
                egui::FontId::proportional(dot.height as f32),
                ink_on(fill),
                f32::INFINITY,
            );
            let around = egui::Rect::from_center_size(
                centre,
                galley.size() + egui::Vec2::splat(2.0 * DOT_PADDING),
            );
            painter.rect_filled(around, 3.0, fill);
            painter.galley(around.min + egui::Vec2::splat(DOT_PADDING), galley, fill);
        }
    }
}
