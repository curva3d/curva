//! The status bar along the bottom of the window, as Rhino's: where the pointer is, the model's
//! unit and current layer on the left, the modelling aids on the right (owner, 23 November
//! 2026: the UI review's proposal, Apple's guidelines' one row per purpose).

use crate::*;

/// How tall the status bar is.
pub(crate) const STATUS_BAR_HEIGHT: f32 = ui_kit::ROW + 2.0 * ui_kit::BAR_PAD;

/// Where the pointer aims, as x, y and z with the decimals the model's unit wants.
pub(crate) fn coordinates(at: Point) -> String {
    format!("x {:.3}   y {:.3}   z {:.3}", at[0], at[1], at[2])
}

impl Curva {
    /// Leaves room for the status bar at the foot of `window`, sliding it away in focus mode as
    /// the top bar does, and returns the room above it.
    pub(crate) fn status_bar_slot(
        &mut self,
        ctx: &egui::Context,
        window: egui::Rect,
    ) -> egui::Rect {
        let bar = egui::Rect::from_min_size(
            egui::pos2(
                window.min.x + layout::GAP,
                window.max.y - layout::GAP - STATUS_BAR_HEIGHT,
            ),
            egui::vec2(window.width() - 2.0 * layout::GAP, STATUS_BAR_HEIGHT),
        );
        let drop = ctx.animate_value_with_time(
            egui::Id::NULL.with("status bar slide"),
            if self.frames.focus {
                window.max.y - bar.min.y
            } else {
                0.0
            },
            ui_kit::motion(SLIDE_TIME),
        );
        self.status_bar(ctx, bar.translate(egui::vec2(0.0, drop)));
        egui::Rect::from_min_max(window.min, egui::pos2(window.max.x, bar.min.y))
    }

    /// The status bar itself, at `rect`.
    fn status_bar(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let glass = self.settings.glass;
        egui::Area::new(egui::Id::NULL.with("status bar"))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(rect.expand(SHADOW_ROOM));
                if glass {
                    ui_kit::glass_backdrop(ui, rect);
                }
                ui_kit::bar_frame(glass).show(ui, |ui| {
                    let inner = rect.shrink2(ui_kit::bar_margin());
                    ui.set_min_size(inner.size());
                    ui.set_max_size(inner.size());
                    ui.horizontal_centered(|ui| {
                        self.status_readouts(ui);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            self.modelling_aids(ui);
                        });
                    });
                });
            });
    }

    /// Where the pointer is, the unit and the current layer, which can be changed here.
    fn status_readouts(&mut self, ui: &mut egui::Ui) {
        // Dashes until the pointer is over a view, so the bar keeps its shape.
        let at = self
            .last_aim
            .map_or_else(|| "x –   y –   z –".to_owned(), coordinates);
        ui.add(egui::Label::new(ui_kit::numbers(at)).selectable(false));
        ui_kit::divider(ui);
        ui.label(ui_kit::quiet(self.doc.state().unit.symbol()));
        ui_kit::divider(ui);
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

    /// Grid snap, ortho, SmartTrack, the gumball and the selection filter, right to left, with
    /// Rhino's keys for ortho and grid snap.
    fn modelling_aids(&mut self, ui: &mut egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::F8)) {
            self.ortho = !self.ortho;
        }
        if ui.input(|input| input.key_pressed(egui::Key::F9)) {
            self.grid_snap = !self.grid_snap;
        }
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let filter_word = say("ui.filter");
        let words = [
            say("ui.grid_snap"),
            say("ui.ortho"),
            say("ui.smart_track"),
            say("ui.gumball"),
        ];
        let filter_lit = self.pick_filter.in_force() || self.filter_row;
        // Right to left: the version at the very end, quiet, then the filter.
        ui.label(ui_kit::caption(format!(
            "Curva {}",
            env!("CARGO_PKG_VERSION")
        )));
        ui_kit::divider(ui);
        let item = ui_kit::list_item(ui, filter_lit, filter_word);
        ui_kit::divider(ui);
        let [grid, ortho, track, gumball] = words;
        ui_kit::toggle(ui, &mut self.gumball_on, gumball);
        ui_kit::toggle(ui, &mut self.snaps.track, track);
        ui_kit::toggle(ui, &mut self.ortho, ortho);
        ui_kit::toggle(ui, &mut self.grid_snap, grid);
        self.selection_filter_panel(ui, &item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_show_three_decimals_for_each_axis() {
        assert_eq!(
            coordinates([1.0, -2.5, 0.125]),
            "x 1.000   y -2.500   z 0.125"
        );
    }
}
