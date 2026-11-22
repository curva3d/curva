//! The viewports: their panes and views, drawing the model in them, and the Display menu.

use crate::*;

/// What the viewports draw, kept between frames.
#[derive(Default)]
pub(crate) struct ViewportCache {
    /// The objects' lines for shaded views, where edges on faces are dark.
    pub(crate) shaded_scene: Arc<Vec<curva_viewport::Vertex>>,
    /// The faces of surfaces and solids not drawn from shared shapes (annotation's fills), for
    /// shaded views.
    pub(crate) faces: Arc<Vec<MeshVertex>>,
    /// Surfaces and solids, each shape held once and drawn at every copy's placement.
    pub(crate) shapes: Arc<Vec<curva_viewport::PlacedShape>>,
    /// Whole circles, drawn by the graphics card from their centre and axes, out of the lines.
    pub(crate) circles: Arc<Vec<curva_viewport::PlacedCircle>>,
    /// What the scene's positions are measured from ([`curva_viewport::SceneParts::origin`]).
    pub(crate) origin: Point,
    /// Meshes of solids, kept until they change.
    pub(crate) meshes: MeshCache,
    /// The faces and shapes with those a preview replaces left out, and which those are.
    pub(crate) faces_without: (
        Vec<u64>,
        Arc<Vec<MeshVertex>>,
        Arc<Vec<curva_viewport::PlacedShape>>,
    ),
    /// Which object each number in the scene's lines and faces stands for
    /// ([`curva_viewport::SceneParts`]).
    pub(crate) objects: Arc<Vec<u64>>,
    /// The selection as the viewport draws it: the graphics card colours the selected objects,
    /// so a new selection is all a selection change sends.
    pub(crate) selection: Arc<Vec<u64>>,
}

/// One viewport: what it looks at and what it is tracking.
pub(crate) struct Pane {
    pub(crate) view: StandardView,
    pub(crate) plane: CPlane,
    pub(crate) camera: Camera,
    pub(crate) grid: Arc<Vec<curva_viewport::Vertex>>,
    /// What the grid's positions are measured from: the scene's origin when it was made (#225).
    pub(crate) grid_origin: Point,
    /// How the grid is drawn: its colours for the display mode, and its spacing for the unit.
    pub(crate) grid_look: curva_viewport::GridLook,
    pub(crate) gestures: LeftGestures,
    /// The saved view it shows, whose name becomes its title.
    pub(crate) named: Option<String>,
    /// How it draws surfaces and solids: by their edges, filled in and lit, or see-through.
    pub(crate) display: DisplayMode,
    /// Views to step back to with `UndoView`, newest last, and the view they were left from.
    pub(crate) views_back: Vec<Camera>,
    /// Views stepped back from, for `RedoView`, newest last.
    pub(crate) views_ahead: Vec<Camera>,
    /// The view as it last stood still, which a camera move leaves behind.
    pub(crate) settled: Camera,
}

impl Pane {
    pub(crate) fn set_plane(&mut self, plane: CPlane) {
        self.plane = plane;
        self.grid = Arc::new(grid_lines_in(&plane, &self.grid_look, self.grid_origin));
    }

    /// Draws the grid in `look` from now on, made afresh if it changed.
    pub(crate) fn restyle_grid(&mut self, look: curva_viewport::GridLook) {
        if look != self.grid_look {
            self.grid_look = look;
            self.set_plane(self.plane);
        }
    }

    /// Keeps the view history: once the camera has moved and stands still again, the view it
    /// left goes on the list `UndoView` steps back through.
    pub(crate) fn track_view(&mut self, moving: bool) {
        if moving || self.camera == self.settled {
            return;
        }
        self.views_back.push(self.settled);
        if self.views_back.len() > MOST_VIEWS_KEPT {
            self.views_back.remove(0);
        }
        self.views_ahead.clear();
        self.settled = self.camera;
    }

    /// Steps back, or with `forward` forward, through the view history.
    pub(crate) fn step_view(&mut self, forward: bool) {
        let (from, to) = if forward {
            (&mut self.views_ahead, &mut self.views_back)
        } else {
            (&mut self.views_back, &mut self.views_ahead)
        };
        if let Some(view) = from.pop() {
            to.push(self.camera);
            self.camera = view;
            self.settled = view;
        }
    }

    pub(crate) fn new(view: StandardView) -> Self {
        let look = curva_viewport::GridLook::new(theme::current().viewport);
        Self {
            view,
            views_back: Vec::new(),
            views_ahead: Vec::new(),
            settled: view.camera(),
            camera: view.camera(),
            plane: view.plane(),
            grid: Arc::new(grid_lines_in(&view.plane(), &look, [0.0; 3])),
            grid_origin: [0.0; 3],
            grid_look: look,
            gestures: LeftGestures::default(),
            named: None,
            // As in Rhino, the perspective view starts shaded and the flat views in wireframe.
            display: if view == StandardView::Perspective {
                DisplayMode::Shaded
            } else {
                DisplayMode::Wireframe
            },
        }
    }
}

/// The four viewports, in the order of [`pane_rects`].
pub(crate) fn standard_panes() -> [Pane; VIEW_SLOTS] {
    [
        Pane::new(StandardView::Top),
        Pane::new(StandardView::Perspective),
        Pane::new(StandardView::Front),
        Pane::new(StandardView::Right),
    ]
}

pub(crate) fn standard_view(name: ViewName) -> StandardView {
    match name {
        ViewName::Top => StandardView::Top,
        ViewName::Front => StandardView::Front,
        ViewName::Right => StandardView::Right,
        ViewName::Perspective => StandardView::Perspective,
    }
}

pub(crate) fn view_label(view: StandardView) -> &'static str {
    match view {
        StandardView::Top => "ui.view.top",
        StandardView::Front => "ui.view.front",
        StandardView::Right => "ui.view.right",
        StandardView::Perspective => "ui.view.perspective",
        StandardView::Bottom => "ui.view.bottom",
        StandardView::Left => "ui.view.left",
        StandardView::Back => "ui.view.back",
        StandardView::TwoPoint => "ui.view.two_point",
    }
}

/// Where each viewport goes: Top and Perspective above Front and Right, as in Rhino, or just the
/// maximised one filling the whole area.
pub(crate) fn pane_rects(area: egui::Rect, maximised: Option<usize>) -> Vec<(usize, egui::Rect)> {
    if let Some(pane) = maximised {
        return vec![(pane, area)];
    }
    let half = PANE_GAP / 2.0;
    let centre = area.center();
    vec![
        (
            0,
            egui::Rect::from_min_max(area.min, centre - egui::Vec2::splat(half)),
        ),
        (
            1,
            egui::Rect::from_min_max(
                egui::pos2(centre.x + half, area.top()),
                egui::pos2(area.right(), centre.y - half),
            ),
        ),
        (
            2,
            egui::Rect::from_min_max(
                egui::pos2(area.left(), centre.y + half),
                egui::pos2(centre.x - half, area.bottom()),
            ),
        ),
        (
            3,
            egui::Rect::from_min_max(centre + egui::Vec2::splat(half), area.max),
        ),
    ]
}

/// A viewport stretched from `area` out to `reach` on each side where it touches the edge of
/// `area`, so it carries on under a glass panel there.
pub(crate) fn reaching(rect: egui::Rect, area: egui::Rect, reach: egui::Rect) -> egui::Rect {
    let touches = |a: f32, b: f32| (a - b).abs() < 1.0;
    let mut stretched = rect;
    if touches(rect.min.x, area.min.x) {
        stretched.min.x = reach.min.x;
    }
    if touches(rect.max.x, area.max.x) {
        stretched.max.x = reach.max.x;
    }
    if touches(rect.min.y, area.min.y) {
        stretched.min.y = reach.min.y;
    }
    if touches(rect.max.y, area.max.y) {
        stretched.max.y = reach.max.y;
    }
    stretched
}

/// A viewport's size for the camera, never zero even while the window is being squashed.
pub(crate) fn pane_size(rect: egui::Rect) -> [f64; 2] {
    [
        f64::from(rect.width()).max(1.0),
        f64::from(rect.height()).max(1.0),
    ]
}

impl Curva {
    /// Rebuilds the drawing only when the model or selection changed, as sampling is the slow part.
    pub(crate) fn refresh_scene(&mut self) {
        let selection = self.doc.selection();
        let (model_same, selection_same) = self
            .drawn
            .as_ref()
            .map_or((false, false), |(state, chosen)| {
                (state.same_as(self.doc.state()), chosen == &selection)
            });
        // A changed model is drawn again, reusing what unchanged objects drew as; a changed
        // selection only goes to the graphics card's selection table (spec
        // `display-pipeline.md` step 4).
        if !model_same {
            // Text fields read the file and the time as well as the model.
            self.drawing.meshes.fields = curva_document::fields::FieldContext::now(self.doc.path());
            self.outlines =
                Outlines::of_space(self.doc.state(), self.doc.space(), &mut self.drawing.meshes)
                    .inked(if self.sheets.open.is_some() {
                        PAPER_INK
                    } else {
                        theme::current().viewport.ink
                    });
            // The trees snapping searches, made away from the frame (#130).
            self.outlines.prepare_snapping();
        }
        if !model_same {
            let parts = curva_viewport::SceneParts::shared(&self.outlines);
            self.scene = Arc::new(parts.lines);
            self.drawing.shaded_scene = Arc::new(parts.shaded_lines);
            self.drawing.faces = Arc::new(parts.faces);
            self.drawing.shapes = Arc::new(parts.shapes);
            self.drawing.circles = Arc::new(parts.circles);
            if parts.origin != self.drawing.origin {
                // Previews measured from the old origin are made again.
                self.ghosted.0.clear();
            }
            self.drawing.origin = parts.origin;
            self.drawing.objects = Arc::new(parts.objects);
            self.drawing.faces_without.0.clear();
        }
        if !model_same || !selection_same {
            self.drawing.selection = Arc::new(selection.clone());
            self.drawn = Some((self.doc.state().clone(), selection));
        }
        // The filter can change without the model changing, so it is passed on every frame.
        self.outlines.set_filter(self.pick_filter);
    }

    /// The surfaces and solids the running command would make, as ghosted triangles. The meshes
    /// are kept while the previewed bodies stay the same, so a still preview costs nothing.
    pub(crate) fn ghosted_preview(&mut self, cursor: Option<Point>) -> Arc<Vec<MeshVertex>> {
        let bodies = self
            .processor
            .preview_bodies(&self.doc, cursor, &self.typed);
        if bodies != self.ghosted.0 {
            let meshes: Vec<curva_kernel::BodyMesh> =
                bodies.iter().filter_map(|body| body.mesh().ok()).collect();
            self.ghosted = (bodies, Arc::new(ghost_faces(&meshes, self.drawing.origin)));
        }
        Arc::clone(&self.ghosted.1)
    }

    /// Tells the document which plane flat objects go on: the last-used viewport's.
    pub(crate) fn share_work_plane(&mut self) {
        let plane = self.panes[self.active].plane;
        self.doc.set_work_plane([plane.x, plane.y]);
        self.doc.set_work_origin(plane.origin);
        let look = self.panes[self.active].camera.look();
        self.doc.set_view(look);
    }

    /// Carries out the changes of view commands asked for, oldest first.
    /// Saves the last-used viewport's camera under `name`.
    fn save_view_as(&mut self, name: String) {
        let pane = &self.panes[self.active];
        let view = pane.camera.saved(name.clone(), pane.view);
        self.doc.save_view(view);
        let said = self
            .text
            .render(&Message::with("done.namedview.save", vec![name]));
        self.processor.note(said);
    }

    /// Each viewport takes the view the file just opened was saved with for it, as Rhino shows a
    /// file as it was left; the others stay fitted to the model (#479).
    fn show_opened_views(&mut self) {
        let opened = self.doc.take_opened_views();
        let keys = self.panes.each_ref().map(|pane| pane.view.key());
        let found = curva_document::views_for_viewports(&opened, &keys);
        for (pane, saved) in self.panes.iter_mut().zip(found) {
            if let Some(saved) = saved {
                pane.camera = Camera::from_saved(saved);
            }
        }
    }

    pub(crate) fn apply_view_request(&mut self) {
        while let Some(request) = self.processor.take_view_request() {
            self.apply_view(request);
        }
    }

    /// Saves the active viewport's plane under `name`.
    fn save_cplane_as(&mut self, name: String) {
        let plane = self.panes[self.active].plane;
        self.doc.save_cplane(curva_document::SavedCPlane {
            name: name.clone(),
            origin: plane.origin,
            x: plane.x,
            y: plane.y,
        });
        let said = self
            .text
            .render(&Message::with("done.namedcplane.save", vec![name]));
        self.processor.note(said);
    }

    /// Shows the view saved as `name` in the active viewport, which becomes the saved view's
    /// kind (top, perspective and so on) if it was another.
    fn restore_view(&mut self, name: &str) {
        if let Some(saved) = self.doc.saved_view(name).cloned() {
            let pane = &mut self.panes[self.active];
            if let Some(kind) = StandardView::from_key(&saved.kind)
                && kind != pane.view
            {
                *pane = Pane::new(kind);
            }
            pane.camera = Camera::from_saved(&saved);
            pane.named = Some(saved.name);
        }
    }

    /// Carries out one change of view.
    pub(crate) fn apply_view(&mut self, request: ViewRequest) {
        match request {
            ViewRequest::UndoView => self.panes[self.active].step_view(false),
            ViewRequest::RedoView => self.panes[self.active].step_view(true),
            ViewRequest::ZoomBy(factor) => {
                self.panes[self.active].camera.zoom(1.0 / factor);
            }
            ViewRequest::ZoomWindow(a, b) => {
                let low = std::array::from_fn(|axis| a[axis].min(b[axis]));
                let high = std::array::from_fn(|axis| a[axis].max(b[axis]));
                self.panes[self.active].camera.frame(low, high);
            }
            ViewRequest::ZoomTarget(at) => {
                self.panes[self.active].camera.target = at;
            }
            ViewRequest::SaveView(name) => self.save_view_as(name),
            ViewRequest::SaveCPlane(name) => self.save_cplane_as(name),
            ViewRequest::RestoreView(name) => self.restore_view(&name),
            ViewRequest::ZoomExtents => {
                self.refresh_scene();
                if let Some((low, high)) = self.outlines.bounds() {
                    self.frame_all(low, high);
                }
            }
            ViewRequest::OpenedViews => self.show_opened_views(),
            ViewRequest::ZoomSelected => {
                self.refresh_scene();
                if let Some((low, high)) = self.outlines.bounds_of(&self.doc.selection()) {
                    self.frame_all(low, high);
                }
            }
            ViewRequest::FourViews => {
                self.panes = standard_panes();
                self.maximised = None;
            }
            ViewRequest::SetCPlane { origin, x, y } => {
                let normal = curva_kernel::cross(x, y);
                let plane = CPlane {
                    origin,
                    x,
                    y,
                    normal,
                };
                self.panes[self.active].set_plane(plane);
            }
            ViewRequest::ResetCPlane => {
                let pane = &mut self.panes[self.active];
                pane.set_plane(pane.view.plane());
            }
            ViewRequest::Show(name) => {
                let display = self.panes[self.active].display;
                self.panes[self.active] = Pane::new(standard_view(name));
                self.panes[self.active].display = display;
            }
            ViewRequest::ShowBlocks => self.open_piece(layout::Piece::Blocks),
            ViewRequest::ShowLayouts => self.open_piece(layout::Piece::Layouts),
            ViewRequest::ShowAnnotationStyles => self.open_piece(layout::Piece::AnnotationStyles),
            ViewRequest::Print(job) => self.print(&job),
            ViewRequest::ShowPrint => self.open_piece(layout::Piece::Print),
            ViewRequest::ShowNotes => self.open_piece(layout::Piece::Notes),
            ViewRequest::ClipIn(frames, on) => self.clip_in(&frames, on),
            // Zebra and Draft angle are set aside while the display moves to Bevy (the owner's
            // decision B, `docs/specs/display-on-bevy.md` §5); they come back as their own work.
            ViewRequest::DraftAnalysis | ViewRequest::Zebra => self
                .processor
                .note(self.text.render(&Message::new("ui.analysis.set-aside"))),
            ViewRequest::Plan => {
                let pane = &mut self.panes[self.active];
                let plane = pane.plane;
                pane.camera.plan(&plane);
            }
            ViewRequest::Display(mode) => self.panes[self.active].display = mode,
            ViewRequest::ShowSelectionFilter => self.filter_row = !self.filter_row,
            ViewRequest::ToggleSelectionFilter => self.pick_filter.on = !self.pick_filter.on,
        }
    }

    /// One viewport's title, and an outline if it is the one view commands act on.
    /// Frames a box in every viewport: where a viewport reaches under floating panels, in the
    /// part they leave free, so what is framed is not hidden behind them.
    pub(crate) fn frame_all(&mut self, low: Point, high: Point) {
        for pane in 0..VIEW_SLOTS {
            self.panes[pane].camera.frame(low, high);
            let Some((rect, visible)) = self.pane_areas[pane] else {
                continue;
            };
            // A perspective view's size on screen does not follow its distance exactly, so the
            // fit is repeated until it settles.
            for _ in 0..4 {
                self.fit_once(pane, low, high, rect, visible);
            }
        }
    }

    /// One step of [`Self::frame_all`]: zooms the box to fill `visible` and moves it there.
    fn fit_once(
        &mut self,
        pane: usize,
        low: Point,
        high: Point,
        rect: egui::Rect,
        visible: egui::Rect,
    ) {
        {
            let size = pane_size(rect);
            let camera = &self.panes[pane].camera;
            let corners: Vec<[f64; 2]> = (0..8)
                .filter_map(|i| {
                    let corner = [
                        if i & 1 == 0 { low[0] } else { high[0] },
                        if i & 2 == 0 { low[1] } else { high[1] },
                        if i & 4 == 0 { low[2] } else { high[2] },
                    ];
                    camera.project(corner, size)
                })
                .collect();
            if corners.len() < 8 || visible.width() < 1.0 || visible.height() < 1.0 {
                return;
            }
            let extent = |k: usize| {
                let values = corners.iter().map(|c| c[k]);
                values.clone().fold(f64::MIN, f64::max) - values.fold(f64::MAX, f64::min)
            };
            let fill = (extent(0) / f64::from(visible.width()))
                .max(extent(1) / f64::from(visible.height()))
                * 1.1;
            let camera = &mut self.panes[pane].camera;
            // Zooming multiplies the distance, so the box's size on screen divides by it.
            if fill > 0.0 {
                camera.zoom(fill);
            }
            // Then the box's middle on screen goes to the middle of the free part. Panning moves
            // what is shown the way the mouse would drag it.
            let after: Vec<[f64; 2]> = (0..8)
                .filter_map(|i| {
                    let corner = [
                        if i & 1 == 0 { low[0] } else { high[0] },
                        if i & 2 == 0 { low[1] } else { high[1] },
                        if i & 4 == 0 { low[2] } else { high[2] },
                    ];
                    camera.project(corner, size)
                })
                .collect();
            if after.len() == 8 {
                let middle = |k: usize| {
                    let values = after.iter().map(|c| c[k]);
                    (values.clone().fold(f64::MIN, f64::max) + values.fold(f64::MAX, f64::min))
                        / 2.0
                };
                let shift_x = f64::from(visible.center().x - rect.min.x) - middle(0);
                let shift_y = f64::from(visible.center().y - rect.min.y) - middle(1);
                camera.pan(shift_x, shift_y, size[1]);
            }
        }
    }

    pub(crate) fn paint_frame(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        if pane == self.active && self.maximised.is_none() {
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
    }

    /// The model's lines a view in `display` draws: dark edges suit solid faces (and Hidden
    /// draws every line in black ink, from the same lines, seams left out); ghosted and
    /// wireframe views keep the light wireframe edges.
    pub(crate) fn scene_for(&self, display: DisplayMode) -> Arc<Vec<curva_viewport::Vertex>> {
        Arc::clone(
            if matches!(
                display,
                DisplayMode::Shaded
                    | DisplayMode::Hidden
                    | DisplayMode::Technical
                    | DisplayMode::Massing
            ) {
                &self.drawing.shaded_scene
            } else {
                &self.scene
            },
        )
    }

    pub(crate) fn viewports(&mut self, ui: &mut egui::Ui, area: egui::Rect, reach: egui::Rect) {
        self.drop_stale_picks();
        let shown = self.lay_out_panes(ui, area, reach);
        self.refresh_scene();
        let hovered = shown
            .iter()
            .find_map(|(pane, rect, response)| response.hover_pos().map(|at| (*pane, *rect, at)));
        let aimed = self.aim_pointer(ui, hovered);
        let cursor = aimed.map(|(at, _)| at);
        self.paint_pointer_tags(ui, hovered, cursor);
        let mut previewed = self.processor.preview(&self.doc, cursor, &self.typed);
        previewed.extend(self.gumball_preview());
        let origin = self.drawing.origin;
        let overlay = Arc::new(ghost_lines(&previewed, origin));
        let preview_faces = self.ghosted_preview(cursor);
        let (faces, shapes) = self.faces_shown(cursor);
        let frame = PaneFrame {
            overlay,
            preview_faces,
            previewed,
            faces,
            shapes,
            hovered,
            aimed,
        };
        for (pane, rect, _) in &shown {
            let drawing = self.pane_drawing(ui, *pane, *rect, &frame);
            ui.painter().add(egui_wgpu_callback(*rect, drawing));
            self.paint_pane_marks(ui, *pane, *rect, &frame);
        }
    }

    /// Picked edges and faces stay picked, as Ctrl+Shift picks them for the gumball and the
    /// commands that follow (they were cleared every frame no command asked for them, so a
    /// Ctrl+Shift pick on a solid vanished at once). They go when the object they belong to
    /// changes, as its numbered edges and faces may then be other ones.
    fn drop_stale_picks(&mut self) {
        let picked = !self.doc.edge_selection().is_empty() || !self.doc.face_selection().is_empty();
        if picked && self.picked_parts_stale() {
            self.doc.select_edges(&[]);
            self.doc.select_faces(&[]);
            self.mouse.parts_on.clear();
        }
    }

    /// Places the viewports in `area`, each reaching to `reach` where it meets its edge, and
    /// handles the mouse in each: the panes shown, their rectangles and their responses.
    fn lay_out_panes(
        &mut self,
        ui: &mut egui::Ui,
        area: egui::Rect,
        reach: egui::Rect,
    ) -> Vec<(usize, egui::Rect, egui::Response)> {
        let mut shown = Vec::new();
        self.pane_areas = [None; VIEW_SLOTS];
        for (pane, visible) in pane_rects(area, self.maximised) {
            let rect = reaching(visible, area, reach);
            self.pane_areas[pane] = Some((rect, visible));
            self.frame_sheet(pane, rect, visible);
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            self.handle_mouse(ui, pane, &response, pane_size(rect));
            shown.push((pane, rect, response));
        }
        shown
    }

    /// Where the pointer over `hovered` aims, and what it snaps to: while the view is being
    /// turned or slid, the aim holds where it was, so a height or a preview does not jump with
    /// the view.
    fn aim_pointer(&mut self, ui: &egui::Ui, hovered: Hovered) -> Aimed {
        let navigating =
            ui.input(|input| input.pointer.secondary_down() || input.pointer.middle_down());
        let held = self.held_aim;
        let aimed = hovered.and_then(|(pane, rect, position)| {
            let local = position - rect.min;
            let screen = [f64::from(local.x), f64::from(local.y)];
            let size = pane_size(rect);
            if self.takes_position() {
                self.aim(ui, pane, screen, size)
            } else {
                let plane = self.panes[pane].plane;
                let camera = &self.panes[pane].camera;
                camera
                    .plane_point(screen, size, &plane)
                    .map(|at| (at, None))
            }
        });
        let aimed = if navigating { held } else { aimed };
        self.held_aim = aimed;
        self.last_aim = aimed.map(|(at, _)| at);
        self.rest_on(ui.ctx(), aimed.and_then(|(_, hit)| hit));
        aimed
    }

    /// Beside the pointer: picking a point shows a crosshair and what the step measures; picking
    /// objects, edges or faces shows a small box.
    fn paint_pointer_tags(&self, ui: &egui::Ui, hovered: Hovered, cursor: Option<Point>) {
        let Some((.., position)) = hovered else {
            return;
        };
        if self.takes_position() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            // A rectangle's far corner shows width × depth.
            let corner = cursor.and_then(|at| self.processor.corner_size(&self.doc, at));
            if let Some((wide, deep)) = corner {
                let text = format!(
                    "{} × {}",
                    self.doc.length_text(wide),
                    self.doc.length_text(deep)
                );
                paint_cursor_tag(ui, position, &text);
            }
            // A number being typed for a size shows in the tag, as it drives the command.
            let typed = curva_document::units::parse_length(&self.typed, self.doc.state().unit);
            if corner.is_none()
                && let Some(value) =
                    typed.or_else(|| cursor.and_then(|at| self.processor.measure(&self.doc, at)))
            {
                paint_cursor_tag(ui, position, &self.doc.length_text(value));
            }
        }
        let picking = self.processor.current_request().is_some_and(|request| {
            matches!(
                request.kind,
                Kind::Objects | Kind::Edges | Kind::Faces | Kind::MeshParts
            )
        });
        if picking {
            let corner = position + egui::vec2(10.0, 10.0);
            ui.painter().rect_stroke(
                egui::Rect::from_min_size(corner, egui::Vec2::splat(8.0)),
                1.0,
                egui::Stroke::new(1.5, theme::current().text),
                egui::StrokeKind::Middle,
            );
        }
    }

    /// The faces and shared shapes to draw: a solid the preview changes in place is left out, so
    /// the changed one shows in its place.
    fn faces_shown(
        &mut self,
        cursor: Option<Point>,
    ) -> (Arc<Vec<MeshVertex>>, Arc<Vec<curva_viewport::PlacedShape>>) {
        let replaced = self
            .processor
            .preview_replaces(&self.doc, cursor, &self.typed);
        if replaced.is_empty() {
            return (
                Arc::clone(&self.drawing.faces),
                Arc::clone(&self.drawing.shapes),
            );
        }
        if self.drawing.faces_without.0 != replaced {
            let origin = self.drawing.origin;
            let objects = &self.drawing.objects;
            let made =
                curva_viewport::scene_faces_without(&self.outlines, objects, &replaced, origin);
            let shapes =
                curva_viewport::scene_shapes_without(&self.drawing.shapes, objects, &replaced);
            self.drawing.faces_without = (replaced, Arc::new(made), Arc::new(shapes));
        }
        (
            Arc::clone(&self.drawing.faces_without.1),
            Arc::clone(&self.drawing.faces_without.2),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    fn area() -> egui::Rect {
        egui::Rect::from_min_size(pos2(10.0, 20.0), vec2(800.0, 600.0))
    }

    #[test]
    fn four_viewports_fill_the_area_without_overlapping() {
        let rects = pane_rects(area(), None);
        assert_eq!(rects.len(), VIEW_SLOTS);
        for (index, (_, one)) in rects.iter().enumerate() {
            assert!(area().contains_rect(*one));
            for (_, other) in &rects[index + 1..] {
                assert!(!one.intersects(*other));
            }
        }
    }

    #[test]
    fn viewports_are_arranged_as_in_rhino() {
        let panes = standard_panes();
        let rects = pane_rects(area(), None);
        let at = |view: StandardView| {
            let index = panes.iter().position(|pane| pane.view == view).unwrap();
            rects
                .iter()
                .find(|(pane, _)| *pane == index)
                .unwrap()
                .1
                .center()
        };
        let (top, perspective) = (at(StandardView::Top), at(StandardView::Perspective));
        let (front, right) = (at(StandardView::Front), at(StandardView::Right));
        assert!(top.x < perspective.x && top.y < front.y);
        assert!(front.x < right.x && perspective.y < right.y);
        assert_eq!(panes[PERSPECTIVE].view, StandardView::Perspective);
    }

    #[test]
    fn a_maximised_viewport_fills_the_area_alone() {
        assert_eq!(pane_rects(area(), Some(2)), [(2, area())]);
    }

    #[test]
    fn a_viewport_size_is_never_zero() {
        let squashed = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0));
        assert_eq!(pane_size(squashed), [1.0, 1.0]);
    }

    #[test]
    fn a_viewport_can_take_a_plane_of_its_own_and_lose_it_again() {
        let mut pane = Pane::new(StandardView::Top);
        let raised = CPlane {
            origin: [0.0, 0.0, 5.0],
            ..CPlane::WORLD_XY
        };
        pane.set_plane(raised);
        assert_eq!(pane.plane, raised);
        assert_eq!(
            pane.grid.len(),
            grid_lines_in(&raised, &pane.grid_look, [0.0; 3]).len()
        );
        assert!(pane.grid.iter().all(|v| v.position[2] == 5.0));
        pane.set_plane(pane.view.plane());
        assert_eq!(pane.plane, CPlane::WORLD_XY);
    }

    #[test]
    fn curva_opens_on_the_perspective_viewport_alone() {
        let rects = pane_rects(area(), STARTING_LAYOUT);
        assert_eq!(rects.len(), 1);
        assert_eq!(standard_panes()[rects[0].0].view, StandardView::Perspective);
        assert_eq!(rects[0].1, area());
    }
}
