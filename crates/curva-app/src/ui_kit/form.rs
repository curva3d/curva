//! Settings laid out as a form: a quiet label on the left and its control beside it, sections
//! with a More switch for their less used settings, sliders, colour wells and list rows with a
//! small swatch, as the Style panel uses them.

use super::{CONTROL_RADIUS, ITEM_GAP, ROW, colours};
use eframe::egui;

/// How wide a form's labels are, so the controls line up.
pub(crate) const LABEL_WIDTH: f32 = 120.0;

/// One row of a form: `label` in the label column, then what `add` puts beside it.
pub(crate) fn form_row<R>(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        let text = egui::RichText::new(label.into()).color(colours::quiet());
        let left = egui::Layout::left_to_right(egui::Align::Center);
        ui.allocate_ui_with_layout(egui::vec2(LABEL_WIDTH, ROW), left, |ui| {
            ui.set_width(LABEL_WIDTH);
            ui.add(egui::Label::new(text).truncate());
        });
        add(ui)
    })
    .inner
}

/// A section's heading with, on the right, a small switch to show or hide its other settings:
/// `more` is its words ("More (3)" or "Less"), or `None` for a section with nothing more.
/// Returns whether the switch was clicked.
pub(crate) fn section_with_more(
    ui: &mut egui::Ui,
    title: impl Into<String>,
    more: Option<String>,
) -> bool {
    ui.add_space(ITEM_GAP / 2.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title.into()).strong());
        let Some(words) = more else {
            return false;
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::Button::new(egui::RichText::new(words).color(colours::quiet())).frame(false),
            )
            .clicked()
        })
        .inner
    })
    .inner
}

/// A slider for `value` between the ends of `range`, its number shown with `suffix`. Returns
/// whether it changed; it changes while being dragged, not only when let go.
pub(crate) fn slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    suffix: &str,
) -> bool {
    ui.add(
        egui::Slider::new(value, range)
            .step_by(f64::from(step))
            .fixed_decimals(decimals(step))
            .suffix(suffix),
    )
    .changed()
}

/// A colour well: a small swatch of `colour` that opens a colour picker. Returns whether it
/// changed.
pub(crate) fn colour_well(ui: &mut egui::Ui, colour: &mut [u8; 3]) -> bool {
    ui.color_edit_button_srgb(colour).changed()
}

/// A row in a list marked as chosen when `chosen`, with a small swatch shading from `top` to
/// `bottom` before its words, as each style in the Style panel shows its background.
pub(crate) fn swatch_item(
    ui: &mut egui::Ui,
    chosen: bool,
    (top, bottom): ([u8; 3], [u8; 3]),
    text: impl Into<String>,
) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, ROW + 2.0), egui::Sense::click());
    let painter = ui.painter();
    if chosen {
        super::chosen_mark(painter, rect);
    } else if response.hovered() {
        super::hover_wash(painter, rect);
    }
    let side = ROW - 6.0;
    let swatch = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + 6.0, rect.center().y - side / 2.0),
        egui::vec2(side, side),
    );
    let colour = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    let (upper, lower) = swatch.split_top_bottom_at_fraction(0.5);
    let round = CONTROL_RADIUS / 2;
    painter.rect_filled(
        upper,
        egui::CornerRadius {
            nw: round,
            ne: round,
            sw: 0,
            se: 0,
        },
        colour(top),
    );
    painter.rect_filled(
        lower,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: round,
            se: round,
        },
        colour(bottom),
    );
    painter.rect_stroke(
        swatch,
        round,
        egui::Stroke::new(super::LINE, colours::edge()),
        egui::StrokeKind::Inside,
    );
    let text = text.into();
    let galley = super::galley(ui, text.clone(), egui::TextStyle::Body.resolve(ui.style()));
    let at = egui::pos2(swatch.max.x + 8.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(at, galley, colours::text());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, chosen, &text));
    response
}

/// How many decimals a number changed in steps of `step` shows: none for whole steps.
fn decimals(step: f32) -> usize {
    match step {
        s if s >= 1.0 => 0,
        s if s >= 0.1 => 1,
        _ => 2,
    }
}
