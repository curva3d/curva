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

    /// A selected clipping plane's side, section fill and the viewports it cuts (spec
    /// `layouts.md` §3).
    fn clipping_properties(&mut self, ui: &mut egui::Ui, frame: u64) {
        use curva_document::clipping::{ClipView, SectionFill};
        let Some(plane) = self.doc.clipping_plane_of(frame).cloned() else {
            return;
        };
        let say = |key: &'static str| self.text.render(&Message::new(key));
        ui.label(egui::RichText::new(say("ui.clipping")).strong());
        let mut changed = plane.clone();
        ui.checkbox(&mut changed.flipped, say("ui.clipping.flipped"));
        ui.horizontal(|ui| {
            ui.label(say("ui.clipping.fill"));
            let mut filled = matches!(changed.section, SectionFill::Solid(_));
            ui.radio_value(&mut filled, false, say("ui.clipping.no_fill"));
            ui.radio_value(&mut filled, true, say("ui.clipping.solid_fill"));
            let mut colour = match changed.section {
                SectionFill::Solid(colour) => colour,
                SectionFill::None => match SectionFill::default() {
                    SectionFill::Solid(colour) => colour,
                    SectionFill::None => [0.5; 3],
                },
            };
            if filled {
                ui.color_edit_button_rgb(&mut colour);
                changed.section = SectionFill::Solid(colour);
            } else {
                changed.section = SectionFill::None;
            }
        });
        ui.label(say("ui.clipping.views"));
        ui.horizontal_wrapped(|ui| {
            for (slot, pane) in self.panes.iter().enumerate() {
                let name = self.text.render(&Message::new(view_label(pane.view)));
                let view = ClipView::Viewport(slot);
                let mut on = changed.views.contains(&view);
                if ui.checkbox(&mut on, name).changed() {
                    changed.views.retain(|v| *v != view);
                    if on {
                        changed.views.push(view);
                    }
                }
            }
        });
        if changed != plane {
            self.undoable(|doc| doc.change_clipping_plane(frame, |c| *c = changed));
        }
    }

    /// A selected dimension's value and its text, which can replace the value with typed text
    /// where `<>` stands for it, as in Rhino (spec `annotation.md` §3).
    fn dimension_properties(
        &mut self,
        ui: &mut egui::Ui,
        id: u64,
        dimension: &curva_document::annotation::Dimension,
    ) {
        use curva_document::annotation::Annotation;
        let say = |key: &'static str| self.text.render(&Message::new(key));
        ui.label(egui::RichText::new(say("ui.dimension")).strong());
        ui.label(self.text.render(&Message::with(
            "ui.dimension.value",
            vec![self.doc.length_text(dimension.value())],
        )));
        let mut typed = dimension.text.clone().unwrap_or_else(|| "<>".to_owned());
        ui.horizontal(|ui| {
            ui.label(say("ui.dimension.text"));
            ui.add(egui::TextEdit::singleline(&mut typed).desired_width(140.0));
        });
        let text = (typed.trim() != "<>" && !typed.trim().is_empty()).then_some(typed);
        if text != dimension.text {
            let changed = curva_document::annotation::Dimension {
                text,
                ..dimension.clone()
            };
            self.undoable(|doc| {
                doc.replace_shape(id, Shape::Annotation(Annotation::Dimension(changed)));
            });
        }
    }

    /// A selected text's words, height and alignment, changed in place (spec `annotation.md`
    /// §2); each change is one undo step.
    fn text_properties(
        &mut self,
        ui: &mut egui::Ui,
        id: u64,
        text: &curva_document::annotation::Text,
    ) {
        use curva_document::annotation::{Annotation, TextAlign};
        let say = |key: &'static str| self.text.render(&Message::new(key));
        ui.label(egui::RichText::new(say("ui.text")).strong());
        let mut changed = text.clone();
        ui.add(
            egui::TextEdit::multiline(&mut changed.content)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.label(say("ui.text.height"));
            ui.add(
                egui::DragValue::new(&mut changed.height)
                    .range(1e-6..=1e9)
                    .speed(0.05 * text.height.max(1e-3)),
            );
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut changed.align, TextAlign::Left, say("ui.text.left"));
            ui.radio_value(&mut changed.align, TextAlign::Centre, say("ui.text.centre"));
            ui.radio_value(&mut changed.align, TextAlign::Right, say("ui.text.right"));
        });
        if changed != *text && !changed.content.trim().is_empty() {
            self.undoable(|doc| {
                doc.replace_shape(id, Shape::Annotation(Annotation::Text(changed)));
            });
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
            self.clipping_properties(ui, *only);
        }
        if let [only] = selection.as_slice()
            && let Some(object) = self.doc.state().objects.iter().find(|o| o.id == *only)
        {
            match &object.shape {
                Shape::Annotation(curva_document::annotation::Annotation::Text(text)) => {
                    let (id, text) = (object.id, text.clone());
                    self.text_properties(ui, id, &text);
                }
                Shape::Annotation(curva_document::annotation::Annotation::Dimension(d)) => {
                    let (id, d) = (object.id, d.clone());
                    self.dimension_properties(ui, id, &d);
                }
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
