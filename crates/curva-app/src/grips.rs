//! Painting and picking control points, and the picked edges and faces of bodies.

use crate::*;
use curva_document::MeshPart;
use curva_viewport::PartShape;

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
        let mut mesh = egui::Mesh::default();
        for pick in picked {
            for triangle in self.outlines.face_triangles(pick) {
                let corners: Vec<egui::Pos2> = triangle
                    .iter()
                    .filter_map(|p| camera.project(*p, size))
                    .map(|p| rect.min + egui::vec2(p[0] as f32, p[1] as f32))
                    .collect();
                add_tinted_triangle(&mut mesh, &corners, tint);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    /// The picked parts of meshes and SubD cages (#334, Ctrl+Shift picking): points as dots,
    /// edges drawn thick and faces tinted, in the selection colour, as Rhino highlights them.
    /// A smooth SubD's parts are shown on its smooth surface, where Rhino shows them.
    pub(crate) fn paint_picked_parts(
        &self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        size: [f64; 2],
    ) {
        let shapes = self.picked_part_shapes();
        if shapes.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let screen = |p: Point| {
            let s = camera.project(p, size)?;
            Some(rect.min + egui::vec2(s[0] as f32, s[1] as f32))
        };
        let tint = egui::Color32::from_rgba_unmultiplied(255, 220, 60, 90);
        let stroke = egui::Stroke::new(3.0, GRIP_PICKED);
        let mut tinted = egui::Mesh::default();
        for shape in shapes {
            match shape {
                PartShape::Point(p) => {
                    if let Some(at) = screen(p) {
                        painter.circle_filled(at, GRIP_SIZE / 2.0, GRIP_PICKED);
                    }
                }
                PartShape::Line(run) => {
                    let run: Option<Vec<egui::Pos2>> = run.into_iter().map(screen).collect();
                    if let Some(run) = run {
                        painter.add(egui::Shape::line(run, stroke));
                    }
                }
                PartShape::Patch(triangles) => {
                    for triangle in triangles {
                        let corners: Vec<egui::Pos2> =
                            triangle.into_iter().filter_map(screen).collect();
                        add_tinted_triangle(&mut tinted, &corners, tint);
                    }
                }
            }
        }
        painter.add(egui::Shape::mesh(tinted));
    }

    /// Where each picked part of a mesh or SubD lies as it is drawn: a SubD's on its smooth
    /// surface when it is shown smooth, as Rhino highlights them (`curva_viewport::subd_parts`).
    fn picked_part_shapes(&self) -> Vec<PartShape> {
        let picked = self.doc.mesh_part_selection();
        let mut shapes = Vec::new();
        for object in &self.doc.state().objects {
            let parts: Vec<MeshPart> = picked
                .iter()
                .filter(|(id, _)| *id == object.id)
                .map(|(_, part)| *part)
                .collect();
            if parts.is_empty() {
                continue;
            }
            match &object.shape {
                Shape::SubD(subd) => {
                    shapes.extend(
                        curva_viewport::subd_parts(subd, &parts)
                            .into_iter()
                            .flatten(),
                    );
                }
                Shape::Mesh(mesh) => shapes.extend(
                    parts
                        .iter()
                        .filter_map(|&part| part_corners(mesh, part))
                        .map(shape_of),
                ),
                _ => {}
            }
        }
        shapes
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

    /// The cage of each SubD being edited, as Rhino 8 shows it (#334): its edges as thin dotted
    /// lines, in the control polygon's grey so they show on any background, and its points as
    /// small white dots, over the smooth surface. A SubD is being edited
    /// while its points are on or a part of it is picked.
    fn paint_cages(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        let points_on = self.doc.showing_points();
        let picked: Vec<u64> = self
            .doc
            .mesh_part_selection()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        // Each SubD being edited, and whether its points are on: then they are drawn as control
        // points are, to show and pick, and the cage draws only its lines.
        let cages: Vec<(&curva_mesh::Mesh, bool)> = self
            .doc
            .state()
            .objects
            .iter()
            .filter(|o| !o.hidden && (points_on.contains(&o.id) || picked.contains(&o.id)))
            .filter_map(|o| Some((o.shape.subd()?.cage(), points_on.contains(&o.id))))
            .collect();
        if cages.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let camera = &self.panes[pane].camera;
        let screen = |at: Point| {
            let p = camera.project(at, size)?;
            Some(rect.min + egui::vec2(p[0] as f32, p[1] as f32))
        };
        let stroke = egui::Stroke::new(1.0, CONTROL_POLYGON);
        for (cage, points_shown) in cages {
            let points: Vec<Option<egui::Pos2>> =
                cage.vertices().iter().map(|&p| screen(p)).collect();
            for [a, b] in cage.wire_edges() {
                if let (Some(p), Some(q)) = (points[a as usize], points[b as usize]) {
                    painter.extend(egui::Shape::dotted_line(&[p, q], CONTROL_POLYGON, 4.0, 0.6));
                }
            }
            for point in points.into_iter().flatten().filter(|_| !points_shown) {
                painter.circle(point, 3.0, GRIP_COLOUR, stroke);
            }
        }
    }

    /// Control points on show: the lines between a smooth curve's points, then the points,
    /// picked ones in yellow.
    pub(crate) fn paint_grips(&self, ui: &egui::Ui, pane: usize, rect: egui::Rect, size: [f64; 2]) {
        self.paint_cages(ui, pane, rect, size);
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

/// A mesh part's `corners` as the shape it is drawn as: a point, an edge, or a face fanned into
/// triangles from its first corner.
fn shape_of(corners: Vec<Point>) -> PartShape {
    match corners.as_slice() {
        [point] => PartShape::Point(*point),
        [_, _] => PartShape::Line(corners),
        _ => PartShape::Patch(
            (1..corners.len() - 1)
                .map(|k| [corners[0], corners[k], corners[k + 1]])
                .collect(),
        ),
    }
}

/// Where a picked `part` of `mesh` is: a point's position, an edge's two ends, or a face's
/// corners in order; `None` when the mesh has no such part.
fn part_corners(mesh: &curva_mesh::Mesh, part: MeshPart) -> Option<Vec<Point>> {
    let point = |i: u32| mesh.vertices().get(i as usize).copied();
    match part {
        MeshPart::Point(i) => Some(vec![point(i)?]),
        MeshPart::Edge(a, b) => Some(vec![point(a)?, point(b)?]),
        MeshPart::Face(f) => {
            if f as usize >= mesh.face_count() {
                return None;
            }
            mesh.face(f as usize).iter().map(|&i| point(i)).collect()
        }
    }
}

/// Adds one triangle at `corners` (three of them, on screen) to `mesh`, filled with `tint`
/// exactly to its corners. Painted as a mesh rather than a polygon, so egui does not soften its
/// edges: on the thin triangles of a curved face that softening reached far past the corners,
/// as long lines across the view.
fn add_tinted_triangle(mesh: &mut egui::Mesh, corners: &[egui::Pos2], tint: egui::Color32) {
    if corners.len() != 3 {
        return;
    }
    let first = mesh.vertices.len() as u32;
    for &at in corners {
        mesh.colored_vertex(at, tint);
    }
    mesh.add_triangle(first, first + 1, first + 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A thin triangle, as a curved face is cut into, fills only between its corners, with
    /// nothing reaching past them; a corner off screen leaves the triangle out.
    #[test]
    fn a_picked_face_is_tinted_only_between_its_corners() {
        let tint = egui::Color32::from_rgba_unmultiplied(255, 220, 60, 90);
        let sliver = [
            egui::pos2(100.0, 100.0),
            egui::pos2(600.0, 100.2),
            egui::pos2(100.0, 100.4),
        ];
        let mut mesh = egui::Mesh::default();
        add_tinted_triangle(&mut mesh, &sliver, tint);
        add_tinted_triangle(&mut mesh, &sliver[..2], tint);
        assert_eq!((mesh.vertices.len(), mesh.indices.len()), (3, 3));
        let around = egui::Rect::from_points(&sliver);
        assert!(mesh.vertices.iter().all(|v| around.contains(v.pos)));
        assert!(mesh.vertices.iter().all(|v| v.color == tint));
    }

    /// A picked point is its position, an edge its two ends and a face its corners in order,
    /// for highlighting; a part the mesh lacks is nothing (#334).
    #[test]
    fn picked_mesh_parts_are_found_where_they_are() {
        let square = curva_mesh::Mesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            &[[0, 1, 2, 3]],
        )
        .unwrap();
        assert_eq!(
            part_corners(&square, MeshPart::Point(2)),
            Some(vec![[1.0, 1.0, 0.0]])
        );
        assert_eq!(
            part_corners(&square, MeshPart::edge(3, 0)),
            Some(vec![[0.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
        );
        assert_eq!(
            part_corners(&square, MeshPart::Face(0)).map(|c| c.len()),
            Some(4)
        );
        assert_eq!(part_corners(&square, MeshPart::Point(9)), None);
        assert_eq!(part_corners(&square, MeshPart::Face(1)), None);
    }
}
