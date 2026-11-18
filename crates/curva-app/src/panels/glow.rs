//! The glow that marks the panel a dragged panel would merge into (owner, 18 November 2026): in
//! the accent, along the inside of the panel's rounded edge, so it follows its corners and never
//! spills past them, strongest at the part of the edge nearest the mouse and fading with
//! distance from it.

use crate::*;

/// How far along the edge the glow stays strong, in points.
const REACH: f32 = 140.0;
/// The faintest the glow gets, far from the mouse, against its strongest.
const FAINTEST: f32 = 0.12;
/// How wide the edge line is, and how far the glow fades in from it, in points.
const LINE: f32 = 2.0;
const DEPTH: f32 = 16.0;
/// Points along a straight edge, at most this far apart, and along each rounded corner.
const STEP: f32 = 6.0;
const CORNER_STEPS: usize = 10;

/// The rounded outline of `rect` with corner `radius`, as points each with the direction out of
/// the panel there, going round once.
fn outline(rect: egui::Rect, radius: f32) -> Vec<(egui::Pos2, egui::Vec2)> {
    let radius = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    let mut points = Vec::new();
    // Corner centres and the angle each corner's arc starts at, going clockwise on screen.
    let corners = [
        (
            egui::pos2(rect.max.x - radius, rect.min.y + radius),
            -90.0_f32,
        ),
        (egui::pos2(rect.max.x - radius, rect.max.y - radius), 0.0),
        (egui::pos2(rect.min.x + radius, rect.max.y - radius), 90.0),
        (egui::pos2(rect.min.x + radius, rect.min.y + radius), 180.0),
    ];
    for (k, &(centre, start)) in corners.iter().enumerate() {
        for step in 0..=CORNER_STEPS {
            let angle = (start + 90.0 * step as f32 / CORNER_STEPS as f32).to_radians();
            let out = egui::vec2(angle.cos(), angle.sin());
            points.push((centre + out * radius, out));
        }
        // The straight edge to the next corner.
        let (next, next_start) = corners[(k + 1) % 4];
        let end_angle = (start + 90.0).to_radians();
        let out = egui::vec2(end_angle.cos(), end_angle.sin());
        let from = centre + out * radius;
        let to = next
            + egui::vec2(next_start.to_radians().cos(), next_start.to_radians().sin()) * radius;
        let length = (to - from).length();
        let pieces = (length / STEP).ceil().max(1.0) as usize;
        for piece in 1..pieces {
            points.push((from + (to - from) * (piece as f32 / pieces as f32), out));
        }
    }
    points
}

/// How strong the glow is at `at`, with the mouse at `pointer`: 1 nearest it, fading to
/// [`FAINTEST`].
fn strength(at: egui::Pos2, pointer: egui::Pos2) -> f32 {
    let far = (at - pointer).length() / REACH;
    FAINTEST + (1.0 - FAINTEST) * (-far * far).exp()
}

/// The glow inside the rounded edge of the panel at `rect`, for the mouse at `pointer`.
pub(crate) fn edge_glow(rect: egui::Rect, radius: f32, pointer: egui::Pos2) -> egui::Shape {
    let points = outline(rect, radius);
    let mut mesh = egui::Mesh::default();
    let accent = theme::ACCENT;
    // Three rings of points: the edge, just inside the line, and where the glow has faded.
    for &(at, out) in &points {
        let s = strength(at, pointer);
        mesh.colored_vertex(at, accent.gamma_multiply(s));
        mesh.colored_vertex(at - out * LINE, accent.gamma_multiply(0.6 * s));
        mesh.colored_vertex(at - out * DEPTH, egui::Color32::TRANSPARENT);
    }
    let count = points.len() as u32;
    for i in 0..count {
        let (a, b) = (i * 3, ((i + 1) % count) * 3);
        for ring in 0..2 {
            mesh.add_triangle(a + ring, b + ring, a + ring + 1);
            mesh.add_triangle(b + ring, b + ring + 1, a + ring + 1);
        }
    }
    egui::Shape::mesh(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glow_stays_inside_the_rounded_panel_and_is_strongest_by_the_mouse() {
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(300.0, 200.0));
        let radius = 12.0;
        let points = outline(rect, radius);
        // Every point of the edge lies on the rounded outline: inside the box, and no nearer a
        // corner of the box than the rounding allows.
        for &(at, out) in &points {
            assert!(rect.expand(1e-3).contains(at), "{at:?}");
            assert!((out.length() - 1.0).abs() < 1e-4);
            let corner = egui::pos2(
                if at.x < rect.center().x {
                    rect.min.x
                } else {
                    rect.max.x
                },
                if at.y < rect.center().y {
                    rect.min.y
                } else {
                    rect.max.y
                },
            );
            let rounded_off = radius * (std::f32::consts::SQRT_2 - 1.0);
            assert!((at - corner).length() >= rounded_off - 1e-3, "{at:?}");
            // The glow fades inwards, never outwards.
            assert!(rect.contains(at - out * DEPTH) || rect.width() < 2.0 * DEPTH);
        }
        // Strongest by the mouse, weaker far along, never gone.
        let pointer = egui::pos2(250.0, 100.0);
        let near = strength(egui::pos2(250.0, 100.0), pointer);
        let mid = strength(egui::pos2(400.0, 150.0), pointer);
        let far = strength(egui::pos2(100.0, 300.0), pointer);
        assert!(near > mid && mid > far);
        assert!((near - 1.0).abs() < 1e-6);
        assert!(far >= FAINTEST);
    }
}
