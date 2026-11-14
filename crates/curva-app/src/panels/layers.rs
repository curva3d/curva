//! The Layers panel, and the linetype and print width choosers it shares with Properties.

use crate::*;

impl Curva {
    /// Runs an edit of the model from a panel as one undo step, if it changed anything.
    pub(crate) fn undoable(&mut self, edit: impl FnOnce(&mut Document)) {
        let before = self.doc.state().clone();
        edit(&mut self.doc);
        if self.doc.state() != &before {
            self.doc.record_change(before);
        }
    }

    /// A drop-down of linetypes, with `ByLayer` when `by_layer`; returns the one chosen.
    pub(crate) fn linetype_choice(
        &self,
        ui: &mut egui::Ui,
        id: egui::Id,
        shown: Option<curva_document::Linetype>,
        by_layer: bool,
    ) -> Option<curva_document::Linetype> {
        use curva_document::Linetype;
        let mut chosen = None;
        let label = shown.map_or_else(
            || self.text.render(&Message::new("ui.width.varies")),
            |l| l.name().to_owned(),
        );
        egui::ComboBox::from_id_salt(id)
            .selected_text(label)
            .width(96.0)
            .show_ui(ui, |ui| {
                let choices = by_layer
                    .then_some(Linetype::ByLayer)
                    .into_iter()
                    .chain(Linetype::ALL);
                for choice in choices {
                    if ui
                        .selectable_label(shown == Some(choice), choice.name())
                        .clicked()
                    {
                        chosen = Some(choice);
                    }
                }
            });
        chosen
    }

    /// A drop-down of print widths (spec `line-weights.md` §5): `By layer` when `by_layer`,
    /// `Default`, Rhino's common widths, and a field for any other. `shown` is what it shows;
    /// returns the width chosen, if one was.
    pub(crate) fn width_choice(
        &self,
        ui: &mut egui::Ui,
        id: egui::Id,
        shown: Option<curva_document::PrintWidth>,
        by_layer: bool,
    ) -> Option<curva_document::PrintWidth> {
        use curva_document::PrintWidth;
        let text = |key: &'static str| self.text.render(&Message::new(key));
        let named = |width: PrintWidth| match width {
            PrintWidth::ByLayer => text("ui.width.by_layer"),
            PrintWidth::Default => text("ui.width.default"),
            PrintWidth::Millimetres(mm) => self
                .text
                .render(&Message::with("ui.width.mm", vec![format!("{mm:.2}")])),
        };
        let mut chosen = None;
        egui::ComboBox::from_id_salt(id)
            .selected_text(shown.map_or_else(|| text("ui.width.varies"), named))
            .width(84.0)
            .show_ui(ui, |ui| {
                let mut choices = Vec::new();
                if by_layer {
                    choices.push(PrintWidth::ByLayer);
                }
                choices.push(PrintWidth::Default);
                choices.extend(PrintWidth::COMMON.map(PrintWidth::Millimetres));
                for choice in choices {
                    if ui
                        .selectable_label(shown == Some(choice), named(choice))
                        .clicked()
                    {
                        chosen = Some(choice);
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(text("ui.width.custom"));
                    let mut mm = match shown {
                        Some(PrintWidth::Millimetres(mm)) => mm,
                        _ => 0.25,
                    };
                    let field = ui.add(
                        egui::DragValue::new(&mut mm)
                            .range(0.01..=PrintWidth::MOST)
                            .speed(0.01)
                            .fixed_decimals(2)
                            .suffix(" mm"),
                    );
                    if field.changed() {
                        chosen = Some(PrintWidth::Millimetres(mm));
                    }
                });
            });
        chosen
    }

    pub(crate) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        let layers = self.doc.layers().to_vec();
        let current = self.doc.current_layer();
        egui::Grid::new(ui.scope_id().with("layers")).show(ui, |ui| {
            for layer in &layers {
                if ui.radio(layer.id == current, "").clicked() {
                    self.undoable(|doc| doc.set_current_layer(layer.id));
                }
                let mut name = layer.name.clone();
                let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(90.0));
                if edit.changed() {
                    self.doc.rename_layer(layer.id, name);
                }
                let mut colour = layer.colour;
                if ui.color_edit_button_rgb(&mut colour).changed() {
                    self.doc.set_layer_colour(layer.id, colour);
                }
                let shown = Some(layer.print_width.map_or(
                    curva_document::PrintWidth::Default,
                    curva_document::PrintWidth::Millimetres,
                ));
                let id = ui.scope_id().with(("layer width", layer.id));
                if let Some(width) = self.width_choice(ui, id, shown, false) {
                    let mm = match width {
                        curva_document::PrintWidth::Millimetres(mm) => Some(mm),
                        _ => None,
                    };
                    self.undoable(|doc| doc.set_layer_print_width(layer.id, mm));
                }
                let id = ui.scope_id().with(("layer linetype", layer.id));
                if let Some(linetype) = self.linetype_choice(ui, id, Some(layer.linetype), false) {
                    self.undoable(|doc| doc.set_layer_linetype(layer.id, linetype));
                }
                let mut visible = layer.visible;
                let shown =
                    ui.checkbox(&mut visible, self.text.render(&Message::new("ui.layer.on")));
                if shown.changed() {
                    self.undoable(|doc| doc.set_layer_visible(layer.id, visible));
                }
                let mut locked = layer.locked;
                let lock = ui.checkbox(
                    &mut locked,
                    self.text.render(&Message::new("ui.layer.lock")),
                );
                if lock.changed() {
                    self.undoable(|doc| doc.set_layer_locked(layer.id, locked));
                }
                if ui
                    .button(self.text.render(&Message::new("ui.layer.delete")))
                    .clicked()
                {
                    self.undoable(|doc| {
                        doc.delete_layer(layer.id);
                    });
                }
                ui.end_row();
            }
        });
        if ui
            .button(self.text.render(&Message::new("ui.layer.new")))
            .clicked()
        {
            let number = layers.iter().map(|layer| layer.id).max().unwrap_or(0) + 1;
            let name = self.text.render(&Message::with(
                "ui.layer.new_name",
                vec![number.to_string()],
            ));
            self.undoable(|doc| {
                doc.add_layer(name);
            });
        }
    }
}
