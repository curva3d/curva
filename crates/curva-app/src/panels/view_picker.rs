//! The view picker in the Display panel (the owner's choice, 18 November 2026): the active view's
//! name, opening the box unfolded like a plan, Back above, Left, Top and Right across, Front and
//! Bottom below, then the perspective views and the lens.

use crate::*;
use curva_viewport::StandardView;

/// The views across the unfolded box, row by row; `None` leaves a gap.
const BOX: [[Option<StandardView>; 3]; 3] = [
    [None, Some(StandardView::Back), None],
    [
        Some(StandardView::Left),
        Some(StandardView::Top),
        Some(StandardView::Right),
    ],
    [None, Some(StandardView::Front), Some(StandardView::Bottom)],
];

/// The size of a view in the unfolded box, in points.
const CELL: egui::Vec2 = egui::vec2(58.0, 46.0);

/// What the picker asks for: a view, or a new lens for the view shown.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Picked {
    View(StandardView),
    Lens(f64),
}

impl Curva {
    /// The active view's name; clicked, the picker opens below it.
    pub(crate) fn view_picker(&mut self, ui: &mut egui::Ui) {
        let pane = &self.panes[self.active];
        let current = pane.view;
        let camera = pane.camera;
        let title = pane
            .named
            .clone()
            .unwrap_or_else(|| self.text.render(&Message::new(view_label(current))));
        // A button with the accent's outline, as in the chosen mock-up, so it reads as one, and
        // "View:" before the name, quieter, to say what it is (#471).
        let prefix = self.text.render(&Message::new("ui.view.button"));
        let button = ui_kit::opener(ui, &prefix, &title);
        let mut picked = None;
        egui::Popup::menu(&button)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| picked = self.view_box(ui, current, &camera));
        match picked {
            Some(Picked::View(view)) => {
                let display = self.panes[self.active].display;
                self.panes[self.active] = Pane::new(view);
                self.panes[self.active].display = display;
            }
            Some(Picked::Lens(millimetres)) => {
                self.panes[self.active].camera.set_lens_length(millimetres);
            }
            None => {}
        }
    }

    /// The unfolded box, the perspective views and the lens; returns what was picked.
    fn view_box(
        &self,
        ui: &mut egui::Ui,
        current: StandardView,
        camera: &curva_viewport::Camera,
    ) -> Option<Picked> {
        let mut picked = None;
        ui.label(ui_kit::caption(
            self.text.render(&Message::new("ui.view.caption")),
        ));
        // Each view a filled tile with the house seen from its side, the one shown in the accent,
        // saying what it looks at when the pointer rests on it (#471).
        let mut view_button = |ui: &mut egui::Ui, view: StandardView, size: egui::Vec2| {
            let name = self.text.render(&Message::new(view_label(view)));
            let hint = self.text.render(&Message::new(view_hint(view)));
            if view_tile(ui, view, &name, view == current, size)
                .on_hover_text(hint)
                .clicked()
            {
                picked = Some(Picked::View(view));
            }
        };
        egui::Grid::new("view box")
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for row in BOX {
                    for cell in row {
                        match cell {
                            Some(view) => view_button(ui, view, CELL),
                            None => {
                                ui.allocate_space(CELL);
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        let wide = egui::vec2(CELL.x * 1.5 + 2.0, CELL.y);
        ui.horizontal(|ui| {
            view_button(ui, StandardView::Perspective, wide);
            view_button(ui, StandardView::TwoPoint, wide);
        });
        if let Some(lens) = self.lens_row(ui, camera) {
            picked = Some(Picked::Lens(lens));
        }
        picked
    }

    /// The lens, in millimetres and as the angle it sees across, as Rhino shows them; only a
    /// perspective has one. Returns the new lens length when either is changed.
    fn lens_row(&self, ui: &mut egui::Ui, camera: &curva_viewport::Camera) -> Option<f64> {
        let perspective = !camera.is_parallel();
        let mut lens = camera.lens_length();
        let mut angle = camera.angle_of_view();
        let mut changed = None;
        ui.add_enabled_ui(perspective, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.text.render(&Message::new("ui.view.lens")));
                let typed = ui.add(
                    egui::DragValue::new(&mut lens)
                        .range(5.0..=2000.0)
                        .speed(0.5)
                        .max_decimals(1)
                        .suffix(" mm"),
                );
                if typed.changed() {
                    changed = Some(lens);
                }
                ui.label(self.text.render(&Message::new("ui.view.angle")));
                let turned = ui.add(
                    egui::DragValue::new(&mut angle)
                        .range(1.0..=150.0)
                        .speed(0.2)
                        .max_decimals(1)
                        .suffix("°"),
                );
                if turned.changed() {
                    changed = Some(lens_for_angle(angle));
                }
            });
        })
        .response
        .on_disabled_hover_text(self.text.render(&Message::new("ui.view.lens_flat")));
        changed
    }
}

/// One view's tile in the picker: filled, the house seen from its side above its name, in the
/// accent when it is the view shown.
fn view_tile(
    ui: &mut egui::Ui,
    view: StandardView,
    name: &str,
    shown: bool,
    size: egui::Vec2,
) -> egui::Response {
    ui_kit::picture_tile(ui, size, shown, name, |painter, room, ink| {
        crate::icons::houses::paint_house(painter, view, room, ink);
    })
}

/// What each view looks at, said when the pointer rests on its tile.
fn view_hint(view: StandardView) -> &'static str {
    match view {
        StandardView::Top => "ui.view.hint.top",
        StandardView::Bottom => "ui.view.hint.bottom",
        StandardView::Front => "ui.view.hint.front",
        StandardView::Back => "ui.view.hint.back",
        StandardView::Left => "ui.view.hint.left",
        StandardView::Right => "ui.view.hint.right",
        StandardView::Perspective => "ui.view.hint.perspective",
        StandardView::TwoPoint => "ui.view.hint.two_point",
    }
}

/// The lens length, in millimetres on 35 mm film, that sees `degrees` across the film's 36 mm
/// width, as `Camera::set_angle_of_view` takes it.
fn lens_for_angle(degrees: f64) -> f64 {
    let mut camera = curva_viewport::Camera::default();
    camera.set_angle_of_view(degrees);
    camera.lens_length()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_standard_view_has_its_place_in_the_picker() {
        let mut placed: Vec<StandardView> = BOX.iter().flatten().flatten().copied().collect();
        placed.extend([StandardView::Perspective, StandardView::TwoPoint]);
        for view in StandardView::ALL {
            assert_eq!(placed.iter().filter(|v| **v == view).count(), 1, "{view:?}");
        }
    }

    #[test]
    fn an_angle_of_view_gives_rhinos_lens() {
        assert!((lens_for_angle(39.6) - 50.0).abs() < 0.1);
        assert!((lens_for_angle(90.0) - 18.0).abs() < 1e-9);
    }

    /// Every view says what it looks at, each in its own words (#471).
    #[test]
    fn every_view_has_its_own_hint() {
        let hints: Vec<&str> = StandardView::ALL.iter().map(|&v| view_hint(v)).collect();
        for (i, hint) in hints.iter().enumerate() {
            assert!(hint.starts_with("ui.view.hint."));
            assert!(!hints[i + 1..].contains(hint), "{hint}");
        }
    }
}
