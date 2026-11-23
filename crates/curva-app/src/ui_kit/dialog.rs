//! Dialogs: one look for every question and every set of options Curva asks for in the middle of
//! the window, such as a format's import or export options, the mesh settings, getting a model
//! back after a crash, or an agent asking leave. A quiet title centred along the top as on a
//! panel, the contents, then the buttons in a row at the bottom right, the main one in the
//! accent. Escape answers with the last button, which is Cancel or its like.

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
/// clicked, as an index into `choices`; Escape answers with the last.
pub(crate) fn dialog(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    choices: &[Choice],
    contents: impl FnOnce(&mut egui::Ui),
) -> Option<usize> {
    let mut answer = None;
    egui::Area::new(egui::Id::NULL.with(("dialog", id)))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            // Always solid: a question must read clearly over whatever is behind it.
            super::panel_frame_for(false).show(ui, |ui| {
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
                        let clicked = if choice.main {
                            main_button(ui, &choice.word)
                        } else {
                            ui.add(egui::Button::new(&choice.word).min_size(button_size()))
                        };
                        if clicked.clicked() {
                            answer = Some(index);
                        }
                    }
                });
            });
        });
    if answer.is_none() && !choices.is_empty() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        answer = Some(choices.len() - 1);
    }
    answer
}

/// The least a dialog's button takes, so short words still make a button worth aiming at.
fn button_size() -> egui::Vec2 {
    egui::vec2(72.0, ROW)
}

/// A button filled with the accent, for the main answer.
pub(crate) fn main_button(ui: &mut egui::Ui, word: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(word).color(colours::on_accent()))
            .fill(colours::accent())
            .min_size(button_size() + egui::vec2(CONTROL_PAD, 0.0)),
    )
}
