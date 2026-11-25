//! The status panel, as in the approved mockup (owner, 23 November 2026): a compact panel at the
//! bottom left, level with the command panel and to its left; a panel like the others, which
//! can be moved, merged and closed (owner, 25 November 2026). Its top row says where the pointer
//! is, the model's unit and the current layer; its bottom row holds the modelling aids, as
//! Rhino's status bar.

use crate::*;

/// One axis of where the pointer aims: its letter and its value with two decimals.
pub(crate) fn coordinates(at: Point) -> [(char, String); 3] {
    [('x', 0), ('y', 1), ('z', 2)].map(|(letter, axis)| (letter, format!("{:.2}", at[axis])))
}

impl Curva {
    /// The status panel's contents: where the pointer is, then the modelling aids.
    pub(crate) fn status_piece(&mut self, ui: &mut egui::Ui) {
        self.status_readouts(ui);
        self.modelling_aids(ui);
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
                // The word "Layer" only where long coordinates leave room for it, so they never run
                // into each other.
                let word = self.text.render(&Message::new("ui.layer"));
                let font = egui::TextStyle::Body.resolve(ui.style());
                if ui.available_width() > ui_kit::text_width(ui, &word, &font) + ui_kit::ITEM_GAP {
                    ui.label(ui_kit::quiet(word));
                }
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
}
