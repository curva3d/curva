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

/// The model as details draw it: its own outlines, meshes and lines, in paper ink, kept while it
/// does not change.
#[derive(Default)]
pub(crate) struct ModelForDetails {
    /// The state they were made from, compared quickly with `State::same_as`.
    made_from: Option<State>,
    /// For each set of layers hidden in details (most details hide none), the model drawn so.
    by_hidden: std::collections::HashMap<Vec<u64>, ModelScene>,
}

/// The model's lines and faces with some layers hidden, as details draw it.
#[derive(Default)]
pub(crate) struct ModelScene {
    meshes: MeshCache,
    scene: Arc<Vec<curva_viewport::Vertex>>,
    shaded_scene: Arc<Vec<curva_viewport::Vertex>>,
    faces: Arc<Vec<curva_viewport::MeshVertex>>,
    made: bool,
}

/// Which sheet, if any, the viewport area shows.
#[derive(Default)]
pub(crate) struct Sheets {
    pub(crate) open: Option<OpenSheet>,
    /// The model drawn in details.
    pub(crate) model: ModelForDetails,
    /// The detail entered by double-clicking it, by its frame: pan and zoom act on its view.
    pub(crate) entered: Option<u64>,
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

    /// Draws each detail on the open sheet: the model, seen through the detail's own camera, in
    /// the detail's frame (spec `layouts.md` §2). A scaled detail shows exactly as much model as
    /// its scale and frame allow.
    pub(crate) fn paint_details(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        size: [f64; 2],
    ) {
        let Some(layout) = self
            .sheets
            .open
            .as_ref()
            .filter(|_| self.shows_sheet(pane))
            .and_then(|open| self.doc.layout(open.layout).cloned())
        else {
            return;
        };
        let details = self.doc.details_on(layout.id);
        if details.is_empty() {
            return;
        }
        self.refresh_model_for_details();
        let sheet_camera = self.panes[pane].camera;
        let model_mm = self.doc.state().unit.millimetres();
        let paper_mm = layout.unit.millimetres();
        for (slot, (detail, [low, high])) in details
            .iter()
            .take(curva_viewport::DETAIL_SLOTS)
            .enumerate()
        {
            let corner = |x: f64, y: f64| {
                sheet_camera
                    .project([x, y, 0.0], size)
                    .map(|[sx, sy]| rect.min + egui::vec2(sx as f32, sy as f32))
            };
            let (Some(a), Some(b)) = (corner(low[0], low[1]), corner(high[0], high[1])) else {
                continue;
            };
            let shown = egui::Rect::from_two_pos(a, b);
            let on_screen = shown.intersect(rect);
            if on_screen.width() < 2.0 || on_screen.height() < 2.0 {
                continue;
            }
            let view = curva_viewport::StandardView::from_key(&detail.projection)
                .unwrap_or(StandardView::Top);
            let mut camera = view.camera();
            match detail.scale {
                Some(scale) => {
                    let model_height = (high[1] - low[1]) * paper_mm / scale / model_mm;
                    camera.show_exactly(detail.target, model_height / 2.0);
                }
                None => camera.show_exactly(detail.target, detail.distance / 2.0),
            }
            let shaded = detail.display != "Wireframe";
            let ghosted = detail.display == "Ghosted";
            let model = self.model_scene(&detail.hidden_layers);
            let scene = Arc::clone(if shaded && !ghosted {
                &model.shaded_scene
            } else {
                &model.scene
            });
            let model_faces = Arc::clone(&model.faces);
            let (clips, faces) = self.clipped_view(
                curva_document::clipping::ClipView::Detail(detail.frame),
                &model_faces,
            );
            let drawing = curva_viewport::Drawing {
                clips,
                slot: VIEW_SLOTS + slot,
                grid: Arc::default(),
                scene,
                faces,
                overlay: Arc::default(),
                preview_faces: Arc::default(),
                matrix: camera.matrix(f64::from(shown.width() / shown.height())),
                light: camera.forward().map(|value| value as f32),
                shaded,
                ghosted,
                zebra: false,
                draft: false,
                area: shown,
                frame: ui.ctx().cumulative_frame_nr(),
                background: PAPER,
                background_low: PAPER,
            };
            ui.painter()
                .with_clip_rect(on_screen)
                .add(egui_wgpu_callback(shown, drawing));
            if self.sheets.entered == Some(detail.frame) {
                ui.painter().with_clip_rect(rect).rect_stroke(
                    shown,
                    0.0,
                    egui::Stroke::new(2.0, theme::ACCENT),
                    egui::StrokeKind::Outside,
                );
            }
        }
    }

    /// Makes the model's lines and faces for details again when the model has changed.
    fn refresh_model_for_details(&mut self) {
        let model = &mut self.sheets.model;
        if !model
            .made_from
            .as_ref()
            .is_some_and(|state| state.same_as(self.doc.state()))
        {
            for scene in model.by_hidden.values_mut() {
                scene.made = false;
            }
            model.made_from = Some(self.doc.state().clone());
        }
    }

    /// The model drawn with `hidden` layers out of sight, made when first wanted after a change.
    fn model_scene(&mut self, hidden: &[u64]) -> &ModelScene {
        let mut key = hidden.to_vec();
        key.sort_unstable();
        let state = self.doc.state();
        let scene = self.sheets.model.by_hidden.entry(key.clone()).or_default();
        if !scene.made {
            // The same model with those layers switched off; objects are shared, not copied.
            let mut seen = state.clone();
            for layer in &mut seen.layers {
                if key.contains(&layer.id) {
                    layer.visible = false;
                }
            }
            let outlines = Outlines::of_space(&seen, None, &mut scene.meshes).inked(PAPER_INK);
            scene.scene = Arc::new(object_lines(&outlines, &[]));
            scene.shaded_scene = Arc::new(shaded_object_lines(&outlines, &[]));
            scene.faces = Arc::new(face_vertices(&outlines, &[]));
            scene.made = true;
        }
        scene
    }

    /// The detail under a point of the sheet pane, by its frame, with its corners on the sheet
    /// and on screen.
    fn detail_at(
        &self,
        pane: usize,
        rect: egui::Rect,
        size: [f64; 2],
        at: egui::Pos2,
    ) -> Option<(u64, egui::Rect)> {
        let layout = self.sheets.open.as_ref()?.layout;
        let camera = &self.panes[pane].camera;
        self.doc
            .details_on(layout)
            .into_iter()
            .rev()
            .find_map(|(detail, [low, high])| {
                let corner = |x: f64, y: f64| {
                    camera
                        .project([x, y, 0.0], size)
                        .map(|[sx, sy]| rect.min + egui::vec2(sx as f32, sy as f32))
                };
                let shown =
                    egui::Rect::from_two_pos(corner(low[0], low[1])?, corner(high[0], high[1])?);
                shown.contains(at).then_some((detail.frame, shown))
            })
    }

    /// On a sheet: double-clicking a detail enters it and double-clicking outside leaves it;
    /// inside, panning moves its view and scrolling changes its scale, unless it is locked.
    /// Returns whether the mouse was used.
    pub(crate) fn detail_mouse(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        response: &egui::Response,
        size: [f64; 2],
    ) -> bool {
        if !self.shows_sheet(pane) {
            self.sheets.entered = None;
            return false;
        }
        let rect = response.rect;
        if response.double_clicked()
            && !self.processor.is_running()
            && let Some(at) = response.interact_pointer_pos()
        {
            self.sheets.entered = self.detail_at(pane, rect, size, at).map(|(frame, _)| frame);
            return true;
        }
        let Some(frame) = self.sheets.entered else {
            return false;
        };
        let Some((_, detail)) = self.doc.detail_of(frame).map(|(l, d)| (l, d.clone())) else {
            self.sheets.entered = None;
            return false;
        };
        let Some((_, shown)) = self
            .doc
            .details_on(self.doc.space().unwrap_or_default())
            .into_iter()
            .find(|(d, _)| d.frame == frame)
            .and_then(|(_, [low, high])| {
                let camera = &self.panes[pane].camera;
                let corner = |x: f64, y: f64| {
                    camera
                        .project([x, y, 0.0], size)
                        .map(|[sx, sy]| rect.min + egui::vec2(sx as f32, sy as f32))
                };
                Some((
                    frame,
                    egui::Rect::from_two_pos(corner(low[0], low[1])?, corner(high[0], high[1])?),
                ))
            })
        else {
            return false;
        };
        let (delta, middle, right, shift, scroll) = ui.input(|input| {
            (
                input.pointer.delta(),
                input.pointer.middle_down(),
                input.pointer.secondary_down(),
                input.modifiers.shift,
                input.smooth_scroll_delta.y,
            )
        });
        let pans = response.is_pointer_button_down_on() && (middle || right || shift);
        let zooms = response.hovered() && scroll != 0.0;
        if !(pans || zooms) {
            return false;
        }
        if detail.locked {
            return true;
        }
        // How much model one point of the frame on screen covers.
        let model_mm = self.doc.state().unit.millimetres();
        let paper_mm = self
            .doc
            .space()
            .and_then(|id| self.doc.layout(id))
            .map_or(1.0, |l| l.unit.millimetres());
        let frame_paper_height = self
            .doc
            .details_on(self.doc.space().unwrap_or_default())
            .into_iter()
            .find(|(d, _)| d.frame == frame)
            .map_or(1.0, |(_, [low, high])| high[1] - low[1]);
        let model_height = detail.scale.map_or(detail.distance, |scale| {
            frame_paper_height * paper_mm / scale / model_mm
        });
        let per_point = model_height / f64::from(shown.height().max(1.0));
        let (across, up) = match detail.projection.as_str() {
            "Front" => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            "Right" => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            _ => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        };
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        self.doc.change_detail(frame, |d| {
            if pans {
                for k in 0..3 {
                    d.target[k] -= (across[k] * dx - up[k] * dy) * per_point;
                }
            }
            if zooms {
                let factor = (f64::from(scroll) * 0.002).exp();
                match d.scale.as_mut() {
                    Some(scale) => *scale = (*scale * factor).clamp(1e-6, 1e3),
                    None => d.distance = (d.distance / factor).clamp(1e-3, 1e9),
                }
            }
        });
        true
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
