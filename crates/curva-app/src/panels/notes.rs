//! The Notes panel (Rhino's `Notes`): the model's notes, kept with the file and read by the
//! `Notes` field.

use crate::*;

impl Curva {
    /// The notes, edited in place; the change is kept (and can be undone) when the box is left.
    pub(crate) fn notes_panel(&mut self, ui: &mut egui::Ui) {
        let id = ui.scope_id().with("notes draft");
        let held = self.doc.state().notes.clone();
        let mut text = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| held.clone());
        let response = ui.add(
            egui::TextEdit::multiline(&mut text)
                .desired_width(f32::INFINITY)
                .desired_rows(12)
                .hint_text(self.text.render(&Message::new("ui.notes.hint"))),
        );
        if response.changed() {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        }
        if response.lost_focus() {
            ui.data_mut(|d| d.remove::<String>(id));
            if text != held {
                self.undoable(|doc| doc.set_notes(&text));
            }
        }
    }
}
