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

/// The pulsing glow along the panel at `rect` whose edge the pointer is on, strongest by the
/// mouse (the owner's choice, 22 November 2026).
fn glow_edges(ui: &egui::Ui, rect: egui::Rect) {
    ui_kit::glow::resize_glow_now(ui, rect);
}

/// The edges and corners a panel can be resized from: near one, the pointer becomes a resize
/// arrow and the panel's edge glows in the accent, strongest by the mouse, pulsing gently. The handles lie just inside the panel, so a drag
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
            glow_edges(ui, rect);
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
