//! What holds the controls: the panel frame, dialogs and notes, scroll areas, and the glass
//! behind a see-through panel.

use super::{LINE, PAD, PANEL_RADIUS, colours};
use eframe::egui;

/// The frame every panel is drawn in: rounded, edged and shadowed, glass or solid.
pub(crate) fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(crate::theme::current().glass)
        .stroke(egui::Stroke::new(LINE, colours::edge()))
        .corner_radius(PANEL_RADIUS)
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: colours::shadow(),
        })
        .inner_margin(PAD)
}

/// The panel frame, see-through (`glass`) or solid.
pub(crate) fn panel_frame_for(glass: bool) -> egui::Frame {
    panel_frame().fill(if glass {
        crate::theme::current().glass
    } else {
        colours::panel()
    })
}

/// A panel frame lit in the accent all round, for a box being typed into (text edited in place).
pub(crate) fn lit_panel_frame() -> egui::Frame {
    panel_frame()
        .stroke(colours::accent_line(super::UNDERLINE))
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 24,
            spread: 0,
            color: colours::accent_at(0.5),
        })
}

/// The blurred view behind a glass panel at `rect`, under the tint its frame paints.
pub(crate) fn glass_backdrop(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().add(egui::Shape::from(
        eframe::egui_wgpu::Callback::new_paint_callback(
            rect,
            curva_viewport::Glass {
                rect,
                radius: f32::from(PANEL_RADIUS),
            },
        ),
    ));
}

/// The light catching the top edge of a glass panel at `rect`, as on Apple's frosted glass: a
/// fine line inside the edge, brightest in the middle, fading towards the corners.
pub(crate) fn glass_edge_light(ui: &egui::Ui, rect: egui::Rect) {
    let radius = f32::from(PANEL_RADIUS);
    let (left, right, y) = (rect.min.x + radius, rect.max.x - radius, rect.min.y + 0.75);
    if right <= left {
        return;
    }
    let mut mesh = egui::Mesh::default();
    let steps = 16;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = left + (right - left) * t;
        let strength = (1.0 - (2.0 * t - 1.0).powi(2)).max(0.0);
        let colour = egui::Color32::from_white_alpha((120.0 * strength) as u8);
        mesh.colored_vertex(egui::pos2(x, y - 0.75), colour);
        mesh.colored_vertex(egui::pos2(x, y + 0.75), colour);
        if step > 0 {
            let i = (step * 2) as u32;
            mesh.add_triangle(i - 2, i - 1, i);
            mesh.add_triangle(i - 1, i, i + 1);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// A short note in a popup outlined in the accent, with its `pivot` corner at `at`, that the
/// pointer passes through: "copied", and the like.
pub(crate) fn note(
    ctx: &egui::Context,
    id: &str,
    at: egui::Pos2,
    pivot: egui::Align2,
    text: String,
) {
    egui::Area::new(egui::Id::NULL.with(("note", id)))
        .order(egui::Order::Tooltip)
        .fixed_pos(at)
        .pivot(pivot)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .stroke(colours::accent_line(LINE))
                .show(ui, |ui| ui.label(text));
        });
}

/// A popup's frame, for a floating list of controls.
pub(crate) fn popup_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::popup(ui.style())
}

/// Which ways a panel's contents scroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scroll {
    Down,
    Both,
    /// Down, kept at the bottom as more is added, as the command history.
    DownFromBottom,
}

/// A panel's contents, scrolling `way` when they do not fit, filling the panel.
pub(crate) fn scroll<R>(ui: &mut egui::Ui, way: Scroll, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let area = match way {
        Scroll::Down => egui::ScrollArea::vertical(),
        Scroll::Both => egui::ScrollArea::both(),
        Scroll::DownFromBottom => egui::ScrollArea::vertical().stick_to_bottom(true),
    };
    area.auto_shrink([false, false]).show(ui, add).inner
}

/// A list at most `height` tall, scrolling down when it is longer.
pub(crate) fn scroll_list<R>(
    ui: &mut egui::Ui,
    height: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::ScrollArea::vertical()
        .max_height(height)
        .show(ui, add)
        .inner
}

/// A conversation or log at most `height` tall, scrolling down, kept at its newest line.
pub(crate) fn scroll_to_bottom<R>(
    ui: &mut egui::Ui,
    height: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::ScrollArea::vertical()
        .max_height(height)
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, add)
        .inner
}

/// Where a dragged panel would dock, outlined in the accent over everything.
pub(crate) fn dock_preview(ctx: &egui::Context, spot: egui::Rect) {
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::NULL.with("dock preview"),
    ))
    .rect(
        spot,
        PANEL_RADIUS,
        colours::dock_fill(),
        egui::Stroke::new(super::MARK, colours::dock_edge()),
        egui::StrokeKind::Inside,
    );
}

/// A quiet caption in the window's corner behind everything, as Curva's version.
pub(crate) fn corner_caption(ctx: &egui::Context, window: egui::Rect, text: String) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::NULL.with("corner caption"),
    ));
    painter.text(
        window.max - egui::vec2(PAD, 4.0),
        egui::Align2::RIGHT_BOTTOM,
        text,
        super::caption_font(),
        colours::quiet(),
    );
}
