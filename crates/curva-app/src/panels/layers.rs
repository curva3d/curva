//! The Layers panel, and the linetype and print width choosers it shares with Properties.

use crate::*;

/// The width of the Layers panel's column for the entered detail.
const DETAIL_COLUMN: f32 = 48.0;

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
        width: f32,
    ) -> Option<curva_document::Linetype> {
        use curva_document::Linetype;
        let mut chosen = None;
        let label = shown.map_or_else(
            || self.text.render(&Message::new("ui.width.varies")),
            |l| l.name().to_owned(),
        );
        egui::ComboBox::from_id_salt(id)
            .selected_text(label)
            .width(width)
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
        width: f32,
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
            .width(width)
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
        // The panel scrolls both ways (`panel_area`), so wide columns can be reached.
        self.layer_table(ui, &layers, current);
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

    /// The layers as a table with a header row whose dividers size each column (spec
    /// `feedback-0.4.2.md` §6).
    fn layer_table(&mut self, ui: &mut egui::Ui, layers: &[curva_document::Layer], current: u64) {
        let table = ui.scope_id().with("layer table");
        // What each column's widest cell measured last frame, for fitting a column to it.
        let widest_id = table.with("widest");
        let last_widest: [f32; 6] = ui.data(|d| d.get_temp(widest_id)).unwrap_or([0.0; 6]);
        let mut widest = [0.0_f32; 6];
        let gap = ui.spacing().item_spacing.x;
        let row = ui.spacing().interact_size.y;
        let mut columns = self.layout.layer_columns;
        // While a detail is entered, a first column hides layers in that detail only (spec
        // `layouts.md` §2).
        let in_detail = self.sheets.entered.and_then(|frame| {
            self.doc
                .detail_of(frame)
                .map(|(_, d)| (frame, d.hidden_layers.clone()))
        });
        let extra = if in_detail.is_some() {
            DETAIL_COLUMN + gap
        } else {
            0.0
        };
        // The Name column takes whatever room the others leave.
        let others: f32 = columns[1..].iter().sum::<f32>() + gap * 5.0 + extra;
        columns[0] = columns[0].max(ui.available_width() - others);
        let total: f32 = columns.iter().sum::<f32>() + gap * 5.0 + extra;
        // It comes first, so it shows however narrow the panel is.
        let detail_cell =
            |rect: egui::Rect| egui::Rect::from_min_size(rect.min, egui::vec2(DETAIL_COLUMN, row));
        let cell = |rect: egui::Rect, index: usize, columns: &[f32; 6]| {
            let x = rect.min.x + extra + columns[..index].iter().sum::<f32>() + gap * index as f32;
            egui::Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(columns[index], row))
        };
        // The header, with a divider after each column to drag or double-click.
        let (header, _) = ui.allocate_exact_size(egui::vec2(total, row), egui::Sense::hover());
        let keys = [
            "ui.layer.name",
            "ui.layer.on",
            "ui.layer.lock",
            "ui.layer.colour",
            "ui.layer.linetype",
            "ui.layer.print_width",
        ];
        let quiet = theme::current().quiet;
        if in_detail.is_some() {
            ui.painter().text(
                detail_cell(header).left_center() + egui::vec2(2.0, 0.0),
                egui::Align2::LEFT_CENTER,
                self.text.render(&Message::new("ui.layer.in_detail")),
                egui::FontId::proportional(11.5),
                theme::ACCENT,
            );
        }
        let mut resized = false;
        let mut finished = false;
        for (index, key) in keys.iter().enumerate() {
            let rect = cell(header, index, &columns);
            ui.painter().text(
                rect.left_center() + egui::vec2(2.0, 0.0),
                egui::Align2::LEFT_CENTER,
                self.text.render(&Message::new(key)),
                egui::FontId::proportional(11.5),
                quiet,
            );
            let divider = egui::Rect::from_center_size(
                egui::pos2(rect.max.x + gap / 2.0, rect.center().y),
                egui::vec2(gap.max(6.0), row),
            );
            let handle = ui.interact(
                divider,
                table.with(("divider", index)),
                egui::Sense::click_and_drag(),
            );
            if handle.hovered() || handle.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
                ui.painter().vline(
                    divider.center().x,
                    divider.y_range(),
                    egui::Stroke::new(1.0, theme::ACCENT),
                );
            }
            if handle.dragged() {
                columns[index] = (columns[index] + handle.drag_delta().x)
                    .max(layout::LEAST_LAYER_COLUMNS[index]);
                resized = true;
            }
            if handle.double_clicked() && last_widest[index] > 0.0 {
                columns[index] = last_widest[index]
                    .ceil()
                    .max(layout::LEAST_LAYER_COLUMNS[index]);
                resized = true;
                finished = true;
            }
            finished |= handle.drag_stopped();
        }
        if resized {
            self.layout.layer_columns = columns;
        }
        if finished {
            self.layout.layer_columns = columns.map(f32::round);
            self.save_layout();
        }
        let mut name_fit = 0.0_f32;
        for layer in layers {
            // The Name column fits the longest name, not the room its field was given.
            let name_width = ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(
                        layer.name.clone(),
                        egui::TextStyle::Body.resolve(ui.style()),
                        quiet,
                    )
                    .size()
                    .x
            });
            name_fit = name_fit.max(name_width + 56.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(total, row), egui::Sense::hover());
            let mut in_cell =
                |ui: &mut egui::Ui, index: usize, add: &mut dyn FnMut(&mut egui::Ui)| {
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(cell(rect, index, &columns))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    child.set_clip_rect(cell(rect, index, &columns).intersect(ui.clip_rect()));
                    add(&mut child);
                    widest[index] = widest[index].max(child.min_rect().width());
                };
            in_cell(ui, 0, &mut |ui| {
                if ui.radio(layer.id == current, "").clicked() {
                    self.undoable(|doc| doc.set_current_layer(layer.id));
                }
                let mut name = layer.name.clone();
                let room = ui.available_width();
                let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(room));
                if edit.changed() {
                    self.doc.rename_layer(layer.id, name.clone());
                }
                edit.context_menu(|ui| {
                    if ui
                        .button(self.text.render(&Message::new("ui.layer.delete")))
                        .clicked()
                    {
                        self.undoable(|doc| {
                            doc.delete_layer(layer.id);
                        });
                        ui.close();
                    }
                });
            });
            in_cell(ui, 1, &mut |ui| {
                let mut visible = layer.visible;
                if ui.checkbox(&mut visible, "").changed() {
                    self.undoable(|doc| doc.set_layer_visible(layer.id, visible));
                }
            });
            in_cell(ui, 2, &mut |ui| {
                let mut locked = layer.locked;
                if ui.checkbox(&mut locked, "").changed() {
                    self.undoable(|doc| doc.set_layer_locked(layer.id, locked));
                }
            });
            in_cell(ui, 3, &mut |ui| {
                let mut colour = layer.colour;
                if ui.color_edit_button_rgb(&mut colour).changed() {
                    self.doc.set_layer_colour(layer.id, colour);
                }
            });
            in_cell(ui, 4, &mut |ui| {
                let id = ui.scope_id().with(("layer linetype", layer.id));
                let width = ui.available_width() - 4.0;
                if let Some(linetype) =
                    self.linetype_choice(ui, id, Some(layer.linetype), false, width)
                {
                    self.undoable(|doc| doc.set_layer_linetype(layer.id, linetype));
                }
            });
            in_cell(ui, 5, &mut |ui| {
                let shown = Some(layer.print_width.map_or(
                    curva_document::PrintWidth::Default,
                    curva_document::PrintWidth::Millimetres,
                ));
                let id = ui.scope_id().with(("layer width", layer.id));
                let width = ui.available_width() - 4.0;
                if let Some(chosen) = self.width_choice(ui, id, shown, false, width) {
                    let mm = match chosen {
                        curva_document::PrintWidth::Millimetres(mm) => Some(mm),
                        _ => None,
                    };
                    self.undoable(|doc| doc.set_layer_print_width(layer.id, mm));
                }
            });
            if let Some((frame, hidden)) = &in_detail {
                let mut shown = !hidden.contains(&layer.id);
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(detail_cell(rect))
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                if child.checkbox(&mut shown, "").changed() {
                    let (frame, id) = (*frame, layer.id);
                    self.undoable(|doc| {
                        doc.change_detail(frame, |d| {
                            d.hidden_layers.retain(|l| *l != id);
                            if !shown {
                                d.hidden_layers.push(id);
                            }
                        });
                    });
                }
            }
        }
        widest[0] = name_fit;
        // A dropdown fits when it shows its usual width.
        widest[4] = widest[4].max(96.0);
        widest[5] = widest[5].max(84.0);
        ui.data_mut(|d| d.insert_temp(widest_id, widest));
    }
}
