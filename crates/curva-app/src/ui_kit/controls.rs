//! The controls: on/off chips, option chips, tabs, the group tabs, the pill of
//! choices, dropdowns, the opener with its chevron, and icon buttons. Stock egui buttons, fields
//! and sliders are themed by [`super::apply`] and used as they are.

use super::{
    CHIP_GAP, CONTROL_PAD, CONTROL_RADIUS, GROUP_GAP, LINE, MARK, PILL_RADIUS, ROW, colours,
};
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
    let corner = rect.center() - galley.size() / 2.0;
    ui.painter().galley(corner, galley, colours::text());
    let shown = *on;
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, shown, ""));
    response
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
    ui.add(egui::Button::new(super::accented(text)).small().frame(true))
}

/// A tab among others: lit when it is the one showing.
pub(crate) fn tab(
    ui: &mut egui::Ui,
    showing: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    chosen_label(ui, showing, text)
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
    let corner = egui::pos2(
        rect.min.x + CONTROL_PAD,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(corner, galley, colours::text());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, ""));
    response
}

/// A row of group names, small, the chosen one strong and marked as chosen, the mark gliding to
/// it when the choice changes; the others quiet. Returns the one clicked, if another, and the
/// row's rectangle.
pub(crate) fn group_tabs(
    ui: &mut egui::Ui,
    names: &[String],
    chosen: usize,
) -> (Option<usize>, egui::Rect) {
    // The mark goes under the names, so it is kept a place before them.
    let behind = ui.painter().add(egui::Shape::Noop);
    let row = ui.horizontal(|ui| {
        // The names start where a tab button's words do, past its padding, so they stand right
        // over the tabs below (owner, 22 November 2026).
        let padding = ui.spacing().button_padding.x;
        super::with_gap(ui, 0.0, |ui| ui.add_space(padding));
        ui.spacing_mut().item_spacing.x = GROUP_GAP;
        let mut clicked = None;
        let mut chosen_rect = None;
        for (index, name) in names.iter().enumerate() {
            let text = egui::RichText::new(name).small();
            let text = if index == chosen {
                text.strong()
            } else {
                text.weak()
            };
            let response = ui
                .add(
                    egui::Label::new(text)
                        .selectable(false)
                        .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if index == chosen {
                chosen_rect = Some(response.rect);
            }
            if response.clicked() && index != chosen {
                clicked = Some(index);
            }
        }
        (clicked, chosen_rect)
    });
    let (clicked, chosen_rect) = row.inner;
    if let Some(chosen) = chosen_rect {
        let ctx = ui.ctx();
        let id = ui.scope_id().with("tab mark");
        let left = ctx.animate_value_with_time(id.with("left"), chosen.min.x, 0.25);
        let width = ctx.animate_value_with_time(id.with("width"), chosen.width(), 0.25);
        let mark = egui::Rect::from_min_size(
            egui::pos2(left, chosen.min.y),
            egui::vec2(width, chosen.height()),
        );
        ui.painter().set(
            behind,
            super::chosen_shape(mark.expand2(egui::vec2(6.0, 2.0))),
        );
    }
    (clicked, row.response.rect)
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
    crate::icons::paint(ui.painter(), rect.shrink(size * 0.15), name);
    response.on_hover_text(hint)
}

/// A small control drawn as a mark, such as a panel's hide or close: a clickable area at `rect`
/// with `mark` drawn in the quiet colour, or the accent under the pointer.
pub(crate) fn mark_button(
    ui: &egui::Ui,
    rect: egui::Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    hint: String,
    mark: impl FnOnce(&egui::Painter, egui::Pos2, egui::Stroke),
) -> egui::Response {
    let response = ui
        .interact(rect, ui.scope_id().with(id), egui::Sense::click())
        .on_hover_text(hint);
    mark(
        ui.painter(),
        rect.center(),
        colours::mark(MARK, response.hovered()),
    );
    response
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
