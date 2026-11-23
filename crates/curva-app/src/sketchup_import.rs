//! Rhino 8's "SketchUp Import Options" (issue #432): shown when `Open` or `Import` reads a
//! SketchUp file. The choices are kept in the settings, so the next file starts with them, and in
//! the model, where the SketchUp format reads them. With "Always use these settings" ticked the
//! dialog is not shown again and the kept choices are used.

use crate::*;
use curva_commands::Panel;
use curva_document::formats::{Panel as FormatPanel, SketchUpImport};

/// The dialog while it is open: the choices being made.
#[derive(Default)]
pub(crate) struct SketchUpDialog {
    choices: Option<(SketchUpImport, bool)>,
}

impl Curva {
    /// Shows the dialog while a command asks for SketchUp's import options, and answers it.
    pub(crate) fn sketchup_import_window(&mut self, ui: &mut egui::Ui) {
        // The model always carries the kept choices, so scripts and agents follow them too.
        if !SketchUpImport::kept_in(&self.doc) {
            self.settings.sketchup.remember(&mut self.doc);
        }
        let asking = self
            .processor
            .current_request()
            .is_some_and(|r| r.panel == Some(Panel::Format(FormatPanel::SketchUpImport)));
        if !asking {
            self.file_dialogs.sketchup.choices = None;
            return;
        }
        if self.settings.sketchup_always {
            self.settings.sketchup.remember(&mut self.doc);
            self.processor.submit(&mut self.doc, "");
            return;
        }
        let (mut choices, mut always) = self
            .file_dialogs
            .sketchup
            .choices
            .unwrap_or((self.settings.sketchup, self.settings.sketchup_always));
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let choices_row = [
            ui_kit::main_choice(say("ui.skp.ok")),
            ui_kit::other_choice(say("ui.units.cancel")),
        ];
        let answer = ui_kit::dialog(
            ui.ctx(),
            "sketchup import",
            &say("ui.skp.title"),
            &choices_row,
            |ui| {
                ui_kit::section(ui, say("ui.skp.faces"));
                let mut mesh = !choices.trimmed_planes;
                if ui_kit::either(ui, &mut mesh, [say("ui.skp.planes"), say("ui.skp.mesh")]) {
                    choices.trimmed_planes = !mesh;
                }
                if choices.trimmed_planes {
                    ui.label(ui_kit::caption(say("ui.skp.planes_warning")));
                }
                ui_kit::section(ui, say("ui.skp.join"));
                ui_kit::chip_row(ui, |ui| {
                    ui_kit::toggle(ui, &mut choices.join_edges, say("ui.skp.join_edges"));
                    let planes = choices.trimmed_planes;
                    ui_kit::toggle_when(
                        ui,
                        planes,
                        &mut choices.join_faces,
                        say("ui.skp.join_faces"),
                    );
                });
                ui_kit::section(ui, say("ui.skp.groups"));
                ui_kit::chip_row(ui, |ui| {
                    ui_kit::toggle(ui, &mut choices.group_layers, say("ui.skp.group_layers"));
                    ui_kit::toggle(ui, &mut choices.add_to_groups, say("ui.skp.add_to_groups"));
                });
                ui.add_space(ui_kit::ROW_GAP);
                ui_kit::toggle(ui, &mut always, say("ui.skp.always"));
            },
        );
        let (ok, cancel) = (answer == Some(0), answer == Some(1));
        if ok {
            self.settings.sketchup = choices;
            self.settings.sketchup_always = always;
            self.save_settings();
            choices.remember(&mut self.doc);
            self.file_dialogs.sketchup.choices = None;
            self.processor.submit(&mut self.doc, "");
        } else if cancel {
            self.file_dialogs.sketchup.choices = None;
            self.processor.cancel(&mut self.doc);
        } else {
            self.file_dialogs.sketchup.choices = Some((choices, always));
        }
    }
}
