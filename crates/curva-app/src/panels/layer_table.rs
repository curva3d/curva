//! The Layers panel's table: a header row whose dividers size each column, then a row for
//! each layer (spec `feedback-0.4.2.md` §6).

use super::layers::{ARROW, DETAIL_COLUMN, GRIP, INDENT, tree_rows};
use crate::*;

/// The words of the header, one for each column.
const HEADINGS: [&str; 6] = [
    "ui.layer.name",
    "ui.layer.on",
    "ui.layer.lock",
    "ui.layer.colour",
    "ui.layer.linetype",
    "ui.layer.print_width",
];

/// The table being drawn: where its cells go, the layer that is current, the entered detail
/// with the layers hidden in it, and how wide each column's widest cell is so far.
struct Table {
    id: egui::Id,
    columns: [f32; 6],
    /// Room for the detail column, before the others.
    extra: f32,
    gap: f32,
    row: f32,
    total: f32,
    current: u64,
    in_detail: Option<(u64, Vec<u64>)>,
    widest: [f32; 6],
    /// The layers the selected objects are on, softly lit.
    lit: std::collections::BTreeSet<u64>,
}

impl Table {
    /// The cell of column `index` in the row at `rect`.
    fn cell(&self, rect: egui::Rect, index: usize) -> egui::Rect {
        let x = rect.min.x
            + self.extra
            + self.columns[..index].iter().sum::<f32>()
            + self.gap * index as f32;
        egui::Rect::from_min_size(
            egui::pos2(x, rect.min.y),
            egui::vec2(self.columns[index], self.row),
        )
    }

    /// The detail column's cell in the row at `rect`. It comes first, so it shows however
    /// narrow the panel is.
    fn detail_cell(&self, rect: egui::Rect) -> egui::Rect {
        egui::Rect::from_min_size(rect.min, egui::vec2(DETAIL_COLUMN, self.row))
    }

    /// Lays out `add` in column `index` of the row at `rect`, clipped to its cell, and notes how
    /// wide it came out.
    fn in_cell(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        index: usize,
        add: impl FnOnce(&mut egui::Ui),
    ) {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(self.cell(rect, index))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.set_clip_rect(self.cell(rect, index).intersect(ui.clip_rect()));
        add(&mut child);
        self.widest[index] = self.widest[index].max(child.min_rect().width());
    }
}

/// A layer as the table lists it: how deep it is under other layers, and whether it has
/// sublayers.
struct Listed<'a> {
    layer: &'a curva_document::Layer,
    depth: usize,
    has_sublayers: bool,
}

impl Listed<'_> {
    fn indent(&self) -> f32 {
        INDENT * self.depth as f32
    }
}

/// `columns` fitted to `room`: the Name column (the first) takes whatever room the others
/// leave. Where they do not all fit, the others give up what they have above their least
/// widths, each in proportion, before the Name column narrows, never below its least.
fn fit_columns(mut columns: [f32; 6], room: f32) -> [f32; 6] {
    let least = layout::LEAST_LAYER_COLUMNS;
    let others = |columns: &[f32; 6]| columns[1..].iter().sum::<f32>();
    let over = columns[0] + others(&columns) - room;
    if over > 0.0 {
        let slack: f32 = (1..6).map(|i| columns[i] - least[i]).sum();
        let share = (over / slack.max(1e-6)).min(1.0);
        for i in 1..6 {
            columns[i] -= (columns[i] - least[i]) * share;
        }
    }
    columns[0] = (room - others(&columns)).max(least[0]);
    columns
}

/// Outlines `rect` while a layer is dragged over it.
fn outline_drop_target(ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) {
    ui_kit::drop_outline::<u64>(ui, response, rect);
}

/// The divider after column `index` of the header, whose cell is `rect`: dragged, it sizes the
/// column; double-clicked, it fits the column to its widest cell last frame. Returns whether
/// the column changed and whether the change is finished.
fn column_divider(
    ui: &egui::Ui,
    table: &mut Table,
    rect: egui::Rect,
    index: usize,
    last_widest: &[f32; 6],
) -> (bool, bool) {
    let (mut resized, mut finished) = (false, false);
    let divider = egui::Rect::from_center_size(
        egui::pos2(rect.max.x + table.gap / 2.0, rect.center().y),
        egui::vec2(table.gap.max(6.0), table.row),
    );
    let handle = ui.interact(
        divider,
        table.id.with(("divider", index)),
        egui::Sense::click_and_drag(),
    );
    if handle.hovered() || handle.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
        ui_kit::divider_lit(ui, divider.center().x, divider.y_range());
    }
    let columns = &mut table.columns;
    if handle.dragged() {
        columns[index] =
            (columns[index] + handle.drag_delta().x).max(layout::LEAST_LAYER_COLUMNS[index]);
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
    (resized, finished)
}

impl Curva {
    /// The layers as a table with a header row whose dividers size each column.
    pub(super) fn layer_table(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[curva_document::Layer],
        current: u64,
    ) {
        let id = ui.scope_id().with("layer table");
        // What each column's widest cell measured last frame, for fitting a column to it.
        let widest_id = id.with("widest");
        let last_widest: [f32; 6] = ui.data(|d| d.get_temp(widest_id)).unwrap_or([0.0; 6]);
        let mut table = self.layer_table_size(ui, id, current);
        self.layer_header(ui, &mut table, &last_widest);
        let mut name_fit = 0.0_f32;
        // What the rows look up about layers (a parent's visibility, lock and parent), without
        // copying the whole model every frame: that took most of a frame on large files.
        let state = curva_document::State {
            layers: layers.to_vec(),
            ..curva_document::State::default()
        };
        // The search and the filters leave out layers, as in Rhino.
        let search = self.frames.layers_ui.search.clone();
        let filter = self.frames.layers_ui.filter;
        let with_objects: std::collections::HashSet<u64> =
            self.doc.state().objects.iter().map(|o| o.layer).collect();
        let rows = tree_rows(layers).into_iter().filter(|(layer, ..)| {
            super::layer_tools::name_matches(&layer.name, &search)
                && filter.passes(
                    layer.visible,
                    layer.locked,
                    with_objects.contains(&layer.id),
                )
        });
        for (layer, depth, has_sublayers) in rows {
            let listed = Listed {
                layer,
                depth,
                has_sublayers,
            };
            name_fit = name_fit.max(self.layer_row(ui, &mut table, &listed, &state));
        }
        let mut widest = table.widest;
        widest[0] = name_fit;
        // A dropdown fits when it shows its usual width.
        widest[4] = widest[4].max(96.0);
        widest[5] = widest[5].max(84.0);
        ui.data_mut(|d| d.insert_temp(widest_id, widest));
    }

    /// The table's columns sized for the panel, with a first column, while a detail is entered,
    /// that hides layers in that detail only (spec `layouts.md` §2).
    fn layer_table_size(&self, ui: &egui::Ui, id: egui::Id, current: u64) -> Table {
        // Close-set columns, so the name has the room (owner, 22 November 2026).
        let gap = 4.0;
        let row = ui.spacing().interact_size.y;
        let mut columns = self.layout.layer_columns;
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
        // The columns fit the panel: no sideways scrolling, the Name column taking what is left.
        columns = fit_columns(columns, ui.available_width() - gap * 5.0 - extra);
        let total: f32 = columns.iter().sum::<f32>() + gap * 5.0 + extra;
        Table {
            id,
            columns,
            extra,
            gap,
            row,
            total,
            current,
            in_detail,
            widest: [0.0; 6],
            lit: self.doc.layers_of_selection(),
        }
    }

    /// The header, with a divider after each column to drag or double-click. A layer dropped on
    /// it goes to the top level (spec `sublayers.md` §2).
    fn layer_header(&mut self, ui: &mut egui::Ui, table: &mut Table, last_widest: &[f32; 6]) {
        let (header, header_response) =
            ui.allocate_exact_size(egui::vec2(table.total, table.row), egui::Sense::hover());
        if let Some(dragged) = header_response.dnd_release_payload::<u64>() {
            let dragged = *dragged;
            self.undoable(|doc| {
                doc.set_layer_parent(dragged, None);
            });
        }
        outline_drop_target(ui, &header_response, header);
        if table.in_detail.is_some() {
            let at = table.detail_cell(header).left_center() + egui::vec2(2.0, 0.0);
            ui_kit::header_note(
                ui,
                at,
                self.text.render(&Message::new("ui.layer.in_detail")),
            );
        }
        let mut resized = false;
        let mut finished = false;
        for (index, key) in HEADINGS.iter().enumerate() {
            let rect = table.cell(header, index);
            // "Name" starts where the rows' grips do, past the room kept for the triangle.
            let inset = if index == 0 { ARROW + 2.0 } else { 0.0 };
            let at = rect.left_center() + egui::vec2(inset, 0.0);
            ui_kit::header_text(ui, at, self.text.render(&Message::new(key)));
            let (r, f) = column_divider(ui, table, rect, index, last_widest);
            resized |= r;
            finished |= f;
        }
        if resized {
            self.layout.layer_columns = table.columns;
        }
        if finished {
            self.layout.layer_columns = table.columns.map(f32::round);
            self.save_layout();
        }
    }

    /// One layer's row. A layer dropped on it becomes its sublayer. Returns how wide the Name
    /// column must be to fit this layer's name.
    fn layer_row(
        &mut self,
        ui: &mut egui::Ui,
        table: &mut Table,
        listed: &Listed<'_>,
        state: &curva_document::State,
    ) -> f32 {
        let layer = listed.layer;
        // The Name column fits the longest name, not the room its field was given.
        let body = egui::TextStyle::Body.resolve(ui.style());
        let name_width = ui_kit::text_width(ui, &layer.name, &body);
        let fit = name_width + 56.0 + listed.indent() + ARROW + GRIP;
        let (rect, row_response) =
            ui.allocate_exact_size(egui::vec2(table.total, table.row), egui::Sense::hover());
        if let Some(dragged) = row_response.dnd_release_payload::<u64>() {
            let (dragged, under) = (*dragged, layer.id);
            if dragged != under {
                self.undoable(|doc| {
                    if doc.set_layer_parent(dragged, Some(under)) {
                        doc.set_layer_collapsed(under, false);
                    }
                });
            }
        }
        outline_drop_target(ui, &row_response, rect);
        // The layer of what is selected is softly lit, so it can be found in a long list.
        if table.lit.contains(&layer.id) {
            ui_kit::related_row(ui, rect);
        }
        // The layer picked in the list, which the toolbar acts on, is lit.
        if self.frames.layers_ui.selected == Some(layer.id) {
            ui_kit::chosen_row(ui, rect);
        }
        let (current, height) = (table.current, table.row);
        table.in_cell(ui, rect, 0, |ui| {
            self.layer_name_cell(ui, listed, current, height, state);
        });
        self.layer_property_cells(ui, table, rect, layer, state);
        self.layer_detail_box(ui, table, rect, layer.id);
        fit
    }

    /// The Name cell: the indent, the expand triangle, the drag grip, the button that makes
    /// the layer current and its name, which has the layer's menu.
    fn layer_name_cell(
        &mut self,
        ui: &mut egui::Ui,
        listed: &Listed<'_>,
        current: u64,
        height: f32,
        state: &curva_document::State,
    ) {
        let layer = listed.layer;
        ui_kit::tight(ui);
        ui.add_space(listed.indent());
        self.sublayer_arrow(ui, listed, height);
        self.layer_grip(ui, layer.id, height);
        if ui.radio(layer.id == current, "").clicked() {
            self.undoable(|doc| doc.set_current_layer(layer.id));
        }
        let mut name = layer.name.clone();
        let room = ui.available_width();
        // Without a frame the name keeps all its room; clicking it picks the layer.
        let edit = ui_kit::plain_field(ui, &mut name, room);
        if edit.changed() {
            self.doc.rename_layer(layer.id, name.clone());
        }
        if edit.gained_focus() || edit.clicked() {
            self.frames.layers_ui.selected = Some(layer.id);
        }
        self.layer_menu(&edit, listed, state);
    }

    /// Rhino's Expand and Collapse Sublayers: a triangle, painted as the font has no arrow. The
    /// room for it is kept on layers without sublayers too.
    fn sublayer_arrow(&mut self, ui: &mut egui::Ui, listed: &Listed<'_>, height: f32) {
        let layer = listed.layer;
        let clicked = ui_kit::tree_arrow(ui, ARROW, height, layer.collapsed, listed.has_sublayers);
        if clicked.clicked() && listed.has_sublayers {
            let (id, folded) = (layer.id, !layer.collapsed);
            self.doc.set_layer_collapsed(id, folded);
        }
    }

    /// A grip to drag the layer `id` onto another, or onto the header for the top.
    fn layer_grip(&self, ui: &mut egui::Ui, id: u64, height: f32) {
        let handle_id = ui.scope_id().with(("layer drag", id));
        let drag_hint = self.text.render(&Message::new("ui.layer.drag"));
        ui.dnd_drag_source(handle_id, id, |ui| {
            let hover = ui_kit::drag_grip(ui, GRIP, height);
            hover.on_hover_text(drag_hint);
        });
    }

    /// The menu on a layer's name: a new sublayer, up one parent, select its objects (and its
    /// sublayers'), delete.
    fn layer_menu(
        &mut self,
        edit: &egui::Response,
        listed: &Listed<'_>,
        state: &curva_document::State,
    ) {
        let (layer, has_sublayers) = (listed.layer, listed.has_sublayers);
        // The menu's words, rendered now, as the menu's actions change the model.
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let new_sublayer = say("ui.layer.new_sublayer");
        let sublayer_name = say("ui.layer.sublayer_name");
        let up_one_parent = say("ui.layer.up_one_parent");
        let select_objects = say("ui.layer.select_objects");
        let select_sublayer_objects = say("ui.layer.select_sublayer_objects");
        let delete = say("ui.layer.delete");
        edit.context_menu(|ui| {
            if ui.button(&new_sublayer).clicked() {
                let name = sublayer_name.clone();
                let id = layer.id;
                self.undoable(|doc| {
                    doc.add_sublayer(id, name);
                    doc.set_layer_collapsed(id, false);
                });
                ui.close();
            }
            if let Some(parent) = layer.parent
                && ui.button(&up_one_parent).clicked()
            {
                let above = state.layer(parent).and_then(|p| p.parent);
                let id = layer.id;
                self.undoable(|doc| {
                    doc.set_layer_parent(id, above);
                });
                ui.close();
            }
            if ui.button(&select_objects).clicked() {
                let picked = self.doc.objects_on_layer(layer.id, false);
                self.doc.select(&picked);
                ui.close();
            }
            if has_sublayers && ui.button(&select_sublayer_objects).clicked() {
                let picked = self.doc.objects_on_layer(layer.id, true);
                self.doc.select(&picked);
                ui.close();
            }
            ui.separator();
            if ui_kit::danger_button(ui, &delete).clicked() {
                self.undoable(|doc| {
                    doc.delete_layer(layer.id);
                });
                ui.close();
            }
        });
    }

    /// The On, Lock, Colour, Linetype and Print width cells. A box the layer's parent rules
    /// is greyed, as it cannot be turned on or unlocked on its own (Rhino's half-drawn icons).
    fn layer_property_cells(
        &mut self,
        ui: &mut egui::Ui,
        table: &mut Table,
        rect: egui::Rect,
        layer: &curva_document::Layer,
        state: &curva_document::State,
    ) {
        let parent_off = layer.parent.is_some_and(|p| !state.layer_shown(p));
        let parent_locked = layer.parent.is_some_and(|p| state.layer_locked(p));
        table.in_cell(ui, rect, 1, |ui| {
            let mut visible = layer.visible && !parent_off;
            if ui_kit::tick_when(ui, !parent_off, &mut visible).changed() {
                self.undoable(|doc| doc.set_layer_visible(layer.id, visible));
            }
        });
        table.in_cell(ui, rect, 2, |ui| {
            let mut locked = layer.locked || parent_locked;
            if ui_kit::tick_when(ui, !parent_locked, &mut locked).changed() {
                self.undoable(|doc| doc.set_layer_locked(layer.id, locked));
            }
        });
        table.in_cell(ui, rect, 3, |ui| {
            let mut colour = layer.colour;
            if ui.color_edit_button_rgb(&mut colour).changed() {
                self.doc.set_layer_colour(layer.id, colour);
            }
        });
        table.in_cell(ui, rect, 4, |ui| {
            let id = ui.scope_id().with(("layer linetype", layer.id));
            let width = ui.available_width() - 4.0;
            if let Some(linetype) = self.linetype_choice(ui, id, Some(layer.linetype), false, width)
            {
                self.undoable(|doc| doc.set_layer_linetype(layer.id, linetype));
            }
        });
        table.in_cell(ui, rect, 5, |ui| {
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
    }

    /// While a detail is entered, the box that shows or hides layer `id` in that detail only.
    fn layer_detail_box(&mut self, ui: &mut egui::Ui, table: &Table, rect: egui::Rect, id: u64) {
        let Some((frame, hidden)) = &table.in_detail else {
            return;
        };
        let mut shown = !hidden.contains(&id);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(table.detail_cell(rect))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        if ui_kit::tick(&mut child, &mut shown).changed() {
            let frame = *frame;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn total(columns: [f32; 6]) -> f32 {
        columns.iter().sum()
    }

    #[test]
    fn the_columns_never_run_past_the_panel() {
        let standard = layout::STANDARD_LAYER_COLUMNS;
        let fitted = fit_columns(standard, 290.0);
        assert!((total(fitted) - 290.0).abs() < 1e-3, "{fitted:?}");
        // The others gave up some of their room before the name narrowed below its own.
        assert!(fitted[0] >= layout::LEAST_LAYER_COLUMNS[0]);
        for i in 1..6 {
            assert!(fitted[i] <= standard[i] && fitted[i] >= layout::LEAST_LAYER_COLUMNS[i]);
        }
    }

    #[test]
    fn a_wide_panel_gives_the_name_the_room_left() {
        let standard = layout::STANDARD_LAYER_COLUMNS;
        let fitted = fit_columns(standard, 600.0);
        assert_eq!(fitted[1..], standard[1..]);
        assert!((total(fitted) - 600.0).abs() < 1e-3);
    }

    #[test]
    fn a_very_narrow_panel_keeps_every_column_at_its_least() {
        let fitted = fit_columns(layout::STANDARD_LAYER_COLUMNS, 50.0);
        assert_eq!(fitted, layout::LEAST_LAYER_COLUMNS);
    }
}
