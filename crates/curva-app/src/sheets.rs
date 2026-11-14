//! Layouts in the window (spec `layouts.md` §1): tabs along the bottom of the viewport area for the
//! model and each layout, and a layout shown as its sheet of paper in one view from above, with
//! pan and zoom as in any viewport. The model's viewports are kept aside while a sheet is open
//! and come back as they were.

use crate::*;

/// The height of the tab row below the viewports, in points.
pub(crate) const TAB_ROW: f32 = 24.0;

/// The colour of a sheet, and of the lines drawn on it.
const PAPER: egui::Color32 = egui::Color32::from_rgb(250, 250, 247);
pub(crate) const PAPER_INK: [f32; 3] = [0.12, 0.12, 0.14];

/// A layout open in the viewport area.
pub(crate) struct OpenSheet {
    /// The layout shown.
    pub(crate) layout: u64,
    /// The model's viewports and which one was maximised, put back when the model is shown again.
    model: Option<([Pane; VIEW_SLOTS], Option<usize>)>,
    /// Whether the sheet has been framed in the part of its pane the panels leave free.
    framed: bool,
}

/// Which sheet, if any, the viewport area shows.
#[derive(Default)]
pub(crate) struct Sheets {
    pub(crate) open: Option<OpenSheet>,
}

/// A layout's name as its tab shows it: its own, or "Layout 2" and so on when it has none.
pub(crate) fn layout_title(
    text: &Translations,
    layout: &curva_document::layouts::Layout,
    index: usize,
) -> String {
    if layout.name.is_empty() {
        text.render(&Message::with(
            "ui.layout.unnamed",
            vec![(index + 1).to_string()],
        ))
    } else {
        layout.name.clone()
    }
}

impl Curva {
    /// Shows whatever the document is working on: the model's viewports, or a layout's sheet.
    pub(crate) fn follow_space(&mut self) {
        let wanted = self.doc.space();
        let showing = self.sheets.open.as_ref().map(|open| open.layout);
        if wanted == showing {
            return;
        }
        let model = match self.sheets.open.take() {
            Some(open) => open.model,
            None => Some((
                std::mem::replace(&mut self.panes, standard_panes()),
                self.maximised,
            )),
        };
        let sheet = wanted.and_then(|id| self.doc.layout(id).cloned());
        match sheet {
            None => {
                if let Some((panes, maximised)) = model {
                    self.panes = panes;
                    self.maximised = maximised;
                }
            }
            Some(layout) => {
                let mut pane = Pane::new(StandardView::Top);
                pane.grid = Arc::default();
                self.panes[PERSPECTIVE] = pane;
                self.maximised = Some(PERSPECTIVE);
                self.sheets.open = Some(OpenSheet {
                    layout: layout.id,
                    model,
                    framed: false,
                });
            }
        }
        self.drawn = None;
    }

    /// The first time a sheet is shown, frames it in `visible`, the part of the pane's `rect` the
    /// panels leave free, with a little room around it.
    pub(crate) fn frame_sheet(&mut self, pane: usize, rect: egui::Rect, visible: egui::Rect) {
        if !self.shows_sheet(pane) || visible.width() < 1.0 || visible.height() < 1.0 {
            return;
        }
        let Some(open) = self.sheets.open.as_mut().filter(|o| !o.framed) else {
            return;
        };
        let Some([w, h]) = self.doc.layout(open.layout).map(|l| l.size) else {
            return;
        };
        open.framed = true;
        // World units per point, so the sheet and a margin fit the free part.
        let margin = 1.08;
        let per_point =
            (w * margin / f64::from(visible.width())).max(h * margin / f64::from(visible.height()));
        let offset = rect.center() - visible.center();
        let centre = [
            w / 2.0 + f64::from(offset.x) * per_point,
            h / 2.0 - f64::from(offset.y) * per_point,
        ];
        let half_height = f64::from(rect.height()) * per_point / 2.0;
        self.panes[pane]
            .camera
            .show_exactly([centre[0], centre[1], 0.0], half_height);
        self.panes[pane].settled = self.panes[pane].camera;
    }

    /// Whether the pane shows a sheet rather than the model.
    pub(crate) fn shows_sheet(&self, pane: usize) -> bool {
        self.sheets.open.is_some() && pane == PERSPECTIVE
    }

    /// The background a pane is drawn on: paper for a sheet, else the theme's.
    pub(crate) fn pane_background(&self, pane: usize) -> (egui::Color32, egui::Color32) {
        if self.shows_sheet(pane) {
            (PAPER, PAPER)
        } else {
            (theme::current().window, theme::current().window_low)
        }
    }

    /// Around an open sheet, the desk it lies on, and its edge.
    pub(crate) fn paint_desk(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        let Some(layout) = self
            .sheets
            .open
            .as_ref()
            .filter(|_| self.shows_sheet(pane))
            .and_then(|open| self.doc.layout(open.layout))
        else {
            return;
        };
        let camera = &self.panes[pane].camera;
        let [w, h] = layout.size;
        let corner = |p: Point| {
            camera
                .project(p, size)
                .map(|[x, y]| rect.min + egui::vec2(x as f32, y as f32))
        };
        let (Some(a), Some(b)) = (corner([0.0, 0.0, 0.0]), corner([w, h, 0.0])) else {
            return;
        };
        let paper = egui::Rect::from_two_pos(a, b);
        let painter = ui.painter_at(rect);
        let desk = theme::current().window;
        for part in [
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, paper.min.y)),
            egui::Rect::from_min_max(egui::pos2(rect.min.x, paper.max.y), rect.max),
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x, paper.min.y),
                egui::pos2(paper.min.x, paper.max.y),
            ),
            egui::Rect::from_min_max(
                egui::pos2(paper.max.x, paper.min.y),
                egui::pos2(rect.max.x, paper.max.y),
            ),
        ] {
            if part.is_positive() {
                painter.rect_filled(part, 0.0, desk);
            }
        }
        painter.rect_stroke(
            paper,
            0.0,
            egui::Stroke::new(1.0, theme::current().panel_edge),
            egui::StrokeKind::Outside,
        );
    }

    /// The tabs below the viewports: the model, then each layout, then a button for a new one.
    pub(crate) fn sheet_tabs(&mut self, ui: &mut egui::Ui, row: egui::Rect) {
        let mut chosen = None;
        let mut new_layout = false;
        let current = self.doc.space();
        let titles: Vec<(u64, String)> = self
            .doc
            .layouts()
            .iter()
            .enumerate()
            .map(|(index, layout)| (layout.id, layout_title(&self.text, layout, index)))
            .collect();
        let model = self.text.render(&Message::new("ui.layout.model"));
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        if child.selectable_label(current.is_none(), model).clicked() {
            chosen = Some(None);
        }
        for (id, title) in titles {
            if child.selectable_label(current == Some(id), title).clicked() {
                chosen = Some(Some(id));
            }
        }
        if child
            .button("+")
            .on_hover_text(self.text.render(&Message::new("ui.layout.new")))
            .clicked()
        {
            new_layout = true;
        }
        if let Some(space) = chosen {
            if self.processor.is_running() {
                self.processor.cancel(&mut self.doc);
            }
            self.doc.set_space(space);
        }
        if new_layout {
            self.run_from_button("Layout");
        }
    }
}
