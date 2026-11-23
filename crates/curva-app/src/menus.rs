//! The menus along the top bar: File, Edit, View and Help (owner, 23 November 2026, after Apple's
//! guidelines: every command has a menu home with its shortcut beside it, and an ellipsis marks
//! the items that ask for more before they act).

use crate::*;

/// An edit asked for from the Edit menu, carried out by the same code as its keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditAction {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
}

/// The pieces the View menu can show, with their names' keys.
const SHOWN_PIECES: [layout::Piece; 6] = [
    layout::Piece::CommandTabs,
    layout::Piece::Display,
    layout::Piece::Layers,
    layout::Piece::Properties,
    layout::Piece::Command,
    layout::Piece::Agent,
];

impl Curva {
    /// The menus' row in the top bar. Returns a command to run.
    pub(crate) fn menus(&mut self, ui: &mut egui::Ui) -> Option<&'static str> {
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let (file, edit, view, help) = (
            say("ui.menu.file"),
            say("ui.menu.edit"),
            say("ui.menu.view"),
            say("ui.menu.help"),
        );
        let mut chosen = None;
        ui.menu_button(file, |ui| chosen = self.file_menu(ui));
        ui.menu_button(edit, |ui| {
            if let Some(name) = self.edit_menu(ui) {
                chosen = Some(name);
            }
        });
        ui.menu_button(view, |ui| {
            if let Some(name) = self.view_menu(ui) {
                chosen = Some(name);
            }
        });
        ui.menu_button(help, |ui| self.help_menu(ui));
        chosen
    }

    /// File: New, Open…, Save, Save As…, then Import…, Export…, Export Selected…, then Print….
    fn file_menu(&mut self, ui: &mut egui::Ui) -> Option<&'static str> {
        let selected = !self.doc.selection().is_empty();
        let items: [(&str, &str, Option<&str>, bool); 9] = [
            ("New", "ui.menu.new", Some("Ctrl+N"), true),
            ("Open", "ui.menu.open", Some("Ctrl+O"), true),
            ("Save", "ui.menu.save", Some("Ctrl+S"), true),
            ("SaveAs", "ui.menu.save_as", Some("Ctrl+Shift+S"), true),
            ("", "", None, true),
            ("Import", "ui.menu.import", None, true),
            ("Export", "ui.menu.export", None, true),
            ("ExportSelected", "ui.menu.export_selected", None, selected),
            ("Print", "ui.menu.print", None, true),
        ];
        self.menu_items(ui, &items)
    }

    /// The items of a menu that each run a command; an empty name is a separator.
    fn menu_items(
        &self,
        ui: &mut egui::Ui,
        items: &[(&'static str, &'static str, Option<&str>, bool)],
    ) -> Option<&'static str> {
        let mut chosen = None;
        for (name, key, keys, enabled) in items {
            if name.is_empty() {
                ui.separator();
                continue;
            }
            let label = self.text.render(&Message::new(key));
            if ui_kit::menu_item(ui, *enabled, label, *keys).clicked() {
                chosen = Some(*name);
                ui.close();
            }
        }
        chosen
    }

    /// Edit: Undo and Redo, the clipboard and Delete, selecting, then Settings….
    fn edit_menu(&mut self, ui: &mut egui::Ui) -> Option<&'static str> {
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let selected = !self.doc.selection().is_empty();
        let held = !self.clipboard.is_empty();
        let edits = [
            (EditAction::Undo, "ui.menu.undo", "Ctrl+Z", true),
            (EditAction::Redo, "ui.menu.redo", "Ctrl+Y", true),
            (EditAction::Cut, "ui.menu.cut", "Ctrl+X", selected),
            (EditAction::Copy, "ui.menu.copy", "Ctrl+C", selected),
            (EditAction::Paste, "ui.menu.paste", "Ctrl+V", held),
        ];
        let mut wanted = None;
        for (index, (action, key, keys, enabled)) in edits.into_iter().enumerate() {
            if index == 2 {
                ui.separator();
            }
            if ui_kit::menu_item(ui, enabled, say(key), Some(keys)).clicked() {
                wanted = Some(action);
                ui.close();
            }
        }
        self.bar.wanted_edit = wanted;
        let rest: [(&str, &str, Option<&str>, bool); 4] = [
            ("Delete", "ui.menu.delete", Some("Del"), selected),
            ("", "", None, true),
            ("SelAll", "ui.menu.select_all", Some("Ctrl+A"), true),
            ("SelNone", "ui.menu.select_none", Some("Esc"), selected),
        ];
        let chosen = self.menu_items(ui, &rest);
        ui.separator();
        if ui_kit::menu_item(ui, true, say("ui.menu.settings"), Some("Ctrl+,")).clicked() {
            self.open_settings(SettingsTab::General);
            ui.close();
        }
        chosen
    }

    /// View: the panels to bring back, focus mode, and Zoom Extents.
    fn view_menu(&mut self, ui: &mut egui::Ui) -> Option<&'static str> {
        for piece in SHOWN_PIECES {
            let name = self.text.render(&Message::new(piece.name_key()));
            if ui_kit::menu_item(ui, true, name, None).clicked() {
                self.open_piece(piece);
                ui.close();
            }
        }
        ui.separator();
        let focus = self.text.render(&Message::new("ui.menu.focus"));
        if ui_kit::menu_item(ui, true, focus, None).clicked() {
            self.toggle_focus();
            ui.close();
        }
        let items: [(&str, &str, Option<&str>, bool); 1] =
            [("ZoomExtents", "ui.menu.zoom_extents", None, true)];
        self.menu_items(ui, &items)
    }

    /// Help: Curva's documentation, and About Curva.
    fn help_menu(&mut self, ui: &mut egui::Ui) {
        let say = |key: &'static str| self.text.render(&Message::new(key));
        if ui_kit::menu_item(ui, true, say("ui.menu.docs"), None).clicked() {
            open_link("https://curva3d.com/docs/");
            ui.close();
        }
        ui.separator();
        if ui_kit::menu_item(ui, true, say("ui.menu.about"), None).clicked() {
            self.open_settings(SettingsTab::About);
            ui.close();
        }
    }
}
