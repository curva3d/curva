//! The view picker in the Display panel (the owner's choice, 18 November 2026): the active view's
//! name, opening the box unfolded like a plan, Back above, Left, Top and Right across, Front and
//! Bottom below, then the perspective views and the lens.

use crate::*;
use curva_viewport::StandardView;

/// The views across the unfolded box, row by row; `None` leaves a gap. The names of views the
/// camera does not offer yet (#356) stand where they will go.
const BOX: [[Option<Cell>; 3]; 3] = [
    [None, Some(Cell::Soon("ui.view.back")), None],
    [
        Some(Cell::Soon("ui.view.left")),
        Some(Cell::View(StandardView::Top)),
        Some(Cell::View(StandardView::Right)),
    ],
    [
        None,
        Some(Cell::View(StandardView::Front)),
        Some(Cell::Soon("ui.view.bottom")),
    ],
];

/// A place in the picker: a view to pick, or one still to come.
#[derive(Clone, Copy)]
enum Cell {
    View(StandardView),
    Soon(&'static str),
}

/// The size of a view in the unfolded box, in points.
const CELL: egui::Vec2 = egui::vec2(58.0, 26.0);

impl Curva {
    /// The active view's name; clicked, the picker opens below it.
    pub(crate) fn view_picker(&mut self, ui: &mut egui::Ui) {
        let pane = &self.panes[self.active];
        let current = pane.view;
        let title = pane
            .named
            .clone()
            .unwrap_or_else(|| self.text.render(&Message::new(view_label(current))));
        let button = ui.add_sized(
            [ui.available_width(), 22.0],
            egui::Button::new(format!("{title} ▾")),
        );
        let mut chosen = None;
        egui::Popup::menu(&button).show(|ui| chosen = self.view_box(ui, current));
        if let Some(view) = chosen {
            let display = self.panes[self.active].display;
            self.panes[self.active] = Pane::new(view);
            self.panes[self.active].display = display;
        }
    }

    /// The unfolded box, the perspective views and the lens; returns the view picked.
    fn view_box(&self, ui: &mut egui::Ui, current: StandardView) -> Option<StandardView> {
        let mut chosen = None;
        let soon = self.text.render(&Message::new("ui.view.soon"));
        ui.label(
            egui::RichText::new(self.text.render(&Message::new("ui.view.caption")))
                .small()
                .weak(),
        );
        egui::Grid::new("view box")
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for row in BOX {
                    for cell in row {
                        match cell {
                            None => {
                                ui.allocate_space(CELL);
                            }
                            Some(Cell::View(view)) => {
                                let name = self.text.render(&Message::new(view_label(view)));
                                let picked = ui.add(
                                    egui::Button::selectable(view == current, name).min_size(CELL),
                                );
                                if picked.clicked() {
                                    chosen = Some(view);
                                }
                            }
                            Some(Cell::Soon(key)) => {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(self.text.render(&Message::new(key)))
                                        .min_size(CELL),
                                )
                                .on_disabled_hover_text(&soon);
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        let wide = egui::vec2(CELL.x * 1.5 + 2.0, CELL.y);
        ui.horizontal(|ui| {
            let name = self
                .text
                .render(&Message::new(view_label(StandardView::Perspective)));
            if ui
                .add(
                    egui::Button::selectable(current == StandardView::Perspective, name)
                        .min_size(wide),
                )
                .clicked()
            {
                chosen = Some(StandardView::Perspective);
            }
            ui.add_enabled(
                false,
                egui::Button::new(self.text.render(&Message::new("ui.view.two_point")))
                    .min_size(wide),
            )
            .on_disabled_hover_text(&soon);
        });
        // The lens, in millimetres and as the angle it sees across, as Rhino shows them.
        ui.horizontal(|ui| {
            ui.label(self.text.render(&Message::new("ui.view.lens")));
            ui.add_enabled(false, egui::Button::new("50 mm"))
                .on_disabled_hover_text(&soon);
            ui.label(self.text.render(&Message::new("ui.view.angle")));
            ui.add_enabled(false, egui::Button::new("39.6°"))
                .on_disabled_hover_text(&soon);
        });
        chosen
    }
}
