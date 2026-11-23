//! Dialogs: one look for every question and every set of options Curva asks for in the middle of
//! the window, such as a format's import or export options, the mesh settings, getting a model
//! back after a crash, or an agent asking leave. A quiet title centred along the top as on a
//! panel, the contents, then the buttons in a row at the bottom right, the main one marked
//! in the accent. Escape answers with the last button, which is Cancel or its like.

use super::{CONTROL_PAD, ITEM_GAP, ROW, ROW_GAP, colours};
use eframe::egui;

/// The narrowest and widest a dialog is.
const NARROWEST: f32 = 320.0;
const WIDEST: f32 = 440.0;

/// A button in a dialog's row: its words, and whether it is the main answer.
pub(crate) struct Choice {
    pub(crate) word: String,
    pub(crate) main: bool,
}

/// The main answer, such as OK or Import, in the accent.
pub(crate) fn main_choice(word: String) -> Choice {
    Choice { word, main: true }
}

/// Another answer, such as Cancel.
pub(crate) fn other_choice(word: String) -> Choice {
    Choice { word, main: false }
}

/// A dialog in the middle of the window titled `title`, its contents drawn by `contents` and
/// `choices` as buttons along its bottom, in the order given, from the left. Returns the button
/// clicked, as an index into `choices`; Return answers with the main one, Escape or a click
/// outside with the last.
pub(crate) fn dialog(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    choices: &[Choice],
    contents: impl FnOnce(&mut egui::Ui),
) -> Option<usize> {
    let mut answer = None;
    let main = choices.iter().position(|c| c.main);
    let last = choices.len().checked_sub(1);
    // Modal: the window dims behind it and takes no clicks until it is answered (Apple's
    // guidelines; owner, 23 November 2026). Always solid, so the question reads clearly.
    let shown = egui::Modal::new(egui::Id::NULL.with(("dialog", id)))
        .frame(super::panel_frame_for(false))
        .backdrop_color(colours::backdrop())
        .show(ctx, |ui| {
            ui.set_min_width(NARROWEST);
            ui.set_max_width(WIDEST);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .font(super::title_font())
                        .color(colours::quiet()),
                );
            });
            ui.add_space(ROW_GAP);
            contents(ui);
            ui.add_space(ITEM_GAP);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (index, choice) in choices.iter().enumerate().rev() {
                    let key = if Some(index) == main {
                        Some("Return")
                    } else if Some(index) == last {
                        Some("Esc")
                    } else {
                        None
                    };
                    let label = with_key(ui, &choice.word, key);
                    let clicked = if choice.main {
                        main_button_job(ui, label)
                    } else {
                        ui.add(egui::Button::new(label).min_size(button_size()))
                    };
                    if clicked.clicked() {
                        answer = Some(index);
                    }
                }
            });
        });
    // Escape or a click on the dimmed window answers with the last button, Return with the main.
    if answer.is_none() && shown.should_close() {
        answer = last;
    }
    let typing = ctx.text_edit_focused();
    if answer.is_none() && !typing && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        answer = main;
    }
    answer
}

/// A button's words with, quieter and smaller after them, the key that presses it.
fn with_key(ui: &egui::Ui, word: &str, key: Option<&str>) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    let format = |font_id, color| egui::TextFormat {
        font_id,
        color,
        valign: egui::Align::Center,
        ..Default::default()
    };
    job.append(word, 0.0, format(font, colours::text()));
    if let Some(key) = key {
        job.append(key, 8.0, format(super::caption_font(), colours::quiet()));
    }
    job
}

/// The least a dialog's button takes, so short words still make a button worth aiming at.
fn button_size() -> egui::Vec2 {
    egui::vec2(72.0, ROW)
}

/// The main answer's button with `label` (words and key), marked as a chosen thing is.
fn main_button_job(ui: &mut egui::Ui, label: egui::text::LayoutJob) -> egui::Response {
    ui.add(
        egui::Button::new(label)
            .fill(colours::chosen_fill())
            .stroke(colours::accent_line(super::LINE))
            .min_size(button_size() + egui::vec2(CONTROL_PAD, 0.0)),
    )
}

/// A button for an action that destroys something, such as Delete Layer: its words in red, so it
/// never reads as the safe choice (Apple's guidelines; owner, 23 November 2026).
pub(crate) fn danger_button(ui: &mut egui::Ui, word: impl Into<String>) -> egui::Response {
    ui.add(egui::Button::new(
        egui::RichText::new(word).color(colours::danger()),
    ))
}

/// An item in a menu: its words, and on the right, quieter, the keys that do the same. Dimmed
/// and inert while it cannot act, so people can still see it is there.
pub(crate) fn menu_item(
    ui: &mut egui::Ui,
    enabled: bool,
    word: String,
    keys: Option<&str>,
) -> egui::Response {
    let mut button = egui::Button::new(word).min_size(egui::vec2(220.0, ROW));
    if let Some(keys) = keys {
        button = button.shortcut_text(keys);
    }
    ui.add_enabled(enabled, button)
}
