//! The Properties panel.

use crate::*;

impl Curva {
    /// A selected detail's view, scale, display mode and lock (spec `layouts.md` §2).
    fn detail_properties(&mut self, ui: &mut egui::Ui, frame: u64) {
        let Some((_, detail)) = self.doc.detail_of(frame).map(|(l, d)| (l, d.clone())) else {
            return;
        };
        let say = |key: &'static str| self.text.render(&Message::new(key));
        ui.label(egui::RichText::new(say("ui.detail")).strong());
        let mut changed = detail.clone();
        egui::Grid::new("detail properties").show(ui, |ui| {
            ui.label(say("ui.detail.projection"));
            egui::ComboBox::from_id_salt("detail projection")
                .selected_text(&changed.projection)
                .show_ui(ui, |ui| {
                    for key in ["Top", "Front", "Right", "Perspective"] {
                        ui.selectable_value(&mut changed.projection, key.to_owned(), key);
                    }
                });
            ui.end_row();
            if changed.projection == "Perspective" {
                changed.scale = None;
            } else if changed.scale.is_none() {
                changed.scale = Some(0.01);
            }
            if let Some(scale) = changed.scale {
                ui.label(say("ui.detail.scale"));
                egui::ComboBox::from_id_salt("detail scale")
                    .selected_text(curva_document::layouts::scale_text(scale))
                    .show_ui(ui, |ui| {
                        for n in [
                            1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 250.0, 500.0,
                            1000.0,
                        ] {
                            let value = 1.0 / n;
                            if ui
                                .selectable_label(
                                    (scale - value).abs() < 1e-12,
                                    curva_document::layouts::scale_text(value),
                                )
                                .clicked()
                            {
                                changed.scale = Some(value);
                            }
                        }
                        for n in [2.0, 5.0, 10.0] {
                            if ui
                                .selectable_label(
                                    (scale - n).abs() < 1e-12,
                                    curva_document::layouts::scale_text(n),
                                )
                                .clicked()
                            {
                                changed.scale = Some(n);
                            }
                        }
                    });
                ui.end_row();
            }
            ui.label(say("ui.detail.display"));
            egui::ComboBox::from_id_salt("detail display")
                .selected_text(&changed.display)
                .show_ui(ui, |ui| {
                    for mode in ["Wireframe", "Shaded", "Ghosted"] {
                        ui.selectable_value(&mut changed.display, mode.to_owned(), mode);
                    }
                });
            ui.end_row();
            ui.label(say("ui.detail.locked"));
            ui.checkbox(&mut changed.locked, "");
            ui.end_row();
        });
        if changed != detail {
            self.undoable(|doc| doc.change_detail(frame, |d| *d = changed));
        }
    }

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
        if let [only] = selection.as_slice() {
            self.detail_properties(ui, *only);
        }
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
