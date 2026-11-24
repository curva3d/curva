//! The keys Curva takes before the command line can see them: undo and redo, stepping through
//! a viewport's views, the File tab's keys and the clipboard's.

use crate::*;

impl Curva {
    /// The undo keys, taken before the command line can see them: Ctrl+Z undoes and Ctrl+Y or
    /// Ctrl+Shift+Z redoes the last change to the model, even while the command line has focus,
    /// unless it holds typed text, which keeps its own undo. Alt+Z and Alt+Shift+Z step through
    /// the last-used viewport's views. Also keeps each viewport's view history.
    pub(crate) fn history_keys(&mut self, ctx: &egui::Context) {
        self.alt_held = ctx.input(|input| input.modifiers.alt);
        use egui::{Key, KeyboardShortcut, Modifiers};
        let moving = ctx.input(|input| {
            input.pointer.any_down() || input.smooth_scroll_delta != egui::Vec2::ZERO
        });
        for pane in &mut self.panes {
            pane.track_view(moving);
        }
        // What the Edit menu asked for, done by the same code as its keys.
        let wanted = self.bar.wanted_edit.take();
        if !self.typed.is_empty() || self.naming_layer {
            return;
        }
        let shortcut = |modifiers, key| KeyboardShortcut::new(modifiers, key);
        let (redo, undo, view_ahead, view_back) = ctx.input_mut(|input| {
            // The fuller shortcuts are taken first, as a plain one also matches them.
            let view_ahead =
                input.consume_shortcut(&shortcut(Modifiers::ALT | Modifiers::SHIFT, Key::Z));
            let view_back = input.consume_shortcut(&shortcut(Modifiers::ALT, Key::Z));
            // Rhino's own keys for them too, Home and End.
            let view_back = view_back || input.consume_key(Modifiers::NONE, Key::Home);
            let view_ahead = view_ahead || input.consume_key(Modifiers::NONE, Key::End);
            let redo = input
                .consume_shortcut(&shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
                || input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Y));
            let undo = input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Z));
            (redo, undo, view_ahead, view_back)
        });
        let redo = redo || wanted == Some(menus::EditAction::Redo);
        let undo = undo || wanted == Some(menus::EditAction::Undo);
        self.file_keys(ctx);
        self.clipboard_keys(ctx, wanted);
        if view_back || view_ahead {
            self.panes[self.active].step_view(view_ahead);
        }
        if undo || redo {
            // A command in progress is ended first, as its changes are not yet a step.
            if self.processor.is_running() {
                self.processor.cancel(&mut self.doc);
            }
            self.processor
                .submit(&mut self.doc, if redo { "redo" } else { "undo" });
        }
    }

    /// The File tab's keys, also taken before the command line sees them.
    fn file_keys(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let shortcut = |modifiers, key| KeyboardShortcut::new(modifiers, key);
        let file_command = ctx.input_mut(|input| {
            if input.consume_shortcut(&shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)) {
                Some("SaveAs")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::S)) {
                Some("Save")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::O)) {
                Some("Open")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::N)) {
                Some("New")
            } else {
                None
            }
        });
        // Ctrl+, opens Settings, the standard key for an app's settings.
        if ctx.input_mut(|input| input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Comma)))
        {
            self.open_settings(SettingsTab::General);
        }
        self.interface_size_keys(ctx);
        if let Some(name) = file_command {
            self.run_from_button(name);
        }
    }

    /// Ctrl+Plus and Ctrl+Minus make the interface larger or smaller a step at a time, and Ctrl+0
    /// puts it back to 100%, as in a browser; Settings shows the size chosen.
    fn interface_size_keys(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let size = &mut self.settings.interface_size;
        let before = *size;
        ctx.input_mut(|input| {
            if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Plus))
                || input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Equals))
            {
                *size = (*size + 25).min(200);
            } else if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Minus))
            {
                *size = size.saturating_sub(25).max(100);
            } else if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0))
            {
                *size = 100;
            }
        });
        if *size != before {
            self.save_settings();
        }
    }

    /// The clipboard keys every program has, on the selected objects. The window turns them
    /// into egui's Copy, Cut and Paste events rather than key presses, so those are taken here,
    /// before the command line could; a paste is the objects' only while the system clipboard
    /// still holds the mark their copy put there.
    fn clipboard_keys(&mut self, ctx: &egui::Context, wanted: Option<menus::EditAction>) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let shortcut = |modifiers, key| KeyboardShortcut::new(modifiers, key);
        let anything_selected = !self.doc.selection().is_empty();
        let (copy, cut, paste, all) = ctx.input_mut(|input| {
            let (mut copy, mut cut, mut paste) = (
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::C)),
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::X)),
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::V)),
            );
            let (c, x, v) = clipboard_requests(&mut input.events, anything_selected);
            (copy, cut, paste) = (copy || c, cut || x, paste || v);
            copy |= wanted == Some(menus::EditAction::Copy);
            cut |= wanted == Some(menus::EditAction::Cut);
            paste |= wanted == Some(menus::EditAction::Paste);
            let all = input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::A));
            (copy, cut, paste, all)
        });
        if copy || cut {
            self.clipboard = self
                .selected_shapes()
                .into_iter()
                .map(|(id, shape)| (shape, self.doc.layer_of(id)))
                .collect();
            if !self.clipboard.is_empty() {
                ctx.copy_text(CLIPBOARD_MARK.to_owned());
            }
        }
        if cut && !self.clipboard.is_empty() {
            self.run_from_button("delete");
        }
        if paste && !self.clipboard.is_empty() {
            // Pasted in place, as in Rhino, and selected.
            let shapes = self.clipboard.clone();
            self.undoable(|doc| {
                let added: Vec<u64> = shapes
                    .into_iter()
                    .map(|(shape, layer)| doc.add_shape_on(shape, layer))
                    .collect();
                doc.select(&added);
            });
        }
        if all {
            self.run_from_button("selall");
        }
    }
}
