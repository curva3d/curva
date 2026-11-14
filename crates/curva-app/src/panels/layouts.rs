//! The Layouts panel (spec `layouts.md` §1): every layout with a picture of its sheet, renamed in
//! place, dragged or stepped into order (the order the sheet index and Print use), opened, copied,
//! deleted, and its paper size and orientation changed.

use crate::*;
use curva_document::layouts::{PAPER_SIZES, PaperUnit};

/// What a row asks for, done after the list is drawn.
enum RowAction {
    Open(u64),
    Copy(u64),
    Delete(u64),
    Move(u64, usize),
    Resize(u64, [f64; 2], PaperUnit),
}

impl Curva {
    /// The Layouts panel's contents.
    pub(crate) fn layouts_panel(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(self.text.render(&Message::new("ui.layout.new")))
            .clicked()
        {
            self.run_from_button("Layout");
        }
        let layouts = self.doc.layouts().to_vec();
        if layouts.is_empty() {
            ui.label(self.text.render(&Message::new("ui.layouts.none")));
            return;
        }
        let open = self.doc.space();
        let mut action = None;
        let count = layouts.len();
        for (index, layout) in layouts.iter().enumerate() {
            let row = ui
                .group(|ui| {
                    ui.horizontal(|ui| {
                        // A handle to drag the layout to another place in the order.
                        let handle = ui
                            .dnd_drag_source(
                                ui.scope_id().with(("layout handle", layout.id)),
                                index,
                                |ui| {
                                    ui.label("≡");
                                },
                            )
                            .response;
                        handle.on_hover_cursor(egui::CursorIcon::Grab);
                        sheet_picture(ui, layout.size, open == Some(layout.id));
                        ui.vertical(|ui| {
                            let mut name = layout.name.clone();
                            let hint = sheets::layout_title(&self.text, layout, index);
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut name)
                                    .hint_text(hint)
                                    .desired_width(130.0),
                            );
                            if edit.changed() {
                                self.doc.rename_layout(layout.id, name);
                            }
                            if let Some(resize) = self.paper_choice(ui, layout) {
                                action = Some(resize);
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        let say = |key: &'static str| self.text.render(&Message::new(key));
                        if ui
                            .add_enabled(
                                open != Some(layout.id),
                                egui::Button::new(say("ui.layouts.open")),
                            )
                            .clicked()
                        {
                            action = Some(RowAction::Open(layout.id));
                        }
                        if ui.button(say("ui.layouts.copy")).clicked() {
                            action = Some(RowAction::Copy(layout.id));
                        }
                        if ui.add_enabled(index > 0, egui::Button::new("▲")).clicked() {
                            action = Some(RowAction::Move(layout.id, index - 1));
                        }
                        if ui
                            .add_enabled(index + 1 < count, egui::Button::new("▼"))
                            .clicked()
                        {
                            action = Some(RowAction::Move(layout.id, index + 1));
                        }
                        if ui.button(say("ui.layer.delete")).clicked() {
                            action = Some(RowAction::Delete(layout.id));
                        }
                    });
                })
                .response;
            // Another layout dropped on this row takes its place in the order.
            if let Some(from) = row.dnd_release_payload::<usize>()
                && let Some(moved) = layouts.get(*from)
            {
                action = Some(RowAction::Move(moved.id, index));
            }
            if row.double_clicked() {
                action = Some(RowAction::Open(layout.id));
            }
        }
        match action {
            Some(RowAction::Open(id)) => {
                if self.processor.is_running() {
                    self.processor.cancel(&mut self.doc);
                }
                self.doc.set_space(Some(id));
            }
            Some(RowAction::Copy(id)) => {
                if self.processor.is_running() {
                    self.processor.cancel(&mut self.doc);
                }
                self.doc.set_space(Some(id));
                self.processor.submit(&mut self.doc, "CopyLayout");
            }
            Some(RowAction::Delete(id)) => self.undoable(|doc| doc.delete_layout(id)),
            Some(RowAction::Move(id, to)) => self.undoable(|doc| doc.move_layout(id, to)),
            Some(RowAction::Resize(id, size, unit)) => {
                self.undoable(|doc| doc.resize_layout(id, size, unit));
                // The open sheet is framed again at its new size.
                if self.doc.space() == Some(id) {
                    self.doc.set_space(None);
                    self.follow_space();
                    self.doc.set_space(Some(id));
                }
            }
            None => {}
        }
    }

    /// The layout's paper size and orientation, as two drop-downs; returns a new size if chosen.
    fn paper_choice(
        &self,
        ui: &mut egui::Ui,
        layout: &curva_document::layouts::Layout,
    ) -> Option<RowAction> {
        let [w, h] = layout.size;
        let landscape = w > h;
        let mut chosen = None;
        ui.horizontal(|ui| {
            let shown = layout.paper_name().map_or_else(
                || format!("{w} × {h} {}", layout.unit.symbol()),
                str::to_owned,
            );
            egui::ComboBox::from_id_salt(("layout paper", layout.id))
                .selected_text(shown)
                .width(80.0)
                .show_ui(ui, |ui| {
                    for paper in PAPER_SIZES {
                        if ui
                            .selectable_label(layout.paper_name() == Some(paper.name), paper.name)
                            .clicked()
                        {
                            let [pw, ph] = paper.size;
                            let size = if landscape {
                                [pw.max(ph), pw.min(ph)]
                            } else {
                                [pw.min(ph), pw.max(ph)]
                            };
                            chosen = Some(RowAction::Resize(layout.id, size, paper.unit));
                        }
                    }
                });
            let say = |key: &'static str| self.text.render(&Message::new(key));
            let turn = if landscape {
                say("ui.layouts.landscape")
            } else {
                say("ui.layouts.portrait")
            };
            if ui
                .button(turn)
                .on_hover_text(say("ui.layouts.turn"))
                .clicked()
            {
                chosen = Some(RowAction::Resize(layout.id, [h, w], layout.unit));
            }
        });
        chosen
    }
}

/// A small picture of a sheet in its proportions, outlined in the accent when it is open.
fn sheet_picture(ui: &mut egui::Ui, size: [f64; 2], open: bool) {
    let box_side = 36.0_f32;
    let [w, h] = size;
    let longest = w.max(h).max(1e-9);
    #[allow(clippy::cast_possible_truncation)]
    let picture = egui::vec2(
        (w / longest) as f32 * box_side,
        (h / longest) as f32 * box_side,
    );
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(box_side), egui::Sense::hover());
    let sheet = egui::Rect::from_center_size(rect.center(), picture);
    ui.painter()
        .rect_filled(sheet, 1.0, egui::Color32::from_rgb(250, 250, 247));
    ui.painter().rect_stroke(
        sheet,
        1.0,
        egui::Stroke::new(
            if open { 2.0 } else { 1.0 },
            if open {
                theme::ACCENT
            } else {
                theme::current().panel_edge
            },
        ),
        egui::StrokeKind::Outside,
    );
}
