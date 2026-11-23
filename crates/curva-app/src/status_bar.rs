//! The status panel, as in the approved mockup (owner, 23 November 2026): a compact panel at the
//! bottom left, level with the command panel and to its left. Its top row says where the pointer
//! is, the model's unit and the current layer; its bottom row holds the modelling aids, as
//! Rhino's status bar.

use crate::*;

/// How wide the status panel is.
const STATUS_WIDTH: f32 = 385.0;

/// One axis of where the pointer aims: its letter and its value with two decimals.
pub(crate) fn coordinates(at: Point) -> [(char, String); 3] {
    [('x', 0), ('y', 1), ('z', 2)].map(|(letter, axis)| (letter, format!("{:.2}", at[axis])))
}

/// Where the status panel goes: left of the command panel `command`, its bottom level with it,
/// from the window's left edge; above the command panel when there is no room beside it.
fn status_spot(window: egui::Rect, command: Option<egui::Rect>) -> egui::Pos2 {
    let left = window.min.x + layout::GAP;
    match command {
        Some(rect) if rect.min.x - layout::GAP - left >= STATUS_WIDTH => {
            egui::pos2(left, rect.max.y)
        }
        Some(rect) => egui::pos2(rect.min.x, rect.min.y - layout::GAP),
        None => egui::pos2(left, window.max.y - layout::GAP),
    }
}

impl Curva {
    /// The status panel, placed by the command panel in `window`, sliding away in focus mode as
    /// the other panels do.
    pub(crate) fn status_panel(&mut self, ctx: &egui::Context, window: egui::Rect) {
        let command = self
            .layout
            .panel_of(layout::Piece::Command)
            .and_then(|panel| {
                self.frames
                    .drawn_panels
                    .iter()
                    .find(|(drawn, _)| *drawn == panel)
                    .map(|(_, rect)| *rect)
            });
        let spot = status_spot(window, command);
        let drop = ctx.animate_value_with_time(
            egui::Id::NULL.with("status panel slide"),
            if self.frames.focus {
                window.max.y - spot.y + 200.0
            } else {
                0.0
            },
            ui_kit::motion(SLIDE_TIME),
        );
        let glass = self.settings.glass;
        egui::Area::new(egui::Id::NULL.with("status panel"))
            .fixed_pos(spot + egui::vec2(0.0, drop))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                if glass {
                    ui_kit::glass_backdrop(ui, egui::Rect::from_min_size(rect.min, ui.min_size()));
                }
                ui_kit::panel_frame_for(glass).show(ui, |ui| {
                    ui.set_width(STATUS_WIDTH - 2.0 * PANEL_MARGIN);
                    self.status_readouts(ui);
                    self.modelling_aids(ui);
                });
            });
    }

    /// Where the pointer is and the unit, then the current layer at the row's end, which can be
    /// changed here.
    fn status_readouts(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match self.last_aim {
                Some(at) => {
                    for (letter, value) in coordinates(at) {
                        ui.label(ui_kit::quiet(letter.to_string()));
                        ui.label(ui_kit::numbers(value));
                    }
                }
                // Dashes until the pointer is over a view, so the panel keeps its shape.
                None => {
                    for letter in ['x', 'y', 'z'] {
                        ui.label(ui_kit::quiet(format!("{letter} –")));
                    }
                }
            }
            ui.label(ui_kit::quiet(self.doc.state().unit.symbol()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.current_layer_choice(ui);
                ui.label(ui_kit::quiet(self.text.render(&Message::new("ui.layer"))));
            });
        });
    }

    /// The current layer, as a dropdown of all the layers.
    fn current_layer_choice(&mut self, ui: &mut egui::Ui) {
        let current = self.doc.current_layer();
        let name = self
            .doc
            .layers()
            .iter()
            .find(|layer| layer.id == current)
            .map_or_else(String::new, |layer| layer.name.clone());
        let mut chosen = None;
        ui_kit::dropdown(ui, "status layer", name, |ui| {
            for layer in self.doc.layers() {
                if ui_kit::list_item(ui, layer.id == current, layer.name.as_str()).clicked() {
                    chosen = Some(layer.id);
                }
            }
        });
        if let Some(id) = chosen.filter(|id| *id != current) {
            self.doc.set_current_layer(id);
        }
    }

    /// Ortho, Planar, grid snap, SmartTrack, the gumball and the selection filter, with Rhino's keys for
    /// ortho and grid snap.
    fn modelling_aids(&mut self, ui: &mut egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::F8)) {
            self.ortho = !self.ortho;
        }
        if ui.input(|input| input.key_pressed(egui::Key::F9)) {
            self.grid_snap = !self.grid_snap;
        }
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let [ortho, planar, grid, track, gumball, filter] = [
            "ui.ortho",
            "ui.planar",
            "ui.grid_aid",
            "ui.smart_track",
            "ui.gumball",
            "ui.filter",
        ]
        .map(say);
        let filter_lit = self.pick_filter.in_force() || self.filter_row;
        let mut item = None;
        ui_kit::chip_line(ui, |ui| {
            ui_kit::toggle(ui, &mut self.ortho, ortho);
            ui_kit::toggle(ui, &mut self.planar, planar);
            ui_kit::toggle(ui, &mut self.grid_snap, grid);
            ui_kit::toggle(ui, &mut self.snaps.track, track);
            ui_kit::toggle(ui, &mut self.gumball_on, gumball);
            item = Some(ui_kit::list_item(ui, filter_lit, filter));
        });
        if let Some(item) = item {
            self.selection_filter_panel(ui, &item);
        }
    }

    /// The software's name and version, small and quiet, at the window's bottom right.
    pub(crate) fn version_label(&self, ctx: &egui::Context, window: egui::Rect) {
        let text = format!("Curva {}", env!("CARGO_PKG_VERSION"));
        ui_kit::corner_caption(ctx, window, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_show_two_decimals_for_each_axis() {
        let shown = coordinates([1.0, -2.5, 0.126]);
        assert_eq!(shown[0], ('x', "1.00".to_owned()));
        assert_eq!(shown[1], ('y', "-2.50".to_owned()));
        assert_eq!(shown[2], ('z', "0.13".to_owned()));
    }

    #[test]
    fn the_status_panel_sits_left_of_the_command_panel_level_with_its_bottom() {
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
        let command = egui::Rect::from_min_max(egui::pos2(480.0, 800.0), egui::pos2(1200.0, 980.0));
        let spot = status_spot(window, Some(command));
        assert_eq!(spot, egui::pos2(layout::GAP, 980.0));
        // With no room beside it, it goes above.
        let narrow = command.translate(egui::vec2(-400.0, 0.0));
        assert_eq!(
            status_spot(window, Some(narrow)),
            egui::pos2(80.0, 800.0 - layout::GAP)
        );
    }
}
