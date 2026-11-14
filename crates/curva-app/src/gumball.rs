//! The gumball: drawing it, dragging it, typing a value, and what a drag does to the selection.

use crate::*;

/// A gumball drag or typed value under way.
#[derive(Default)]
pub(crate) struct GumballState {
    pub(crate) gumball_drag: Option<GumballDrag>,
    /// A gumball handle clicked without a drag, and the value being typed for it.
    pub(crate) gumball_typing: Option<(Handle, String)>,
}

/// Most objects shown moving during a gumball drag.
pub(crate) const MOST_GUMBALL_PREVIEW: usize = 2000;

pub(crate) const GUMBALL_TIP: f32 = 5.0;

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
        Shape::Instance(_) | Shape::Annotation(_) => None,
    }
}

impl Curva {
    /// The gumball for the current selection, if it should show.
    pub(crate) fn gumball(&self) -> Option<Gumball> {
        if !self.gumball_on || self.processor.is_running() {
            return None;
        }
        // Picked control points take the gumball, as in Rhino; it sits at their middle.
        let grips = self.picked_grip_points();
        if !grips.is_empty() {
            let count = grips.len() as f64;
            let sum = grips.iter().fold([0.0; 3], |total, p| {
                [total[0] + p[0], total[1] + p[1], total[2] + p[2]]
            });
            return Some(Gumball {
                centre: sum.map(|value| value / count),
            });
        }
        let centre = self.outlines.centre_of(&self.doc.selection())?;
        Some(Gumball { centre })
    }

    /// Where the picked control points are.
    pub(crate) fn picked_grip_points(&self) -> Vec<Point> {
        let picked = self.doc.grip_selection();
        if picked.is_empty() {
            return Vec::new();
        }
        self.doc
            .shown_grips()
            .into_iter()
            .filter(|(grip, _)| picked.contains(grip))
            .map(|(_, at)| at)
            .collect()
    }

    /// What the drag under way would do.
    pub(crate) fn gumball_change(&self, drag: &GumballDrag) -> Option<GumballChange> {
        let gumball = self.gumball()?;
        let camera = &self.panes[drag.pane].camera;
        // The circle extrudes curves and open surfaces and stretches closed solids; Shift
        // stretches anything, and Shift with Ctrl scales evenly.
        let handle = match drag.handle {
            Handle::Extrude(index) if drag.even || self.selection_all_solid() => {
                Handle::Scale(index)
            }
            other => other,
        };
        let even_scale = drag.even && drag.ctrl;
        let change =
            gumball.change_with(camera, drag.size, handle, drag.from, drag.now, even_scale)?;
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

    /// Whether everything selected is a closed solid, which the gumball's circle stretches
    /// rather than extrudes.
    pub(crate) fn selection_all_solid(&self) -> bool {
        let shapes = self.selected_shapes();
        !shapes.is_empty()
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
                // Ctrl-dragging an arrow extrudes along it, as in Rhino.
                let handle = match handle {
                    Handle::Move(axis) if ui.input(|input| input.modifiers.command) => {
                        Handle::Extrude(axis)
                    }
                    other => other,
                };
                self.gumball_state.gumball_drag = Some(GumballDrag {
                    pane,
                    handle,
                    size,
                    from: local(*pos),
                    now: local(*pos),
                    even: false,
                    ctrl: false,
                });
                return true;
            }
        }
        false
    }

    /// Applies a finished drag as one undo step.
    pub(crate) fn finish_gumball(&mut self, drag: &GumballDrag) {
        // A click without a drag asks for an exact value, as in Rhino.
        if drag.from == drag.now {
            self.gumball_state.gumball_typing = Some((drag.handle, String::new()));
            return;
        }
        let Some(change) = self.gumball_change(drag) else {
            return;
        };
        self.apply_gumball(change);
    }

    /// Applies a gumball change to the selection, or its picked control points, as one undo step.
    pub(crate) fn apply_gumball(&mut self, change: GumballChange) {
        let before = self.doc.state().clone();
        let grips = self.doc.grip_selection();
        if !grips.is_empty() {
            match self.doc.grips_changed(&grips, |p| change.moved_point(p)) {
                Ok(made) => {
                    for (id, curve) in made {
                        self.doc.replace(id, curve);
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
            Handle::Extrude(index) if self.selection_all_solid() => Handle::Scale(index),
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
            .and_then(|drag| self.gumball_change(&drag))
        else {
            return Vec::new();
        };
        let grips = self.doc.grip_selection();
        if !grips.is_empty() {
            return self
                .doc
                .grips_changed(&grips, |p| change.moved_point(p))
                .map(|made| made.into_iter().map(|(_, curve)| curve).collect())
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

#[cfg(test)]
mod tests {
    use super::*;

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
