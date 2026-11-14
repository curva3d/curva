//! The Properties panel.

use crate::*;

impl Curva {
    pub(crate) fn properties_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text.render(&Message::new("ui.properties")));
        let selection = self.doc.selection();
        if selection.is_empty() {
            ui.label(self.text.render(&Message::new("ui.properties.none")));
            return;
        }
        ui.label(self.text.render(&Message::with(
            "ui.properties.count",
            vec![selection.len().to_string()],
        )));
        if let [only] = selection.as_slice()
            && let Some(object) = self.doc.state().objects.iter().find(|o| o.id == *only)
        {
            match &object.shape {
                Shape::Instance(instance) => {
                    let name = self
                        .doc
                        .definitions()
                        .iter()
                        .find(|d| d.id == instance.definition)
                        .map_or_else(String::new, |d| d.name.clone());
                    ui.label(
                        self.text
                            .render(&Message::with("ui.properties.instance", vec![name])),
                    );
                }
                Shape::Curve(curve) => {
                    ui.label(self.text.render(&Message::new(curve_kind(curve))));
                    if let Ok(length) = curve.length() {
                        ui.label(self.text.render(&Message::with(
                            "ui.properties.length",
                            vec![format!("{length:.3}")],
                        )));
                    }
                }
                Shape::Body(body) => {
                    // Measuring a curved solid takes a moment, so it is done once per change.
                    let fresh = self
                        .measured
                        .as_ref()
                        .is_some_and(|(id, held, _)| *id == object.id && held == body);
                    if !fresh {
                        let volume = body.volume().ok().filter(|v| *v > 0.0);
                        let reading = match volume {
                            Some(volume) => ("ui.kind.solid", "ui.properties.volume", volume),
                            None => (
                                "ui.kind.surface",
                                "ui.properties.area",
                                body.area().unwrap_or(0.0),
                            ),
                        };
                        self.measured = Some((object.id, body.clone(), reading));
                    }
                    if let Some((_, _, (kind, key, value))) = &self.measured {
                        ui.label(self.text.render(&Message::new(kind)));
                        if object.live.is_some() {
                            let inputs = self.doc.inputs_of(object.id).len();
                            ui.label(self.text.render(&Message::with(
                                "ui.properties.live",
                                vec![inputs.to_string()],
                            )));
                        }
                        ui.label(
                            self.text
                                .render(&Message::with(key, vec![format!("{value:.3}")])),
                        );
                    }
                }
            }
        }
        let layers = self.doc.layers().to_vec();
        let shared = self.doc.layer_of(selection[0]).filter(|first| {
            selection
                .iter()
                .all(|id| self.doc.layer_of(*id) == Some(*first))
        });
        let shown = shared
            .and_then(|id| layers.iter().find(|layer| layer.id == id))
            .map_or_else(
                || self.text.render(&Message::new("ui.properties.mixed")),
                |layer| layer.name.clone(),
            );
        let mut chosen = None;
        egui::ComboBox::from_label(self.text.render(&Message::new("ui.properties.layer")))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for layer in &layers {
                    if ui
                        .selectable_label(shared == Some(layer.id), &layer.name)
                        .clicked()
                    {
                        chosen = Some(layer.id);
                    }
                }
            });
        if let Some(layer) = chosen {
            self.undoable(|doc| doc.move_to_layer(&selection, layer));
        }
        let widths: Vec<curva_document::PrintWidth> = self
            .doc
            .state()
            .objects
            .iter()
            .filter(|object| selection.contains(&object.id))
            .map(|object| object.print_width)
            .collect();
        let shown = widths
            .first()
            .copied()
            .filter(|first| widths.iter().all(|width| width == first));
        ui.horizontal(|ui| {
            ui.label(self.text.render(&Message::new("ui.properties.width")));
            let id = ui.scope_id().with("object width");
            if let Some(width) = self.width_choice(ui, id, shown, true, 84.0) {
                self.undoable(|doc| doc.set_print_width(&selection, width));
            }
        });
        let linetypes: Vec<curva_document::Linetype> = self
            .doc
            .state()
            .objects
            .iter()
            .filter(|object| selection.contains(&object.id))
            .map(|object| object.linetype)
            .collect();
        let shown = linetypes
            .first()
            .copied()
            .filter(|first| linetypes.iter().all(|l| l == first));
        ui.horizontal(|ui| {
            ui.label(self.text.render(&Message::new("ui.properties.linetype")));
            let id = ui.scope_id().with("object linetype");
            if let Some(linetype) = self.linetype_choice(ui, id, shown, true, 96.0) {
                self.undoable(|doc| doc.set_linetype(&selection, linetype));
            }
        });
    }
}
