//! The viewports: their panes and views, drawing the model in them, and the Display menu.

use crate::*;

/// What the viewports draw, kept between frames.
#[derive(Default)]
pub(crate) struct ViewportCache {
    /// The objects' lines for shaded views, where edges on faces are dark.
    pub(crate) shaded_scene: Arc<Vec<curva_viewport::Vertex>>,
    /// The faces of surfaces and solids, for shaded views.
    pub(crate) faces: Arc<Vec<MeshVertex>>,
    /// Meshes of solids, kept until they change.
    pub(crate) meshes: MeshCache,
    /// The faces with those a preview replaces left out, and which those are.
    pub(crate) faces_without: (Vec<u64>, Arc<Vec<MeshVertex>>),
}

/// One viewport: what it looks at and what it is tracking.
pub(crate) struct Pane {
    pub(crate) view: StandardView,
    pub(crate) plane: CPlane,
    pub(crate) camera: Camera,
    pub(crate) grid: Arc<Vec<curva_viewport::Vertex>>,
    pub(crate) gestures: LeftGestures,
    /// The saved view it shows, whose name becomes its title.
    pub(crate) named: Option<String>,
    /// How it draws surfaces and solids: by their edges, filled in and lit, or see-through.
    pub(crate) display: DisplayMode,
    /// Whether shaded faces show zebra stripes (`Zebra`).
    pub(crate) zebra: bool,
    /// Whether shaded faces show draft angles (`DraftAnalysis`).
    pub(crate) draft: bool,
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
        self.grid = Arc::new(grid_lines_in(&plane, &theme::current().viewport));
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
        Self {
            view,
            views_back: Vec::new(),
            views_ahead: Vec::new(),
            settled: view.camera(),
            camera: view.camera(),
            plane: view.plane(),
            grid: Arc::new(grid_lines_in(&view.plane(), &theme::current().viewport)),
            gestures: LeftGestures::default(),
            named: None,
            // As in Rhino, the perspective view starts shaded and the flat views in wireframe.
            display: if view == StandardView::Perspective {
                DisplayMode::Shaded
            } else {
                DisplayMode::Wireframe
            },
            zebra: false,
            draft: false,
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
        // selection only recolours.
        if !model_same {
            self.outlines =
                Outlines::of_space(self.doc.state(), self.doc.space(), &mut self.drawing.meshes)
                    .inked(if self.sheets.open.is_some() {
                        PAPER_INK
                    } else {
                        theme::current().viewport.ink
                    });
        }
        if !model_same || !selection_same {
            self.scene = Arc::new(object_lines(&self.outlines, &selection));
            self.drawing.shaded_scene = Arc::new(shaded_object_lines(&self.outlines, &selection));
            self.drawing.faces = Arc::new(face_vertices(&self.outlines, &selection));
            self.drawn = Some((self.doc.state().clone(), selection));
            self.drawing.faces_without.0.clear();
        }
        // The filter can change without the model changing, so it is passed on every frame.
        self.outlines.set_filter(self.pick_filter);
    }

    /// The undo keys, taken before the command line can see them: Ctrl+Z undoes and Ctrl+Y or
    /// Ctrl+Shift+Z redoes the last change to the model, even while the command line has focus,
    /// unless it holds typed text, which keeps its own undo. Alt+Z and Alt+Shift+Z step through
    /// the last-used viewport's views. Also keeps each viewport's view history.
    pub(crate) fn history_keys(&mut self, ctx: &egui::Context) {
        self.alt_held = ctx.input(|input| input.modifiers.alt);
        use egui::{Key, KeyboardShortcut, Modifiers};
        let moving = ctx.input(|input| {
            input.pointer.any_down() || input.smooth_scroll_delta != egui::Vec2::ZERO
        });
        for pane in &mut self.panes {
            pane.track_view(moving);
        }
        if !self.typed.is_empty() || self.naming_layer {
            return;
        }
        let shortcut = |modifiers, key| KeyboardShortcut::new(modifiers, key);
        let (redo, undo, view_ahead, view_back) = ctx.input_mut(|input| {
            // The fuller shortcuts are taken first, as a plain one also matches them.
            let view_ahead =
                input.consume_shortcut(&shortcut(Modifiers::ALT | Modifiers::SHIFT, Key::Z));
            let view_back = input.consume_shortcut(&shortcut(Modifiers::ALT, Key::Z));
            // Rhino's own keys for them too, Home and End.
            let view_back = view_back || input.consume_key(Modifiers::NONE, Key::Home);
            let view_ahead = view_ahead || input.consume_key(Modifiers::NONE, Key::End);
            let redo = input
                .consume_shortcut(&shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
                || input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Y));
            let undo = input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Z));
            (redo, undo, view_ahead, view_back)
        });
        // The File tab's keys, also taken before the command line sees them.
        let file_command = ctx.input_mut(|input| {
            if input.consume_shortcut(&shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)) {
                Some("SaveAs")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::S)) {
                Some("Save")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::O)) {
                Some("Open")
            } else if input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::N)) {
                Some("New")
            } else {
                None
            }
        });
        if let Some(name) = file_command {
            self.run_from_button(name);
        }
        // The clipboard keys every program has, on the selected objects. The window turns them
        // into egui's Copy, Cut and Paste events rather than key presses, so those are taken here,
        // before the command line could; a paste is the objects' only while the system clipboard
        // still holds the mark their copy put there.
        let anything_selected = !self.doc.selection().is_empty();
        let (copy, cut, paste, all) = ctx.input_mut(|input| {
            let (mut copy, mut cut, mut paste) = (
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::C)),
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::X)),
                input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::V)),
            );
            let (c, x, v) = clipboard_requests(&mut input.events, anything_selected);
            (copy, cut, paste) = (copy || c, cut || x, paste || v);
            let all = input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::A));
            (copy, cut, paste, all)
        });
        if copy || cut {
            self.clipboard = self
                .selected_shapes()
                .into_iter()
                .map(|(id, shape)| (shape, self.doc.layer_of(id)))
                .collect();
            if !self.clipboard.is_empty() {
                ctx.copy_text(CLIPBOARD_MARK.to_owned());
            }
        }
        if cut && !self.clipboard.is_empty() {
            self.run_from_button("delete");
        }
        if paste && !self.clipboard.is_empty() {
            // Pasted in place, as in Rhino, and selected.
            let shapes = self.clipboard.clone();
            self.undoable(|doc| {
                let added: Vec<u64> = shapes
                    .into_iter()
                    .map(|(shape, layer)| doc.add_shape_on(shape, layer))
                    .collect();
                doc.select(&added);
            });
        }
        if all {
            self.run_from_button("selall");
        }
        if view_back || view_ahead {
            self.panes[self.active].step_view(view_ahead);
        }
        if undo || redo {
            // A command in progress is ended first, as its changes are not yet a step.
            if self.processor.is_running() {
                self.processor.cancel(&mut self.doc);
            }
            self.processor
                .submit(&mut self.doc, if redo { "redo" } else { "undo" });
        }
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
            self.ghosted = (bodies, Arc::new(ghost_faces(&meshes)));
        }
        Arc::clone(&self.ghosted.1)
    }

    /// Tells the document which plane flat objects go on: the last-used viewport's.
    pub(crate) fn share_work_plane(&mut self) {
        let plane = self.panes[self.active].plane;
        self.doc.set_work_plane([plane.x, plane.y]);
        self.doc.set_work_origin(plane.origin);
    }

    /// Carries out the changes of view commands asked for, oldest first.
    pub(crate) fn apply_view_request(&mut self) {
        while let Some(request) = self.processor.take_view_request() {
            self.apply_view(request);
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
            ViewRequest::SaveView(name) => {
                let pane = &self.panes[self.active];
                let view = pane.camera.saved(name.clone(), pane.view);
                self.doc.save_view(view);
                let said = self
                    .text
                    .render(&Message::with("done.namedview.save", vec![name]));
                self.processor.note(said);
            }
            ViewRequest::SaveCPlane(name) => {
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
            ViewRequest::RestoreView(name) => {
                if let Some(saved) = self.doc.saved_view(&name).cloned() {
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
            ViewRequest::ZoomExtents => {
                self.refresh_scene();
                if let Some((low, high)) = self.outlines.bounds() {
                    for pane in &mut self.panes {
                        pane.camera.frame(low, high);
                    }
                }
            }
            ViewRequest::ZoomSelected => {
                self.refresh_scene();
                if let Some((low, high)) = self.outlines.bounds_of(&self.doc.selection()) {
                    for pane in &mut self.panes {
                        pane.camera.frame(low, high);
                    }
                }
            }
            ViewRequest::FourViews => {
                self.panes = standard_panes();
                self.maximised = None;
            }
            ViewRequest::SetCPlane { origin, x, y } => {
                let normal = curva_kernel::cross(x, y);
                self.panes[self.active].set_plane(CPlane {
                    origin,
                    x,
                    y,
                    normal,
                });
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
            ViewRequest::DraftAnalysis => {
                let pane = &mut self.panes[self.active];
                pane.draft = !pane.draft;
                if pane.display == DisplayMode::Wireframe {
                    pane.display = DisplayMode::Shaded;
                }
            }
            ViewRequest::Zebra => {
                let pane = &mut self.panes[self.active];
                pane.zebra = !pane.zebra;
                if pane.display == DisplayMode::Wireframe {
                    pane.display = DisplayMode::Shaded;
                }
            }
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
    pub(crate) fn paint_frame(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        painter.text(
            rect.left_top() + egui::vec2(6.0, 3.0),
            egui::Align2::LEFT_TOP,
            self.panes[pane].named.clone().unwrap_or_else(|| {
                self.text
                    .render(&Message::new(view_label(self.panes[pane].view)))
            }),
            egui::TextStyle::Body.resolve(ui.style()),
            theme::current().quiet,
        );
        if pane == self.active && self.maximised.is_none() {
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
    }

    /// Clicking a viewport's title opens a menu of display modes, as in Rhino.
    pub(crate) fn display_menu(&mut self, ui: &egui::Ui, pane: usize, rect: egui::Rect) {
        let title = egui::Rect::from_min_size(rect.min, TITLE_SIZE);
        let response = ui.interact(
            title,
            ui.scope_id().with(("display menu", pane)),
            egui::Sense::click(),
        );
        let mut chosen = None;
        egui::Popup::menu(&response).show(|ui| {
            for (mode, key) in [
                (DisplayMode::Wireframe, "ui.display.wireframe"),
                (DisplayMode::Shaded, "ui.display.shaded"),
                (DisplayMode::Ghosted, "ui.display.ghosted"),
            ] {
                let current = self.panes[pane].display == mode;
                if ui
                    .selectable_label(current, self.text.render(&Message::new(key)))
                    .clicked()
                {
                    chosen = Some(mode);
                }
            }
        });
        if let Some(mode) = chosen {
            self.panes[pane].display = mode;
            self.active = pane;
        }
    }

    pub(crate) fn viewports(&mut self, ui: &mut egui::Ui, area: egui::Rect, reach: egui::Rect) {
        // Picked edges and faces only mean something while a command asks for them.
        let wants = |kind: Kind| {
            self.processor
                .current_request()
                .is_some_and(|request| request.kind == kind)
        };
        if !wants(Kind::Edges) && !self.doc.edge_selection().is_empty() {
            self.doc.select_edges(&[]);
        }
        if !wants(Kind::Faces) && !self.doc.face_selection().is_empty() {
            self.doc.select_faces(&[]);
        }
        // The tabs for the model and the layouts run along the bottom.
        let row = egui::Rect::from_min_max(egui::pos2(area.min.x, area.max.y - TAB_ROW), area.max);
        let area = egui::Rect::from_min_max(area.min, egui::pos2(area.max.x, row.min.y));
        let reach = egui::Rect::from_min_max(
            reach.min,
            egui::pos2(reach.max.x, reach.max.y.min(row.min.y)),
        );
        self.sheet_tabs(ui, row);
        let mut shown = Vec::new();
        for (pane, visible) in pane_rects(area, self.maximised) {
            let rect = reaching(visible, area, reach);
            self.frame_sheet(pane, rect, visible);
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            self.display_menu(ui, pane, rect);
            self.handle_mouse(ui, pane, &response, pane_size(rect));
            shown.push((pane, rect, response));
        }
        self.refresh_scene();
        let hovered = shown
            .iter()
            .find_map(|(pane, rect, response)| response.hover_pos().map(|at| (*pane, *rect, at)));
        // While the view is being turned or slid, the aim holds where it was, so a height or a
        // preview does not jump with the view.
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
        let cursor = aimed.map(|(at, _)| at);
        self.last_aim = cursor;
        self.rest_on(ui.ctx(), aimed.and_then(|(_, hit)| hit));
        // Picking a point shows a crosshair, and what the step measures beside it.
        if let Some((.., position)) = hovered
            && self.takes_position()
        {
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
        // Picking objects, edges or faces shows the pointer with a small box beside it.
        let picking = self.processor.current_request().is_some_and(|request| {
            matches!(request.kind, Kind::Objects | Kind::Edges | Kind::Faces)
        });
        if let Some((.., position)) = hovered
            && picking
        {
            let corner = position + egui::vec2(10.0, 10.0);
            ui.painter().rect_stroke(
                egui::Rect::from_min_size(corner, egui::Vec2::splat(8.0)),
                1.0,
                egui::Stroke::new(1.5, theme::current().text),
                egui::StrokeKind::Middle,
            );
        }
        let mut previewed = self.processor.preview(&self.doc, cursor, &self.typed);
        previewed.extend(self.gumball_preview());
        let preview = ghost_lines(&previewed);
        let overlay = Arc::new(preview);
        let preview_faces = self.ghosted_preview(cursor);
        // A solid the preview changes in place is hidden, so the changed one shows in its place.
        let replaced = self
            .processor
            .preview_replaces(&self.doc, cursor, &self.typed);
        let faces = if replaced.is_empty() {
            Arc::clone(&self.drawing.faces)
        } else {
            if self.drawing.faces_without.0 != replaced {
                let selection = self.doc.selection();
                let made =
                    curva_viewport::face_vertices_without(&self.outlines, &selection, &replaced);
                self.drawing.faces_without = (replaced, Arc::new(made));
            }
            Arc::clone(&self.drawing.faces_without.1)
        };
        for (pane, rect, _) in &shown {
            let size = pane_size(*rect);
            let camera = &self.panes[*pane].camera;
            let display = self.panes[*pane].display;
            let drawing = Drawing {
                slot: *pane,
                grid: if self.settings.grid {
                    Arc::clone(&self.panes[*pane].grid)
                } else {
                    Arc::default()
                },
                // Dark edges suit solid faces; ghosted views keep the light wireframe edges.
                scene: Arc::clone(if display == DisplayMode::Shaded {
                    &self.drawing.shaded_scene
                } else {
                    &self.scene
                }),
                faces: Arc::clone(&faces),
                overlay: Arc::clone(&overlay),
                preview_faces: Arc::clone(&preview_faces),
                matrix: camera.matrix(size[0] / size[1]),
                light: camera.forward().map(|value| value as f32),
                shaded: display != DisplayMode::Wireframe,
                ghosted: display == DisplayMode::Ghosted,
                zebra: self.panes[*pane].zebra,
                draft: self.panes[*pane].draft,
                area: *rect,
                frame: ui.ctx().cumulative_frame_nr(),
                background: self.pane_background(*pane).0,
                background_low: self.pane_background(*pane).1,
            };
            ui.painter().add(egui_wgpu_callback(*rect, drawing));
            self.paint_desk(ui, *pane, *rect, size);
            self.paint_frame(ui, *pane, *rect);
            self.paint_points(ui, *pane, *rect, size, &previewed);
            self.paint_grips(ui, *pane, *rect, size);
            self.paint_picked_edges(ui, *pane, *rect, size);
            self.paint_picked_faces(ui, *pane, *rect, size);
            self.paint_gumball(ui, *pane, *rect, size);
            self.paint_guides(ui, *pane, *rect, size);
            if let Some((hover_pane, ..)) = hovered
                && hover_pane == *pane
                && let Some(hit) = aimed.and_then(|(_, hit)| hit)
            {
                self.paint_snap_marker(ui, *pane, *rect, size, hit);
            }
            let dragging = self.panes[*pane]
                .gestures
                .pressed_at
                .zip(ui.input(|input| input.pointer.latest_pos()))
                .filter(|(from, to)| !self.takes_position() && is_box_drag(*from, *to));
            if let Some((from, to)) = dragging {
                paint_selection_box(ui, *rect, from, to);
            }
        }
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
            grid_lines_in(&raised, &theme::current().viewport).len()
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
