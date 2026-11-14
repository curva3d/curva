//! Editing text in place (spec `annotation.md` §2): double-clicking a note opens its words in a
//! small editor beside it, as Rhino does. Ctrl+Enter or clicking away keeps the change, as one
//! undo step; Escape leaves the note as it was.

use crate::*;
use curva_document::annotation::Annotation;

/// A note being edited.
pub(crate) struct EditingText {
    /// The note.
    id: u64,
    /// Its words as they are being typed.
    words: String,
    /// Where the editor opens, on screen.
    at: egui::Pos2,
    /// Whether the editor has been given the keyboard yet.
    focused: bool,
}

/// The note being edited, if any.
#[derive(Default)]
pub(crate) struct TextEditing {
    pub(crate) open: Option<EditingText>,
}

impl Curva {
    /// Opens the editor when a note is double-clicked; returns whether one was.
    pub(crate) fn text_double_click(
        &mut self,
        pane: usize,
        response: &egui::Response,
        size: [f64; 2],
    ) -> bool {
        if !response.double_clicked() || self.processor.is_running() {
            return false;
        }
        let Some(at) = response.interact_pointer_pos() else {
            return false;
        };
        let local = at - response.rect.min;
        let screen = [f64::from(local.x), f64::from(local.y)];
        let camera = self.panes[pane].camera;
        let Some(id) = self.outlines.pick(&camera, screen, size, PICK_REACH) else {
            return false;
        };
        let Some(Shape::Annotation(Annotation::Text(text))) = self
            .doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.shape.clone())
        else {
            return false;
        };
        self.text_editing.open = Some(EditingText {
            id,
            words: text.content,
            at,
            focused: false,
        });
        true
    }

    /// The editor, while a note is being edited.
    pub(crate) fn text_editor(&mut self, ctx: &egui::Context) {
        let Some(editing) = self.text_editing.open.as_mut() else {
            return;
        };
        let mut keep = false;
        let mut cancel = false;
        egui::Area::new(egui::Id::NULL.with("text editor"))
            .fixed_pos(editing.at + egui::vec2(12.0, 12.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                panel_frame(true).show(ui, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::multiline(&mut editing.words)
                            .desired_rows(3)
                            .desired_width(260.0),
                    );
                    if !editing.focused {
                        edit.request_focus();
                        editing.focused = true;
                    }
                    let (enter, escape) = ui.input(|input| {
                        (
                            input.modifiers.command && input.key_pressed(egui::Key::Enter),
                            input.key_pressed(egui::Key::Escape),
                        )
                    });
                    keep = enter || edit.lost_focus() && !escape;
                    cancel = escape;
                    ui.label(
                        egui::RichText::new(self.text.render(&Message::new("ui.text.editing")))
                            .small()
                            .color(theme::current().quiet),
                    );
                });
            });
        if cancel {
            self.text_editing.open = None;
        } else if keep {
            let Some(editing) = self.text_editing.open.take() else {
                return;
            };
            let words = editing.words;
            if words.trim().is_empty() {
                return;
            }
            let current = self
                .doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == editing.id)
                .map(|o| o.shape.clone());
            if let Some(Shape::Annotation(Annotation::Text(text))) = current
                && text.content != words
            {
                let changed = curva_document::annotation::Text {
                    content: words,
                    ..text
                };
                self.undoable(|doc| {
                    doc.replace_shape(editing.id, Shape::Annotation(Annotation::Text(changed)));
                });
            }
        }
    }
}
