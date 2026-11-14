//! Painting and picking control points, and the picked edges and faces of bodies.

use crate::*;

pub(crate) const POINT_MARKER_SIZE: f32 = 6.0;

/// Size of a control point's square on screen.
pub(crate) const GRIP_SIZE: f32 = 7.0;

/// A control point that is not picked.
pub(crate) const GRIP_COLOUR: egui::Color32 = egui::Color32::from_rgb(235, 235, 245);

/// A picked control point, in the accent like selected objects.
pub(crate) const GRIP_PICKED: egui::Color32 = theme::ACCENT;

/// The lines between a smooth curve's control points.
pub(crate) const CONTROL_POLYGON: egui::Color32 = egui::Color32::from_rgb(150, 150, 165);

impl Curva {
    /// Point objects and point previews, which have no lines to draw.
    pub(crate) fn paint_points(
        &self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        size: [f64; 2],
        previewed: &[Curve],
    ) {
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let mark = |at: Point, colour: [f32; 3]| {
            if let Some(p) = camera.project(at, size) {
                let [r, g, b] = colour.map(|c| (c * 255.0) as u8);
                painter.rect_filled(
                    egui::Rect::from_center_size(
                        rect.min + egui::vec2(p[0] as f32, p[1] as f32),
                        egui::Vec2::splat(POINT_MARKER_SIZE),
                    ),
                    0.0,
                    egui::Color32::from_rgb(r, g, b),
                );
            }
        };
        for (at, colour) in self.outlines.point_markers(&self.doc.selection()) {
            mark(at, colour);
        }
        for curve in previewed {
            if let Curve::Point { at } = curve {
                mark(*at, curva_viewport::PREVIEW_COLOUR);
            }
        }
    }

    /// The faces of a solid picked for a command, tinted in the selection colour.
    pub(crate) fn paint_picked_faces(
        &self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        size: [f64; 2],
    ) {
        let picked = self.doc.face_selection();
        if picked.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let tint = egui::Color32::from_rgba_unmultiplied(255, 220, 60, 90);
        for pick in picked {
            for triangle in self.outlines.face_triangles(pick) {
                let corners: Vec<egui::Pos2> = triangle
                    .iter()
                    .filter_map(|p| camera.project(*p, size))
                    .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
                    .collect();
                if corners.len() == 3 {
                    painter.add(egui::Shape::convex_polygon(
                        corners,
                        tint,
                        egui::Stroke::NONE,
                    ));
                }
            }
        }
    }

    /// The edges of a solid picked for a command, drawn thick in the selection colour.
    pub(crate) fn paint_picked_edges(
        &self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        size: [f64; 2],
    ) {
        let picked = self.doc.edge_selection();
        if picked.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        for pick in picked {
            let Some(points) = self.outlines.edge_points(pick) else {
                continue;
            };
            let screen: Vec<egui::Pos2> = points
                .iter()
                .filter_map(|p| camera.project(*p, size))
                .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
                .collect();
            for pair in screen.windows(2) {
                painter.line_segment([pair[0], pair[1]], egui::Stroke::new(3.0, GRIP_PICKED));
            }
        }
    }

    /// Control points on show: the lines between a smooth curve's points, then the points,
    /// picked ones in yellow.
    pub(crate) fn paint_grips(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        let shown = self.doc.shown_grips();
        if shown.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let screen = |at: Point| {
            camera
                .project(at, size)
                .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
        };
        for object in &self.doc.state().objects {
            let straight = matches!(
                object.curve(),
                Some(Curve::Line { .. } | Curve::Polyline { .. })
            );
            if straight || !shown.iter().any(|((id, _), _)| *id == object.id) {
                continue;
            }
            let corners: Vec<egui::Pos2> = shown
                .iter()
                .filter(|((id, _), _)| *id == object.id)
                .filter_map(|(_, at)| screen(*at))
                .collect();
            for pair in corners.windows(2) {
                painter.line_segment([pair[0], pair[1]], egui::Stroke::new(1.0, CONTROL_POLYGON));
            }
        }
        let picked = self.doc.grip_selection();
        for (grip, at) in shown {
            if let Some(centre) = screen(at) {
                let square = egui::Rect::from_center_size(centre, egui::Vec2::splat(GRIP_SIZE));
                let colour = if picked.contains(&grip) {
                    GRIP_PICKED
                } else {
                    GRIP_COLOUR
                };
                painter.rect_filled(square, 0.0, colour);
                painter.rect_stroke(
                    square,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::BLACK),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    /// The control points a click or box hits, nearest first for a click.
    pub(crate) fn grips_hit(
        &self,
        camera: &Camera,
        (from, to): ([f64; 2], [f64; 2]),
        boxed: bool,
        size: [f64; 2],
    ) -> Vec<Grip> {
        let on_screen = self
            .doc
            .shown_grips()
            .into_iter()
            .filter_map(|(grip, at)| camera.project(at, size).map(|p| (grip, p)));
        if boxed {
            let (low, high) = (
                [from[0].min(to[0]), from[1].min(to[1])],
                [from[0].max(to[0]), from[1].max(to[1])],
            );
            return on_screen
                .filter(|(_, p)| {
                    (low[0]..=high[0]).contains(&p[0]) && (low[1]..=high[1]).contains(&p[1])
                })
                .map(|(grip, _)| grip)
                .collect();
        }
        on_screen
            .map(|(grip, p)| (grip, (p[0] - from[0]).hypot(p[1] - from[1])))
            .filter(|(_, reach)| *reach <= PICK_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(grip, _)| grip)
            .into_iter()
            .collect()
    }
}
