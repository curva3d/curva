//! The mouse in the viewports: clicks, selection boxes, sketching, aiming with snaps and guides,
//! and the clipboard keys.

use crate::*;

/// What the mouse is in the middle of in the viewports.
#[derive(Default)]
pub(crate) struct MouseState {
    /// While Sketch draws with the button held: where the last point was fed, and how many.
    pub(crate) sketch_stroke: Option<(egui::Pos2, usize)>,
    /// Snap points held by resting on them, which send out guidelines, as Rhino's SmartTrack.
    pub(crate) held: Vec<Point>,
    /// The snap point the pointer rests on, since when, and whether resting there has acted.
    pub(crate) resting: Option<(Point, f64, bool)>,
    /// Rhino's selection menu, while a click over several objects asks which one (#530).
    pub(crate) choosing: Option<crate::selection_menu::Choosing>,
    /// The objects whose edges or faces are picked, each with the copy of it they were picked
    /// on: when one changes, its numbered edges and faces may no longer be the same ones.
    pub(crate) parts_on: Vec<(u64, curva_kernel::Body)>,
}

/// Finds left-button clicks and drags from the raw mouse events.
///
/// egui's own click signal drops a press if the mouse travels 6 points before release, and
/// reports at most one per frame, which loses points clicked quickly while moving to the next.
#[derive(Default)]
pub(crate) struct LeftGestures {
    pub(crate) pressed_at: Option<egui::Pos2>,
    /// The keys held when the last press began, which decide what the click picks: they may be
    /// let go before the frame ends, as in a quick Ctrl+Shift click (#686).
    pub(crate) pressed_with: egui::Modifiers,
}

impl LeftGestures {
    /// Where each press finished in these events began and ended. A press only counts if it
    /// was `inside` the viewport.
    pub(crate) fn take(
        &mut self,
        events: &[egui::Event],
        inside: impl Fn(egui::Pos2) -> bool,
    ) -> Vec<(egui::Pos2, egui::Pos2)> {
        let mut gestures = Vec::new();
        for event in events {
            if let egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
                ..
            } = event
            {
                if *pressed {
                    self.pressed_at = inside(*pos).then_some(*pos);
                    self.pressed_with = *modifiers;
                } else {
                    gestures.extend(self.pressed_at.take().map(|from| (from, *pos)));
                }
            }
        }
        gestures
    }
}

/// The selection after a click or box, as in Rhino: Ctrl removes what was hit, Shift adds it,
/// and with neither it replaces the selection. Ctrl wins if both are held, as it loses nothing.
pub(crate) fn combined_selection<T: PartialEq>(
    current: Vec<T>,
    hits: Vec<T>,
    add: bool,
    remove: bool,
) -> Vec<T> {
    if remove {
        return current
            .into_iter()
            .filter(|id| !hits.contains(id))
            .collect();
    }
    let mut chosen = if add { current } else { Vec::new() };
    for hit in hits {
        if !chosen.contains(&hit) {
            chosen.push(hit);
        }
    }
    chosen
}

/// Whether a press that moved from one place to another counts as a drag, not a click.
pub(crate) fn is_box_drag(from: egui::Pos2, to: egui::Pos2) -> bool {
    (to - from).abs().max_elem() >= BOX_MIN_DRAG
}

/// The small tag beside the pointer saying what a step measures there, as in Serpentine3D.
pub(crate) fn paint_cursor_tag(ui: &egui::Ui, pointer: egui::Pos2, text: &str) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::NULL.with("cursor tag"),
    ));
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        egui::FontId::monospace(12.0),
        theme::current().quiet,
    );
    let corner = pointer + egui::vec2(18.0, 14.0);
    let frame = egui::Rect::from_min_size(corner, galley.size()).expand2(egui::vec2(6.0, 3.0));
    painter.rect(
        frame,
        4.0,
        theme::current().panel,
        egui::Stroke::new(1.0, theme::current().panel_edge),
        egui::StrokeKind::Inside,
    );
    painter.galley(corner, galley, theme::current().quiet);
}

/// The selection box: solid for a window, dashed for a crossing box, as in Rhino.
pub(crate) fn paint_selection_box(
    ui: &egui::Ui,
    viewport: egui::Rect,
    from: egui::Pos2,
    to: egui::Pos2,
) {
    let painter = ui.painter_at(viewport);
    let area = egui::Rect::from_two_pos(from, to);
    painter.rect_filled(area, 0.0, theme::SELECTION_BOX_FILL);
    let stroke = egui::Stroke::new(1.0, theme::SELECTION_BOX_EDGE);
    let edge = vec![
        area.left_top(),
        area.right_top(),
        area.right_bottom(),
        area.left_bottom(),
        area.left_top(),
    ];
    if to.x >= from.x {
        painter.add(egui::Shape::line(edge, stroke));
    } else {
        painter.add(egui::Shape::dashed_line(&edge, stroke, 6.0, 4.0));
    }
}

/// What decides where a cursor position lands for the point a command asks for.
pub(crate) struct Aiming<'a> {
    pub(crate) outlines: &'a Outlines,
    pub(crate) request: &'a curva_commands::Request,
    pub(crate) camera: &'a Camera,
    pub(crate) plane: &'a CPlane,
    pub(crate) display: DisplayMode,
    pub(crate) snaps: SnapSet,
    pub(crate) guides: curva_viewport::Guides,
    pub(crate) ortho: bool,
    /// Rhino's Planar: the pointer stays level with the last point.
    pub(crate) planar: bool,
    pub(crate) grid_snap: bool,
    /// Shift held on a two-corner step: the corners make a square.
    pub(crate) square: bool,
    /// Alt held: no object snaps for the moment.
    pub(crate) snaps_off: bool,
    pub(crate) screen: [f64; 2],
    pub(crate) size: [f64; 2],
}

/// Where a cursor position lands for the point being asked for: an object snap if one is in reach
/// (in a shaded view only one that can be seen), then the held points' guidelines, then a height
/// along its line, otherwise the viewport's plane with ortho and grid snap applied.
pub(crate) fn aim_at(aiming: &Aiming<'_>) -> Option<(Point, Option<SnapHit>)> {
    let Aiming {
        outlines,
        request,
        camera,
        plane,
        screen,
        size,
        ..
    } = *aiming;
    if request.kind == Kind::PickPart {
        let on = outlines.pick_point(camera, screen, size, PICK_REACH)?;
        return Some((on, None));
    }
    let base = request.preview.last().copied().or(request.from);
    if !aiming.snaps_off {
        let (set, drawing, guides) = aiming::snaps_for(request, aiming.snaps);
        let set = SnapSet {
            // Shaded, hidden and technical views hide what is behind faces; ghosted and wireframe
            // show it all.
            visible_only: !matches!(
                aiming.display,
                DisplayMode::Wireframe | DisplayMode::Ghosted
            ),
            ..set
        };
        let hit = outlines
            .snap_while_drawing(camera, screen, size, &set, base, drawing)
            .or_else(|| {
                guides
                    .then(|| outlines.guide_snap(camera, screen, size, &aiming.guides))
                    .flatten()
            });
        if let Some(mut hit) = hit {
            // Rhino's Project: the snapped point moves straight onto the construction plane.
            if set.project {
                hit.point = aiming::onto(plane, hit.point);
            }
            return Some((hit.point, Some(hit)));
        }
    }
    // A height or extrusion follows the pointer along its line, as in Rhino.
    if let Some((origin, direction)) = request.along {
        return camera
            .line_point(screen, size, origin, direction)
            .map(|at| (at, None));
    }
    // With Planar on, the pointer moves on a plane level with the last point, as in Rhino.
    let level = match base.filter(|_| aiming.planar) {
        Some(base) => aiming::level_with(plane, base),
        None => *plane,
    };
    let cursor = camera.plane_point(screen, size, &level)?;
    let at = constrained(&level, cursor, base, aiming.ortho, aiming.grid_snap);
    Some((
        match base.filter(|_| aiming.square) {
            Some(base) => aiming::squared(plane, base, at),
            None => at,
        },
        None,
    ))
}

/// The window's Copy, Cut and Paste events that are Curva's to act on, taken out of `events`:
/// copy and cut while objects are selected, and a paste of the mark Curva's own copy left on
/// the system clipboard. Anything else stays for the command line.
pub(crate) fn clipboard_requests(
    events: &mut Vec<egui::Event>,
    anything_selected: bool,
) -> (bool, bool, bool) {
    let (mut copy, mut cut, mut paste) = (false, false, false);
    events.retain(|event| match event {
        egui::Event::Copy if anything_selected => {
            copy = true;
            false
        }
        egui::Event::Cut if anything_selected => {
            cut = true;
            false
        }
        egui::Event::Paste(text) if text == CLIPBOARD_MARK => {
            paste = true;
            false
        }
        _ => true,
    });
    (copy, cut, paste)
}

/// How a click or a box pick in a viewport is read: its view, its corner on screen, its size,
/// and whether Shift adds to the selection or Ctrl takes away from it.
pub(crate) struct Picking {
    pub(crate) camera: Camera,
    pub(crate) origin: egui::Pos2,
    pub(crate) size: [f64; 2],
    pub(crate) shift: bool,
    pub(crate) remove: bool,
    /// Ctrl and Shift held together: parts of objects are picked, as in Rhino (#485).
    pub(crate) parts: bool,
}

impl Picking {
    /// A point on screen in the viewport's own pixels.
    pub(crate) fn local(&self, position: egui::Pos2) -> [f64; 2] {
        let offset = position - self.origin;
        [f64::from(offset.x), f64::from(offset.y)]
    }
}

impl Curva {
    /// The held points and their guidelines on a viewport's plane: along its X and Y, and at
    /// 45° unless ortho is on (Shift flipping it for the moment).
    pub(crate) fn guides(&self, pane: usize, shift: bool) -> curva_viewport::Guides {
        let plane = self.panes[pane].plane;
        curva_viewport::Guides {
            held: self.mouse.held.clone(),
            axes: [plane.x, plane.y],
            diagonals: self.ortho == shift,
        }
    }

    /// Holds a snap point the pointer has rested on for a moment, or lets it go if it was held,
    /// as Rhino's SmartTrack does. Everything held is let go when no command is asking for a
    /// point.
    pub(crate) fn rest_on(&mut self, ctx: &egui::Context, hit: Option<SnapHit>) {
        if !self.takes_position() {
            self.mouse.held.clear();
            self.mouse.resting = None;
            return;
        }
        let holdable = hit.filter(|hit| {
            matches!(
                hit.kind,
                SnapKind::End | SnapKind::Mid | SnapKind::Center | SnapKind::Intersection
            )
        });
        let Some(hit) = holdable else {
            self.mouse.resting = None;
            return;
        };
        let now = ctx.input(|input| input.time);
        match &mut self.mouse.resting {
            Some((point, since, acted)) if *point == hit.point => {
                if !*acted && now - *since >= HOLD_AFTER {
                    *acted = true;
                    let mut guides = curva_viewport::Guides {
                        held: std::mem::take(&mut self.mouse.held),
                        ..curva_viewport::Guides::default()
                    };
                    guides.toggle(hit.point);
                    self.mouse.held = guides.held;
                } else if !*acted {
                    // Come back when the rest is long enough, even if the mouse stays still.
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD_AFTER));
                }
            }
            _ => {
                self.mouse.resting = Some((hit.point, now, false));
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD_AFTER));
            }
        }
    }

    /// The held points and their guidelines, dashed and see-through in the accent orange.
    pub(crate) fn paint_guides(
        &self,
        ui: &egui::Ui,
        pane: usize,
        viewport: egui::Rect,
        size: [f64; 2],
    ) {
        if self.mouse.held.is_empty() {
            return;
        }
        let camera = &self.panes[pane].camera;
        let painter = ui.painter_at(viewport);
        let on_screen = |p: Point| {
            camera
                .project(p, size)
                .map(|at| viewport.min + egui::vec2(at[0] as f32, at[1] as f32))
        };
        let faint = theme::ACCENT.gamma_multiply(0.45);
        let reach = camera.distance * 20.0;
        for (point, direction) in self.guides(pane, false).lines() {
            let end = |sign: f64| std::array::from_fn(|i| point[i] + direction[i] * reach * sign);
            if let (Some(a), Some(b)) = (on_screen(end(-1.0)), on_screen(end(1.0))) {
                painter.extend(egui::Shape::dashed_line(
                    &[a, b],
                    egui::Stroke::new(1.0, faint),
                    6.0,
                    4.0,
                ));
            }
        }
        for point in &self.mouse.held {
            if let Some(at) = on_screen(*point) {
                painter.circle_filled(at, 3.5, theme::ACCENT);
            }
        }
    }

    pub(crate) fn takes_position(&self) -> bool {
        self.processor.current_request().is_some_and(|request| {
            matches!(
                request.kind,
                Kind::Point | Kind::PointOrNumber | Kind::PickPart
            ) || (request.kind == Kind::Number && request.from.is_some())
        })
    }

    /// Where a cursor position in a viewport lands for the point being asked for: an object snap
    /// if one is in reach, otherwise the viewport's plane with ortho and grid snap applied. Shift
    /// flips ortho for the moment and Alt turns object snaps off for the moment, as in Rhino.
    pub(crate) fn aim(
        &self,
        ui: &egui::Ui,
        pane: usize,
        screen: [f64; 2],
        size: [f64; 2],
    ) -> Option<(Point, Option<SnapHit>)> {
        let (shift, alt) = ui.input(|input| (input.modifiers.shift, input.modifiers.alt));
        let request = self.processor.current_request()?;
        let view = &self.panes[pane];
        aim_at(&Aiming {
            outlines: &self.outlines,
            request,
            camera: &view.camera,
            plane: &view.plane,
            display: view.display,
            snaps: self.snaps,
            guides: self.guides(pane, shift),
            // On a rectangle's or box's opposite corner Shift makes it square (#524), where
            // Ortho would only flatten it; elsewhere Shift is Ortho for the moment, as in Rhino.
            ortho: self.ortho != (shift && !request.corner),
            planar: self.planar,
            square: shift && request.corner,
            grid_snap: self.grid_snap,
            snaps_off: alt,
            screen,
            size,
        })
    }

    /// Sketch draws while the mouse button is held: a point each few pixels the pointer moves,
    /// and letting go ends the curve. Returns whether it took the mouse.
    pub(crate) fn sketch_mouse(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        response: &egui::Response,
        size: [f64; 2],
    ) -> bool {
        let sketching = self
            .processor
            .current_request()
            .is_some_and(|request| request.prompt.key == "prompt.sketch.point");
        if !sketching {
            self.mouse.sketch_stroke = None;
            return false;
        }
        let (down, released, at) = ui.input(|input| {
            (
                input.pointer.primary_down(),
                input.pointer.primary_released(),
                input.pointer.latest_pos(),
            )
        });
        if released {
            if let Some((_, fed)) = self.mouse.sketch_stroke.take()
                && fed >= 2
            {
                self.processor.submit(&mut self.doc, "");
            }
            return true;
        }
        let Some(at) = at.filter(|p| response.rect.contains(*p)) else {
            return self.mouse.sketch_stroke.is_some();
        };
        if !down {
            return false;
        }
        let far_enough = self
            .mouse
            .sketch_stroke
            .is_none_or(|(last, _)| last.distance(at) >= SKETCH_STEP);
        if far_enough {
            let offset = at - response.rect.min;
            let local = [f64::from(offset.x), f64::from(offset.y)];
            if let Some((point, _)) = self.aim(ui, pane, local, size) {
                self.active = pane;
                self.processor.click(&mut self.doc, point);
                let fed = self.mouse.sketch_stroke.map_or(0, |(_, n)| n) + 1;
                self.mouse.sketch_stroke = Some((at, fed));
            }
        }
        true
    }

    pub(crate) fn handle_mouse(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        response: &egui::Response,
        size: [f64; 2],
    ) {
        // Double-clicking a note edits it in place.
        if self.text_double_click(pane, response, size) {
            self.active = pane;
            return;
        }
        // In a detail entered on a sheet, panning and zooming move the detail's view instead.
        if self.detail_mouse(ui, pane, response, size) {
            self.active = pane;
            return;
        }
        self.navigate(ui, pane, response, size);
        let in_title = |position: egui::Pos2| position.y < response.rect.top() + TITLE_HEIGHT;
        if response.double_clicked() && response.interact_pointer_pos().is_some_and(in_title) {
            self.maximised = if self.maximised.is_some() {
                None
            } else {
                Some(pane)
            };
        }
        let events = ui.input(|input| input.events.clone());
        let on_viewport = |position: egui::Pos2| {
            response.rect.contains(position)
                && !in_title(position)
                && ui.ctx().layer_id_at(position) == Some(response.layer_id)
        };
        if self.gumball_mouse(ui, pane, response, size, &events) {
            return;
        }
        if self.sketch_mouse(ui, pane, response, size) {
            return;
        }
        let gestures = self.panes[pane].gestures.take(&events, on_viewport);
        // The keys held when the press began, not when the frame ends (#686).
        let keys = if gestures.is_empty() {
            ui.input(|input| input.modifiers)
        } else {
            self.panes[pane].gestures.pressed_with
        };
        let picking = Picking {
            camera: self.panes[pane].camera,
            origin: response.rect.min,
            size,
            shift: keys.shift,
            remove: keys.command,
            parts: self.picks_parts(keys),
        };
        // Checked once, so the click that ends a command does not also change the selection.
        if self.takes_position() {
            for (pressed, _) in gestures {
                if !self.takes_position() {
                    break;
                }
                // Where it was pressed, as that is where the user aimed.
                if let Some((at, _)) = self.aim(ui, pane, picking.local(pressed), size) {
                    self.active = pane;
                    self.share_work_plane();
                    self.processor.click(&mut self.doc, at);
                }
            }
            return;
        }
        self.route_picks(&picking, gestures);
    }

    /// Each click or box to what the moment asks for: parts of objects (Ctrl+Shift, or a command
    /// asking for mesh parts), faces or edges a command asks for, or else objects.
    fn route_picks(&mut self, picking: &Picking, gestures: Vec<(egui::Pos2, egui::Pos2)>) {
        let wants = |kind: Kind| {
            self.processor
                .current_request()
                .is_some_and(|request| request.kind == kind)
        };
        let (wants_edges, wants_faces) = (wants(Kind::Edges), wants(Kind::Faces));
        let wants_parts = wants(Kind::MeshParts) || picking.parts;
        for (from, to) in gestures {
            if wants_parts {
                self.pick_parts(picking, from, to);
            } else if !is_box_drag(from, to) && self.pick_cage_point(picking, from) {
                // A point of a SubD cage on show, taken with a plain click.
            } else if wants_faces {
                self.pick_faces(picking, from, to);
            } else if wants_edges {
                self.pick_edges(picking, from, to);
            } else {
                self.pick_objects(picking, from, to);
            }
        }
    }

    /// Moves the view: Ctrl + right-drag zooms about where the drag began (dragging up comes
    /// closer), a middle drag or Shift + right-drag pans (a right drag too in a parallel view), a
    /// right drag orbits, and the wheel zooms towards the mouse.
    fn navigate(&mut self, ui: &egui::Ui, pane: usize, response: &egui::Response, size: [f64; 2]) {
        // Not `drag_delta`: egui holds that back for the first 6 points to tell a click from a
        // drag, and only the left button clicks here. The view must move from the first pixel.
        let (delta, shift, ctrl, middle, right, pressed) = ui.input(|input| {
            (
                input.pointer.delta(),
                input.modifiers.shift,
                input.modifiers.command,
                input.pointer.middle_down(),
                input.pointer.secondary_down(),
                input.pointer.press_origin(),
            )
        });
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        if response.is_pointer_button_down_on() {
            self.active = pane;
            let plane = self.panes[pane].plane;
            let origin = response.rect.min;
            let about = pressed.and_then(|at| {
                let offset = at - origin;
                let screen = [f64::from(offset.x), f64::from(offset.y)];
                self.panes[pane].camera.plane_point(screen, size, &plane)
            });
            let camera = &mut self.panes[pane].camera;
            if right && ctrl {
                if let Some(about) = about {
                    camera.zoom_about((dy * ZOOM_DRAG_SPEED).exp(), about);
                } else {
                    camera.zoom((dy * ZOOM_DRAG_SPEED).exp());
                }
            } else if middle || (right && (shift || camera.is_parallel())) {
                camera.pan(dx, dy, size[1]);
            } else if right {
                camera.orbit(dx, dy);
            }
        }
        if response.hovered() {
            let scroll = f64::from(ui.input(|input| input.smooth_scroll_delta.y));
            let mouse = ui.input(|input| input.pointer.hover_pos());
            if scroll != 0.0 {
                let camera = &mut self.panes[pane].camera;
                let factor = (-scroll * 0.002).exp();
                match mouse {
                    Some(at) => {
                        let offset = at - response.rect.min;
                        let screen = [f64::from(offset.x), f64::from(offset.y)];
                        camera.zoom_at(factor, screen, size);
                    }
                    None => camera.zoom(factor),
                }
            }
        }
    }

    /// A command is asking for faces, so a click or a box from `from` to `to` picks faces, not
    /// objects.
    fn pick_faces(&mut self, p: &Picking, from: egui::Pos2, to: egui::Pos2) {
        let hits = if is_box_drag(from, to) {
            self.outlines
                .faces_in_box(&p.camera, p.local(from), p.local(to), p.size)
        } else {
            self.outlines
                .pick_face(&p.camera, p.local(from), p.size)
                .into_iter()
                .collect()
        };
        let picked = combined_selection(self.doc.face_selection(), hits, p.shift, p.remove);
        self.doc.select_faces(&picked);
        self.remember_parts_picked();
    }

    /// Lets everything go: objects, control points, and picked edges, faces and mesh parts.
    pub(crate) fn select_nothing(&mut self) {
        self.doc.select(&[]);
        self.doc.select_grips(&[]);
        self.doc.select_edges(&[]);
        self.doc.select_faces(&[]);
        self.doc.select_mesh_parts(&[]);
    }

    /// Notes which body each picked edge and face belongs to (see [`MouseState::parts_on`]).
    pub(crate) fn remember_parts_picked(&mut self) {
        let ids: Vec<u64> = self
            .doc
            .edge_selection()
            .iter()
            .chain(self.doc.face_selection().iter())
            .map(|(id, _)| *id)
            .collect();
        self.mouse.parts_on = self
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .filter_map(|o| o.shape.body().map(|b| (o.id, b.clone())))
            .collect();
    }

    /// Whether an object whose edges or faces are picked has changed since (or is gone), so the
    /// numbers picked may now name other edges and faces.
    pub(crate) fn picked_parts_stale(&self) -> bool {
        crate::part_picking::changed_since(&self.mouse.parts_on, &self.doc.state().objects)
    }

    /// A command is asking for edges, so a click or a box from `from` to `to` picks edges, not
    /// objects.
    fn pick_edges(&mut self, p: &Picking, from: egui::Pos2, to: egui::Pos2) {
        let hits = if is_box_drag(from, to) {
            self.outlines
                .edges_in_box(&p.camera, p.local(from), p.local(to), p.size)
        } else {
            self.outlines
                .pick_edge(&p.camera, p.local(from), p.size, PICK_REACH)
                .into_iter()
                .collect()
        };
        let picked = combined_selection(self.doc.edge_selection(), hits, p.shift, p.remove);
        self.doc.select_edges(&picked);
        self.remember_parts_picked();
    }

    /// A click or a box from `from` to `to` picks control points if it hits any, and objects
    /// otherwise; picking one member of a group picks the whole group, as in Rhino.
    fn pick_objects(&mut self, p: &Picking, from: egui::Pos2, to: egui::Pos2) {
        // A click away from an open selection menu closes it, keeping the selection as it was.
        if let Some(choosing) = self.mouse.choosing.take() {
            self.doc.select(&choosing.before);
            return;
        }
        let boxed = is_box_drag(from, to);
        let mut grips = self.grips_hit(&p.camera, (p.local(from), p.local(to)), boxed, p.size);
        grips.retain(|_| self.pick_filter.allows_control_points());
        if !grips.is_empty() {
            // A control point was hit, so the points picked change and the objects do not.
            let picked = combined_selection(self.doc.grip_selection(), grips, p.shift, p.remove);
            self.doc.select_grips(&picked);
            return;
        }
        if !p.shift && !p.remove {
            // A plain pick starts afresh: picked points and parts of objects let go, as in Rhino.
            self.doc.select_grips(&[]);
            self.doc.select_edges(&[]);
            self.doc.select_faces(&[]);
            self.doc.select_mesh_parts(&[]);
        }
        let hits = if boxed {
            self.outlines
                .in_box(&p.camera, p.local(from), p.local(to), p.size)
        } else {
            let under = self.filtered_picks(self.outlines.pick_all(
                &p.camera,
                p.local(from),
                p.size,
                PICK_REACH,
            ));
            // Several objects under the pointer: ask which one, as Rhino does (#530).
            if under.len() > 1 {
                self.open_selection_menu(from, under, (p.shift, p.remove));
                return;
            }
            under
        };
        let hits = self.doc.with_groups(&self.filtered_picks(hits));
        let before = self.doc.selection();
        let chosen = combined_selection(before.clone(), hits, p.shift, p.remove);
        self.doc.select(&chosen);
        // What was picked, as Rhino says it: "1 open curve added to selection." (#520)
        let said = curva_commands::selection_report::selection_change(
            &self.doc, &before, &chosen, &self.text,
        );
        if let Some(line) = said {
            self.processor.note(line);
        }
    }

    pub(crate) fn paint_snap_marker(
        &self,
        ui: &egui::Ui,
        pane: usize,
        viewport: egui::Rect,
        size: [f64; 2],
        hit: SnapHit,
    ) {
        let Some(at) = self.panes[pane].camera.project(hit.point, size) else {
            return;
        };
        let centre = viewport.min + egui::vec2(at[0] as f32, at[1] as f32);
        let painter = ui.painter_at(viewport);
        // ArchiCAD's symbols: a cross for an intersection, a triangle for a midpoint, a circle
        // for a centre, a square for an end and the rest.
        let stroke = egui::Stroke::new(2.0, theme::ACCENT);
        let half = SNAP_MARKER_SIZE / 2.0;
        match hit.kind {
            SnapKind::Intersection => {
                for (a, b) in [(-1.0, -1.0), (-1.0, 1.0)] {
                    let offset = egui::vec2(a * half, b * half);
                    painter.line_segment([centre + offset, centre - offset], stroke);
                }
            }
            SnapKind::Mid => {
                let corners = vec![
                    centre + egui::vec2(0.0, -half),
                    centre + egui::vec2(half, half * 0.8),
                    centre + egui::vec2(-half, half * 0.8),
                ];
                painter.add(egui::Shape::closed_line(corners, stroke));
            }
            SnapKind::Center => {
                painter.circle_stroke(centre, half, stroke);
            }
            // Along a guideline: a small dot on it, the guideline itself saying where (#547).
            SnapKind::Track => {
                painter.circle_filled(centre, 3.0, theme::ACCENT);
            }
            _ => {
                painter.rect_stroke(
                    egui::Rect::from_center_size(centre, egui::Vec2::splat(SNAP_MARKER_SIZE)),
                    0.0,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
        let label = snap_label(hit.kind);
        if !label.is_empty() {
            painter.text(
                centre + egui::vec2(SNAP_MARKER_SIZE, -SNAP_MARKER_SIZE),
                egui::Align2::LEFT_BOTTOM,
                self.text.render(&Message::new(label)),
                egui::TextStyle::Body.resolve(ui.style()),
                theme::ACCENT,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Sense, pos2, vec2};

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn anywhere(_: Pos2) -> bool {
        true
    }

    #[test]
    fn a_quick_click_while_moving_is_kept_though_egui_drops_it() {
        let context = egui::Context::default();
        let mut clicks = LeftGestures::default();
        let (start, end) = (pos2(100.0, 100.0), pos2(130.0, 100.0));
        let frames = [
            vec![],
            vec![Event::PointerMoved(start)],
            vec![button(start, true)],
            vec![Event::PointerMoved(end)],
            vec![button(end, false)],
        ];
        let (mut egui_clicked, mut found) = (false, Vec::new());
        for events in frames {
            let input = RawInput {
                events,
                ..RawInput::default()
            };
            let output = context.run_ui(input, |ui| {
                let (_, response) =
                    ui.allocate_exact_size(vec2(400.0, 400.0), Sense::click_and_drag());
                egui_clicked |= response.clicked();
                let events = ui.input(|input| input.events.clone());
                found.extend(clicks.take(&events, anywhere));
            });
            output.drop_without_applying_deltas();
        }
        assert!(!egui_clicked, "this is the behaviour that lost the points");
        assert_eq!(found, [(start, end)], "both ends of the press are kept");
    }

    #[test]
    fn a_height_follows_the_mouse_not_the_corners_just_clicked() {
        use curva_commands::{Kind, Request};
        let every = SnapSet {
            near: true,
            ..SnapSet::default()
        };
        let mut point = Request::new("prompt.box.other", Kind::Point);
        point.preview = vec![[0.0; 3], [4.0, 3.0, 0.0]];
        let (set, drawing, guides) = aiming::snaps_for(&point, every);
        assert_eq!(set, every);
        assert_eq!(drawing.len(), 2);
        assert!(guides);
        let mut height = Request::new("prompt.box.height", Kind::Number);
        height.preview = point.preview.clone();
        height.along = Some(([4.0, 3.0, 0.0], [0.0, 0.0, 1.0]));
        let (set, drawing, guides) = aiming::snaps_for(&height, every);
        assert!(!set.near && !set.perpendicular);
        assert_eq!(set.end, every.end, "other objects' points still snap");
        assert!(drawing.is_empty());
        assert!(!guides);
    }

    #[test]
    fn several_clicks_in_one_frame_all_count() {
        let (a, b) = (pos2(10.0, 10.0), pos2(50.0, 50.0));
        let events = [
            button(a, true),
            button(a, false),
            button(b, true),
            button(b, false),
        ];
        assert_eq!(
            LeftGestures::default().take(&events, anywhere),
            [(a, a), (b, b)]
        );
    }

    #[test]
    fn a_press_outside_the_viewport_is_not_a_click() {
        let at = pos2(10.0, 10.0);
        let events = [button(at, true), button(at, false)];
        assert!(LeftGestures::default().take(&events, |_| false).is_empty());
    }

    #[test]
    fn a_press_and_release_in_separate_frames_is_one_click() {
        let at = pos2(10.0, 10.0);
        let mut clicks = LeftGestures::default();
        assert!(clicks.take(&[button(at, true)], anywhere).is_empty());
        assert_eq!(clicks.take(&[button(at, false)], anywhere), [(at, at)]);
        assert!(clicks.take(&[button(at, false)], anywhere).is_empty());
    }

    #[test]
    fn with_no_key_a_selection_is_replaced() {
        assert_eq!(combined_selection(vec![1, 2], vec![3], false, false), [3]);
        assert!(combined_selection(vec![1, 2], vec![], false, false).is_empty());
    }

    #[test]
    fn shift_adds_without_repeating() {
        assert_eq!(
            combined_selection(vec![1, 2], vec![2, 3], true, false),
            [1, 2, 3]
        );
    }

    #[test]
    fn ctrl_removes_only_what_was_hit() {
        assert_eq!(
            combined_selection(vec![1, 2, 3], vec![2, 9], false, true),
            [1, 3]
        );
    }

    #[test]
    fn ctrl_on_nothing_keeps_the_selection() {
        assert_eq!(combined_selection(vec![1, 2], vec![], false, true), [1, 2]);
    }

    #[test]
    fn ctrl_wins_over_shift() {
        assert_eq!(combined_selection(vec![1, 2], vec![2], true, true), [1]);
    }

    #[test]
    fn a_small_movement_is_a_click_and_a_larger_one_a_box() {
        let at = pos2(100.0, 100.0);
        assert!(!is_box_drag(at, pos2(103.0, 104.0)));
        assert!(is_box_drag(at, pos2(120.0, 101.0)));
        assert!(is_box_drag(at, pos2(100.0, 80.0)));
    }

    #[test]
    fn other_buttons_are_not_left_clicks() {
        let at = pos2(10.0, 10.0);
        let events = [Event::PointerButton {
            pos: at,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }];
        let mut clicks = LeftGestures::default();
        assert!(clicks.take(&events, anywhere).is_empty());
        assert!(clicks.pressed_at.is_none());
    }

    #[test]
    fn clicks_snap_to_curves_and_polylines_in_every_display_mode() {
        for display in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
            DisplayMode::Hidden,
            DisplayMode::Technical,
            DisplayMode::Massing,
        ] {
            let mut doc = Document::default();
            doc.add(Curve::line([0.0; 3], [40.0, 0.0, 0.0]).unwrap());
            doc.add(
                Curve::polyline(
                    vec![[0.0, 30.0, 0.0], [30.0, 30.0, 0.0], [30.0, 60.0, 0.0]],
                    false,
                )
                .unwrap(),
            );
            doc.add_body(curva_kernel::Body::cuboid([60.0, 0.0, 0.0], [10.0, 10.0, 10.0]).unwrap());
            let outlines = Outlines::of(doc.state());
            let mut processor = Processor::default();
            processor.submit(&mut doc, "line");
            processor.submit(&mut doc, "0,-20");
            let pane = Pane::new(StandardView::Perspective);
            let size = [800.0, 600.0];
            for target in [[40.0, 0.0, 0.0], [30.0, 30.0, 0.0], [20.0, 0.0, 0.0]] {
                let screen = pane.camera.project(target, size).unwrap();
                let aimed = aim_at(&Aiming {
                    outlines: &outlines,
                    request: processor.current_request().unwrap(),
                    camera: &pane.camera,
                    plane: &pane.plane,
                    display,
                    snaps: SnapSet::default(),
                    guides: curva_viewport::Guides::default(),
                    ortho: false,
                    planar: false,
                    grid_snap: false,
                    square: false,
                    snaps_off: false,
                    screen: [screen[0] + 3.0, screen[1] - 2.0],
                    size,
                });
                let (at, hit) = aimed.unwrap();
                assert!(hit.is_some(), "no snap at {target:?} in {display:?}");
                assert!(
                    curva_kernel::gap(at, target) < 1e-6,
                    "{at:?} for {target:?} in {display:?}"
                );
            }
            // A click there ends the line exactly on the snapped point.
            let end = pane.camera.project([30.0, 30.0, 0.0], size).unwrap();
            let (at, _) = aim_at(&Aiming {
                outlines: &outlines,
                request: processor.current_request().unwrap(),
                camera: &pane.camera,
                plane: &pane.plane,
                display,
                snaps: SnapSet::default(),
                guides: curva_viewport::Guides::default(),
                ortho: false,
                planar: false,
                grid_snap: false,
                square: false,
                snaps_off: false,
                screen: end,
                size,
            })
            .unwrap();
            processor.click(&mut doc, at);
            let made = doc
                .state()
                .objects
                .last()
                .unwrap()
                .curve()
                .unwrap()
                .sample()
                .unwrap();
            assert!(
                curva_kernel::gap(*made.last().unwrap(), [30.0, 30.0, 0.0]) < 1e-6,
                "{display:?}"
            );
        }
    }

    #[test]
    fn copy_and_cut_are_taken_for_a_selection_and_left_for_the_command_line_otherwise() {
        let mut events = vec![Event::Copy, Event::Cut, Event::Text("x".to_owned())];
        assert_eq!(clipboard_requests(&mut events, true), (true, true, false));
        assert_eq!(events, [Event::Text("x".to_owned())]);
        let mut events = vec![Event::Copy, Event::Cut];
        assert_eq!(
            clipboard_requests(&mut events, false),
            (false, false, false)
        );
        assert_eq!(events, [Event::Copy, Event::Cut]);
    }

    #[test]
    fn only_curvas_own_mark_pastes_objects() {
        let mut events = vec![Event::Paste(CLIPBOARD_MARK.to_owned())];
        assert_eq!(clipboard_requests(&mut events, false), (false, false, true));
        assert!(events.is_empty(), "the mark never reaches the command line");
        let mut events = vec![Event::Paste(r"C:\models\house.3dm".to_owned())];
        assert_eq!(clipboard_requests(&mut events, true), (false, false, false));
        assert_eq!(
            events.len(),
            1,
            "other text is pasted into the command line"
        );
    }
}
