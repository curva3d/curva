//! The controls: on/off chips, option chips, tabs, the group tabs, the pill of
//! choices, dropdowns, the opener with its chevron, and icon buttons. Stock egui buttons, fields
//! and sliders are themed by [`super::apply`] and used as they are.

use super::{CHIP_GAP, CONTROL_PAD, CONTROL_RADIUS, LINE, MARK, PILL_RADIUS, ROW, colours};
use eframe::egui;

/// A named on/off chip: marked as chosen when on (see [`super::chosen_shape`]), like a control
/// at rest when off.
/// Clicking it turns it over; the response says `changed` when it did.
pub(crate) fn toggle(
    ui: &mut egui::Ui,
    on: &mut bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let galley = text.into().into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let size = egui::vec2(galley.size().x + 2.0 * CONTROL_PAD, ROW);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    ui.painter().add(chip_shape(rect, *on, response.hovered()));
    pressed_wash(ui, &response, rect);
    let name = galley.text().to_owned();
    let corner = rect.center() - galley.size() / 2.0;
    ui.painter().galley(corner, galley, colours::text());
    let shown = *on;
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, shown, &name));
    response
}

/// A darker wash over a control while it is pressed, so every click is answered (Apple's
/// guidelines: custom buttons always have a pressed state).
fn pressed_wash(ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) {
    if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, CONTROL_RADIUS, colours::press());
    }
}

/// A [`toggle`] that can only be changed while `enabled`, greyed out otherwise, for a choice that
/// only means something with another one on.
pub(crate) fn toggle_when(
    ui: &mut egui::Ui,
    enabled: bool,
    on: &mut bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| toggle(ui, on, text)).inner
}

/// A chip's ground: marked as chosen when on; the hover or rest colour when off.
fn chip_shape(rect: egui::Rect, on: bool, hovered: bool) -> egui::Shape {
    match (on, hovered) {
        (true, _) => super::chosen_shape(rect),
        (false, false) => egui::Shape::rect_filled(rect, CONTROL_RADIUS, colours::control()),
        (false, true) => egui::Shape::rect_filled(rect, CONTROL_RADIUS, colours::hover()),
    }
}

/// A run of chips side by side, [`CHIP_GAP`] apart, wrapping when the row is full.
pub(crate) fn chip_row<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = CHIP_GAP;
        add(ui)
    })
    .inner
}

/// A small button with its name in the accent, for an answer a click gives, such as a command's
/// options.
pub(crate) fn option_chip(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    // Outlined and neutral: the accent marks only what is chosen (owner, 23 November 2026).
    ui.add(
        egui::Button::new(egui::RichText::new(text))
            .small()
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(LINE, colours::edge())),
    )
}

/// A tab among others: lit when it is the one showing.
pub(crate) fn tab(
    ui: &mut egui::Ui,
    showing: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    chosen_label(ui, showing, text)
}

/// An item in a list, as the command matches: marked as chosen when `chosen`.
pub(crate) fn list_item(
    ui: &mut egui::Ui,
    chosen: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    chosen_label(ui, chosen, text)
}

/// A choice filling the row, its words in the middle, marked as chosen when `chosen`, as the
/// Layouts panel's Model.
pub(crate) fn wide_choice(
    ui: &mut egui::Ui,
    chosen: bool,
    text: impl Into<String>,
) -> egui::Response {
    let size = egui::vec2(ui.available_width(), ROW + 4.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if chosen {
        super::chosen_mark(ui.painter(), rect);
    } else {
        let fill = if response.hovered() {
            colours::hover()
        } else {
            colours::control()
        };
        ui.painter().rect_filled(rect, CONTROL_RADIUS, fill);
    }
    let galley = super::galley(ui, text.into(), egui::TextStyle::Button.resolve(ui.style()));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, colours::text());
    response
}

/// Words that can be chosen among others, as a tab or a list's item: marked as chosen when
/// `chosen`, washed under the pointer otherwise.
fn chosen_label(
    ui: &mut egui::Ui,
    chosen: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let galley = text.into().into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let size = egui::vec2(galley.size().x + 2.0 * CONTROL_PAD, ROW);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if chosen {
        super::chosen_mark(ui.painter(), rect);
    } else if response.hovered() {
        super::hover_wash(ui.painter(), rect);
    }
    pressed_wash(ui, &response, rect);
    let corner = egui::pos2(
        rect.min.x + CONTROL_PAD,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(corner, galley, colours::text());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, ""));
    response
}

/// Room either side of a name inside its part of a pill, above and below it, and between the
/// pill's edge and its parts.
pub(crate) const PILL_PAD: egui::Vec2 = egui::vec2(12.0, 4.0);
pub(crate) const PILL_INSET: f32 = 3.0;

/// The left and right edges of each part of a pill, for names `widths` wide, side by side and
/// centred on `centre`.
pub(crate) fn pill_segments(widths: &[f32], centre: f32) -> Vec<(f32, f32)> {
    let total: f32 = widths.iter().map(|w| w + 2.0 * PILL_PAD.x).sum();
    let mut left = centre - total / 2.0;
    widths
        .iter()
        .map(|w| {
            let right = left + w + 2.0 * PILL_PAD.x;
            let segment = (left, right);
            left = right;
            segment
        })
        .collect()
}

/// Choices side by side in one pill centred in `rect`, the chosen one lit in the accent, in bold
/// (the owner's choice E, 21 November 2026, #481). Returns the one clicked.
pub(crate) fn pill(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    names: &[String],
    chosen: usize,
) -> Option<usize> {
    let font = super::bold_font();
    let galleys: Vec<_> = names
        .iter()
        .map(|name| super::galley(ui, name.clone(), font.clone()))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x).collect();
    let segments = pill_segments(&widths, rect.center().x);
    let height = galleys.first().map_or(16.0, |g| g.size().y) + 2.0 * PILL_PAD.y;
    let row = |(left, right): (f32, f32)| {
        egui::Rect::from_min_max(
            egui::pos2(left, rect.center().y - height / 2.0),
            egui::pos2(right, rect.center().y + height / 2.0),
        )
    };
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return None;
    };
    let painter = ui.painter().clone();
    painter.rect_filled(
        row((first.0, last.1)).expand(PILL_INSET),
        PILL_RADIUS,
        colours::window(),
    );
    let mut clicked = None;
    for (index, (segment, galley)) in segments.iter().zip(galleys).enumerate() {
        let cell = row(*segment);
        let hit = ui.interact(
            cell,
            ui.scope_id().with(("pill", index)),
            egui::Sense::click(),
        );
        if index == chosen {
            super::chosen_mark(&painter, cell);
        } else if hit.hovered() {
            painter.rect_filled(cell, CONTROL_RADIUS, colours::panel());
        }
        let colour = if index == chosen {
            colours::text()
        } else {
            colours::quiet()
        };
        painter.galley(cell.center() - galley.size() / 2.0, galley, colour);
        if hit.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

/// A dropdown showing `selected`, its list made by `add`, as wide as the stock one.
pub(crate) fn dropdown<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    selected: impl Into<egui::WidgetText>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .show_ui(ui, add)
}

/// A dropdown `width` wide, for short choices.
pub(crate) fn dropdown_sized<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    selected: impl Into<egui::WidgetText>,
    width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(width)
        .show_ui(ui, add)
}

/// One item in a dropdown's or a menu's list, lit when it is the one chosen.
pub(crate) fn choice(
    ui: &mut egui::Ui,
    chosen: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    chosen_label(ui, chosen, text)
}

/// One item in a dropdown's list that sets `current` to `value` when clicked, marked as chosen
/// while it is the one set.
pub(crate) fn pick<T: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    value: T,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let mut response = chosen_label(ui, *current == value, text);
    if response.clicked() && *current != value {
        *current = value;
        response.mark_changed();
    }
    response
}

/// A dropdown with its name beside it.
pub(crate) fn labelled_dropdown<R>(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    selected: impl Into<egui::WidgetText>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    egui::ComboBox::from_label(label)
        .selected_text(selected)
        .show_ui(ui, add)
}

/// A control that opens something, filling the row: a quiet `prefix`, the `name` it shows, and
/// a chevron at its end, washed and outlined in the accent so it reads as a button (#471).
pub(crate) fn opener(ui: &mut egui::Ui, prefix: &str, name: &str) -> egui::Response {
    let mut label = egui::text::LayoutJob::default();
    let font = egui::TextStyle::Button.resolve(ui.style());
    let small = egui::FontId::proportional(font.size - 2.0);
    let format = |font_id, color| egui::TextFormat {
        font_id,
        color,
        valign: egui::Align::Center,
        ..Default::default()
    };
    label.append(prefix, 0.0, format(small, colours::quiet()));
    label.append(name, 4.0, format(font.clone(), colours::text()));
    // Room at the end for the chevron, so it never sits on the words.
    label.append(" ", 14.0, format(font, colours::text()));
    let button = ui.add_sized(
        [ui.available_width(), ROW + 2.0],
        egui::Button::new(label)
            .fill(colours::opener_fill())
            .stroke(egui::Stroke::new(LINE, colours::opener_edge())),
    );
    let at = egui::pos2(button.rect.max.x - 12.0, button.rect.center().y);
    super::chevron(ui.painter(), at, super::DOWN, colours::mark(MARK, false));
    button
}

/// A square button `size` across showing the icon `name`, centred on its ink, with a wash under
/// the pointer and the name on hover. Draws the name instead where there is no icon yet.
pub(crate) fn icon_button(ui: &mut egui::Ui, name: &str, size: f32, hint: &str) -> egui::Response {
    if !crate::icons::has(name) {
        return ui.button(name).on_hover_text(hint);
    }
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::click());
    if response.hovered() {
        super::hover_wash(ui.painter(), rect);
    }
    pressed_wash(ui, &response, rect);
    crate::icons::paint(ui.painter(), rect.shrink(size * 0.15), name);
    // Named for screen readers by what it does, as its tooltip says.
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, hint));
    response.on_hover_text(hint)
}

/// A small control drawn as a mark, such as a panel's hide or close: a clickable area at `rect`
/// with `mark` drawn in the quiet colour, or the accent under the pointer. It shows only while
/// the pointer is `near` (over its panel), fading in, so resting panels stay quiet (Apple's
/// guidelines: controls appear when wanted).
pub(crate) fn mark_button(
    ui: &egui::Ui,
    rect: egui::Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    hint: String,
    near: bool,
    mark: impl FnOnce(&egui::Painter, egui::Pos2, egui::Stroke),
) -> egui::Response {
    let id = ui.scope_id().with(id);
    let shown = super::fade(ui.ctx(), id.with("shown"), near);
    let response = ui.interact(rect, id, egui::Sense::click());
    if shown <= 0.0 {
        return response;
    }
    let response = response.on_hover_text(hint);
    let stroke = colours::mark(MARK, response.hovered());
    let stroke = egui::Stroke::new(stroke.width, stroke.color.gamma_multiply(shown));
    mark(ui.painter(), rect.center(), stroke);
    response
}

/// Closely related choices side by side in one control, the chosen one marked: Apple's segmented
/// control, for the command groups and for settings such as Theme and Units. Returns the one
/// clicked, if another.
pub(crate) fn segmented(ui: &mut egui::Ui, names: &[String], chosen: usize) -> Option<usize> {
    let mut clicked = None;
    egui::Frame::new()
        .fill(colours::control())
        .corner_radius(CONTROL_RADIUS)
        .inner_margin(2)
        .show(ui, |ui| {
            super::with_gap(ui, super::TIGHT_GAP, |ui| {
                ui.horizontal(|ui| {
                    for (index, name) in names.iter().enumerate() {
                        let response = segment(ui, index == chosen, name);
                        if response.clicked() && index != chosen {
                            clicked = Some(index);
                        }
                    }
                });
            });
        });
    clicked
}

/// One segment of a [`segmented`] control.
fn segment(ui: &mut egui::Ui, chosen: bool, name: &str) -> egui::Response {
    let galley = super::galley(
        ui,
        name.to_owned(),
        egui::TextStyle::Button.resolve(ui.style()),
    );
    let size = egui::vec2(galley.size().x + 2.0 * CONTROL_PAD, ROW - 4.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if chosen {
        super::chosen_mark(ui.painter(), rect);
    } else if response.hovered() {
        super::hover_wash(ui.painter(), rect);
    }
    pressed_wash(ui, &response, rect);
    let at = rect.center() - galley.size() / 2.0;
    ui.painter().galley(at, galley, colours::text());
    let named = name.to_owned();
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, &named));
    response
}

/// A choice between two, as a segmented control: `words[0]` when `on` is false, `words[1]` when
/// true. Returns whether it changed.
pub(crate) fn either(ui: &mut egui::Ui, on: &mut bool, words: [String; 2]) -> bool {
    match segmented(ui, &words, usize::from(*on)) {
        Some(index) => {
            *on = index == 1;
            true
        }
        None => false,
    }
}

/// A thin upright line between groups of controls in a row, such as the command groups.
pub(crate) fn divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(13.0, ROW), egui::Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.shrink2(egui::vec2(0.0, 3.0)).y_range(),
        egui::Stroke::new(LINE, colours::edge()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chip_shows_on_in_the_accent_and_off_like_a_control() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(40.0, ROW));
        // On: the accent's wash and edge, as every chosen thing (owner, 23 November 2026).
        assert_eq!(
            chip_shape(rect, true, false),
            super::super::chosen_shape(rect)
        );
        let off = |fill| egui::Shape::rect_filled(rect, CONTROL_RADIUS, fill);
        assert_eq!(chip_shape(rect, false, false), off(colours::control()));
        assert_eq!(chip_shape(rect, false, true), off(colours::hover()));
    }
}
