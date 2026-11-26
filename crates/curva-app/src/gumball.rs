//! The gumball: drawing it, dragging it, typing a value, and what a drag does to the selection.

use crate::*;

/// A gumball drag or typed value under way.
#[derive(Default)]
pub(crate) struct GumballState {
    pub(crate) gumball_drag: Option<GumballDrag>,
    /// A gumball handle clicked without a drag, and the value being typed for it.
    pub(crate) gumball_typing: Option<(Handle, String)>,
    /// A handle was double-clicked: the next drag moves the gumball itself, not what it holds,
    /// as Rhino 8 relocates it (`gumball_relocate`).
    pub(crate) relocating: bool,
    /// Where the gumball was moved to, for what was picked then.
    pub(crate) moved_to: Option<(crate::gumball_relocate::Holding, Point)>,
    /// The handle last clicked without a drag, and when, to tell a double-click (#685).
    pub(crate) last_click: Option<(Handle, std::time::Instant)>,
}

/// How soon a second click on the same handle must come to make a double-click.
const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

/// Whether a click on `handle` at `now` is the second of a double-click after `last`.
fn is_double(
    last: Option<(Handle, std::time::Instant)>,
    handle: Handle,
    now: std::time::Instant,
) -> bool {
    last.is_some_and(|(held, at)| held == handle && now.duration_since(at) < DOUBLE_CLICK)
}

/// Most objects shown moving during a gumball drag.
pub(crate) const MOST_GUMBALL_PREVIEW: usize = 2000;

pub(crate) const GUMBALL_TIP: f32 = 5.0;
/// The dash and gap of the dotted line out to each scale square, in points.
const GUMBALL_DASH: f32 = 3.0;

pub(crate) const GUMBALL_AXES: [egui::Color32; 3] = [
    egui::Color32::from_rgb(230, 80, 80),
    egui::Color32::from_rgb(90, 200, 100),
    egui::Color32::from_rgb(90, 130, 240),
];

/// A drag on the gumball under way, in the pixels of its viewport.
#[derive(Clone, Copy)]
pub(crate) struct GumballDrag {
    pub(crate) pane: usize,
    pub(crate) handle: Handle,
    pub(crate) size: [f64; 2],
    pub(crate) from: [f64; 2],
    pub(crate) now: [f64; 2],
    /// Whether Shift is held, for an even scale.
    pub(crate) even: bool,
    /// Whether Ctrl is held: with Shift, the circle scales evenly.
    pub(crate) ctrl: bool,
    /// Whether the drag makes a copy, the original staying put: Alt tapped during the drag
    /// switches it, as in Rhino (#486).
    pub(crate) copy: bool,
    /// Whether Alt was down at the last frame, to see it tapped.
    pub(crate) alt_was: bool,
    /// Whether the drag began on an arrow with Ctrl held, which extrudes along it however the
    /// circle would act.
    pub(crate) arrow_extrude: bool,
    /// Whether the drag moves the gumball itself, after a double-click on a handle.
    pub(crate) relocate: bool,
}

/// Whether a drag makes a copy after this frame: Alt going down switches it, as in Rhino, where
/// tapping Alt during a gumball drag toggles copying (#486).
fn copy_after(copy: bool, alt_was: bool, alt: bool) -> bool {
    if alt && !alt_was { !copy } else { copy }
}

/// What the gumball's extrude circle makes of one object: a curve becomes a surface (a closed
/// one a solid), a surface is pulled straight along the axis into a solid, as ExtrudeSrf does;
/// a solid makes nothing.
pub(crate) fn gumball_extruded(
    shape: &Shape,
    axis: Point,
    distance: f64,
) -> Option<Result<curva_kernel::Body, curva_kernel::KernelError>> {
    match shape {
        Shape::Curve(curve) => {
            let closed = curve.is_closed().unwrap_or(false);
            Some(curva_kernel::Body::extruded(
                std::slice::from_ref(curve),
                axis,
                distance,
                closed,
            ))
        }
        Shape::Body(body) if !body.is_solid() => Some(body.extruded_straight(axis, distance)),
        Shape::Body(_) => None,
        Shape::Instance(_) | Shape::Annotation(_) | Shape::Mesh(_) | Shape::SubD(_) => None,
    }
}

/// A stretch by `factor` along `axis` (of length 1) about `origin`, as one affine map in
/// Curva's row form (`p' = p·A + t`).
fn stretch_map(origin: Point, axis: Point, factor: f64) -> [[f64; 4]; 4] {
    let stretched = |v: Point| -> Point {
        let along = curva_kernel::dot(v, axis) * (factor - 1.0);
        std::array::from_fn(|k| v[k] + axis[k] * along)
    };
    let row = |v: Point| {
        let p = stretched(v);
        [p[0], p[1], p[2], 0.0]
    };
    let moved = stretched(origin);
    [
        row([1.0, 0.0, 0.0]),
        row([0.0, 1.0, 0.0]),
        row([0.0, 0.0, 1.0]),
        [
            origin[0] - moved[0],
            origin[1] - moved[1],
            origin[2] - moved[2],
            1.0,
        ],
    ]
}

impl Curva {
    /// The gumball for the current selection, if it should show, where it sits by itself: the
    /// middle of what it holds.
    pub(crate) fn natural_gumball(&self) -> Option<Gumball> {
        if !self.gumball_on || self.processor.is_running() {
            return None;
        }
        // Picked control points and parts (Ctrl+Shift) take the gumball, as in Rhino; it sits
        // at their middle.
        if self.doc.has_picked_parts() {
            let points = self.doc.picked_part_points().unwrap_or_default();
            if points.is_empty() {
                return None;
            }
            let count = points.len() as f64;
            let sum = points.iter().fold([0.0; 3], |total, p| {
                [total[0] + p[0], total[1] + p[1], total[2] + p[2]]
            });
            // The scale squares sit on the picked points' box, as on an object's (#693).
            let low = std::array::from_fn(|k| points.iter().map(|p| p[k]).fold(f64::MAX, f64::min));
            let high =
                std::array::from_fn(|k| points.iter().map(|p| p[k]).fold(f64::MIN, f64::max));
            return Some(Gumball {
                centre: sum.map(|value| value / count),
                extent: Some((low, high)),
            });
        }
        let selection = self.doc.selection();
        let centre = self.outlines.centre_of(&selection)?;
        // The scale squares sit on the selection's box, on each axis's negative side, as in
        // Rhino 8 (#693).
        Some(Gumball {
            centre,
            extent: self.outlines.bounds_of(&selection),
        })
    }

    /// What the drag under way would do.
    pub(crate) fn gumball_change(&self, drag: &GumballDrag) -> Option<GumballChange> {
        let gumball = self.gumball_base()?;
        let camera = &self.panes[drag.pane].camera;
        let handle = match drag.handle {
            Handle::Extrude(index)
                if !drag.arrow_extrude && (drag.even || self.circle_stretches(index)) =>
            {
                Handle::Scale(index)
            }
            other => other,
        };
        let even_scale = drag.even && drag.ctrl;
        // Shift snaps a turn to every 5 degrees (#529).
        let turn_step = drag.even.then_some(5.0);
        let change = gumball.change_with(
            camera, drag.size, handle, drag.from, drag.now, even_scale, turn_step,
        )?;
        // Object snaps apply to the point dragged: the centre lines up with the snapped point
        // along the axis, or moves to it in the plane, as in Rhino. Alt turns them off.
        let alt = self.alt_held;
        let Some(hit) = self
            .outlines
            .snap(camera, drag.now, drag.size, &self.snaps, None)
            .filter(|_| !alt)
        else {
            return Some(change);
        };
        let axis_of =
            |index: usize| -> Point { std::array::from_fn(|i| f64::from(u8::from(i == index))) };
        let from_centre = |p: Point| -> Point { std::array::from_fn(|i| p[i] - gumball.centre[i]) };
        let to_hit = from_centre(hit.point);
        let along = |v: Point, axis: Point| (0..3).map(|i| v[i] * axis[i]).sum::<f64>();
        Some(match (handle, change) {
            (Handle::Move(index), _) => {
                let axis = axis_of(index);
                GumballChange::Move(axis.map(|v| v * along(to_hit, axis)))
            }
            (Handle::Extrude(index), _) => GumballChange::Extrude {
                axis: axis_of(index),
                distance: along(to_hit, axis_of(index)),
            },
            // A stretch takes the snapped point to where the circle was.
            (Handle::Scale(index), GumballChange::Stretch { origin, axis, .. }) => {
                let held = gumball
                    .handle_point(camera, drag.size, Handle::Scale(index))
                    .map(|p| along(from_centre(p), axis))
                    .unwrap_or(1.0);
                let factor = along(to_hit, axis) / held;
                if factor.is_finite() && factor > 1e-9 {
                    GumballChange::Stretch {
                        origin,
                        axis,
                        factor,
                    }
                } else {
                    change
                }
            }
            (Handle::Plane(_) | Handle::Centre, _) => {
                let index = match handle {
                    Handle::Plane(index) => index,
                    _ => 2,
                };
                let normal = axis_of(index);
                let off = along(to_hit, normal);
                GumballChange::Move(std::array::from_fn(|i| to_hit[i] - normal[i] * off))
            }
            (_, change) => change,
        })
    }

    /// Whether the gumball's circle stretches what it holds rather than extruding it: for closed
    /// solids, which cannot be extruded (`docs/specs/gumball-rhino.md`). Scaling along an axis is
    /// the scale squares' work, as in Rhino 8 (owner, 25 November 2026, #693). Picked parts move
    /// along the axis. Shift stretches anything, and Shift with Ctrl scales evenly.
    pub(crate) fn circle_stretches(&self, _index: usize) -> bool {
        let shapes = self.selected_shapes();
        !self.doc.has_picked_parts()
            && !shapes.is_empty()
            && shapes
                .iter()
                .all(|(_, shape)| shape.body().is_some_and(curva_kernel::Body::is_solid))
    }

    /// Runs the gumball for one viewport. Returns whether it took the mouse.
    pub(crate) fn gumball_mouse(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        response: &egui::Response,
        size: [f64; 2],
        events: &[egui::Event],
    ) -> bool {
        let origin = response.rect.min;
        let local = |position: egui::Pos2| {
            let offset = position - origin;
            [f64::from(offset.x), f64::from(offset.y)]
        };
        if let Some(mut drag) = self.gumball_state.gumball_drag {
            if drag.pane != pane {
                return false;
            }
            if let Some(now) = ui.input(|input| input.pointer.latest_pos()) {
                drag.now = local(now);
                drag.even = ui.input(|input| input.modifiers.shift);
                drag.ctrl = ui.input(|input| input.modifiers.command);
                let alt = ui.input(|input| input.modifiers.alt);
                drag.copy = copy_after(drag.copy, drag.alt_was, alt);
                drag.alt_was = alt;
                self.gumball_state.gumball_drag = Some(drag);
            }
            let released = events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    }
                )
            });
            if released {
                self.finish_gumball(&drag);
                self.gumball_state.gumball_drag = None;
            }
            return true;
        }
        let Some(gumball) = self.gumball() else {
            return false;
        };
        for event in events {
            if let egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } = event
                && response.rect.contains(*pos)
                && let Some(handle) = gumball.handle_at(&self.panes[pane].camera, local(*pos), size)
            {
                self.active = pane;
                self.press_gumball(ui, pane, handle, (local(*pos), size));
                return true;
            }
        }
        false
    }

    /// A press on the gumball's `handle` at `at` in a viewport of `size`: starts a drag, or after
    /// a double-click readies the gumball to be moved itself, as in Rhino 8.
    fn press_gumball(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        handle: Handle,
        (at, size): ([f64; 2], [f64; 2]),
    ) {
        // Ctrl-dragging an arrow extrudes along it, as in Rhino.
        let ctrl = ui.input(|input| input.modifiers.command);
        let arrow_extrude = matches!(handle, Handle::Move(_)) && ctrl;
        let handle = match handle {
            Handle::Move(axis) if ctrl => Handle::Extrude(axis),
            other => other,
        };
        self.gumball_state.gumball_drag = Some(GumballDrag {
            pane,
            handle,
            size,
            from: at,
            now: at,
            even: false,
            ctrl: false,
            copy: false,
            alt_was: ui.input(|input| input.modifiers.alt),
            arrow_extrude,
            relocate: self.gumball_state.relocating,
        });
    }

    /// Applies a finished drag as one undo step.
    pub(crate) fn finish_gumball(&mut self, drag: &GumballDrag) {
        if drag.relocate {
            self.finish_relocating(drag);
            return;
        }
        // A click without a drag asks for an exact value, as in Rhino; a second one on the same
        // handle straight after is a double-click, and the next drag moves the gumball itself, as
        // Rhino 8 relocates it (#685: egui says a double-click only once the second click is let go,
        // after the press that started this drag).
        if drag.from == drag.now {
            let now = std::time::Instant::now();
            if is_double(self.gumball_state.last_click, drag.handle, now) {
                self.gumball_state.gumball_typing = None;
                self.gumball_state.relocating = true;
                self.gumball_state.last_click = None;
            } else {
                self.gumball_state.gumball_typing = Some((drag.handle, String::new()));
                self.gumball_state.last_click = Some((drag.handle, now));
            }
            return;
        }
        let Some(change) = self.gumball_change(drag) else {
            return;
        };
        if drag.copy {
            // The drag moves a copy: the selection is copied in place, the copies take the
            // change, and both are one undo step.
            let (mark, before) = (self.doc.undo_mark(), self.doc.state().clone());
            let selection = self.doc.selection();
            let in_place = curva_kernel::Similarity::scaling([0.0; 3], 1.0);
            let copies = self.doc.copy_transformed(&selection, &in_place);
            self.doc.select(&copies);
            self.apply_gumball(change);
            self.doc.squash_undo(mark, before);
            return;
        }
        self.apply_gumball(change);
    }

    /// Applies a gumball change to the selection, or its picked control points, as one undo step.
    pub(crate) fn apply_gumball(&mut self, change: GumballChange) {
        let before = self.doc.state().clone();
        if self.doc.has_picked_parts() {
            match self.doc.picked_parts_reshaped(|p| change.moved_point(p)) {
                Ok(made) => {
                    for (id, shape) in made {
                        self.doc.replace_shape(id, shape);
                    }
                    self.doc.refresh_history();
                    self.doc.record_change(before);
                }
                Err(error) => self.processor.note(error.to_string()),
            }
            return;
        }
        let selection = self.doc.selection();
        match change {
            // Live booleans' inputs move with them, so they stay live.
            GumballChange::Move(by) => self
                .doc
                .translate(&self.doc.with_live_inputs(&selection), by),
            GumballChange::Turn(turn) => self
                .doc
                .transform(&self.doc.with_live_inputs(&selection), &turn),
            GumballChange::Stretch {
                origin,
                axis,
                factor,
            } => {
                for (id, shape) in self.selected_shapes() {
                    let stretched = match &shape {
                        Shape::Curve(curve) => {
                            curve.stretched(origin, axis, factor).map(Shape::Curve)
                        }
                        Shape::Body(body) => body.stretched(origin, axis, factor).map(Shape::Body),
                        Shape::Mesh(_) | Shape::SubD(_) => {
                            Ok(curva_document::blocks::shape_placed(
                                &shape,
                                &stretch_map(origin, axis, factor),
                            )
                            .unwrap_or(shape.clone()))
                        }
                        // A block stays a block and text keeps its shape: their stretch is
                        // skipped.
                        Shape::Instance(_) | Shape::Annotation(_) => continue,
                    };
                    match stretched {
                        Ok(made) => self.doc.replace_shape(id, made),
                        Err(error) => self.processor.note(error.to_string()),
                    }
                }
            }
            GumballChange::Extrude { axis, distance } => self.gumball_extrude(axis, distance),
        }
        self.doc.refresh_history();
        self.doc.record_change(before);
    }

    /// The selected objects and their shapes.
    pub(crate) fn selected_shapes(&self) -> Vec<(u64, Shape)> {
        let selection = self.doc.selection();
        self.doc
            .state()
            .objects
            .iter()
            .filter(|object| selection.contains(&object.id))
            .map(|object| (object.id, object.shape.clone()))
            .collect()
    }

    /// The gumball's extrude circle, as `ExtrudeCrv` and `ExtrudeSrf` do: curves become surfaces
    /// (closed ones solids), and flat surfaces facing that way become solids. The originals
    /// stay, as in Rhino.
    pub(crate) fn gumball_extrude(&mut self, axis: Point, distance: f64) {
        if distance.abs() < 1e-9 {
            return;
        }
        for (id, shape) in self.selected_shapes() {
            let layer = self.doc.layer_of(id);
            let Some(made) = gumball_extruded(&shape, axis, distance) else {
                continue;
            };
            match made {
                Ok(body) => {
                    self.doc.add_shape_on(Shape::Body(body), layer);
                }
                Err(error) => self.processor.note(error.to_string()),
            }
        }
    }

    /// The small box at the cursor for an exact gumball value, after a click on a handle
    /// without a drag: a distance, an angle or a factor. Enter applies it; Escape closes it.
    pub(crate) fn gumball_value_box(&mut self, ctx: &egui::Context) {
        let Some((handle, mut typed)) = self.gumball_state.gumball_typing.take() else {
            return;
        };
        // The circle asks for a factor where it stretches, a distance where it extrudes.
        let asked = match handle {
            Handle::Extrude(index) if self.circle_stretches(index) => Handle::Scale(index),
            other => other,
        };
        let Some(at) = ctx.pointer_latest_pos() else {
            return;
        };
        let key = match asked {
            Handle::Turn(_) => "ui.gumball.angle",
            Handle::Scale(_) => "ui.gumball.factor",
            _ => "ui.gumball.distance",
        };
        let mut done = false;
        let mut keep = true;
        egui::Area::new(egui::Id::NULL.with("gumball value"))
            .fixed_pos(at + egui::vec2(14.0, 14.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(self.text.render(&Message::new(key)));
                    let field = ui.add(egui::TextEdit::singleline(&mut typed).desired_width(90.0));
                    field.request_focus();
                    if ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    }) {
                        done = true;
                    }
                    if ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    }) {
                        keep = false;
                    }
                });
            });
        if done {
            let even = ctx.input(|input| input.modifiers.shift);
            if let (Ok(value), Some(gumball)) = (typed.trim().parse::<f64>(), self.gumball())
                && let Some(change) = gumball.typed(asked, value, even)
            {
                self.apply_gumball(change);
            }
        } else if keep {
            self.gumball_state.gumball_typing = Some((handle, typed));
        }
    }

    /// The selected objects as the drag under way would leave them.
    pub(crate) fn gumball_preview(&self) -> Vec<Curve> {
        let Some(change) = self
            .gumball_state
            .gumball_drag
            .filter(|drag| !drag.relocate)
            .and_then(|drag| self.gumball_change(&drag))
        else {
            return Vec::new();
        };
        if self.doc.has_picked_parts() {
            return self
                .doc
                .picked_parts_reshaped(|p| change.moved_point(p))
                .map(|made| {
                    made.iter()
                        .flat_map(|(_, shape)| shape.outlines())
                        .collect()
                })
                .unwrap_or_default();
        }
        let selection = self.doc.selection();
        self.doc
            .state()
            .objects
            .iter()
            .filter(|object| selection.binary_search(&object.id).is_ok())
            .take(MOST_GUMBALL_PREVIEW)
            .flat_map(|object| object.shape.outlines())
            .map(|curve| change.applied(&curve))
            .collect()
    }

    pub(crate) fn paint_gumball(
        &self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        size: [f64; 2],
    ) {
        let Some(gumball) = self.gumball() else {
            return;
        };
        let camera = &self.panes[pane].camera;
        let hovered = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|position| rect.contains(*position))
            .and_then(|position| {
                let offset = position - rect.min;
                gumball.handle_at(camera, [f64::from(offset.x), f64::from(offset.y)], size)
            });
        let dragged = self
            .gumball_state
            .gumball_drag
            .filter(|drag| drag.pane == pane)
            .map(|d| d.handle);
        let painter = ui.painter_at(rect);
        if hovered.is_some() || dragged.is_some() {
            ui.ctx().set_cursor_icon(if dragged.is_some() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
        // A dotted line from the centre out to each scale square, in its axis's colour (#693).
        let at = |p: [f64; 2]| rect.min + egui::vec2(p[0] as f32, p[1] as f32);
        for (axis, [from, to]) in gumball.square_lines(camera, size) {
            painter.extend(egui::Shape::dashed_line(
                &[at(from), at(to)],
                egui::Stroke::new(1.0, GUMBALL_AXES[axis]),
                GUMBALL_DASH,
                GUMBALL_DASH,
            ));
        }
        for (handle, run) in gumball.outlines(camera, size) {
            let lit = Some(handle) == hovered || Some(handle) == dragged;
            let colour = if lit {
                theme::ACCENT
            } else {
                handle
                    .axis()
                    .map_or(theme::current().text, |axis| GUMBALL_AXES[axis])
            };
            let points: Vec<egui::Pos2> = run
                .iter()
                .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
                .collect();
            paint_handle(&painter, handle, points, colour, lit);
        }
        // While dragging, what the drag amounts to beside the pointer.
        if let Some(drag) = self
            .gumball_state
            .gumball_drag
            .filter(|drag| drag.pane == pane)
            && let Some(change) = self.gumball_change(&drag)
            && let Some(pointer) = ui.input(|input| input.pointer.latest_pos())
        {
            let (value, amount) = change.amount();
            let text = match amount {
                curva_viewport::Amount::Distance => self.doc.length_text(value),
                curva_viewport::Amount::Angle => format!("{value:.2}°"),
                curva_viewport::Amount::Factor => format!("× {value:.4}"),
            };
            paint_cursor_tag(ui, pointer, &text);
        }
    }
}

/// One gumball handle drawn through `points` in `colour`: an arrow to move, an arc to turn, a
/// ring to extrude, a square to move on a plane (fuller while `lit`), a box to scale.
fn paint_handle(
    painter: &egui::Painter,
    handle: Handle,
    points: Vec<egui::Pos2>,
    colour: egui::Color32,
    lit: bool,
) {
    let stroke = egui::Stroke::new(2.0, colour);
    match handle {
        Handle::Move(_) => {
            // An arrowhead at the tip.
            if let (Some(start), Some(tip)) = (points.first(), points.last()) {
                let along = (*tip - *start).normalized();
                let side = egui::vec2(-along.y, along.x);
                let base = *tip - along * GUMBALL_TIP * 2.2;
                painter.add(egui::Shape::convex_polygon(
                    vec![*tip, base + side * GUMBALL_TIP, base - side * GUMBALL_TIP],
                    colour,
                    egui::Stroke::NONE,
                ));
            }
            painter.add(egui::Shape::line(points, stroke));
        }
        Handle::Turn(_) => {
            painter.add(egui::Shape::line(points, stroke));
        }
        Handle::Extrude(_) => {
            painter.add(egui::Shape::closed_line(points, stroke));
        }
        Handle::Plane(_) => {
            let fill = colour.gamma_multiply(if lit { 0.8 } else { 0.45 });
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        }
        Handle::Scale(_) | Handle::Centre => {
            painter.add(egui::Shape::convex_polygon(
                points,
                colour,
                egui::Stroke::NONE,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_click_on_the_same_handle_soon_after_is_a_double_click() {
        let start = std::time::Instant::now();
        let soon = start + std::time::Duration::from_millis(200);
        let late = start + std::time::Duration::from_millis(900);
        let first = Some((Handle::Move(0), start));
        assert!(is_double(first, Handle::Move(0), soon));
        assert!(!is_double(first, Handle::Move(1), soon));
        assert!(!is_double(first, Handle::Move(0), late));
        assert!(!is_double(None, Handle::Move(0), soon));
    }

    #[test]
    fn tapping_alt_during_a_drag_switches_copying() {
        assert!(copy_after(false, false, true), "Alt goes down: copy");
        assert!(copy_after(true, true, true), "held down: still copying");
        assert!(copy_after(true, false, false), "let go: still copying");
        assert!(!copy_after(true, false, true), "tapped again: no copy");
    }

    #[test]
    fn the_gumball_extrudes_curves_into_surfaces_and_flat_surfaces_into_solids() {
        let up = [0.0, 0.0, 1.0];
        let line = Shape::Curve(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        let wall = gumball_extruded(&line, up, 5.0).unwrap().unwrap();
        assert!(!wall.is_solid());
        let square = Curve::polyline(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        )
        .unwrap();
        let block = gumball_extruded(&Shape::Curve(square.clone()), up, 5.0)
            .unwrap()
            .unwrap();
        assert!(block.is_solid());
        let sheet = curva_kernel::Body::planar(&[square]).unwrap();
        let slab = gumball_extruded(&Shape::Body(sheet), up, 5.0)
            .unwrap()
            .unwrap();
        assert!(slab.is_solid());
        assert!((slab.volume().unwrap() - 500.0).abs() < 1e-3);
        assert!(gumball_extruded(&Shape::Body(slab), up, 5.0).is_none());
    }
}
