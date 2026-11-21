//! The edges and corners a panel is resized from.

use super::frames::PanelEvent;
use crate::*;

/// The edges and corners a panel at `rect` can be resized from, with where each is held and the
/// pointer it shows.
fn resize_zones(rect: egui::Rect) -> [(layout::Sides, egui::Rect, egui::CursorIcon); 8] {
    let side = |left, right, top, bottom| layout::Sides {
        left,
        right,
        top,
        bottom,
    };
    let reach = RESIZE_REACH;
    let (l, r, t, b) = (rect.min.x, rect.max.x, rect.min.y, rect.max.y);
    let zone = |x0: f32, y0: f32, x1: f32, y1: f32| {
        egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
    };
    [
        (
            side(true, false, true, false),
            zone(l, t, l + 2.0 * reach, t + 2.0 * reach),
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            side(false, true, false, true),
            zone(r - 2.0 * reach, b - 2.0 * reach, r, b),
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            side(false, true, true, false),
            zone(r - 2.0 * reach, t, r, t + 2.0 * reach),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            side(true, false, false, true),
            zone(l, b - 2.0 * reach, l + 2.0 * reach, b),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            side(true, false, false, false),
            zone(l, t + 2.0 * reach, l + reach, b - 2.0 * reach),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            side(false, true, false, false),
            zone(r - reach, t + 2.0 * reach, r, b - 2.0 * reach),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            side(false, false, true, false),
            zone(l + 2.0 * reach, t, r - 2.0 * reach, t + reach),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            side(false, false, false, true),
            zone(l + 2.0 * reach, b - reach, r - 2.0 * reach, b),
            egui::CursorIcon::ResizeVertical,
        ),
    ]
}

/// A gentle pulse along the edges of `rect` being held.
fn pulse_edges(ui: &egui::Ui, rect: egui::Rect, sides: layout::Sides) {
    let time = ui.input(|input| input.time) as f32;
    let strength = 0.55 + 0.45 * (time * 5.0).sin();
    let stroke = egui::Stroke::new(2.0, theme::ACCENT.gamma_multiply(strength));
    let painter = ui.painter();
    if sides.left {
        painter.line_segment([rect.left_top(), rect.left_bottom()], stroke);
    }
    if sides.right {
        painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    }
    if sides.top {
        painter.line_segment([rect.left_top(), rect.right_top()], stroke);
    }
    if sides.bottom {
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
    }
    ui.ctx().request_repaint();
}

/// The edges and corners a panel can be resized from: near one, the pointer becomes a resize
/// arrow and the edge pulses gently in the accent. The handles lie just inside the panel, so a drag
/// from one never reaches the viewport behind. Returns the drag, if one is under way.
pub(crate) fn resize_handles(
    ui: &egui::Ui,
    place: &layout::Place,
    rect: egui::Rect,
) -> Option<PanelEvent> {
    let movable = layout::Sides::movable(place);
    let mut event = None;
    for (sides, area, cursor) in resize_zones(rect) {
        // A corner needs both its sides to be movable, an edge its one.
        if sides.within(movable) != sides {
            continue;
        }
        let key = (sides.left, sides.right, sides.top, sides.bottom);
        let response = ui
            .interact(
                area,
                ui.scope_id().with(("resize", key)),
                egui::Sense::drag(),
            )
            .on_hover_cursor(cursor);
        if response.hovered() || response.dragged() {
            pulse_edges(ui, rect, sides);
        }
        // The whole way since the press, not this frame's step, so the edge stays under the
        // pointer even after the panel stopped at its smallest and the pointer went on (#470).
        let since = ui.input(|input| input.pointer.press_origin());
        if response.dragged()
            && let (Some(now), Some(from)) = (response.interact_pointer_pos(), since)
        {
            event = Some(PanelEvent::Resized(sides, now - from));
        }
    }
    event
}
